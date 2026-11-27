//! IPC commands for terminals: start a shell or an agent in a PTY, send
//! input, resize, and stop. Output goes back over a per-session channel as
//! raw bytes, so the web view gets an `ArrayBuffer` with no JSON encoding.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use brachio_agent::{AgentKind, Installed, PromptDelivery, ResolvedCommand};
use brachio_core::status::AgentState;
use brachio_pty::{EventSink, PtyEvent, SessionId, SessionManager, SpawnSpec};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::hook_setup::HookSetup;
use crate::runtime_terminals::{Launch, RuntimeTerminals};
use crate::terminal_status::TerminalStatus;
use crate::{task_ports, terminal_authorization, workspace::Database};

/// App state that owns every terminal.
pub struct Terminals(pub Arc<SessionManager>, pub SessionKeys);

/// Which task terminal key each in-app session was opened for; the manager only knows handles.
#[derive(Default)]
pub struct SessionKeys(Mutex<HashMap<SessionId, String>>);

impl SessionKeys {
    pub(crate) fn record(&self, id: SessionId, key: String) {
        if let Ok(mut keys) = self.0.lock() {
            keys.insert(id, key);
        }
    }

    /// Keys of the task's in-app terminals whose program is still running.
    ///
    /// # Errors
    /// Fails when the session table cannot be read, so the caller can refuse rather than guess.
    pub fn live_for_task(
        &self,
        manager: &SessionManager,
        task_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        let live = manager.ids()?;
        let mut keys = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal key list lock poisoned"))?;
        keys.retain(|id, _| live.contains(id));
        let prefix = format!("{task_id}:");
        let mut found: Vec<String> = keys
            .values()
            .filter(|key| key.starts_with(&prefix))
            .cloned()
            .collect();
        found.sort();
        Ok(found)
    }

    /// Kills the task's in-app terminals. One that already ended is skipped.
    pub fn kill_for_task(&self, manager: &SessionManager, task_id: &str) {
        let prefix = format!("{task_id}:");
        let ids: Vec<SessionId> = match self.0.lock() {
            Ok(keys) => keys
                .iter()
                .filter(|(_, key)| key.starts_with(&prefix))
                .map(|(id, _)| *id)
                .collect(),
            Err(_) => return,
        };
        for id in ids {
            if let Err(error) = manager.kill(id) {
                tracing::debug!(%error, "an in-app terminal had already ended");
            }
        }
    }
}

/// What to run in a new terminal.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OpenRequest {
    /// `None` opens the user's shell.
    pub agent: Option<AgentKind>,
    /// First prompt for the agent. Ignored for a shell.
    pub prompt: Option<String>,
    /// Continue the task's conversation instead of starting a new one.
    #[serde(default)]
    pub resume: bool,
    /// The task's conversation id, for agents that take one.
    pub session_id: Option<String>,
    /// A Codex conversation picked explicitly from the worktree's history.
    pub codex_conversation_id: Option<String>,
    /// The project folder of the task, filled in by the app, never by the window.
    #[serde(skip)]
    pub project_root: Option<PathBuf>,
    /// Working directory.
    pub cwd: PathBuf,
    /// Initial height in rows.
    pub rows: u16,
    /// Initial width in columns.
    pub cols: u16,
    /// Stable name of this terminal, such as the task and tab. With it the terminal can live in
    /// the runtime and be found again after the window closes.
    #[serde(default)]
    pub key: Option<String>,
}

/// A terminal that started.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// Session handle for later calls.
    pub id: SessionId,
    /// Whether the UI still has to type the first prompt in.
    pub prompt_delivery: PromptDelivery,
    /// The first prompt as the agent gets it, with a child task's preamble above it. The UI
    /// types this text, not its own, when it has to type the prompt in.
    pub first_prompt: Option<String>,
    /// Whether setup input and first prompts should be suppressed.
    pub resumed: bool,
    /// Whether the runtime already owned this live process.
    pub attached: bool,
    /// Whether the runtime hosts it, so the UI may park it instead of stopping it.
    pub hosted: bool,
}

/// Sent as JSON on the channel. Output frames are raw bytes instead.
#[derive(Debug, Serialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub(crate) enum ChannelMessage {
    /// The child exited.
    Exit { exit_code: Option<u32> },
    /// The agent reported its state.
    Status {
        status: AgentState,
        message: Option<String>,
    },
    /// The connection to the runtime ended while the program may still run.
    Detached { detached: bool },
    /// The screen just sent is the last one of a shell that is gone, shown before its restart.
    Restored { restored: bool },
    /// Output was lost on the way: clear the screen, a fresh frame follows.
    Reset { reset: bool },
}

/// Errors cross the IPC boundary as display strings.
type CommandResult<T> = Result<T, String>;

fn to_message(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

/// Lists the agent CLIs found on `PATH`.
#[tauri::command]
pub async fn agents_detect() -> Vec<Installed> {
    tokio::task::spawn_blocking(brachio_agent::detect_all)
        .await
        .unwrap_or_default()
}

/// Returns Codex resume choices for a task tab that has no live runtime session.
#[tauri::command]
pub async fn terminal_codex_conversations(
    database: State<'_, Database>,
    accounts: State<'_, crate::agent_account_env::AccountsFolder>,
    hosted: State<'_, RuntimeTerminals>,
    key: String,
) -> CommandResult<Vec<brachio_agent::CodexConversation>> {
    let owner = terminal_authorization::resolve(&*database.lock()?, &key)?;
    if owner.agent != Some(AgentKind::Codex) {
        return Err("This terminal is not a Codex tab.".to_owned());
    }
    if hosted
        .has_session(&key)
        .await
        .map_err(|error| to_message(&error))?
    {
        return Ok(Vec::new());
    }
    // The folder the Codex terminal would be given, assembled as for the launch.
    let setup = agent_setup(&*database.lock()?, &accounts, &owner.task, AgentKind::Codex)?;
    let home =
        assemble_environment(Vec::new(), setup, &mut Vec::new()).config_home(AgentKind::Codex);
    tauri::async_runtime::spawn_blocking(move || {
        if home == ConfigHome::Unknown {
            return Vec::new();
        }
        account_codex_conversations(home.folder(), &owner.task.worktree_path)
            .into_iter()
            .filter(|candidate| uuid::Uuid::parse_str(&candidate.id).is_ok())
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}

/// Clear a specific uncertain launch after the user has confirmed the risk.
#[tauri::command]
pub async fn terminal_forget_uncertain_launch(
    hosted: State<'_, RuntimeTerminals>,
    database: State<'_, Database>,
    key: String,
    operation_id: String,
) -> CommandResult<()> {
    terminal_authorization::resolve(&*database.lock()?, &key)?;
    hosted
        .forget_uncertain_launch(&key, &operation_id)
        .await
        .map_err(|error| error.to_string())
}

/// The last screen of a terminal whose program is gone, without starting anything.
#[tauri::command]
pub async fn terminal_saved_screen(
    hosted: State<'_, RuntimeTerminals>,
    database: State<'_, Database>,
    key: String,
) -> CommandResult<Option<crate::runtime_terminals::SavedScreen>> {
    terminal_authorization::resolve(&*database.lock()?, &key)?;
    hosted
        .saved_screen(&key)
        .await
        .map_err(|error| error.to_string())
}

/// Opens a terminal and streams its events to `on_event`.
#[tauri::command]
pub async fn terminal_open(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    database: State<'_, Database>,
    hook: State<'_, HookSetup>,
    accounts: State<'_, crate::agent_account_env::AccountsFolder>,
    mut request: OpenRequest,
    on_event: Channel<InvokeResponseBody>,
) -> CommandResult<Opened> {
    let key = request
        .key
        .clone()
        .ok_or_else(|| "A registered task terminal is required.".to_owned())?;
    let owner = terminal_authorization::resolve(&*database.lock()?, &key)?;
    let worktree =
        owner.task.worktree_path.canonicalize().map_err(|error| {
            crate::folder_access::tag(&owner.task.worktree_path, error.to_string())
        })?;
    let requested = request
        .cwd
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if requested != worktree || request.agent != owner.agent {
        return Err("Terminal request does not match the registered task tab.".to_owned());
    }
    request.cwd = owner.task.worktree_path.clone();
    request.session_id = owner.conversation;
    if request.agent.is_some() && !request.resume {
        request.prompt =
            crate::worker_preamble::first_prompt(&*database.lock()?, &owner.task, request.prompt)?;
    }
    let shell_id = owner.shell;
    request.project_root = brachio_store::RepoRecord::get(&*database.lock()?, &owner.task.repo_id)
        .ok()
        .map(|repo| repo.path);
    if request.codex_conversation_id.is_some()
        && (!request.resume || request.agent != Some(AgentKind::Codex))
    {
        return Err("A Codex conversation can only be resumed in a Codex tab.".to_owned());
    }
    let first_prompt = request.prompt.clone();
    let prepare = || {
        prepare_terminal(
            &request,
            &owner.task,
            shell_id.as_deref(),
            &database,
            &hook,
            &accounts,
        )
    };
    let mut fresh_launch = None;
    let launch = async {
        let (spec, delivery, resumed) = prepare().await.map_err(anyhow::Error::msg)?;
        fresh_launch = Some((delivery, resumed));
        Ok(Launch {
            cwd: spec.cwd,
            program: spec.program,
            args: spec.args,
            env: spec
                .env
                .into_iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect(),
            rows: spec.rows,
            cols: spec.cols,
        })
    };
    if let Some((id, attached)) = hosted
        .open(
            key.to_owned(),
            launch,
            on_event.clone(),
            request.agent.is_some(),
        )
        .await
        .map_err(|error| to_message(&error))?
    {
        // Recovered pending launches already ran their original prompt and setup path.
        let (prompt_delivery, resumed) = fresh_launch.unwrap_or((PromptDelivery::None, true));
        return Ok(Opened {
            id,
            first_prompt,
            prompt_delivery,
            resumed,
            attached,
            hosted: true,
        });
    }
    let (spec, prompt_delivery, resumed) = prepare().await?;
    let manager = Arc::clone(&terminals.0);
    let sink = sink_for(on_event, request.agent.is_some());
    let spawned = tauri::async_runtime::spawn_blocking(move || manager.spawn(&spec, sink))
        .await
        .map_err(|error| error.to_string())?;
    let id =
        spawned.map_err(|error| crate::folder_access::tag(&request.cwd, to_message(&error)))?;
    if let Some(key) = request.key.clone() {
        terminals.1.record(id, key);
    }
    Ok(Opened {
        id,
        first_prompt,
        prompt_delivery,
        resumed,
        attached: false,
        hosted: false,
    })
}

async fn prepare_terminal(
    request: &OpenRequest,
    task: &brachio_store::TaskRecord,
    shell_id: Option<&str>,
    database: &Database,
    hook: &HookSetup,
    accounts: &crate::agent_account_env::AccountsFolder,
) -> CommandResult<(SpawnSpec, PromptDelivery, bool)> {
    // Claude Code can ask the app instead of the terminal when a hook is available.
    let mut environment = terminal_env();
    if let Some(key) = request.key.as_deref() {
        match task_ports::port_for_terminal(database, key, &request.cwd) {
            Ok(Some(port)) => environment.push(("BRACHIO_PORT".to_owned(), port.to_string())),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not assign a development port"),
        }
    }
    let setup = match request.agent {
        Some(kind) => agent_setup(&*database.lock()?, accounts, task, kind)?,
        None => AgentSetup::default(),
    };
    let mut options = Vec::new();
    if let (Some(AgentKind::ClaudeCode), Some(key)) = (request.agent, request.key.as_deref()) {
        if !hook.environment(key).is_empty() {
            options = hook.claude_arguments();
        }
    }
    if let (Some(AgentKind::Codex), Some(_)) = (request.agent, request.key.as_deref()) {
        options = hook.codex_arguments();
    }
    // Every agent, not only Claude Code, can send and read messages.
    if let (Some(_), Some(key)) = (request.agent, request.key.as_deref()) {
        environment.extend(hook.mail_environment(key));
    }
    crate::agent_accounts::check_isolated_login(request.agent, &setup.account).await?;
    let environment = assemble_environment(environment, setup, &mut options);
    let (command, prompt_delivery, resumed, environment) = match request.agent {
        Some(kind) => {
            let request = request.clone();
            tauri::async_runtime::spawn_blocking(move || {
                agent_command(kind, &request, options, &environment)
                    .map(|(command, delivery, resumed)| (command, delivery, resumed, environment))
            })
            .await
            .map_err(|error| error.to_string())??
        }
        None => (
            shell_command(shell_id),
            PromptDelivery::None,
            false,
            environment,
        ),
    };
    let environment = environment.finish(command.env);
    let spec = SpawnSpec {
        program: command.program.to_string_lossy().into_owned(),
        args: command.args,
        cwd: request.cwd.clone(),
        env: environment,
        rows: request.rows.max(1),
        cols: request.cols.max(1),
    };
    Ok((spec, prompt_delivery, resumed))
}

/// Sends keyboard input to a terminal.
#[tauri::command]
pub async fn terminal_write(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    id: SessionId,
    data: String,
) -> CommandResult<()> {
    if let Some(result) = hosted.write(id, data.clone()).await {
        return result.map_err(|error| to_message(&error));
    }
    terminals
        .0
        .write(id, data.as_bytes())
        .map_err(|error| to_message(&error))
}

/// Changes a terminal's size.
#[tauri::command]
pub async fn terminal_resize(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    id: SessionId,
    rows: u16,
    cols: u16,
) -> CommandResult<()> {
    let (rows, cols) = (rows.max(1), cols.max(1));
    if let Some(result) = hosted.resize(id, rows, cols).await {
        return result.map_err(|error| to_message(&error));
    }
    let resized = terminals.0.resize(id, rows, cols);
    resized.map_err(|error| to_message(&error))
}

/// Stops a terminal's program for good. The exit event still arrives on its channel.
/// Use `terminal_detach` to only stop watching it.
#[tauri::command]
pub async fn terminal_close(
    terminals: State<'_, Terminals>,
    hosted: State<'_, RuntimeTerminals>,
    id: SessionId,
) -> CommandResult<()> {
    if let Some(result) = hosted.stop(id).await {
        return result.map_err(|error| to_message(&error));
    }
    terminals.0.kill(id).map_err(|error| to_message(&error))
}

/// Stops the UI from watching a runtime-hosted terminal; its program keeps running and opening the
/// same key reattaches. An in-app terminal has no runtime to keep it, so it is refused rather than
/// killed or orphaned.
#[tauri::command]
pub fn terminal_detach(hosted: State<'_, RuntimeTerminals>, id: SessionId) -> CommandResult<()> {
    match hosted.detach(id) {
        Some(result) => result.map_err(|error| to_message(&error)),
        None => Err("this terminal is not hosted by the runtime and cannot be detached".to_owned()),
    }
}

/// Lets the UI unmount a runtime-hosted terminal while its status keeps flowing on the same channel.
/// Output is no longer sent; opening the key again replaces the parked attachment. An in-app
/// terminal cannot be parked.
#[tauri::command]
pub fn terminal_park(hosted: State<'_, RuntimeTerminals>, id: SessionId) -> CommandResult<()> {
    match hosted.park(id) {
        Some(result) => result.map_err(|error| to_message(&error)),
        None => Err("this terminal is not hosted by the runtime and cannot be parked".to_owned()),
    }
}

/// What an agent's terminal gets as its environment. Later entries win, so the task's account
/// stays authoritative over the app's own variables, the profile's and the adapter's.
pub(crate) struct LaunchEnvironment {
    /// The app's variables and the profile's.
    pub base: Vec<(String, String)>,
    /// The task's account, applied last.
    pub account: Vec<(String, String)>,
}

impl LaunchEnvironment {
    /// The config folder the agent will use, where it keeps its conversations.
    pub fn config_home(&self, kind: AgentKind) -> ConfigHome {
        self.config_home_with(kind, &|name| env::var_os(name))
    }

    /// Same as [`Self::config_home`], reading the app's own environment through `app`: the value of
    /// the agent's variable that the terminal ends up with, given or inherited from the app.
    fn config_home_with(
        &self,
        kind: AgentKind,
        app: &dyn Fn(&str) -> Option<std::ffi::OsString>,
    ) -> ConfigHome {
        let Some(variable) = crate::agent_account_env::config_variable(kind) else {
            return ConfigHome::Default;
        };
        let given = self
            .base
            .iter()
            .chain(&self.account)
            .rev()
            .find(|(name, _)| name == variable)
            .map(|(_, value)| std::ffi::OsString::from(value));
        match given.or_else(|| app(variable)) {
            None => ConfigHome::Default,
            Some(value) => {
                let folder = PathBuf::from(value);
                // An empty or relative folder depends on how the CLI reads it, so it is not guessed.
                if folder.as_os_str().is_empty() || !folder.is_absolute() {
                    ConfigHome::Unknown
                } else {
                    ConfigHome::Folder(folder)
                }
            }
        }
    }

    /// The finished environment, with the adapter's variables placed before the account's.
    pub fn finish(self, adapter: Vec<(String, String)>) -> Vec<(String, String)> {
        let mut all = self.base;
        all.extend(adapter);
        all.extend(self.account);
        all
    }
}

/// The config folder an agent will use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigHome {
    /// No folder is set, so the CLI uses its default one.
    Default,
    /// This folder.
    Folder(PathBuf),
    /// A folder is set but cannot be told for sure, such as an empty or relative one.
    Unknown,
}

impl ConfigHome {
    /// The folder for lookups that take `None` as the CLI's default folder.
    pub fn folder(&self) -> Option<&Path> {
        match self {
            Self::Folder(folder) => Some(folder),
            Self::Default | Self::Unknown => None,
        }
    }
}

/// What a task's agent launch takes from the database: the task profile's additions and the
/// environment of the task's account.
#[derive(Default)]
pub(crate) struct AgentSetup {
    pub profile: Option<crate::agent_profiles::LaunchAdditions>,
    pub account: Vec<(String, String)>,
}

fn agent_setup(
    connection: &rusqlite::Connection,
    accounts: &crate::agent_account_env::AccountsFolder,
    task: &brachio_store::TaskRecord,
    kind: AgentKind,
) -> CommandResult<AgentSetup> {
    let repo = brachio_store::RepoRecord::get(connection, &task.repo_id)
        .map_err(|error| error.to_string())?;
    Ok(AgentSetup {
        profile: crate::agent_profiles::launch_additions(connection, &task.id, &repo, kind)?,
        account: crate::agent_account_env::launch_environment(
            connection,
            &accounts.0,
            &task.id,
            kind,
        )?,
    })
}

/// The terminal's environment: `base` (the app's variables), then the profile's, then the
/// account's last. The profile's arguments are added to `options`.
fn assemble_environment(
    mut base: Vec<(String, String)>,
    setup: AgentSetup,
    options: &mut Vec<String>,
) -> LaunchEnvironment {
    if let Some(added) = setup.profile {
        options.extend(added.arguments);
        base.extend(added.environment);
    }
    LaunchEnvironment {
        base,
        account: setup.account,
    }
}

/// Where the agent keeps its conversations, read from the folder the terminal will be given.
pub(crate) struct ConversationLookups<'a> {
    /// Whether Claude Code kept a conversation id under a config folder.
    pub claude: &'a dyn Fn(Option<&Path>, &str) -> Option<bool>,
    /// The Codex conversations started in a folder, under a config folder.
    pub codex: &'a dyn Fn(Option<&Path>, &Path) -> Vec<brachio_agent::CodexConversation>,
}

const DISK_LOOKUPS: ConversationLookups<'static> = ConversationLookups {
    claude: &brachio_agent::claude_conversation_recorded,
    codex: &account_codex_conversations,
};

/// The command for an agent: its earlier conversation when asked to resume
/// and one exists, otherwise a new one.
fn agent_command(
    kind: AgentKind,
    request: &OpenRequest,
    options: Vec<String>,
    environment: &LaunchEnvironment,
) -> CommandResult<(ResolvedCommand, PromptDelivery, bool)> {
    agent_launch(
        &installed(kind)?,
        request,
        options,
        environment,
        &DISK_LOOKUPS,
    )
}

fn agent_launch(
    installed: &Installed,
    request: &OpenRequest,
    mut options: Vec<String>,
    environment: &LaunchEnvironment,
    lookups: &ConversationLookups<'_>,
) -> CommandResult<(ResolvedCommand, PromptDelivery, bool)> {
    let kind = installed.kind;
    let home = environment.config_home(kind);
    let config_home = home.folder();
    let conversations = |cwd: &Path| match home {
        ConfigHome::Unknown => Vec::new(),
        ConfigHome::Default | ConfigHome::Folder(_) => (lookups.codex)(config_home, cwd),
    };
    let from_project = kind == AgentKind::Codex
        && request.codex_conversation_id.is_none()
        && request.session_id.as_deref().is_some_and(|chosen| {
            codex_conversation_allowed(chosen, request, Some(chosen), &conversations)
        });
    let earlier = if let Some(chosen) = &request.codex_conversation_id {
        if !codex_conversation_allowed(
            chosen,
            request,
            request.session_id.as_deref(),
            &conversations,
        ) {
            return Err(
                "The chosen Codex conversation is no longer available for this worktree."
                    .to_owned(),
            );
        }
        Some(chosen.clone())
    } else if from_project {
        request.session_id.clone()
    } else if request.resume {
        earlier_conversation(kind, request.session_id.as_deref()).filter(|id| {
            conversation_may_exist(kind, id, &|id| match home {
                // Resuming is tried, as when the folder cannot be read.
                ConfigHome::Unknown => None,
                ConfigHome::Default | ConfigHome::Folder(_) => (lookups.claude)(config_home, id),
            })
        })
    } else {
        None
    };
    if let Some(conversation) = earlier {
        let mut launch = kind.resume(&conversation);
        options.append(&mut launch.args);
        launch.args = options;
        let command = installed
            .resolve(&launch)
            .map_err(|error| error.to_string())?;
        // A task created from a project conversation still owes the user's first prompt.
        let delivery = if from_project
            && !request.resume
            && request
                .prompt
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())
        {
            PromptDelivery::TypeIn
        } else {
            PromptDelivery::None
        };
        return Ok((command, delivery, true));
    }
    let session_id = request
        .session_id
        .as_deref()
        .filter(|_| kind.accepts_session_id());
    let (command, delivery) = installed
        .resolve_interactive(request.prompt.as_deref(), session_id, options)
        .map_err(|error| error.to_string())?;
    Ok((command, delivery, false))
}

fn account_codex_conversations(
    home: Option<&Path>,
    cwd: &Path,
) -> Vec<brachio_agent::CodexConversation> {
    match home {
        Some(home) => brachio_agent::codex_conversations_in(home, cwd),
        None => brachio_agent::codex_conversations(cwd),
    }
}

/// Whether Codex may resume `chosen` in this launch: a conversation recorded for the worktree,
/// or one recorded for the project folder when it is the conversation the task was created from.
fn codex_conversation_allowed(
    chosen: &str,
    request: &OpenRequest,
    created_from: Option<&str>,
    lookup: &dyn Fn(&Path) -> Vec<brachio_agent::CodexConversation>,
) -> bool {
    let recorded_for =
        |folder: &Path| crate::workspace::project_conversation_exists(&lookup(folder), chosen);
    if recorded_for(&request.cwd) {
        return true;
    }
    created_from == Some(chosen) && request.project_root.as_deref().is_some_and(recorded_for)
}

/// Resume only an identity known to belong to this tab.
fn earlier_conversation(kind: AgentKind, session_id: Option<&str>) -> Option<String> {
    match kind {
        AgentKind::ClaudeCode => session_id.map(str::to_owned),
        // The Codex tab UUID identifies the PTY, not a Codex conversation.
        AgentKind::Codex | AgentKind::Grok => None,
    }
}

/// Whether resuming `id` can find a conversation. Claude Code is given its conversation id before
/// it starts, but writes nothing under it until the first message, so a tab whose agent never
/// started or was never spoken to has nothing to resume: it starts a new conversation under the
/// same id instead of failing. When that cannot be told, resuming is tried.
fn conversation_may_exist(
    kind: AgentKind,
    id: &str,
    claude_recorded: &dyn Fn(&str) -> Option<bool>,
) -> bool {
    kind != AgentKind::ClaudeCode || claude_recorded(id) != Some(false)
}

fn installed(kind: AgentKind) -> CommandResult<Installed> {
    brachio_agent::locate(kind)
        .ok_or_else(|| format!("{} is not installed or not on PATH", kind.program()))
}

/// The shell a tab asked for, else the one chosen in Settings, else the system's own.
fn shell_command(requested: Option<&str>) -> ResolvedCommand {
    match requested
        .and_then(crate::terminal_shells::find)
        .or_else(crate::terminal_shells::chosen_default)
    {
        Some(shell) => ResolvedCommand {
            program: shell.program,
            args: shell.args,
            env: Vec::new(),
        },
        None => {
            let program = default_shell();
            ResolvedCommand {
                args: brachio_pty::login_shell_arguments(&program),
                program,
                env: Vec::new(),
            }
        }
    }
}

fn sink_for(channel: Channel<InvokeResponseBody>, agent: bool) -> EventSink {
    let status = agent.then(|| TerminalStatus::new(channel.clone()));
    Arc::new(move |event| {
        let message = match event {
            PtyEvent::Output(bytes) => {
                if let Some(status) = &status {
                    status.activity();
                }
                send(&channel, InvokeResponseBody::Raw(bytes));
                return;
            }
            PtyEvent::Exit(exit_code) => ChannelMessage::Exit { exit_code },
            PtyEvent::Status(report) => {
                if let Some(status) = &status {
                    status.osc(report.state, report.message);
                }
                return;
            }
        };
        if let Ok(json) = serde_json::to_string(&message) {
            send(&channel, InvokeResponseBody::Json(json));
        }
    })
}

pub(crate) fn send_message(channel: &Channel<InvokeResponseBody>, message: &ChannelMessage) {
    if let Ok(json) = serde_json::to_string(message) {
        send(channel, InvokeResponseBody::Json(json));
    }
}

pub(crate) fn send(channel: &Channel<InvokeResponseBody>, body: InvokeResponseBody) {
    if let Err(error) = channel.send(body) {
        tracing::debug!("terminal channel closed: {error}");
    }
}

/// Which command language the shell that new shell tabs start speaks, so setup commands can
/// report how they ended in that shell's own syntax.
#[tauri::command]
pub fn terminal_shell_kind() -> &'static str {
    shell_kind(&shell_command(None).program)
}

fn shell_kind(program: &std::path::Path) -> &'static str {
    let name = program
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match name.as_str() {
        "bash" | "zsh" | "sh" | "dash" | "ksh" => "posix",
        "powershell" | "pwsh" => "powershell",
        "cmd" => "cmd",
        _ => "none",
    }
}

fn default_shell() -> PathBuf {
    let (variable, fallback) = if cfg!(windows) {
        ("COMSPEC", "cmd.exe")
    } else if cfg!(target_os = "macos") {
        ("SHELL", "/bin/zsh")
    } else {
        ("SHELL", "/bin/sh")
    };
    env::var_os(variable).map_or_else(|| PathBuf::from(fallback), PathBuf::from)
}

fn terminal_env() -> Vec<(String, String)> {
    vec![
        ("TERM".to_owned(), "xterm-256color".to_owned()),
        ("COLORTERM".to_owned(), "truecolor".to_owned()),
        ("TERM_PROGRAM".to_owned(), "Brachio".to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn shells_are_told_apart_by_program_name() {
        let kind = |path: &str| shell_kind(std::path::Path::new(path));
        assert_eq!(kind("/bin/zsh"), "posix");
        assert_eq!(kind("/usr/bin/bash"), "posix");
        assert_eq!(kind("cmd.exe"), "cmd");
        assert_eq!(kind("pwsh.exe"), "powershell");
        assert_eq!(kind("/usr/bin/fish"), "none");
    }

    fn open_request(cwd: &str, root: Option<&str>) -> OpenRequest {
        OpenRequest {
            agent: Some(AgentKind::Codex),
            prompt: None,
            resume: false,
            session_id: None,
            codex_conversation_id: None,
            project_root: root.map(PathBuf::from),
            cwd: PathBuf::from(cwd),
            rows: 24,
            cols: 80,
            key: None,
        }
    }

    #[test]
    fn a_project_conversation_is_accepted_only_for_the_task_created_from_it() {
        const ID: &str = "11111111-2222-4333-8444-555555555555";
        let lookup = |folder: &Path| {
            if folder == Path::new("project") {
                vec![brachio_agent::CodexConversation {
                    id: ID.to_owned(),
                    timestamp: None,
                }]
            } else {
                Vec::new()
            }
        };
        let request = open_request("worktree", Some("project"));
        assert!(codex_conversation_allowed(ID, &request, Some(ID), &lookup));
        assert!(!codex_conversation_allowed(ID, &request, None, &lookup));
        assert!(!codex_conversation_allowed(
            ID,
            &open_request("worktree", None),
            Some(ID),
            &lookup
        ));
        assert!(!codex_conversation_allowed(
            "not-a-uuid",
            &request,
            Some("not-a-uuid"),
            &lookup
        ));
    }

    #[test]
    fn a_claude_tab_without_a_kept_conversation_starts_a_new_one() {
        const ID: &str = "11111111-2222-4333-8444-555555555555";
        let kept = |answer: Option<bool>| move |_: &str| answer;
        assert!(conversation_may_exist(
            AgentKind::ClaudeCode,
            ID,
            &kept(Some(true))
        ));
        assert!(!conversation_may_exist(
            AgentKind::ClaudeCode,
            ID,
            &kept(Some(false))
        ));
        assert!(
            conversation_may_exist(AgentKind::ClaudeCode, ID, &kept(None)),
            "resuming is tried when the folder cannot be read"
        );
        assert!(conversation_may_exist(
            AgentKind::Codex,
            ID,
            &kept(Some(false))
        ));
    }

    fn variable(name: &str, value: &str) -> (String, String) {
        (name.to_owned(), value.to_owned())
    }

    /// The value of `name` the terminal ends up with.
    fn last_value(environment: &[(String, String)], name: &str) -> Option<String> {
        environment
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn conversations_are_looked_up_in_the_config_folder_the_agent_is_given() {
        const ID: &str = "11111111-2222-4333-8444-555555555555";
        let claude = Installed {
            kind: AgentKind::ClaudeCode,
            path: PathBuf::from("/fake-home/bin/claude"),
            version: None,
        };
        let request = OpenRequest {
            agent: Some(AgentKind::ClaudeCode),
            resume: true,
            session_id: Some(ID.to_owned()),
            ..open_request("worktree", None)
        };
        let profile = || {
            Some(crate::agent_profiles::LaunchAdditions {
                arguments: vec!["--model".to_owned(), "x".to_owned()],
                environment: vec![variable("CLAUDE_CONFIG_DIR", "/fake-home/profile-claude")],
            })
        };
        let setups = [
            AgentSetup {
                profile: profile(),
                account: Vec::new(),
            },
            AgentSetup {
                profile: profile(),
                account: vec![variable("CLAUDE_CONFIG_DIR", "/fake-home/account-claude")],
            },
        ];
        let expected = ["/fake-home/profile-claude", "/fake-home/account-claude"];
        for (setup, expected) in setups.into_iter().zip(expected) {
            let mut options = Vec::new();
            let environment = assemble_environment(
                vec![variable("TERM", "xterm-256color")],
                setup,
                &mut options,
            );
            assert_eq!(options, ["--model", "x"]);
            let asked = Mutex::new(Vec::new());
            let claude_lookup = |folder: Option<&Path>, id: &str| {
                asked
                    .lock()
                    .expect("lock")
                    .push((folder.map(Path::to_path_buf), id.to_owned()));
                Some(true)
            };
            let lookups = ConversationLookups {
                claude: &claude_lookup,
                codex: &|_, _| Vec::new(),
            };
            let (command, _, resumed) =
                agent_launch(&claude, &request, Vec::new(), &environment, &lookups)
                    .expect("launch");
            assert!(resumed);
            assert!(command.args.contains(&"--resume".to_owned()), "{command:?}");
            let given = last_value(&environment.finish(command.env), "CLAUDE_CONFIG_DIR")
                .expect("the terminal is given a config folder");
            assert_eq!(given, expected);
            assert_eq!(
                asked.into_inner().expect("lock"),
                [(Some(PathBuf::from(given)), ID.to_owned())]
            );
        }
    }

    #[test]
    fn a_codex_conversation_is_checked_in_the_folder_codex_is_given() {
        const ID: &str = "11111111-2222-4333-8444-555555555555";
        let codex = Installed {
            kind: AgentKind::Codex,
            path: PathBuf::from("/fake-home/bin/codex"),
            version: None,
        };
        let request = OpenRequest {
            resume: true,
            codex_conversation_id: Some(ID.to_owned()),
            ..open_request("worktree", None)
        };
        let environment = assemble_environment(
            Vec::new(),
            AgentSetup {
                profile: Some(crate::agent_profiles::LaunchAdditions {
                    arguments: Vec::new(),
                    environment: vec![variable("CODEX_HOME", "/fake-home/profile-codex")],
                }),
                account: Vec::new(),
            },
            &mut Vec::new(),
        );
        let codex_lookup = |folder: Option<&Path>, _: &Path| {
            if folder == Some(Path::new("/fake-home/profile-codex")) {
                vec![brachio_agent::CodexConversation {
                    id: ID.to_owned(),
                    timestamp: None,
                }]
            } else {
                Vec::new()
            }
        };
        let lookups = ConversationLookups {
            claude: &|_, _| None,
            codex: &codex_lookup,
        };
        let (command, _, resumed) =
            agent_launch(&codex, &request, Vec::new(), &environment, &lookups)
                .expect("the conversation is found in the profile's folder");
        assert!(resumed);
        assert!(command.args.contains(&ID.to_owned()), "{command:?}");
    }

    #[test]
    fn an_empty_or_relative_config_folder_is_not_guessed() {
        let given = |value: &str| LaunchEnvironment {
            base: Vec::new(),
            account: vec![variable("CLAUDE_CONFIG_DIR", value)],
        };
        let none = |_: &str| None;
        let claude = AgentKind::ClaudeCode;
        assert_eq!(
            given("").config_home_with(claude, &none),
            ConfigHome::Unknown
        );
        assert_eq!(
            given("relative/claude").config_home_with(claude, &none),
            ConfigHome::Unknown
        );
        assert_eq!(
            given("/fake-home/claude").config_home_with(claude, &none),
            ConfigHome::Folder(PathBuf::from("/fake-home/claude"))
        );
        let unset = LaunchEnvironment {
            base: Vec::new(),
            account: Vec::new(),
        };
        assert_eq!(unset.config_home_with(claude, &none), ConfigHome::Default);
        let app = |value: &'static str| move |_: &str| Some(std::ffi::OsString::from(value));
        assert_eq!(
            unset.config_home_with(claude, &app("/fake-home/app")),
            ConfigHome::Folder(PathBuf::from("/fake-home/app"))
        );
        assert_eq!(
            unset.config_home_with(claude, &app("")),
            ConfigHome::Unknown,
            "an empty folder the app passes on is not guessed either"
        );
        assert_eq!(
            unset.config_home_with(AgentKind::Grok, &app("/fake-home/app")),
            ConfigHome::Default
        );
    }

    #[test]
    fn a_claude_tab_with_an_unknown_config_folder_still_resumes() {
        const ID: &str = "11111111-2222-4333-8444-555555555555";
        let claude = Installed {
            kind: AgentKind::ClaudeCode,
            path: PathBuf::from("/fake-home/bin/claude"),
            version: None,
        };
        let request = OpenRequest {
            agent: Some(AgentKind::ClaudeCode),
            resume: true,
            session_id: Some(ID.to_owned()),
            ..open_request("worktree", None)
        };
        for folder in ["", "relative/claude"] {
            let environment = LaunchEnvironment {
                base: Vec::new(),
                account: vec![variable("CLAUDE_CONFIG_DIR", folder)],
            };
            let lookups = ConversationLookups {
                claude: &|_, _| panic!("an unknown folder is not searched"),
                codex: &|_, _| Vec::new(),
            };
            let (command, _, resumed) =
                agent_launch(&claude, &request, Vec::new(), &environment, &lookups)
                    .expect("launch");
            assert!(resumed);
            assert!(command.args.contains(&"--resume".to_owned()), "{command:?}");
        }
    }

    #[test]
    fn channel_messages_use_camel_case_json() {
        let exit = serde_json::to_string(&ChannelMessage::Exit { exit_code: Some(3) })
            .expect("serialize exit");
        assert_eq!(exit, r#"{"exitCode":3}"#);

        let status = serde_json::to_string(&ChannelMessage::Status {
            status: AgentState::Working,
            message: None,
        })
        .expect("serialize status");
        assert_eq!(status, r#"{"status":{"state":"working"},"message":null}"#);
    }
}

#[cfg(test)]
mod session_key_tests {
    use std::sync::Arc;

    use brachio_pty::{EventSink, SessionId, SessionManager, SpawnSpec};

    use super::SessionKeys;

    fn long_running() -> SpawnSpec {
        let (program, args) = if cfg!(windows) {
            ("cmd.exe", vec!["/c", "ping", "-n", "60", "127.0.0.1"])
        } else {
            ("sleep", vec!["60"])
        };
        SpawnSpec {
            program: program.to_owned(),
            args: args.into_iter().map(str::to_owned).collect(),
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn a_running_in_app_session_is_found_by_its_task_and_a_gone_one_is_not() {
        let manager = Arc::new(SessionManager::default());
        let sink: EventSink = Arc::new(|_| {});
        let live = manager.spawn(&long_running(), sink).expect("spawn");
        let keys = SessionKeys::default();
        keys.record(live, "t1:agent".to_owned());
        keys.record(SessionId::from_raw(999), "t1:shell".to_owned());
        keys.record(live, "t1:agent".to_owned());

        assert_eq!(
            keys.live_for_task(&manager, "t1").expect("live"),
            ["t1:agent"]
        );
        assert!(keys.live_for_task(&manager, "t2").expect("live").is_empty());
        manager.kill(live).expect("kill");
    }
}

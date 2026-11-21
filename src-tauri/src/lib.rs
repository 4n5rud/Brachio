//! App shell: window, tray, IPC commands. Domain logic stays in the crates.

mod agent_accounts;
mod child_task_request;
mod external_link;
mod hook_setup;
mod hook_socket;
mod issue_report;
mod log_file;
mod mailbox;
mod orchestration;
mod pr_status;
mod qa_run;
mod resource_usage;
mod runtime_client;
mod runtime_terminals;
mod task_ports;
mod task_tabs;
mod terminal;
mod terminal_authorization;
mod terminal_status;
mod update_release;
mod workspace;

use std::fs;
use std::sync::{Arc, Mutex};

use brachio_pty::SessionManager;
use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager};

const TRAY_TASK_PREFIX: &str = "task:";
const TRAY_TASK_LIMIT: usize = 15;

struct TrayTasks(Submenu<tauri::Wry>);

#[derive(Deserialize)]
struct TrayTask {
    id: String,
    label: String,
}

#[tauri::command]
fn tray_update_tasks(app: AppHandle, tasks: Vec<TrayTask>, label: String) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let Some(menu) = handle.try_state::<TrayTasks>() else {
            return;
        };
        if let Err(error) = update_tray_tasks(&handle, &menu.0, tasks, label) {
            tracing::warn!(%error, "could not update tray tasks");
        }
    })
    .map_err(|error| error.to_string())
}

fn update_tray_tasks(
    app: &AppHandle,
    menu: &Submenu<tauri::Wry>,
    tasks: Vec<TrayTask>,
    label: String,
) -> tauri::Result<()> {
    let mut items = Vec::new();
    for task in tasks.into_iter().take(TRAY_TASK_LIMIT) {
        let id = format!("{TRAY_TASK_PREFIX}{}", task.id);
        let title = task.label.replace('&', "&&");
        items.push(MenuItem::with_id(app, id, title, true, None::<&str>)?);
    }
    for item in menu.items()? {
        menu.remove(&item)?;
    }
    for item in &items {
        menu.append(item)?;
    }
    menu.set_text(label)?;
    menu.set_enabled(!items.is_empty())
}

/// The app version, for the about box.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Opens a loopback development URL in the system browser.
#[tauri::command]
async fn open_local_port(port: std::num::NonZeroU16, secure: bool) -> Result<(), String> {
    let scheme = if secure { "https" } else { "http" };
    let url = format!("{scheme}://127.0.0.1:{port}/");
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let mut command = {
            use std::os::windows::process::CommandExt;

            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let mut command = std::process::Command::new("cmd.exe");
            command
                .creation_flags(CREATE_NO_WINDOW)
                .args(["/d", "/c", "start", ""]);
            command
        };
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let mut command = std::process::Command::new("xdg-open");
        let status = command
            .arg(url)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Could not open the default browser ({status})."))
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Logs to a file next to the app data, because the release build has no console to print to.
/// `RUST_LOG` overrides the default `info` level.
fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let file = log_file_path().and_then(|path| {
        let dir = path.parent()?;
        fs::create_dir_all(dir).ok()?;
        log_file::RotatingLog::open(
            dir,
            path.file_name()?.to_str()?,
            log_file::MAX_FILE_BYTES,
            log_file::MAX_ARCHIVES,
        )
        .ok()
    });
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let _ = match file {
        Some(file) => builder
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .try_init(),
        None => builder.try_init(),
    };
}

fn log_file_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")?.into()
    } else if cfg!(target_os = "macos") {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join("Library/Logs")
    } else {
        std::env::var_os("XDG_STATE_HOME").map_or_else(
            || {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join(".local/state")
            },
            std::path::PathBuf::from,
        )
    };
    Some(base.join("Brachio").join("brachio.log"))
}

/// Builds and runs the Tauri app.
///
/// # Panics
/// Panics if the Tauri runtime fails to start, which leaves nothing to show.
pub fn run() {
    init_logging();
    tauri::Builder::default()
        // A second launch would open the same database and start agents the first knows nothing of.
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _cwd| {
                show_main_window(app);
            },
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(terminal::Terminals(
            Arc::new(SessionManager::default()),
            terminal::SessionKeys::default(),
        ))
        .manage(runtime_terminals::RuntimeTerminals::default())
        .manage(pr_status::PrStatuses::default())
        .manage(Arc::new(qa_run::QaResults::default()))
        .manage(resource_usage::ResourceMonitor::default())
        .setup(|app| {
            open_database(app)?;
            start_approvals(app)?;
            if let Err(error) = install_tray(app) {
                tracing::warn!(%error, "could not install tray");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            resource_usage::resource_usage,
            tray_update_tasks,
            terminal::agents_detect,
            update_release::open_update_release,
            terminal::terminal_open,
            terminal::terminal_forget_uncertain_launch,
            terminal::terminal_codex_conversations,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            terminal::terminal_detach,
            terminal::terminal_park,
            task_tabs::task_secondary_tabs,
            task_tabs::task_set_secondary_tab,
            workspace::task_drop_paths,
            hook_socket::approval_answer,
            hook_socket::pending_approvals,
            mailbox::mailbox_unread,
            mailbox::mailbox_inbox,
            orchestration::orchestration_view,
            orchestration::handoff_review,
            orchestration::orchestration_set_scheduler,
            orchestration::orchestration_schedule,
            workspace::repo_list,
            workspace::repo_add,
            workspace::repo_remove,
            workspace::folder_source_repos,
            agent_accounts::agent_auth_status,
            agent_accounts::codex_auth_action,
            agent_accounts::codex_usage,
            agent_accounts::open_claude_usage,
            workspace::task_list,
            workspace::task_create,
            workspace::task_archive,
            workspace::task_retry_setup_copy,
            workspace::task_list_archived,
            workspace::task_restore,
            workspace::task_delete_preview,
            workspace::task_delete,
            workspace::task_files,
            workspace::task_file_preview,
            workspace::task_file_media,
            workspace::task_changes,
            pr_status::task_pr_status,
            qa_run::qa_run,
            qa_run::qa_last_result,
            external_link::open_external_link,
            issue_report::open_issue_draft,
            workspace::task_merge_preview,
            task_ports::task_assign_port,
            workspace::task_pr_feedback,
            workspace::task_git_action,
            open_local_port,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Brachio");
}

/// Starts listening for permission questions from agents' hooks and prepares the hook that
/// Claude Code sessions get. If the socket cannot be taken, agents keep asking in the terminal.
fn start_approvals(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = runtime_client::hooks_endpoint();
    let hooks_folder = app.path().app_data_dir()?.join("hooks");
    app.manage(hook_setup::HookSetup::prepare(
        &hooks_folder,
        endpoint.clone(),
    ));

    let knows = app.handle().clone();
    let notify = app.handle().clone();
    let mail_app = app.handle().clone();
    let delivered_app = app.handle().clone();
    let approvals = hook_socket::Approvals::with_delivery(
        move |key| task_is_open(&knows, key),
        move |notice| {
            let sent = match notice {
                hook_socket::Notice::Asked(question) => notify.emit("approval-asked", question),
                hook_socket::Notice::Closed(id) => notify.emit("approval-closed", id),
            };
            if let Err(error) = sent {
                tracing::debug!(%error, "could not tell the window about a permission question");
            }
        },
        move |request| mailbox::serve(&mail_app, request),
        move |ids| mailbox::mark_delivered(&delivered_app, ids),
        child_task_request::AppChildTasks(app.handle().clone()),
    );
    app.manage(Arc::clone(&approvals));
    tauri::async_runtime::spawn(async move {
        match brachio_runtime::transport::listen(&endpoint) {
            Ok(listener) => approvals.serve(listener).await,
            Err(error) => tracing::warn!(%error, "in-app approval is off"),
        }
    });
    Ok(())
}

/// Whether the primary or secondary terminal belongs to an open task.
fn task_is_open(app: &tauri::AppHandle, key: &str) -> bool {
    let database = app.state::<workspace::Database>();
    let Ok(connection) = database.lock() else {
        return false;
    };
    terminal_authorization::resolve(&connection, key)
        .is_ok_and(|owner| owner.agent == Some(brachio_agent::AgentKind::ClaudeCode))
}

fn install_tray(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Brachio", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Brachio", true, None::<&str>)?;
    let tasks = Submenu::with_id(app, "open-tasks", "Open tasks", false)?;
    let menu = Menu::with_items(app, &[&show, &tasks, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("Brachio")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            id => {
                if let Some(task_id) = id.strip_prefix(TRAY_TASK_PREFIX) {
                    show_main_window(app);
                    if let Err(error) = app.emit_to("main", "tray-open-task", task_id) {
                        tracing::warn!(%error, "could not open task from the tray");
                    }
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    app.manage(TrayTasks(tasks));
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let result = window
            .show()
            .and_then(|()| window.unminimize())
            .and_then(|()| window.set_focus());
        if let Err(error) = result {
            tracing::warn!(%error, "could not show the main window");
        }
    }
}

/// Opens `brachio.db` in the app's data folder and makes it app state.
fn open_database(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let folder = app.path().app_data_dir()?;
    fs::create_dir_all(&folder)?;
    let connection = brachio_store::open(&folder.join("brachio.db"))?;
    const READ_MAIL_KEPT_MS: i64 = 7 * 24 * 60 * 60 * 1000;
    let cutoff = pr_status::now_ms() - READ_MAIL_KEPT_MS;
    if let Err(error) = brachio_store::expire_read_messages(&connection, cutoff) {
        tracing::warn!(%error, "could not remove old read messages");
    }
    app.manage(workspace::Database(Mutex::new(connection)));
    app.manage(workspace::TaskWorktreeLocks::default());
    Ok(())
}

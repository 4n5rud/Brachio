//! App shell: window, tray, IPC commands. Domain logic stays in the crates.

mod agent_accounts;
mod agent_profiles;
mod always_allow;
mod auto_archive;
mod child_escalation;
mod child_task_request;
mod claude_usage;
mod command_words;
mod decision_gate;
mod external_link;
mod file_in_use;
mod folder_access;
mod handoff_summary;
mod hook_setup;
mod hook_socket;
mod issue_report;
mod keep_awake;
mod log_file;
mod mailbox;
mod orchestration;
mod pending_worktrees;
mod pr_status;
mod project_add;
mod project_cues;
mod project_permissions;
mod prompt_history;
mod qa_run;
mod quit;
mod request_receipts;
mod resource_usage;
mod runtime_client;
mod runtime_terminals;
mod scratch_workspace;
mod session_tokens;
mod system_check;
mod task_activity;
mod task_attachments;
mod task_checkpoints;
mod task_commits;
mod task_file_actions;
mod task_file_edit;
mod task_file_ops;
mod task_folder;
mod task_overlaps;
mod task_pin;
mod task_ports;
mod task_tabs;
#[cfg(any(windows, test))]
mod taskbar_badge;
mod terminal;
mod terminal_authorization;
mod terminal_shells;
mod terminal_status;
mod tray_menu;
mod update_release;
mod usage_analytics;
mod worker_preamble;
mod workspace;
mod worktree_import;

#[cfg(test)]
mod ui_event_tests;

use std::fs;
use std::sync::{Arc, Mutex};

use brachio_pty::SessionManager;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager};

const TRAY_TASK_PREFIX: &str = "task:";

struct TrayHandle(tauri::tray::TrayIcon<tauri::Wry>);

/// Rebuilds the tray menu from the sections the window lists and sets the taskbar badge.
/// The window owns task statuses, so it sends the lists rather than Rust re-deriving them.
#[tauri::command]
fn tray_update(
    app: AppHandle,
    needs: Vec<tray_menu::TrayTask>,
    working: Vec<tray_menu::TrayTask>,
    labels: tray_menu::TrayLabels,
    badge: bool,
) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Err(error) = apply_tray(&handle, &needs, &working, &labels, badge) {
            tracing::warn!(%error, "could not update the tray");
        }
    })
    .map_err(|error| error.to_string())
}

fn apply_tray(
    app: &AppHandle,
    needs: &[tray_menu::TrayTask],
    working: &[tray_menu::TrayTask],
    labels: &tray_menu::TrayLabels,
    badge: bool,
) -> tauri::Result<()> {
    if let Some(tray) = app.try_state::<TrayHandle>() {
        tray.0
            .set_menu(Some(build_tray_menu(app, needs, working, labels)?))?;
    }
    set_taskbar_badge(app, if badge { needs.len() } else { 0 });
    Ok(())
}

fn build_tray_menu(
    app: &AppHandle,
    needs: &[tray_menu::TrayTask],
    working: &[tray_menu::TrayTask],
    labels: &tray_menu::TrayLabels,
) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    let entries = tray_menu::task_entries(labels, needs, working);
    for entry in &entries {
        match entry {
            tray_menu::Entry::Header(title) => {
                let title = title.replace('&', "&&");
                menu.append(&MenuItem::new(app, title, false, None::<&str>)?)?;
            }
            tray_menu::Entry::Task { id, label } => {
                let id = format!("{TRAY_TASK_PREFIX}{id}");
                let title = label.replace('&', "&&");
                menu.append(&MenuItem::with_id(app, id, title, true, None::<&str>)?)?;
            }
            tray_menu::Entry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    if !entries.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(
        app,
        "new-task",
        &labels.new_task,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "show",
        &labels.show,
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        &labels.quit,
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

/// Windows shows the count as an overlay on the taskbar button; macOS keeps the Dock badge the
/// window sets, and Linux has no equivalent.
fn set_taskbar_badge(app: &AppHandle, count: usize) {
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window("main") {
        let icon = (count > 0).then(|| {
            tauri::image::Image::new_owned(
                taskbar_badge::badge_rgba(count),
                taskbar_badge::badge_size(),
                taskbar_badge::badge_size(),
            )
        });
        if let Err(error) = window.set_overlay_icon(icon) {
            tracing::warn!(%error, "could not set the taskbar badge");
        }
    }
    #[cfg(not(windows))]
    let _ = (app, count);
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(brachio_power::KeepAwake::start())
        .plugin(tauri_plugin_notification::init())
        .manage(terminal::Terminals(
            Arc::new(SessionManager::default()),
            terminal::SessionKeys::default(),
        ))
        .manage(runtime_terminals::RuntimeTerminals::default())
        .manage(project_add::CloneJobs::default())
        .manage(pr_status::PrStatuses::default())
        .manage(session_tokens::TokenTrackers::default())
        .manage(usage_analytics::UsageScanLock::default())
        .manage(task_overlaps::ChangedPathCache::default())
        .manage(task_checkpoints::TurnTracker::default())
        .manage(agent_accounts::SignInRun::default())
        .manage(claude_usage::ClaudeUsageCache::default())
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
            resource_usage::task_processes,
            resource_usage::task_process_kill,
            tray_update,
            keep_awake::keep_awake_status,
            keep_awake::keep_awake_set,
            terminal::agents_detect,
            update_release::open_update_release,
            terminal::terminal_open,
            terminal::terminal_forget_uncertain_launch,
            terminal::terminal_saved_screen,
            terminal::terminal_codex_conversations,
            workspace::repo_codex_conversations,
            terminal::terminal_write,
            terminal::terminal_shell_kind,
            terminal::terminal_resize,
            terminal::terminal_close,
            terminal::terminal_detach,
            terminal::terminal_park,
            quit::terminals_stop,
            quit::app_quit,
            session_tokens::tasks_token_usage,
            usage_analytics::usage_providers,
            usage_analytics::usage_scan,
            usage_analytics::usage_report,
            usage_analytics::usage_limit_snapshot,
            project_cues::cue_list,
            project_cues::cue_add,
            project_cues::cue_delete,
            prompt_history::prompt_history_list,
            prompt_history::prompt_history_add,
            system_check::system_check,
            task_pin::task_pins_list,
            task_pin::task_pin_set,
            task_activity::task_activity_list,
            task_activity::task_activity_record,
            task_tabs::task_open_tabs,
            task_tabs::task_add_tab,
            task_tabs::task_close_tab,
            task_tabs::task_rename_tab,
            workspace::task_rename,
            task_tabs::task_reorder_tabs,
            workspace::repo_reorder,
            workspace::task_reorder,
            terminal_shells::shell_list,
            terminal_shells::shell_set_default,
            workspace::task_drop_paths,
            hook_socket::approval_answer,
            hook_socket::approval_answer_always,
            hook_socket::always_allow_list,
            hook_socket::always_allow_revoke,
            hook_socket::pending_approvals,
            hook_socket::approval_unavailable,
            mailbox::mailbox_unread,
            mailbox::mailbox_inbox,
            mailbox::mailbox_dismiss,
            orchestration::orchestration_view,
            child_escalation::child_agent_stopped,
            decision_gate::decision_gate_answer,
            orchestration::handoff_review,
            orchestration::orchestration_set_scheduler,
            orchestration::orchestration_schedule,
            orchestration::orchestration_reopen_child,
            worker_preamble::task_worker_preamble,
            handoff_summary::task_handoff_summary,
            handoff_summary::task_handoff_send,
            agent_profiles::repo_profiles,
            agent_profiles::task_profiles,
            agent_profiles::task_join_compare_group,
            workspace::task_has_live_terminals,
            workspace::repo_list,
            workspace::repo_add,
            project_add::project_clone,
            project_add::project_clone_cancel,
            project_add::project_init,
            project_add::repo_remote_urls,
            workspace::repo_branches,
            workspace::repo_remove,
            workspace::folder_source_repos,
            agent_accounts::agent_auth_status,
            agent_accounts::codex_auth_action,
            agent_accounts::codex_auth_cancel,
            folder_access::open_folder_access_settings,
            agent_accounts::codex_usage,
            claude_usage::claude_usage,
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
            task_attachments::task_attach_images,
            task_file_edit::task_file_read,
            task_file_edit::task_file_write,
            task_attachments::task_attach_pasted_image,
            task_file_ops::task_move_path,
            task_file_ops::task_copy_conflicts,
            task_file_ops::task_copy_in,
            task_file_actions::task_editors,
            task_file_actions::task_open_in_editor,
            task_file_actions::task_file_rename,
            task_file_actions::task_file_create,
            task_file_actions::task_file_trash,
            task_file_actions::task_file_restore,
            task_file_actions::task_discard_file,
            task_file_actions::task_discard_hunk,
            workspace::task_changes,
            worktree_import::worktree_import_candidates,
            worktree_import::worktree_import,
            worktree_import::task_subagent_worktrees,
            task_overlaps::task_overlaps,
            pr_status::task_pr_status,
            pr_status::task_link_pr,
            pr_status::task_unlink_pr,
            auto_archive::task_archive_if_merged,
            qa_run::qa_run,
            qa_run::qa_last_result,
            qa_run::qa_delete_output,
            external_link::open_external_link,
            issue_report::open_issue_draft,
            workspace::task_merge_preview,
            task_checkpoints::task_checkpoint_turn,
            task_checkpoints::task_checkpoint_list,
            task_checkpoints::task_checkpoint_diff,
            task_checkpoints::task_checkpoint_restore,
            task_commits::task_commit_list,
            task_commits::task_commit_diff,
            task_ports::task_assign_port,
            workspace::task_pr_feedback,
            scratch_workspace::scratch_create,
            scratch_workspace::scratch_list,
            scratch_workspace::scratch_convert,
            scratch_workspace::scratch_discard,
            project_permissions::repo_permissions,
            workspace::task_git_action,
            open_local_port,
            task_folder::task_reveal,
            task_folder::path_reveal,
            task_folder::task_open_folder,
            task_folder::open_log_folder,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Brachio");
}

/// Starts listening for permission questions from agents' hooks and prepares the hook that
/// Claude Code sessions get. If the socket cannot be taken, agents keep asking in the terminal.
fn start_approvals(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = runtime_client::hooks_endpoint();
    let hooks_folder = app.path().app_data_dir()?.join("hooks");
    let hook = hook_setup::HookSetup::prepare(&hooks_folder, endpoint.clone());
    // Without a token anything on this machine could answer for the user, so approval stays off.
    let token = brachio_runtime::hook_token::issue(&endpoint);

    let knows = app.handle().clone();
    let notify = app.handle().clone();
    let mail_app = app.handle().clone();
    let delivered_app = app.handle().clone();
    let approvals = hook_socket::Approvals::with_delivery(
        move |key| task_is_open(&knows, key),
        move |notice| forward_notice(&notify, notice),
        move |request| mailbox::serve(&mail_app, request),
        move |ids| mailbox::mark_delivered(&delivered_app, ids),
        child_task_request::AppChildTasks(app.handle().clone()),
    );
    let token = match token {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(%error, "in-app approval is off: no hook token");
            app.manage(hook.without_approval());
            app.manage(hook_socket::ApprovalOff(Some(error.to_string())));
            app.manage(approvals);
            return Ok(());
        }
    };
    approvals.require_token(token);
    app.manage(hook);
    app.manage(hook_socket::ApprovalOff(None));
    let policy_app = app.handle().clone();
    approvals.set_policy(move |key, tool, input| {
        project_permissions::answer(&policy_app, key, tool, input)
    });
    app.manage(Arc::clone(&approvals));
    tauri::async_runtime::spawn(async move {
        match brachio_runtime::transport::listen(&endpoint) {
            Ok(listener) => approvals.serve(listener).await,
            Err(error) => tracing::warn!(%error, "in-app approval is off"),
        }
    });
    Ok(())
}

/// Writes a request that a remembered rule allowed into the task's Activity history.
/// Tells the window about a change to the permission questions.
fn forward_notice<R: tauri::Runtime>(app: &AppHandle<R>, notice: hook_socket::Notice) {
    let sent = match notice {
        hook_socket::Notice::Asked(question) => app.emit("approval-asked", question),
        hook_socket::Notice::Closed(id) => app.emit("approval-closed", id),
        hook_socket::Notice::HungUp(id) => app.emit("approval-hung-up", id),
        hook_socket::Notice::AutoAllowed { key, summary } => {
            record_auto_allowed(app, &key, "approval_auto_allowed", &summary);
            Ok(())
        }
        hook_socket::Notice::PolicyDecided {
            key,
            summary,
            allowed,
            rule,
        } => {
            let kind = if allowed {
                "approval_policy_allowed"
            } else {
                "approval_policy_denied"
            };
            record_auto_allowed(app, &key, kind, &format!("{rule} - {summary}"));
            Ok(())
        }
    };
    if let Err(error) = sent {
        tracing::debug!(%error, "could not tell the window about a permission question");
    }
}

fn record_auto_allowed<R: tauri::Runtime>(
    app: &AppHandle<R>,
    key: &str,
    kind: &str,
    summary: &str,
) {
    let task_id = always_allow::task_of(key);
    let detail: String = brachio_qa::redact_secrets(summary, false)
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(brachio_store::MAX_ACTIVITY_DETAIL_CHARS)
        .collect();
    let database = app.state::<workspace::Database>();
    let Ok(connection) = database.lock() else {
        return;
    };
    let now = pr_status::now_ms();
    if let Err(error) =
        brachio_store::record_activity(&connection, task_id, kind, Some(&detail), now)
    {
        tracing::warn!(%error, "could not record an automatically allowed request");
    }
}

/// Whether the primary or secondary terminal belongs to an open task.
fn task_is_open(app: &tauri::AppHandle, key: &str) -> bool {
    let database = app.state::<workspace::Database>();
    let Ok(connection) = database.lock() else {
        return false;
    };
    terminal_authorization::resolve(&connection, key)
        .is_ok_and(|owner| owner.agent.is_some_and(|agent| agent.spec().approval_relay))
}

fn install_tray(app: &App) -> tauri::Result<()> {
    let labels = tray_menu::TrayLabels {
        needs: "Needs you".into(),
        working: "Working".into(),
        new_task: "New task".into(),
        show: "Show Brachio".into(),
        quit: "Quit Brachio".into(),
    };
    let menu = build_tray_menu(app.handle(), &[], &[], &labels)?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("Brachio")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => request_quit(app),
            "new-task" => {
                show_main_window(app);
                if let Err(error) = app.emit_to("main", "tray-new-task", ()) {
                    tracing::warn!(%error, "could not start a new task from the tray");
                }
            }
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
    // macOS keeps its convention of a menu on left click; elsewhere left click jumps to a waiting task.
    #[cfg(not(target_os = "macos"))]
    {
        tray = tray
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|tray, event| {
                if let tauri::tray::TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left,
                    button_state: tauri::tray::MouseButtonState::Up,
                    ..
                } = event
                {
                    let app = tray.app_handle();
                    show_main_window(app);
                    if let Err(error) = app.emit_to("main", "tray-activate", ()) {
                        tracing::warn!(%error, "could not jump to the waiting task");
                    }
                }
            });
    }
    let tray = tray.build(app)?;
    app.manage(TrayHandle(tray));
    Ok(())
}

/// Asks the window to confirm the quit; quits at once when there is no window to ask.
fn request_quit(app: &AppHandle) {
    show_main_window(app);
    if app.emit_to("main", "quit-requested", ()).is_err() {
        app.exit(0);
    }
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
    let pending = folder.join("pending-worktrees");
    pending_worktrees::recover(&pending, |worktree| {
        brachio_store::TaskRecord::list_open(&connection)
            .into_iter()
            .flatten()
            .chain(
                brachio_store::TaskRecord::list_archived(&connection)
                    .into_iter()
                    .flatten(),
            )
            .any(|task| task.id == worktree.task_id)
    });
    pending_worktrees::init(pending);
    app.manage(workspace::Database(Mutex::new(connection)));
    app.manage(workspace::TaskWorktreeLocks::default());
    app.manage(workspace::TaskCreateReceipts::default());
    Ok(())
}

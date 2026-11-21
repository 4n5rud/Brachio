# Screens and states

Every screen and every interaction state of Brachio, drawn from the features on `main` up to #261 and the strings in `ui/src/i18n/en.ts`. Each card uses only this system's tokens, so the theme switcher on this page shows any of them in all eight color sets. Sample tasks and numbers are example content.

Cards are grouped under Components: **Screens** (full windows, 1440 × 900, plus Coverage), **Interactions** and **Errors** (full windows for every interaction and failure), **States** (sheets of every state of one area), **Later** (planned, not built), plus the new components ThemePicker, PrChip, Notice and ConfirmDialog.

## The window

| Region | Size | Holds |
|---|---|---|
| Title bar | 40px | app mark, sidebar toggle, active project, Search tasks (`⌘P`), Tasks and Board, inspector toggle, Activity bell with the Needs you count, Settings; on Windows the minimize, maximize and close buttons |
| Sidebar | 288px, can hide | projects (Git repositories and folder workspaces) with their tasks; children under their lead; Archived tasks at the foot |
| Main | at least 352px | task header, tabs (agent, second agent, shell, split), pane head, notices, terminal, approval bar |
| Inspector | 360px, 560px for a side-by-side diff, can hide | Changes, Files, QA, Activity |
| Status bar | 24px | Core OK, Sessions, Memory (app, runtime and agents together), CPU |

The smallest window is 1000 × 600.

## Screens

| Card | When it shows |
|---|---|
| ScreenFirstRun | no project yet |
| ScreenNoTask | projects exist, no task selected |
| ScreenMain | macOS; the selected task asks to run a tool |
| ScreenMainWindows | Windows chrome; Codex and a shell side by side; Files tab |
| ScreenFocus | both panels hidden; a lead asks to create a child task |
| ScreenBoard | Board view, columns by status, filter by project |
| ScreenReview | diff side by side in the wide inspector, writing a line comment |
| ScreenCrashed | the agent exited with code 101; error toast with Report problem |
| ScreenRestore | after a restart: recovered screen and an unknown launch |
| ScreenNewTask | New task (`⌘N`, Windows `Ctrl+Shift+N`) |
| ScreenSettings | Settings, Appearance with mode and color theme |
| ScreenQuickOpen | Quick open with tasks, files and commands |
| ScreenActivity | every notification of this session |
| ScreenDelete | delete a task for good, with what is lost |
| ScreenFlow | the path of one task, and where each overlay opens |

## State sheets

| Card | Covers |
|---|---|
| StatesSidebar | every task status, hover, selected, focus, long and Korean titles, lead and workers, unread mail, all PR chips, empty, collapsed and folder projects |
| StatesChrome | title bar on macOS and Windows, badge counts, tooltips, status bar loading and unreadable resource use |
| StatesTerminal | tabs, every terminal notice (restore, reattach, resume, Codex conversation choice, lost connection, exited, could not start, prompt held back, setup copy failed), unknown launch in two steps, original prompt, file drop, background tabs |
| StatesApproval | tool requests, sending, long command, setup commands, QA run, answered in the terminal, focus |
| StatesChanges | loading, empty, file states, staged and untracked, conflicts, cut diff, merge check (idle, checking, clean, conflicts, local only), PR feedback, commit, agent drafts, push and create PR, done, failed |
| StatesDiff | unified, selected line, line comment, agent prompt not ready, draft check before paste |
| StatesFilesQA | file list, no match, too many, previews; QA never run, waiting, not allowed, passed, failed, unverified, timed out, too much output, could not start |
| StatesPullRequest | PR panel in every state including offline and a different commit; local ports; activity; inbox |
| StatesNewTask | empty, Codex with resume, Codex missing, bad base branch, creating, discard draft |
| StatesDialogs | archive, delete (Git, folder, nothing to lose), sign out of Codex, forget launch, archived list and empty, report a problem, quick open with no match |
| StatesToasts | error, PR created, prompt inserted, git done, update available, update checks |
| StatesSettings | General, Appearance in Sand light, Terminal with an invalid value, Agents (signed in with usage, checking, signing in, none found), Notifications, Shortcuts, Updates |
| StatesSystem | tray menu, macOS notifications of each kind, Windows notification, Dock badge |

## Coverage
The **Coverage** card (group Screens) maps every line of the feature list to the cards that draw it. Start there to check nothing is missing.

## Interactions
Whole windows, 1440 × 900, one moment each: menus, tooltips, drags, dialogs, panels and every settings page. A pointer marks where the person is hovering or clicking.

| Card | Moment |
|---|---|
| ScreenTooltips | Hover: tooltips on the title bar and status bar |
| ScreenSidebarHidden | Sidebar hidden, inspector open on Activity |
| ScreenMinWindow | Smallest window, 1000 by 600: the inspector closes first |
| ScreenKorean | Korean UI: the same main screen with 한국어 selected |
| ScreenAddProjectMenu | Add project: pick a Git repository or a folder |
| ScreenDropFolder | Drag a folder onto the window to add it as a project |
| ScreenTaskMenu | Right-click a task row (design proposal) |
| ScreenSidebarCollapsed | A project collapsed; hovering its heading shows the counts |
| ScreenArchiveConfirm | Archive a task: confirm |
| ScreenArchivedList | Archived tasks: restore or delete for good |
| ScreenRestored | After Restore: the task is back in its project and selected |
| ScreenNewTaskCodex | New task with Codex: choose a conversation to resume |
| ScreenNewTaskBranch | New task: the base branch list filters as you type |
| ScreenNewTaskParent | New task: pick a parent so it starts as a worker |
| ScreenNewTaskCreating | New task: creating the worktree and starting the agent |
| ScreenDiscardDraft | Close New task with a draft: confirm |
| ScreenSetupApproval | A new task asks to run setup commands from brachio.toml |
| ScreenSetupDone | Setup commands ran in the shell tab |
| ScreenAddTabMenu | The + on the tab row: open a shell or a second agent |
| ScreenSplitMac | Agent and shell side by side on macOS |
| ScreenCloseShell | Close a shell tab that still runs a program (design proposal) |
| ScreenManyTabs | Seven tabs: a tab beyond the six kept on screen (design proposal) |
| ScreenOtherTabWaiting | Another tab waits for approval while you read this one |
| ScreenApprovalEdit | Approval bar: a file edit shows the whole change |
| ScreenApprovalSending | Approval bar: sending the answer |
| ScreenChildCreated | After allowing a child task: the worker appears under its lead |
| ScreenUnknownLaunchConfirm | Unknown launch, step 2: the second click is asked in place |
| ScreenFileDrop | Drag files over the terminal |
| ScreenFileDropped | After the drop: paths wait in the agent input |
| ScreenInbox | Inbox: messages from worker tasks |
| ScreenOriginalPrompt | You typed first, so the first prompt waits under Original task prompt |
| ScreenCodexResume | Reopen a Codex tab: choose which conversation to resume |
| ScreenReattached | Reopen the window: terminals reattach to what kept running |
| ScreenChangesEmpty | Changes: nothing changed yet |
| ScreenChangesLoading | Changes: loading |
| ScreenDiffUnified | Unified diff: click a line number to comment |
| ScreenDraftPaste | Insert a line comment: check the draft before it is pasted |
| ScreenCommentInserted | After Insert: the comment waits in the agent input |
| ScreenStageCommit | Stage everything and write a commit message |
| ScreenCommitDraft | Ask the agent for a commit message |
| ScreenCreatePr | After push: write the pull request |
| ScreenPrCreated | Pull request created: the chip and panel follow its checks |
| ScreenPrFeedback | Load PR feedback: failed checks and review comments, ready for the agent |
| ScreenMergeClean | Check merge conflicts: clean |
| ScreenFilesPreview | Files: search, then preview; clicking adds @path to the prompt |
| ScreenFilesNoMatch | Files: nothing matches |
| ScreenFilesBinary | Files: a binary file has no preview |
| ScreenQaApprove | Run QA: the exact command, folder and commit come first |
| ScreenQaRunning | QA running |
| ScreenQaPassed | QA passed, verified by Brachio |
| ScreenActivityTab | Inspector Activity: this task's events |
| ScreenPorts | Local ports: add a port for this task's dev server |
| ScreenQuickOpenEmpty | Quick open: nothing matches |
| ScreenQuickOpenEmptyQuery | Quick open: just opened, recent first |
| ScreenActivityEmpty | Activity: nothing yet |
| ScreenSettingsGeneral | Settings, General with the language menu open |
| ScreenSettingsTerminal | Settings, Terminal |
| ScreenSettingsAgents | Settings, Agents: versions, sign-in and usage |
| ScreenSignOutCodex | Sign out of Codex: confirm |
| ScreenSettingsAgentsSignIn | Settings, Agents: signing in to Codex in the browser |
| ScreenSettingsNotifications | Settings, Notifications (design proposal: the app has one switch today) |
| ScreenSettingsShortcuts | Settings, Shortcuts for macOS and Windows |
| ScreenSettingsUpdates | Settings, Updates with a new version |
| ScreenSettingsUpdatesChecking | Settings, Updates: checking |
| ScreenSettingsArchived | Settings, Archived tasks page |
| ScreenReport | Report a problem: check the text, then open GitHub |
| ScreenBoardEmpty | Board filtered to a project with no tasks |
| ScreenDesktopMac | Brachio in the background on macOS: tray menu, notification, Dock badge |
| ScreenDesktopWindows | Brachio in the background on Windows: tray menu, notification, taskbar badge |

## Errors
Whole windows for every failure. Each one says what broke, what is kept, and offers one next step. Red is used only here and for destructive actions.

| Card | Failure |
|---|---|
| ScreenErrorCoreStopped | Cannot read resource use: terminals keep running |
| ScreenErrorRuntimeLost | Lost the connection to the terminal runtime |
| ScreenErrorStartFailed | Could not start the terminal: the agent is not on PATH |
| ScreenErrorExited | The agent exited normally: reopen to continue |
| ScreenErrorInputFailed | Typing could not reach the terminal |
| ScreenErrorSignIn | The agent needs you to sign in |
| ScreenErrorPromptHeld | The first prompt was not sent: the terminal was not ready |
| ScreenErrorCodexCheck | Could not look for Codex conversations to resume |
| ScreenErrorNotGit | Add project: the folder is not a Git repository |
| ScreenErrorAlreadyAdded | Add project: the folder is already a project |
| ScreenErrorFolderAccess | macOS blocked access to the project folder |
| ScreenErrorAgentMissing | New task: Codex is not installed |
| ScreenErrorBaseBranch | New task: the base branch does not exist |
| ScreenErrorWorktree | New task: the worktree could not be created |
| ScreenErrorSetupFailed | A setup command from brachio.toml failed |
| ScreenErrorConfigCopy | Setup file copy failed |
| ScreenErrorArchive | Archive failed: a terminal would not close |
| ScreenErrorDelete | Delete failed on Windows: a file is in use |
| ScreenErrorChanges | Changes could not be read |
| ScreenErrorConflicts | Merge check: conflicts with main |
| ScreenErrorRebaseStopped | Unresolved conflicts in the worktree block commits |
| ScreenErrorCommit | Commit failed: a pre-commit hook |
| ScreenErrorPushRejected | Push rejected: the remote has newer commits |
| ScreenErrorPushDenied | Push denied: no write access to the repository |
| ScreenErrorCreatePr | Create pull request failed: one already exists |
| ScreenErrorOffline | GitHub cannot be reached: last results stay, marked offline |
| ScreenErrorPrFeedback | PR feedback: inline reviews could not load |
| ScreenErrorDifferentCommit | PR results are for a different commit than the worktree |
| ScreenErrorUpdate | Update check failed |
| ScreenErrorReport | Report a problem: the browser did not open |
| ScreenErrorCommentInsert | Line comment: the agent prompt is not ready |
| ScreenErrorQaFailed | QA failed: output and the commit it ran on |
| ScreenErrorQaStale | QA result is not verified: the commit changed since |
| ScreenErrorQaTimeout | QA timed out |
| ScreenErrorQaOutput | QA stopped: too much output |
| ScreenErrorQaStart | QA could not start |
| ScreenErrorQaDenied | QA was not allowed |
| ScreenErrorPorts | Local ports could not be saved |
| ScreenErrorInbox | Inbox could not load |
| ScreenErrorSettingsValue | Settings: a value out of range |
| ScreenErrorAgentsList | Settings: the agent list could not be read |
| ScreenErrorDraftPaste | Draft paste refused: the selected tab already has input |

## Later

| Card | What |
|---|---|
| ScreenPlan | task dependencies as a graph and the handoff review with its QA result (#176 data is stored; #178 links QA to the handoff) |
| LaterConvenience | from the Orca parity list: pinned tasks, unread finish, overlap warning across tasks, compare agents on one prompt, switch recent tasks, import worktrees, keep awake, shortcut list |

## Moving between screens
- Tasks and Board show the same tasks. Opening a tile opens that task in Tasks.
- New task, Quick open, Settings, Activity and dialogs sit over the current screen; `Esc` closes them and nothing behind them changes.
- A desktop notification fires only while Brachio is not in front. Clicking it opens that task.
- Closing the window keeps terminals running in the runtime; reopening reattaches. Only the six most recent tabs stay drawn; the rest keep their status.

## Keyboard
| macOS | Windows | Action |
|---|---|---|
| `⌘P` | `Ctrl+Shift+P` | quick open |
| `⌘N` | `Ctrl+Shift+N` | new task |
| `⌘B` | `Ctrl+Shift+B` | toggle sidebar |
| `⌘⌥B` | `Ctrl+Shift+Alt+B` | toggle inspector |
| `⌘⏎` | `Ctrl+⏎` | create task in New task |
| `↵` `Esc` | `↵` `Esc` | allow once or deny in the approval bar |

## Where the design goes past the app today
- The PR chip and the unread count move to their own line in the task row; the app joins them with a middle dot.
- A side-by-side diff widens the inspector to 560px.
- Settings get a Notifications page and the color themes; the app has a theme select and one notifications switch.
- Background tabs beyond the six kept on screen get a one-line placeholder.
- Right-clicking a task row opens a menu (ScreenTaskMenu).
- Closing a shell tab that still runs a program asks first (ScreenCloseShell).
- Settings, Notifications splits the one switch into one per kind.

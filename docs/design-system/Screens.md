# Screens and states

Every screen and every interaction state of Brachio, drawn from the features on `main` up to #261 and the strings in `ui/src/i18n/en.ts`. Each card uses only this system's tokens, so the theme switcher on this page shows any of them in all eight color sets. Sample tasks and numbers are example content.

Cards are grouped under Components: **Screens** (full windows, 1440 × 900), **States** (sheets of every state of one area), **Later** (planned, not built), plus the new components ThemePicker, PrChip, Notice and ConfirmDialog.

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
| StatesChrome | title bar on macOS and Windows, badge counts, tooltips, status bar loading and core stopped |
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

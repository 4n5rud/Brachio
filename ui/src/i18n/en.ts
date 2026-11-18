// English is the default locale. Every UI string lives here; other locales
// must provide the same keys (the type checker enforces it).
export const en = {
  "status.working": "Working",
  "status.blocked.permission": "Needs approval",
  "status.blocked.question": "Has a question",
  "status.blocked.auth": "Sign-in needed",
  "status.blocked.other": "Waiting for you",
  "status.idle": "Ready",
  "status.error": "Error",
  "status.exited": "Exited",
  "status.crashed": "Crashed",
  "status.unknown": "Checking",

  "group.needs": "Needs you",
  "group.working": "Working",
  "group.review": "Ready for review",
  "group.done": "Done",

  "titlebar.search": "Search tasks, files, commands",
  "titlebar.view": "View",
  "view.tasks": "Tasks",
  "view.board": "Board",
  "titlebar.notifications": "Notifications",
  "titlebar.settings": "Settings",

  "sidebar.title": "Tasks",
  "sidebar.newTask": "New task",
  "sidebar.empty.title": "No tasks yet",
  "sidebar.empty.body": "Create a task to start an agent in its own worktree.",

  "task.reviewChanges": "Review changes",
  "task.createPr": "Create PR",
  "task.openInEditor": "Open in editor",
  "task.newTab": "New tab",
  "task.split": "Split",
  "task.more": "More",

  "approval.title": "Run this command?",
  "approval.inWorktree": "runs inside the worktree",
  "approval.allowOnce": "Allow once",
  "approval.allowAlways": "Always allow in this task",
  "approval.deny": "Deny",
  "approval.hint": "You can also answer in the terminal.",
  "terminal.exited": "Process exited",
  "terminal.startFailed": "Could not start the terminal",

  "inspector.changes": "Changes",
  "inspector.qa": "QA",
  "inspector.activity": "Activity",
  "inspector.changedFiles": "Changed files",
  "inspector.qa.passed": "passed",
  "inspector.qa.failed": "failed",
  "inspector.qa.none": "QA has not run for this task yet.",
  "inspector.activity.none": "Activity appears here as the agent works.",

  "board.title": "Board",
  "board.all": "All",
  "board.open": "Open",

  "statusbar.core": "Core OK",
  "statusbar.sessions": "Sessions",
  "statusbar.memory": "Memory",
  "statusbar.cpu": "CPU",

  "empty.repo.title": "Add a repository to get started",
  "empty.repo.body": "Choose a git repository folder, or drop one on this window.",
  "empty.repo.add": "Add repository",

  "newTask.title": "New task",
  "newTask.prompt": "What should the agent do?",
  "newTask.name": "Task name",
  "newTask.base": "Base",
  "newTask.agent": "Agent",
  "newTask.cancel": "Cancel",
  "newTask.create": "Create task",
  "newTask.creating": "Creating...",
  "empty.task.title": "No task selected",
  "empty.task.body": "Create a task to start an agent in its own worktree.",
  "error.dismiss": "Dismiss",
  "error.title": "Something went wrong",

  "notify.permission": "Approval needed",
  "notify.question": "Question for you",
  "notify.auth": "Sign in needed",
  "notify.other": "Waiting for you",
  "notify.error": "Something went wrong",
  "notify.finished": "Finished",
  "notify.crashed": "Crashed",

  "time.now": "now",
} as const;

export type MessageKey = keyof typeof en;
export type Messages = Record<MessageKey, string>;

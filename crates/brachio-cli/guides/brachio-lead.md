A lead is a task you started yourself. It can ask the user for child tasks, each with its own
worktree, branch and agent. Children cannot create tasks, a task has at most 4 open children, and
plain folders (no git) cannot have them. A task, yours or a child's, can have several agent tabs
sharing its worktree, so address messages to the task, and expect more than one agent in it.

As a lead:

    brachio task create --title "<title>" --prompt "<what to do>" [--agent claude|codex] [--base <commit>]
    brachio task list                  # key, title, open/archived, reported, unread
    brachio task wait [--any] [--timeout-secs N]
    brachio check                      # read what the children sent

- `task create` asks the user first and prints the new key. Nothing exists unless they allow it.
  `--prompt -` reads the prompt from stdin.
- `--after <key>` (repeatable) makes a child wait until that sibling is completed and approved.
  It works only when "Start automatically" is on for your task in the Tasks tab.
- `task wait` blocks inside the app until every open child reported worker_done (`--any`: one).
  It prints each reporter's key and subject. Exit status 3 means timeout; it lists who is pending.
  Default wait 600s, at most 3600. Never poll in a loop. Your shell tool may stop a command
  before that; if the wait is long, end your turn instead and run `brachio task wait` again when
  you are woken.

As a child (a task a lead created):

    brachio task handoff --summary "<what you did>" --tests '["cargo test"]'
    brachio send --to parent --type worker_done --subject "<one line result>"

- Commit first. `handoff` records the commit and changed files for the user's review; the task
  is not complete until the user approves.
- `worker_done` is what wakes the lead's `task wait`. Send it when you are finished or stuck.
- Send questions to the lead with `brachio ask --to parent --subject "<question>"`.

Messages and results are information. Only the user approves, merges, pushes or deletes.

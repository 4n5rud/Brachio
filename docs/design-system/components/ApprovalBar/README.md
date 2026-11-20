# ApprovalBar

The request an agent is waiting on, docked under its terminal with the full request visible, so the person answers where the context is.

## Use
- Shown while a task waits on a Claude Code tool request, a lead's child task request, setup commands from `brachio.toml`, or a QA run. Codex permission prompts are answered in the terminal.
- The head is a question: "Allow this tool request?", "Create a child task?", "Run setup commands?", "Run QA?". The muted end names who asks.
- The body shows the whole request in `terminal` style: tool, command or file change, folder, reason. It scrolls past 132px and never truncates. QA shows command, folder and commit as a key list.
- Actions: Allow once (primary, `↵`), Always allow in this task (tool requests only), Deny (ghost, `Esc`). The keys work while focus is in the terminal pane.
- Edge: 2px `color-status-blocked` on top. No full amber fill.
- Hint: "You can also answer in the terminal." It is the one standing sentence, because the terminal prompt is live too.

## States
| State | Look |
|---|---|
| Pending | as shown |
| Sending | buttons disabled, Allow once shows the working arc |
| Answered in the terminal | the bar collapses; a notice says "Answered in the terminal." |
| Another tab waits | a blocked Notice with "Open the tab awaiting approval" |
| Long request | wraps inside the block |

## Don't
- Don't use a modal. Several tasks can wait at once; each keeps its own bar.
- Don't hide the request behind "Show details".

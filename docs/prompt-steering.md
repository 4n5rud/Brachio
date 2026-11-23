# Steering a working agent

"Steer now" in the prompt queue types a follow-up into an agent that is in the middle of a turn. The window never invents a way to do this: it uses what each agent's own terminal interface documents.

| Agent | Mechanism | Enter pressed for you |
| --- | --- | --- |
| Claude Code | Type the message and press Enter while it works. Claude Code queues it and passes it to the model as soon as the running tool calls finish, within the same turn. | Yes, because that is the documented way |
| Codex | Submit a message while it works. The Codex terminal interface treats it as steering of the running turn. | Yes, because that is the documented way |
| Grok | No documented way to take input mid-turn. | The button is disabled and says why |

The button is also disabled when the agent is idle (use the queue), when it waits for an answer (typing then would answer its question), and when the agent's input already holds text.

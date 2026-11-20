# Notice

One line at the top of a terminal pane that says what happened to that terminal and offers at most two fixes.

## Tones
| Tone | Glyph | Used for |
|---|---|---|
| Info | dashed ring | Last screen recovered, output skipped, prompt held back |
| Progress | working arc | Reopening session, starting a conversation, checking Codex conversations |
| Success | ring | Reattached to the running terminal |
| Needs you | diamond, `color-blocked-bg` | unknown launch, prompt not sent, another tab awaits approval |
| Error | square x, `color-error-bg` | lost runtime connection, could not start, input failed, setup copy failed |

## Rules
- The first words, in `strong`, are the fact. The rest is the cause or the next step, one sentence.
- Buttons are verbs: Reopen session, Retry copying files, Forget previous launch. A destructive second step replaces the first in place (Forget and reopen anyway, `button-danger`) rather than opening a dialog.
- Notices stack in the order they happened and leave when the state ends. Two at most.

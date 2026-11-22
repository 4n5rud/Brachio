# Agent replay fixtures

Recorded-by-hand inputs that are replayed through the real status code. Nothing here runs an
agent or touches the network, so the tests are deterministic. The formats were written from the
parsers, not captured from live sessions; update a case when the format changes on purpose.

- `status/*.json` is replayed by `crates/brachio-core/tests/status_replay.rs`. It feeds terminal
  output through `OscScanner`, `ProgramStatusMessage::parse`, `resolve` and `Debouncer`, which
  mirrors `src-tauri/src/terminal_status.rs`, and checks the published status.
- `hooks/*.json` is replayed by `crates/brachio-runtime/tests/hook_replay.rs`. It feeds an agent's
  hook stdin through `request_from_hook_input` and the user's decision through `hook_output`.

## Add a status case

Create `status/<agent>-<scenario>.json`. The directory is scanned, so there is nothing to register.

```json
{
  "description": "One line on what the case shows.",
  "steps": [
    { "at_ms": 0, "output": "\u001b]7501;state=working\u001b\\" },
    { "at_ms": 300, "expect": { "state": "working" } }
  ]
}
```

Steps run in order and `at_ms` must not decrease. Each step applies its action, then lets the
debouncer observe that moment, so a state is published only after a later step at least 300 ms on.

| Key | Meaning |
| --- | --- |
| `output` | Terminal output as a JSON string. Write ESC as `\u001b`, BEL as `\u0007`, ST as `\u001b\`. |
| `output_hex` | The same, as hex, for bytes that are not valid UTF-8. |
| `exit` | `{ "code": 1 }`, or `{ "code": null }` for a signal. The process ended. |
| `expect` | The published state, in the serialized form of `AgentState`, such as `{ "state": "blocked", "reason": "permission" }`. |
| `expect_message` | The status detail shown with the state, or `null` for none. |
| `expect_replies` | How many protocol queries (`?`) have been answered so far. |
| `note` | Free text shown in a failure. |

Split a sequence across several `output` steps to test chunk boundaries.

## Add a hook case

Create `hooks/<agent>-<scenario>.json` with an `events` list. Each event has the hook's `stdin`
(a JSON string), the terminal `key`, and either `expect_request: null` (the payload must be
ignored) or the expected request fields plus a `decision` (`allow`, `deny` with `message`, or
`ask`) and the `expect_output` JSON the hook prints (`null` when it prints nothing).

## Run

```
cargo test -p brachio-core --test status_replay
cargo test -p brachio-runtime --test hook_replay
```

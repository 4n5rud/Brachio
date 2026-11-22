# Usage providers: how more agents plug in

This is a plan. Only Claude Code and Codex are implemented.

## How a provider plugs in today
- **Rust:** implement `UsageProvider` in `crates/brachio-agent/src/usage_scan.rs` (id, display name, icon key, `log_files()`, `scan()`) and add it to `providers()`. `scan()` reads from the saved byte offset in a `FileState` and returns daily `Bucket`s. Everything else (storage, incremental rescans, charts, filters) is shared.
- **UI:** the provider list comes from the `usage_providers` command, so the filter and the share chart show a new provider automatically. Add its mark to `ProviderIcon` in `ui/src/components/settings/UsageAnalytics.tsx` (a `BrandIcon` name; `gemini` and `geminiCli` marks already exist).
- **Plan limit history:** record readings with `recordLimit(provider, window, percent)` from the code that reads that provider's limits.
- **Rules:** no network beyond an agent's own usage endpoint, tokens only, no money estimates, and a missing log means nothing is shown.

## Gemini CLI
- **What it exposes:** documented OpenTelemetry output. With `telemetry.target: "local"` and an `outfile` in `.gemini/settings.json`, it writes logs and metrics to a file. The token metric is `gemini_cli.token.usage`, split by model and type (input, output, thought, cache, tool).
- **Uncertain:** I found no source for automatic session logs with token counts under `~/.gemini`. Log event names differ between guides and versions. Telemetry is off by default, so a user must enable it.
- **Steps:** (1) read `~/.gemini/settings.json` to see whether local telemetry is on and where `outfile` is; (2) if it is off, show "turn on telemetry to see usage" and never change the setting silently; (3) parse the outfile (OTLP JSON lines) for token metrics, keyed by session id and timestamp; (4) add `GeminiProvider` and use the `geminiCli` icon.

## Antigravity
- **What it exposes:** community reports say conversations live under `~/.gemini/antigravity-ide` (older installs `~/.gemini/antigravity`), as `.pb` files in older versions and SQLite `.db` in newer ones, plus a per-conversation brain folder. One report shows `.system_generated/logs/transcript_full.jsonl`.
- **Uncertain:** nothing confirms token counts in any of these. The formats are not documented by Google and the layout has changed between versions.
- **Steps:** (1) inspect a real install to see whether `transcript_full.jsonl` or the database carries usage; (2) only if it does, write a reader per format with version checks; (3) otherwise do not ship a provider, because estimating tokens is out of scope.

## Grok (xAI Grok Build CLI)
- **What it exposes:** third-party tools report per-turn token totals, cache reads and reasoning tokens in `~/.grok/sessions/**/updates.jsonl` (`turn_completed` usage), and xAI's headless docs define the token fields (`input_tokens` is uncached input; `total_tokens` includes cache). Plan usage is reachable through `grok agent stdio` (an `x.ai/billing` method).
- **Uncertain:** the exact session folder layout and event names come from third-party docs. A different project, `@webdevtoday/grok-cli`, uses `~/.grok-cli/history/` with another format.
- **Steps:** (1) confirm the layout on a real install; (2) write `GrokProvider` with the same offset-based reader, mapping `turn_completed` usage to the four token counts; (3) add limit history through the billing method only if it needs no network beyond xAI's own endpoint.

## Sources
- Gemini CLI telemetry docs: https://google-gemini.github.io/gemini-cli/docs/cli/telemetry.html
- Antigravity storage reports: https://discuss.ai.google.dev/t/missing-chats-trajectories-in-new-antigravity-ide/176112
- Grok session usage (third-party): https://github.com/tsouth89/ceiling/pull/137 and https://raw.githubusercontent.com/steipete/CodexBar/main/docs/grok.md

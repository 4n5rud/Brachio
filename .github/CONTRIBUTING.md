# Contributing to Brachio

Thank you for helping improve Brachio. The project is pre-alpha and targets Windows and macOS.

For a security issue, use [private vulnerability reporting](SECURITY.md).

## Setup

Install Git 2.38+, Rust with rustfmt and Clippy, Node.js 22+, pnpm 10+, and the Tauri 2 platform prerequisites. Windows needs WebView2 and macOS needs Xcode command-line tools.

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Making a change

- Discuss substantial changes in an issue and keep each pull request focused.
- Create a branch from an up-to-date `main`. Preserve existing behavior unless the change calls for otherwise.
- Follow the Rust workspace lints and existing SolidJS patterns. Keep UI strings in both `ui/src/i18n/en.ts` and `ko.ts`.
- Use a new migration for database changes and preserve existing migration history.
- Add regression coverage for meaningful behavior and failure paths. For UI changes, inspect the affected states in the app when possible.
- Keep secrets, certificates, and real `.env` files out of commits.

## Checks

Run the checks relevant to your change. Windows and macOS CI runs the full Rust workspace; Linux CI excludes the Tauri app shell.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
pnpm --dir ui typecheck
pnpm --dir ui test
pnpm --dir ui build
```

Include the commands and results in your pull request, along with platforms or behavior you could not verify. Fill in the [pull request template](pull_request_template.md). Maintainers handle merges and [releases](RELEASING.md).

Coding agents should also follow the [agent contribution guide](CONTRIBUTING-AGENTS.md).

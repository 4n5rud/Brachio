# Contributing to Brachio

Thank you for helping improve Brachio. The project is pre-alpha and targets Windows and macOS.

For a security issue, use [private vulnerability reporting](SECURITY.md).

## Where to start

Look for issues labeled [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) or [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation). The labels `platform: windows` and `platform: macos` mark platform-specific work.

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

## Dependency checks

The **Dependencies** workflow checks published security advisories against the lockfiles. It runs on pull requests that change dependencies, `deny.toml` or workflows, on `main`, and every Monday, because new advisories appear for dependencies that did not change.

| What | Tool | Fails on |
|---|---|---|
| Rust crates in `Cargo.lock`, for the Windows and macOS builds | `cargo deny check advisories` | any vulnerability or unsoundness; an unmaintained crate the workspace depends on directly |
| UI packages that ship in the app (`dependencies`) | `pnpm audit --prod` | moderate, high or critical |
| UI build and test tools (`devDependencies`) | `pnpm audit --dev` | high or critical |
| Advisory exceptions | `node scripts/check-advisory-exceptions.mjs` | an exception without an expiry date, or past it |

When the check fails, update the dependency first (`cargo update -p <crate>`, or `pnpm --dir ui update <package>`). If no fixed version exists or the advisory does not affect Brachio, add an exception in the same pull request and explain it there:

- Cargo: an entry in `ignore` in `deny.toml`, such as `{ id = "RUSTSEC-2026-0000", reason = "expires 2026-03-31: only reachable from code Brachio does not call, #123" }`.
- UI: a line under `auditConfig.ignoreGhsas` in `ui/pnpm-workspace.yaml`, such as `- GHSA-xxxx-xxxx-xxxx # expires 2026-03-31: dev server only, #123`.

Keep the expiry within 90 days. When it passes, CI fails until someone fixes the advisory or reviews and renews the exception.

Dependabot opens grouped update pull requests every Monday for GitHub Actions, Cargo and the UI, after a new release is a week old. Workflows pin each action to a full commit SHA with the version as a comment; Dependabot updates both.

## Public documents

Everything in this repository is public. `node scripts/check-public-docs.mjs` runs in CI and fails on:

- paths from a real machine in documents (write `/Users/you` or `C:\Users\you` instead),
- notes, tool commands or IDs from private design or planning tools,
- wording that compares Brachio feature by feature with another product,
- README download links to a release that is a draft or does not exist (`--links`).

Describe what Brachio does and how to use it. Keep plans, comparisons and work notes in issues or pull requests, and mark design documents as implemented or planned.

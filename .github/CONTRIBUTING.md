# Contributing to Brachio

Thanks for helping. Brachio is a small desktop app for running coding agents in parallel, so keep changes small and easy to review.

Found a security problem? Do not open a public issue. Follow [`SECURITY.md`](./SECURITY.md).

## Local setup

You need Rust stable (the toolchain is pinned to `stable` in [`rust-toolchain.toml`](../rust-toolchain.toml)), Node 22 or later, pnpm 10 or later, and git 2.38 or later. To build the app shell you also need the web view of your OS: WebView2 on Windows, and the Xcode Command Line Tools on macOS. Linux can build and test everything except `src-tauri`.

```sh
pnpm --dir ui install
cargo install tauri-cli --version "^2" --locked   # once, for `cargo tauri`
cargo tauri dev                                   # builds brachio-runtime, then runs the app
```

`cargo tauri dev` also builds `brachio-runtime`, the separate process that keeps terminals running when the window closes.

## Branch names

Name the branch after the change: `feat/delete-task-worktree`, `fix/windows-ime-composition`, `docs/readme-current-state`, `ci/pull-request-checks`, `refactor/split-runtime-service`. Avoid `test`, `misc` or `changes`.

## Code guidelines

- One purpose per pull request. Over about 400 changed lines, consider splitting it.
- Code, comments, UI strings, commits and pull requests are in English. UI text lives in `ui/src/i18n/en.ts` with the same keys in `ko.ts`; never put a string inline.
- Brachio runs on Windows and macOS. Guard platform-specific code at runtime, build paths with `Path` and `PathBuf`, and show `⌘` and `⇧` on macOS and `Ctrl+` and `Shift+` elsewhere.
- Rust: `rustfmt` defaults, no `unwrap()` outside tests, no `unsafe`, external programs run with argument arrays, never shell strings. Files stay under 300 lines and functions under 50. Every public item has a `///` doc. Lint rules live in `[workspace.lints]` in [`Cargo.toml`](../Cargo.toml).
- UI: colors and sizes only through the variables in `ui/src/styles/tokens.css`. Nothing smaller than 12px. Status is never shown by color alone.
- Database changes only through a new file in `crates/brachio-store/migrations/`.

## Checks before opening a pull request

CI runs the same commands.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
pnpm --dir ui typecheck
pnpm --dir ui test
pnpm --dir ui build
```

Add a test when it would fail if your change were reverted. Do not add tests for copy, CSS classes, constants or timing. If your change affects what users see or do, try it in the running app on the platform it affects.

## Commits and pull requests

- Commit subjects follow Conventional Commits, in the present tense: `feat: ...`, `fix: ...`, `docs: ...`, `ci: ...`, `refactor: ...`, `chore: ...`.
- Fill in every section of the [pull request template](./pull_request_template.md). The pull request title is the line that appears in the release notes, so write it for a user.
- Base pull requests on `main`. If you stack one on another, retarget it to `main` as soon as the lower one merges.
- Maintainers merge. Do not force-push a branch someone else started.

## Versions and releases

There is one version, kept in `Cargo.toml` (`[workspace.package] version`) and `ui/package.json`. Do not change it in a feature pull request. Maintainers cut releases as described in [`RELEASING.md`](./RELEASING.md).

## Never commit

Keys, tokens, certificates and `.env` files. Use environment variable names and placeholders in examples. If a secret leaks, tell a maintainer so it is rotated before anything else.

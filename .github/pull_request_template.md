## Summary

<!-- What changed, in plain words. This pull request's title goes into the release notes. -->

## Why

<!-- The problem, and why this approach over the alternatives you considered. -->

## Linked issue

<!-- Fixes #123. Outside contributors: open or find an issue first. -->

## What users will see

<!-- Before and after for any change to the screen or to behavior: screenshots or a short video. If nothing visible or interactive changes, write exactly `N/A` and say why. Attach files here; do not commit them. -->

## Verification

<!-- What you ran, on which OS, and what you could not check. -->

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked` pass
- [ ] `pnpm --dir ui typecheck`, `pnpm --dir ui test` and `pnpm --dir ui build` pass
- [ ] I tried the change in the running app on: <!-- Windows / macOS / not applicable -->
- [ ] Every test I added fails without this change

## Risk

<!-- What could this break, and for whom? Name the riskiest part. Call out saved data or settings, the terminal runtime protocol, other platforms, and agents. Say how to back it out: a plain revert, or what a revert would not undo. -->

**Level:** <!-- Low, Medium or High -->

## Checklist

- [ ] This pull request is small and has one purpose
- [ ] Windows and macOS impact considered, or not applicable
- [ ] No version change (maintainers do that when releasing)
- [ ] No secrets, and no files that are not needed to run the project

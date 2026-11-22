# Releasing Brachio

Maintainers only. Contributors never change the version in a feature pull request.

## One-time setup

Do this once per repository, not per release.

- Turn on private vulnerability reporting (Settings, Code security) so [`SECURITY.md`](./SECURITY.md) works.
- Make sure Actions can create releases (Settings, Actions, Workflow permissions).

## Versions and policy

One version, in two files that must match: `[workspace.package] version` in `Cargo.toml` and `version` in `ui/package.json`. CI fails when they differ.

- Pre-alpha (current): versions are `0.x`. The app is unsigned, has no automatic update, and may change anything between releases. Only the latest release is supported.
- Release candidate: `0.2.0-rc.1`, `0.2.0-rc.2`, and so on. Published as a pre-release, for testing a build on real machines before a stable release.
- Stable: `0.2.0`. Before 1.0, a new feature raises the minor number and a fix raises the patch number. The version must be higher than the last stable release. A release is called stable only after its release candidate was installed and checked on each platform.

## Per-release steps

1. Open a pull request titled `chore: release 0.2.0-rc.1` that changes only the two version files (and the `brachio-*` entries of `Cargo.lock`, which `cargo update --workspace` rewrites). Merge it.
2. Tag the merge commit on `main` and push the tag:

   ```sh
   git switch main && git pull
   git tag v0.2.0-rc.1
   git push origin v0.2.0-rc.1
   ```

3. The **Release** workflow checks that the tag matches both version files and is on `main`, drafts a GitHub release with generated notes, builds the Windows and macOS installers and attaches them.
4. Open the draft. Read the notes, install the build on a real machine, and publish. A draft that should not ship is deleted together with its tag.
5. After you publish, update the download links in `README.md` and `docs/readme/README.*.md` to the new version in a small pull request. Never point the README at a draft or a release that does not exist yet; `node scripts/check-public-docs.mjs --links` checks this.

## Release notes

GitHub writes them from the pull requests merged since the previous release (for a stable release, since the previous stable one). Each entry is the pull request title with its author, and first-time contributors are listed separately. That is why pull request titles are written for users, and why the title prefix matters: `labels.yml` turns `feat:`, `fix:`, `perf:`, `docs:` and `ci:`/`chore:`/`refactor:` into the labels that [`release.yml`](./release.yml) groups into New, Fixes, Performance, Documentation and Maintenance. Add the label `skip-release-notes` to leave a pull request out.

## When the terminal runtime changed

`brachio-runtime` keeps agents alive across app restarts, so after an update an old runtime may still be running with the user's agents in it. If a release contains a fix that lives in the runtime, raise `GENERATION` in `src-tauri/src/runtime_client.rs`. The updated app then starts a new runtime beside the old one, which is told to take no new terminals and ends when its last agent does; agents in it stay reachable, and every new terminal uses the new runtime. Without raising it the old runtime keeps serving, fixes and all, until the user stops their agents.

## What a release does not do yet

- The installers are not signed. Windows shows SmartScreen and macOS asks the user to allow the app. Say so in the draft until signing is set up.
- The release build bundles `brachio-runtime` and the `brachio` helper next to the app (`tauri.release.conf.json` lists them as `externalBin`, and `scripts/stage-sidecars.mjs` builds them before the bundle). On Windows the bundle also carries the newer ConPTY (`conpty.dll` and `OpenConsole.exe` from Microsoft's NuGet package, pinned by hash in `scripts/stage-sidecars.mjs` and listed in `tauri.release.windows.conf.json`), because the ConPTY built into Windows answers terminal queries itself and agents then never report their status. Before a release, check on each platform that both files sit beside the installed app, and that closing the window leaves a shell running. CI builds and tests the Rust workspace on macOS, but nobody has run the terminal runtime on a real Mac yet, so the README calls macOS support experimental. Remove that label only after the checks above pass on macOS.
- The macOS build is for Apple silicon only.
- There is no automatic update.

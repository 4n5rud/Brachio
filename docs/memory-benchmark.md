# Memory benchmark methodology

This page describes how to measure Brachio's memory use on Windows and macOS, and records the results so far, the size and dependency baseline, and the tab area budget. Only the results in [App memory baseline](#app-memory-baseline) are measured; make no other memory claims from this page.

Use a release build on a single machine and start the sampler from a separate terminal window (PowerShell on Windows). Use one disposable repository and the same app build, terminal runtime mode, agent versions and configuration, task fixture, window size, scrollback settings, and warm-up for every run. Record the Windows or macOS version, CPU and logical processor count, RAM, app commit and build type, WebView2 version on Windows, runtime mode, and enabled agent tool versions.

## Scenarios

1. Measure 0, 1, 5, and 10 open agent sessions. Start each scenario from a fresh app instance, launch the chosen agent for each task, wait for each prompt to settle, then warm up for 30 seconds before capturing 60 seconds.
2. With 10 sessions open, capture sustained terminal output. In each task, start the same local PowerShell workload and keep it running across the capture. For example, this fixture emits 256 identical lines about every 50 ms for 90 seconds, with no network access.

   ```powershell
   $line = 'x' * 255
   $batch = ($line + [Environment]::NewLine) * 256
   $end = [DateTime]::UtcNow.AddSeconds(90)
   while ([DateTime]::UtcNow -lt $end) {
       [Console]::Write($batch)
       Start-Sleep -Milliseconds 50
   }
   ```

   Record how the workload was launched and how many tabs were visible. Start the capture shortly after launching the last workload.
3. After the 10-session scenario, stop every session through Brachio, leave the app running, warm up for 30 seconds, then capture another 60 seconds. Keep any still-running owned runtime in the capture.

Repeat each scenario at least three times. Keep the order and idle warm-up consistent, and close unrelated workloads that could add machine-level noise.

On macOS, use the same steps with this shell workload in place of the PowerShell one. It prints the same 256 lines of 255 characters about every 50 ms for 90 seconds.

```sh
line=$(printf '%255s' | tr ' ' x)
end=$(($(date +%s) + 90))
while [ "$(date +%s)" -lt "$end" ]; do
  yes "$line" | head -n 256
  sleep 0.05
done
```

## Capture on Windows

Identify the measured app's PID in Task Manager or Process Explorer. For detached mode, inspect runtime candidates locally with `Get-CimInstance Win32_Process` and match their executable and `--pipe` endpoint to the Brachio instance under test. If several owned runtime generations host sessions, include all of their PIDs. Identify the launch root for each agent session in the process tree. For a CLI launched via a shell wrapper, supply the wrapper PID so its child processes land in the agent group.

From the repository root, run:

```powershell
.\scripts\measure-memory.ps1 -AppPid 1234 -RuntimePid 2345,2346 -AgentPid 3456,3457 `
    -DurationSeconds 60 -IntervalSeconds 1 -OutputDirectory "$env:TEMP\brachio-5-sessions-run-1"
```

Replace the example PIDs with the observed values. Omit `-RuntimePid` for in-app terminals and `-AgentPid` when no agents are running. Use a new output directory for each capture. If an app or runtime exits during capture, discard the partial run and capture the new process set separately. Processes appearing and disappearing under the same roots are sampled as they exist at each observation.

For an ownership check, run a separate Brachio instance with its own `BRACHIO_RUNTIME_ENDPOINT` and keep its PIDs out of the command above. Check that its app, runtime and agent PIDs are absent from `processes.csv`. The script includes the explicit app and runtime trees and counts each PID once.

## Capture on macOS

On macOS the web view is not part of the app process. WebKit runs the page in separate helper processes (`com.apple.WebKit.WebContent`, `GPU` and `Networking`) that launchd starts for the app, so their parent is not the app and Activity Monitor lists them apart. The web content process usually holds most of the memory, so always include the helpers. `launchctl print pid/<app pid>` lists them under `services`.

Find the app PID with `pgrep -x brachio-app`, and the runtime with `pgrep -fl brachio-runtime`, matching its `--pipe` endpoint to the instance under test. From the repository root, run:

```sh
scripts/measure-memory-macos.sh 1234 60 1 2345
```

The arguments are the app PID, the number of samples, the pause between samples in seconds, and any runtime PIDs. The script finds the WebKit helpers at each sample and adds every descendant of a runtime PID, so agents and shells started by the runtime count in the runtime group. It prints one CSV row per sample and then the median and peak of each group. Each sample takes one to two seconds, because `footprint` is slow, so state the actual spacing from the `elapsed_s` column.

- Footprint is `phys_footprint` from `footprint(1)`: the process's dirty memory, including memory compressed or swapped out. It is what Activity Monitor shows as Memory, and the closest macOS figure to Windows private bytes.
- RSS is the resident size from `ps`, the closest figure to the working set. On macOS it is often much lower than the footprint, because idle pages are compressed. Pages shared between processes count in each.

A second instance on the same Mac needs its own bundle identifier, because a single-instance lock keyed on the identifier sends a second launch to the first one. To measure a build beside an installed Brachio, build it with `--config` pointing at a file that sets another `identifier`, start it with its own `BRACHIO_RUNTIME_ENDPOINT` and `HOME`, and leave the installed app's PIDs out of the command. The installed app's tasks and settings are then not loaded and not touched. WebKit ignores `HOME` and keeps its data and cache in the real `~/Library/WebKit` and `~/Library/Caches`, under the other identifier; delete those two folders after the runs.

## Report

- `samples.csv` records elapsed seconds, private bytes per group and in total, process counts, and CPU usage as a percentage of total logical processor capacity. CPU uses consecutive process-time deltas, so the first sample and a process's first appearance lack a CPU delta. Short-lived processes between samples are outside the observation window.
- `processes.csv` records contributing PIDs, parent PIDs, executable basenames and groups. The app and WebView share one group, owned runtimes and their other descendants make up another, and explicit agent trees include their CLI, shells, and tool processes. Root selection and capture exclusions should accompany the results.
- Private bytes measure committed memory private to each process. Sum these values across the owned processes. The app status bar uses working set on Windows, so publish the private-bytes figure with its metric label. Samples where `ExpectedProcesses` and `MeasuredProcesses` differ can undercount memory, so report how many samples were affected and repeat steady runs with missing measurements.
- For each run, report the sample median and the highest observed `TotalPrivateBytes`, along with CPU median and peak. For each scenario, report the median of the run medians, the range across runs and the highest observed peak. State the sample interval with the results.
- Before committing baseline results, remove account names, private paths, sensitive executable names and machine-identifying details. Keep the aggregate results, versions, settings and a command with placeholder PIDs.

## App memory baseline

The app process and its web view, in the three states that a change is judged against: just started, one task open with one agent tab, and ten tasks open. No bound exists for these yet.

| State | macOS footprint | macOS RSS | Windows |
| --- | --- | --- | --- |
| Just started | 104.0 MiB (runs 100.8 to 105.6, peak 121.4) | 142.5 MiB (runs 99.6 to 145.2, peak 176.0) | not measured yet |
| One task, one agent tab | not measured yet | not measured yet | not measured yet |
| Ten tasks | not measured yet | not measured yet | not measured yet |

Each figure is the median of three run medians, the range of the run medians, and the highest sample of any run. Of the 104.0 MiB just started, the app process is 30.7 MiB (runs 30.3 to 33.1) and the WebKit helpers 72.5 MiB (runs 70.1 to 73.5). One sample split the helpers into about 45 MiB for web content, 21 MiB for GPU and 7 MiB for networking. The runtime had not started in any run, so no runtime memory is included.

How the macOS figures were taken:

- Machine: Mac with Apple M2, 16 GB RAM, macOS 26.6 (25G72).
- Build: the local, unsigned release build of commit `f4313f5` described in [Release binary, macOS](#release-binary-macos), built a second time with the bundle identifier changed so it could run beside an installed Brachio. Started from its binary with an empty `HOME` and its own `BRACHIO_RUNTIME_ENDPOINT`, so with no projects, tasks or settings.
- Capture: for each run a fresh instance, 30 seconds after launch, `scripts/measure-memory-macos.sh <app pid> 60 1`, 60 samples over about 70 seconds; three runs. The installed Brachio and other apps kept running on the same Mac.

The other two states need a project, tasks and a signed-in agent, set up through the app's window, and were not reached from the command line. On Windows, all three states are still to be measured with `scripts/measure-memory.ps1`.

## Automated upper bound

A smoke check guards against large regressions between full benchmark runs. It does not replace the methodology above and its numbers are not benchmark results.

`crates/brachio-runtime/tests/memory_bound.rs` starts a real runtime and, for 1, 5 and 10 shell sessions, runs a short local print workload in each, reads every session the way the app does, then takes five working-set samples 0.5 seconds apart. It compares the highest sample of two measurements with a bound:

| Measured | Bound |
| --- | --- |
| Working set of the runtime process tree (runtime and its shells) | 48 MiB plus 24 MiB for each session |
| Working set of the test process, which stands in for the app side | 32 MiB plus 2 MiB for each session |

The test process holds the client library and the output pages it reads, but not the web view, so it is not the figure the status bar shows. Working set is not the private bytes the manual procedure reports, and pages shared between processes can be counted more than once. The test runs a debug build.

The bounds were set about 1.5 times above the first local run on one Windows machine (runtime tree 31, 103 and 194 MiB; client 12 to 13 MiB). They are meant to catch a leak or a per-session jump, not a small drift. Change them only with a measurement and the reason in the same pull request.

Run it locally on Windows with:

```powershell
cargo test -p brachio-runtime --test memory_bound -- --ignored --nocapture
```

It is ignored by default, so `cargo test --workspace` skips it. The `Memory bound` workflow runs it on `windows-latest` for pull requests that touch the runtime or the workflow, on a weekly schedule and on demand. The job does not block other checks: it is not a required check and its failure does not fail the workflow. The output is a CSV of the measured and bound values per session count. Make it blocking only after several runs show that its noise is well below the bounds.

## Size and dependency baseline

Installer sizes are the release assets as published. A MiB is 1,048,576 bytes.

| Asset | 0.1.0 | 0.2.0-rc.2 | 0.2.0-rc.3 |
| --- | --- | --- | --- |
| Windows setup (`x64-setup.exe`) | 8.02 MiB | 8.76 MiB | 8.81 MiB |
| Windows `.msi` | 10.79 MiB | not published | not published |
| macOS disk image (`aarch64.dmg`) | 9.23 MiB | 10.00 MiB | 10.06 MiB |
| macOS `app.tar.gz` | 9.16 MiB | 9.92 MiB | 9.98 MiB |

Read them with `gh release view v0.2.0-rc.3 --json assets --jq '.assets[] | "\(.name) \(.size)"'` and divide the bytes by 1,048,576.

### Release binary, macOS

From a local, unsigned release build of commit `f4313f5` on `main`, a few commits after 0.2.0-rc.3, built the way the release workflow builds it: `pnpm --dir ui install --frozen-lockfile`, then `pnpm dlx @tauri-apps/cli@2.12.4 build --config src-tauri/tauri.release.conf.json --bundles app,dmg` with Rust 1.95.0 on Apple silicon.

| File | Size |
| --- | --- |
| App binary (`Contents/MacOS/brachio-app`) | 17.18 MiB |
| Runtime sidecar (`Contents/MacOS/brachio-runtime`) | 1.78 MiB |
| Helper sidecar (`Contents/MacOS/brachio`) | 1.35 MiB |
| `Brachio.app`, all files | 20.39 MiB |
| Disk image (`aarch64.dmg`) | 10.14 MiB |

The disk image is 0.08 MiB larger than the published 0.2.0-rc.3 one. The commits since the tag or differences between this Mac and the CI runner could explain it; which one was not checked. Compare releases with the published assets above, not with local builds. The Windows release binary is not measured yet.

### Dependencies

Dependency counts at commit `f4313f5`, from `node scripts/size-baseline.mjs`:

| Measure | Value |
| --- | --- |
| Rust crates in the app's normal and build graph, Windows (`x86_64-pc-windows-msvc`) | 345 |
| Rust crates in the app's normal and build graph, macOS (`aarch64-apple-darwin`) | 336 |
| Rust crates in the app's normal and build graph, Linux (`x86_64-unknown-linux-gnu`) | 425 |
| Packages in `Cargo.lock` | 606 |
| Packages in `ui/pnpm-lock.yaml` | 351 |
| UI packages reachable from `dependencies` (production) | 14 |
| UI packages reachable only from `devDependencies` | 337 |

The crate count is the unique crates `cargo tree --locked -p brachio-app --target <target> -e normal,build --prefix none` lists for the target, including the app and workspace crates and excluding dev dependencies. It needs no installed target or cross toolchain. The Linux graph is larger because the web view there is WebKitGTK, reached through the GTK bindings; Brachio publishes no Linux build. The UI counts walk the `snapshots` section of the lock file from the `dependencies` and `devDependencies` of `ui/package.json`; a package is its name and version, so one resolved with different peers counts once, and the two UI counts add up to the lock file total. Only the production packages ship in the app, bundled into the UI scripts. A pull request that adds a dependency or a periodic job should state these figures before and after. The "Version files agree" job in CI prints the same table in its summary on every run. It only reports and never fails the check.

## Tab area budget

Applies to every pull request that adds code or a view to the task tab area: file tabs and browser tabs.

### Baseline

Taken from a fresh `pnpm --dir ui build` at the commit that added this section, and printed by `node scripts/size-baseline.mjs` after a build. Sizes are minified bytes and gzip, in kB of 1,000 bytes.

| Measure | Value |
| --- | --- |
| Main UI script (`index.js`) | 973.44 kB, 275.42 gzip |
| Lazy chunks | none |

An editor with the markdown language bundled by itself measures about 616 kB, 211 kB gzip. Grammars for other languages add more, and load one by one.

### Budget

- The editor and its language grammars are lazy chunks, loaded by a dynamic `import()` when the first file tab opens. Nothing from the editor is in the main script.
- The main script grows by no more than 25 kB minified (about 8 kB gzip) for the whole tab area, not per pull request. That covers the tab kinds, the file tab shell and the strings.
- The editor core with the markdown language stays under 700 kB minified (about 240 kB gzip). Each further language grammar is its own chunk and loads only for a file of that language.
- The Windows setup grows by no more than 1 MiB for file tabs. Browser tabs use the system web view and add no engine to the installer.
- The app's memory with one open file tab, once the editor has loaded, grows by no more than 40 MiB working set over the same app with none open. Each further file tab adds no more than 3 MiB for a source file under 1 MiB, and closing every file tab returns within 10 MiB of the start. A browser tab's budget is set after the native web view decision.
- A pull request states the main script and lazy chunk sizes, minified and gzip, before and after, and the figures that apply to it from the lists above.

### Not measured yet

The memory figures above are proposed limits, not measurements. They need an installed release build, the procedure in this page with 0, 1 and 5 file tabs and 0, 1 and 3 browser tabs, and the setup size of a build that contains the editor. Until then the build sizes above are the only enforced numbers. Replace each proposed limit with the measured result and its margin when those runs are done.

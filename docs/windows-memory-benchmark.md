# Memory benchmark methodology

This page describes how to measure Brachio's memory use on Windows. No results are published yet: the numbers will be added here, with their methodology and environment, after the runs are done. Until then, make no performance claims from this page.

Use a release build on a single Windows machine and start the sampler from a separate PowerShell window. Use one disposable repository and the same app build, terminal runtime mode, agent versions and configuration, task fixture, window size, scrollback settings, and warm-up for every run. Record the Windows build, CPU and logical processor count, RAM, app commit and build type, WebView2 version, runtime mode, and enabled agent tool versions.

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

## Capture

Identify the measured app's PID in Task Manager or Process Explorer. For detached mode, inspect runtime candidates locally with `Get-CimInstance Win32_Process` and match their executable and `--pipe` endpoint to the Brachio instance under test. If several owned runtime generations host sessions, include all of their PIDs. Identify the launch root for each agent session in the process tree. For a CLI launched via a shell wrapper, supply the wrapper PID so its child processes land in the agent group.

From the repository root, run:

```powershell
.\scripts\measure-memory.ps1 -AppPid 1234 -RuntimePid 2345,2346 -AgentPid 3456,3457 `
    -DurationSeconds 60 -IntervalSeconds 1 -OutputDirectory "$env:TEMP\brachio-5-sessions-run-1"
```

Replace the example PIDs with the observed values. Omit `-RuntimePid` for in-app terminals and `-AgentPid` when no agents are running. Use a new output directory for each capture. If an app or runtime exits during capture, discard the partial run and capture the new process set separately. Processes appearing and disappearing under the same roots are sampled as they exist at each observation.

For an ownership check, run a separate Brachio instance with its own `BRACHIO_RUNTIME_ENDPOINT` and keep its PIDs out of the command above. Check that its app, runtime and agent PIDs are absent from `processes.csv`. The script includes the explicit app and runtime trees and counts each PID once.

## Report

- `samples.csv` records elapsed seconds, private bytes per group and in total, process counts, and CPU usage as a percentage of total logical processor capacity. CPU uses consecutive process-time deltas, so the first sample and a process's first appearance lack a CPU delta. Short-lived processes between samples are outside the observation window.
- `processes.csv` records contributing PIDs, parent PIDs, executable basenames and groups. The app and WebView share one group, owned runtimes and their other descendants make up another, and explicit agent trees include their CLI, shells, and tool processes. Root selection and capture exclusions should accompany the results.
- Private bytes measure committed memory private to each process. Sum these values across the owned processes. The app status bar uses working set on Windows, so publish the private-bytes figure with its metric label. Samples where `ExpectedProcesses` and `MeasuredProcesses` differ can undercount memory, so report how many samples were affected and repeat steady runs with missing measurements.
- For each run, report the sample median and the highest observed `TotalPrivateBytes`, along with CPU median and peak. For each scenario, report the median of the run medians, the range across runs and the highest observed peak. State the sample interval with the results.
- Before committing baseline results, remove account names, private paths, sensitive executable names and machine-identifying details. Keep the aggregate results, versions, settings and a command with placeholder PIDs.

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

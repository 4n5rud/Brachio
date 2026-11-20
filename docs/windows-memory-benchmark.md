# Windows memory benchmark

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

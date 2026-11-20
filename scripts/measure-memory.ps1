[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateRange(1, [int]::MaxValue)]
    [int] $AppPid,

    [int[]] $RuntimePid = @(),
    [int[]] $AgentPid = @(),

    [ValidateRange(1, 86400)]
    [int] $DurationSeconds = 60,

    [ValidateRange(1, 3600)]
    [int] $IntervalSeconds = 1,

    [Parameter(Mandatory)]
    [string] $OutputDirectory
)

$ErrorActionPreference = 'Stop'

function Get-ProcessSnapshot {
    $snapshot = @{}
    foreach ($entry in Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, CreationDate, Name) {
        $snapshot[[int] $entry.ProcessId] = $entry
    }
    return $snapshot
}

function Get-Descendants($Snapshot, [int[]] $Roots) {
    $owned = @{}
    foreach ($root in $Roots) {
        $owned[$root] = $true
    }
    do {
        $before = $owned.Count
        foreach ($entry in $Snapshot.Values) {
            $parent = [int] $entry.ParentProcessId
            if ($owned.ContainsKey($parent) -and $Snapshot[$parent].CreationDate -le $entry.CreationDate) {
                $owned[[int] $entry.ProcessId] = $true
            }
        }
    } while ($owned.Count -gt $before)
    return $owned
}

$initial = Get-ProcessSnapshot
$roots = @($AppPid) + $RuntimePid + $AgentPid
foreach ($root in $roots) {
    if (-not $initial.ContainsKey($root) -or -not $initial[$root].CreationDate) {
        throw "Process $root is unavailable. Verify the app, runtime and agent roots."
    }
}
$initialOwned = Get-Descendants $initial (@($AppPid) + $RuntimePid)
foreach ($root in $AgentPid) {
    if (-not $initialOwned.ContainsKey($root) -or $root -eq $AppPid -or $root -in $RuntimePid) {
        throw "Agent process $root must be below the app or an explicitly supplied runtime."
    }
}

New-Item -ItemType Directory -Path $OutputDirectory -ErrorAction Stop | Out-Null
$samplesFile = Join-Path $OutputDirectory 'samples.csv'
$processesFile = Join-Path $OutputDirectory 'processes.csv'
$clock = [Diagnostics.Stopwatch]::StartNew()
$lastSample = $null
$previousCpu = @{}
$logicalProcessors = [Environment]::ProcessorCount
$sample = 0

do {
    $snapshot = Get-ProcessSnapshot
    # Match each root against its initial creation time to reject a reused PID.
    foreach ($root in @($AppPid) + $RuntimePid) {
        if (-not $snapshot.ContainsKey($root) -or $snapshot[$root].CreationDate -ne $initial[$root].CreationDate) {
            throw "App or runtime process $root exited. Start a separate capture for the new process set."
        }
    }
    $owned = Get-Descendants $snapshot (@($AppPid) + $RuntimePid)
    $runtime = Get-Descendants $snapshot $RuntimePid
    $activeAgents = @($AgentPid | Where-Object {
        $owned.ContainsKey($_) -and $snapshot[$_].CreationDate -eq $initial[$_].CreationDate
    })
    $agents = Get-Descendants $snapshot $activeAgents
    $elapsed = $clock.Elapsed.TotalSeconds
    $nextCpu = @{}
    $rows = @(foreach ($ownedPid in ($owned.Keys | Sort-Object)) {
        $entry = $snapshot[$ownedPid]
        $process = $null
        try {
            $process = [Diagnostics.Process]::GetProcessById($ownedPid)
            # CIM CreationDate has microsecond precision, unlike Process.StartTime.
            if ([math]::Abs(($process.StartTime - $entry.CreationDate).TotalMilliseconds) -gt 1) {
                continue
            }
            $memory = $process.PrivateMemorySize64
            $cpu = $process.TotalProcessorTime.TotalSeconds
        } catch [ArgumentException], [InvalidOperationException], [ComponentModel.Win32Exception] {
            # The process exited or became inaccessible after the snapshot.
            continue
        } finally {
            if ($null -ne $process) { $process.Dispose() }
        }

        $key = "$ownedPid/$($entry.CreationDate.Ticks)"
        $nextCpu[$key] = $cpu
        $cpuPercent = $null
        if ($null -ne $lastSample -and $previousCpu.ContainsKey($key)) {
            $cpuPercent = [math]::Round(100 * ($cpu - $previousCpu[$key]) / (($elapsed - $lastSample) * $logicalProcessors), 4)
        }
        $group = if ($agents.ContainsKey($ownedPid)) { 'agent-tools' }
                 elseif ($runtime.ContainsKey($ownedPid)) { 'runtime' }
                 else { 'app-webview' }
        [pscustomobject] @{
            Sample = $sample
            Pid = $ownedPid
            ParentPid = $entry.ParentProcessId
            Process = $entry.Name
            Group = $group
            PrivateBytes = $memory
            CpuMachinePercent = $cpuPercent
        }
    })
    if ($rows.Pid -notcontains $AppPid) {
        throw "The app process $AppPid could not be measured."
    }
    $rows | Export-Csv -LiteralPath $processesFile -Append -NoTypeInformation
    [pscustomobject] @{
        Sample = $sample
        ElapsedSeconds = [math]::Round($elapsed, 3)
        ExpectedProcesses = $owned.Count
        MeasuredProcesses = $rows.Count
        TotalPrivateBytes = [long] ($rows | Measure-Object PrivateBytes -Sum).Sum
        AppWebViewPrivateBytes = [long] ($rows | Where-Object Group -eq 'app-webview' | Measure-Object PrivateBytes -Sum).Sum
        RuntimePrivateBytes = [long] ($rows | Where-Object Group -eq 'runtime' | Measure-Object PrivateBytes -Sum).Sum
        AgentToolsPrivateBytes = [long] ($rows | Where-Object Group -eq 'agent-tools' | Measure-Object PrivateBytes -Sum).Sum
        CpuMachinePercent = if ($null -ne $lastSample) { ($rows | Measure-Object CpuMachinePercent -Sum).Sum }
    } | Export-Csv -LiteralPath $samplesFile -Append -NoTypeInformation
    $lastSample = $elapsed
    $previousCpu = $nextCpu
    $sample++
    $remaining = $DurationSeconds - $clock.Elapsed.TotalSeconds
    if ($remaining -gt 0) {
        Start-Sleep -Milliseconds ([int] (1000 * [math]::Min($IntervalSeconds, $remaining)))
    }
} while ($remaining -gt 0)

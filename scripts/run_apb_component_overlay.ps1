param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [int]$ReplaceIdleLauncherId = 0,
    [string]$ReplaceIdleSessionDirectory = ''
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) { throw "Output already exists: $directory" }
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) { throw "Output parent missing" }
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)." }
Set-Location -LiteralPath $root
try {
    $scanAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    "Launcher administrator: $scanAdmin" | Tee-Object -FilePath $log
    if ($ReplaceIdleLauncherId -ne 0) {
        $previousEvents = Join-Path $ReplaceIdleSessionDirectory 'events.jsonl'
        $lastEvent = Get-Content -LiteralPath $previousEvents -Tail 1 | ConvertFrom-Json
        if ($lastEvent.state -ne 'waiting') { throw 'Previous overlay has started; close it with F11 before replacement.' }
        $previousLauncher = Get-CimInstance Win32_Process -Filter "ProcessId = $ReplaceIdleLauncherId"
        if ($previousLauncher -and $previousLauncher.CommandLine -notlike '*run_apb_component_overlay.ps1*') { throw 'Replacement PID is not the expected overlay launcher.' }
        $previousChildren = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $ReplaceIdleLauncherId" | Where-Object { $_.Name -eq 'python.exe' -and $_.CommandLine -like '*apb_component_overlay.py*' })
        foreach ($previousChild in $previousChildren) { Stop-Process -Id $previousChild.ProcessId -ErrorAction Stop }
        if ($previousLauncher) { Stop-Process -Id $ReplaceIdleLauncherId -ErrorAction Stop }
        '{"state":"replaced_while_idle","reason":"updated visible administrator status and readable details"}' | Add-Content -LiteralPath $previousEvents
        'Previous waiting overlay replaced; captured files retained.' | Tee-Object -FilePath $log -Append
    }
    & python -u -B (Join-Path $PSScriptRoot 'apb_component_overlay.py') $game[0].Id $directory 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "Component overlay failed; inspect $log" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

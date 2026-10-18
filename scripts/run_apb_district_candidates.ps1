param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [int]$Seconds = 120,
    [switch]$InspectOnly,
    [switch]$Lifecycle,
    [switch]$Overlay,
    [switch]$ManualFilter,
    [switch]$StructureFilter,
    [switch]$OwnershipFilter,
    [switch]$MissionLabels,
    [switch]$MissionClassify
)

$ErrorActionPreference = 'Stop'
$Host.UI.RawUI.WindowTitle = 'APB district candidates | F8 starts and stops'
if ($Overlay) { $Host.UI.RawUI.WindowTitle = 'APB visual candidates | F8 start-stop | F9 save frame' }
if ($ManualFilter) { $Host.UI.RawUI.WindowTitle = 'APB manual filter | F6 exclude | F4 player | F7 undo' }
if ($OwnershipFilter) { $Host.UI.RawUI.WindowTitle = 'APB current pawn ownership | self excluded | F8 start-stop | F9 frame' }
if ($MissionLabels) { $Host.UI.RawUI.WindowTitle = 'APB mission labels | F1 outside | F2 teammate | F3 opposition | Ctrl-F2-F3 own state | F10 pause | F11 exit' }
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output already exists: $directory"
}
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) {
    throw "Output parent missing: $directory"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($game.Count -ne 1) { throw "Expected one APB process; found $($game.Count)." }
Set-Location -LiteralPath $root
try {
    $taskArguments = @($game[0].Id, $directory, '--seconds', $Seconds)
    if ($InspectOnly) { $taskArguments += '--inspect-only' }
    if ($Lifecycle) { $taskArguments += '--lifecycle' }
    if ($Overlay) { $taskArguments += '--overlay' }
    if ($ManualFilter) { $taskArguments += '--manual-filter' }
    if ($StructureFilter) { $taskArguments += '--structure-filter' }
    if ($OwnershipFilter) { $taskArguments += '--ownership-filter' }
    if ($MissionLabels) { $taskArguments += '--mission-labels' }
    if ($MissionClassify) { $taskArguments += '--mission-classify' }
    & python -u -B (Join-Path $PSScriptRoot 'apb_district_candidates.py') @taskArguments 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "District diagnostic failed; inspect $log" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

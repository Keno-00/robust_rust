param(
    [Parameter(Mandatory=$true)][string]$EvidenceReport,
    [string]$OutputDirectory,
    [double]$Hz = 10
)
$ErrorActionPreference = 'Stop'
$reportPath = [System.IO.Path]::GetFullPath($EvidenceReport)
if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) { throw 'Recency evidence report is missing.' }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $visibilityRoot = Join-Path (Join-Path $PSScriptRoot '../..') 'drayebr/captures/visibility'
    $OutputDirectory = Join-Path $visibilityRoot ((Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '-recency-live')
}
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) { throw "Output exists: $directory" }
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) { throw 'Output parent must exist.' }
$administrator = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $administrator) { throw 'Run from elevated PowerShell for EchoDriver.' }
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($game.Count -ne 1) { throw 'Expected exactly one APB process. Enter a district first.' }
Write-Host "Float64 recency live output: $directory"
try {
    & python -u -B (Join-Path $PSScriptRoot 'apb_visibility_recency_live.py') $game[0].Id $reportPath $directory --hz $Hz 2>&1 | Tee-Object -FilePath $log
    if ($LASTEXITCODE -ne 0) { throw "Live recency reader exited with code $LASTEXITCODE; inspect $log and retained evidence." }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

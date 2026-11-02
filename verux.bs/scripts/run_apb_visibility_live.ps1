param(
    [Parameter(Mandatory=$true)][string]$SourceDirectory,
    [Parameter(Mandatory=$true)][string]$CandidateId,
    [string]$OutputDirectory,
    [double]$Hz = 5
)
$ErrorActionPreference = 'Stop'
$source = [System.IO.Path]::GetFullPath($SourceDirectory)
if (-not (Test-Path -LiteralPath (Join-Path $source 'candidates.json') -PathType Leaf)) { throw 'Source must be a completed v2 discovery directory with candidates.json.' }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path (Split-Path -Parent $source) ((Split-Path -Leaf $source) + '-live-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
}
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) { throw "Output exists: $directory" }
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) { throw 'Output parent must exist.' }
$administrator = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $administrator) { throw 'Run from elevated PowerShell for EchoDriver.' }
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($game.Count -ne 1) { throw 'Expected exactly one APB process.' }
Write-Host "Live visibility candidate output: $directory"
try {
    & python -u -B (Join-Path $PSScriptRoot 'apb_visibility_live.py') $game[0].Id $source $CandidateId $directory --hz $Hz 2>&1 | Tee-Object -FilePath $log
    if ($LASTEXITCODE -ne 0) { throw "Live reader exited with code $LASTEXITCODE; inspect $log and retained evidence." }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

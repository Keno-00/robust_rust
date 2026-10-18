param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$WitnessOnly
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) {
    throw "Output parent directory does not exist: $(Split-Path -Parent $directory)"
}
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output directory or log already exists: $directory"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)." }
Set-Location -LiteralPath $root
$pythonArguments = @($game[0].Id, $directory)
if ($WitnessOnly) { $pythonArguments += '--witness-only' }
& python -u -B (Join-Path $PSScriptRoot 'apb_players_f8.py') @pythonArguments 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Witness scan failed with exit $LASTEXITCODE. Inspect $log" }
Write-Host "Witness scan complete. Log: $log"

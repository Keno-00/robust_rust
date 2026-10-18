param(
    [Parameter(Mandatory = $true)][string]$Label,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [ValidateRange(32768, 131072)][int]$Entries = 65536
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
& python -u -B (Join-Path $PSScriptRoot 'apb_camera_capture_f8.py') $game[0].Id $Label $directory --entries $Entries 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Single-state camera capture failed with exit $LASTEXITCODE. Inspect $log" }
Write-Host "Single-state camera capture complete. Log: $log"

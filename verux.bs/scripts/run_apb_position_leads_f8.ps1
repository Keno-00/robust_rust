param(
    [Parameter(Mandatory = $true)][string]$Manifest,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$InspectOnly
)

$ErrorActionPreference = 'Stop'
$Host.UI.RawUI.WindowTitle = if ($InspectOnly) { 'APB position leads | Header inspection' } else { 'APB position leads | Laptop jump trace' }
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = [System.IO.Path]::GetFullPath($Manifest)
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Lead manifest missing: $source" }
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) { throw "Output parent missing" }
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) { throw "Output or log already exists: $directory" }
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)." }
Set-Location -LiteralPath $root
try {
    $pythonArguments = @($game[0].Id, $source, $directory)
    if ($InspectOnly) { $pythonArguments += '--inspect-only' }
    & python -u -B (Join-Path $PSScriptRoot 'apb_position_leads_f8.py') @pythonArguments 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "Targeted position trace failed with exit $result. Inspect $log" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

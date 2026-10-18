param(
    [Parameter(Mandatory = $true)][string]$SourceDirectory,
    [Parameter(Mandatory = $true)][string]$Address,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$FieldTrace
)

$ErrorActionPreference = 'Stop'
$Host.UI.RawUI.WindowTitle = 'APB player position | Module root references'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = [System.IO.Path]::GetFullPath($SourceDirectory)
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if (-not (Test-Path -LiteralPath $source -PathType Container)) { throw "Witness source directory missing: $source" }
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) {
    throw "Output parent directory missing: $(Split-Path -Parent $directory)"
}
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output directory or log already exists: $directory"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)." }
Set-Location -LiteralPath $root
try {
    $pythonArguments = @($game[0].Id, $source, $Address, $directory)
    if ($FieldTrace) { $pythonArguments += '--field-trace' }
    & python -u -B (Join-Path $PSScriptRoot 'apb_player_refs_f8.py') @pythonArguments 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "Player-position reference scan failed with exit $result. Inspect $log" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

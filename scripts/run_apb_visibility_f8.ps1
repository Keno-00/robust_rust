param([Parameter(Mandatory = $true)][string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
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
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count). Start APB and enter a district." }
Set-Location -LiteralPath $root
try {
    & python -u -B (Join-Path $PSScriptRoot 'apb_visibility_f8.py') $game[0].Id $directory 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "Visibility annotation session failed; inspect $log" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

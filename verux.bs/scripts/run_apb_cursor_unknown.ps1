param([Parameter(Mandatory = $true)][string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output directory or log already exists for $directory"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) {
    throw "Expected one APB.exe process; found $($game.Count)."
}
Set-Location -LiteralPath $root
& python -u -B (Join-Path $PSScriptRoot 'apb_cursor_unknown.py') $game[0].Id $directory 2>&1 | Tee-Object -FilePath $log
Write-Host "Cursor discovery exited. Log: $log"

param(
    [Parameter(Mandatory = $true)][string]$SourceDirectory,
    [Parameter(Mandatory = $true)][string]$Output
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = [System.IO.Path]::GetFullPath($SourceDirectory)
$outputPath = [System.IO.Path]::GetFullPath($Output)
$log = "$outputPath.log"
if ((Test-Path -LiteralPath $outputPath) -or (Test-Path -LiteralPath $log)) {
    throw "Recheck output or log already exists for $outputPath"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) {
    throw "Expected one APB.exe process; found $($game.Count)."
}
Set-Location -LiteralPath $root
& python -u -B (Join-Path $PSScriptRoot 'apb_cursor_recheck.py') $game[0].Id $source $outputPath 2>&1 | Tee-Object -FilePath $log
Write-Host "Cursor recheck exited. Log: $log"

param([Parameter(Mandatory = $true)][string]$Output)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$outputPath = [System.IO.Path]::GetFullPath($Output)
$log = $outputPath + '.log'
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)." }
Set-Location -LiteralPath $root
$ErrorActionPreference = 'Continue'
& python -u -B (Join-Path $PSScriptRoot 'apb_cursor_modules.py') $game[0].Id $outputPath 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "EchoDriver module inventory failed; see $log" }

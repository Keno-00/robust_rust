$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$snapshotDir = [System.IO.Path]::GetFullPath((Join-Path $root '..\drayebr\build-rpm\snapshots'))
if (-not (Test-Path -LiteralPath $snapshotDir -PathType Container)) {
    throw "EchoDriver snapshot directory is missing: $snapshotDir"
}
$status = Join-Path $snapshotDir 'apb-discovery-status.txt'
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue |
    Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) {
    "Expected one APB.exe process; found $($game.Count)." | Set-Content -LiteralPath $status
    exit 1
}
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $snapshotDir "apb-discovery-$stamp.json"
$ErrorActionPreference = 'Continue'
$result = & python (Join-Path $PSScriptRoot 'apb_discover_readonly.py') $game[0].Id $output 2>&1
$code = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
$result | Out-String | Set-Content -LiteralPath $status
if ($code -ne 0) { exit $code }
exit 0

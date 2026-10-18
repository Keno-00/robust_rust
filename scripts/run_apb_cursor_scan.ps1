param(
    [Parameter(Mandatory = $true)][ValidateSet('first', 'next')][string]$Phase,
    [Parameter(Mandatory = $true)][ValidateRange(0, 65535)][int]$Value,
    [Parameter(Mandatory = $true)][string]$OutputPrefix,
    [string]$SourcePrefix,
    [ValidateRange(0, 60)][int]$StartDelaySeconds = 0
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$output = [System.IO.Path]::GetFullPath($OutputPrefix)
$log = "$output.log"
$status = "$output.exitcode"
if ((Test-Path -LiteralPath $log) -or (Test-Path -LiteralPath $status)) {
    throw "Capture status already exists for $output"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) {
    "Expected one APB.exe process; found $($game.Count)." | Set-Content -LiteralPath $log
    1 | Set-Content -LiteralPath $status
    exit 1
}
Set-Location -LiteralPath $root
if ($StartDelaySeconds -gt 0) {
    "Waiting $StartDelaySeconds seconds before capture; return focus to APB." | Set-Content -LiteralPath $log
    Start-Sleep -Seconds $StartDelaySeconds
}
$arguments = @('-B', (Join-Path $PSScriptRoot 'apb_cursor_scan.py'), $game[0].Id, $Phase, $Value, $output)
if ($Phase -eq 'next') {
    if (-not $SourcePrefix) { throw 'Next scan requires SourcePrefix.' }
    $arguments += @('--from', [System.IO.Path]::GetFullPath($SourcePrefix))
} elseif ($SourcePrefix) {
    throw 'First scan does not accept SourcePrefix.'
}
$ErrorActionPreference = 'Continue'
& python @arguments *> $log
$code = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
$code | Set-Content -LiteralPath $status
exit $code

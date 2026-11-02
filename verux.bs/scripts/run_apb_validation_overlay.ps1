param(
    [Parameter(Mandatory = $true)][string]$Profile,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$profilePath = (Resolve-Path -LiteralPath $Profile).Path
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
$logPath = "$outputPath.log"
if ((Test-Path -LiteralPath $outputPath) -or (Test-Path -LiteralPath $logPath)) {
    throw "Output already exists: $outputPath"
}
if (-not (Test-Path -LiteralPath (Split-Path -Parent $outputPath) -PathType Container)) {
    throw 'Output parent is missing'
}
$games = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($games.Count -ne 1) { throw "Expected one APB process; found $($games.Count)" }
Set-Location -LiteralPath $root
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
$ErrorActionPreference = 'Continue'
& python -u -B (Join-Path $PSScriptRoot 'apb_validation_overlay.py') $games[0].Id $profilePath $outputPath 2>&1 |
    ForEach-Object { $_.ToString() } | Out-File -LiteralPath $logPath -Encoding utf8
$overlayExitCode = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
"Python exit code: $overlayExitCode" | Out-File -LiteralPath $logPath -Append -Encoding utf8
if ($overlayExitCode -ne 0) { throw "Validation overlay failed: inspect $logPath" }

param(
    [Parameter(Mandatory = $true)][string]$Profile,
    [Parameter(Mandatory = $true)][string]$Output,
    [string]$Tag = 'pumpkin'
)
$ErrorActionPreference = 'Stop'
$profilePath = (Resolve-Path -LiteralPath $Profile).Path
$outputPath = [System.IO.Path]::GetFullPath($Output)
$logPath = "$outputPath.log"
if ((Test-Path -LiteralPath $outputPath) -or (Test-Path -LiteralPath $logPath)) {
    throw "Output already exists: $outputPath"
}
if (-not (Test-Path -LiteralPath (Split-Path -Parent $outputPath) -PathType Container)) {
    throw 'Output parent must exist'
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)" }
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
try {
    "Explicit discovery profile: $profilePath; unverified build identity will be logged" | Tee-Object -FilePath $logPath
    $ErrorActionPreference = 'Continue'
    & python -u -B (Join-Path $PSScriptRoot 'apb_object_annotation_overlay.py') --pid $game[0].Id --profile $profilePath --output $outputPath --tag $Tag 2>&1 | ForEach-Object { $_.ToString() } | Tee-Object -FilePath $logPath -Append
    $overlayExitCode = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($overlayExitCode -ne 0) { throw "Object annotation overlay failed (exit $overlayExitCode): inspect $logPath" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $logPath -Append
    throw
}

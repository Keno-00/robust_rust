param(
    [string]$OutputDirectory,
    [ValidateSet('target-motion', 'observer-los')]
    [string]$Protocol = 'target-motion',
    [double]$Hz = 15,
    [double]$Seconds = 4,
    [double]$TrimSeconds = 0.5,
    [double]$CameraPositionTolerance = 1.0,
    [double]$CameraMatrixTolerance = 0.001,
    [double]$TargetPositionTolerance = 2.0
)

$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $visibilityRoot = Join-Path (Join-Path $PSScriptRoot '../..') 'drayebr/captures/visibility'
    New-Item -ItemType Directory -Path $visibilityRoot -Force | Out-Null
    $OutputDirectory = Join-Path $visibilityRoot (Get-Date -Format 'yyyyMMdd-HHmmss-fff')
}
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
Write-Host "Visibility discovery output: $directory"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output already exists: $directory"
}
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) {
    throw "Output parent missing: $directory"
}
$administrator = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $administrator) { throw 'Run this launcher from elevated PowerShell for the selected EchoDriver reader.' }
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($game.Count -ne 1) { throw 'Expected one APB process. Start APB and enter a district.' }
try {
    & python -u -B (Join-Path $PSScriptRoot 'apb_visibility_trace.py') $game[0].Id $directory --protocol $Protocol --hz $Hz --seconds $Seconds --trim-seconds $TrimSeconds --camera-position-tolerance $CameraPositionTolerance --camera-matrix-tolerance $CameraMatrixTolerance --target-position-tolerance $TargetPositionTolerance 2>&1 | Tee-Object -FilePath $log
    if ($LASTEXITCODE -ne 0) { throw "Visibility discovery exited with code $LASTEXITCODE. Inspect $log and retained evidence." }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

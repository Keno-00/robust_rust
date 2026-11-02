param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$RestartExisting,
    [string]$VerifyReadFrom
)

$ErrorActionPreference = 'Stop'
$Host.UI.RawUI.WindowTitle = 'APB LAPTOP health + stamina | F8 paced | F5 select | F11 exit'
$scanRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$scanDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
$scanLog = "$scanDirectory.log"
if ((Test-Path -LiteralPath $scanDirectory) -or (Test-Path -LiteralPath $scanLog)) {
    throw "Output already exists. Choose a NEW directory: $scanDirectory"
}
if (-not (Test-Path -LiteralPath (Split-Path -Parent $scanDirectory) -PathType Container)) {
    throw "Output parent missing: $scanDirectory"
}
$scanAdmin = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $scanAdmin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'EchoDriver capture requires an administrator terminal. Run this same launcher elevated; no alternative memory reader is used.'
}
$scanGames = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
if ($scanGames.Count -ne 1) { throw "Expected one APB process; found $($scanGames.Count)." }
$scanScript = Join-Path $PSScriptRoot 'apb_health_stamina_scan.py'
$scanExisting = @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -match '^python(w)?\.exe$' -and $_.CommandLine -match [regex]::Escape($scanScript)
})
if ($scanExisting.Count -and -not $RestartExisting) {
    throw 'Another health/stamina scan is listening for F8. Exit it with F11, or explicitly use -RestartExisting.'
}
foreach ($scanPrevious in $scanExisting) {
    Write-Host "Stopping previous health/stamina scan PID $($scanPrevious.ProcessId); saved files remain intact."
    Stop-Process -Id $scanPrevious.ProcessId -ErrorAction Stop
    Wait-Process -Id $scanPrevious.ProcessId -Timeout 5 -ErrorAction SilentlyContinue
    if (Get-Process -Id $scanPrevious.ProcessId -ErrorAction SilentlyContinue) {
        throw "Previous scan PID $($scanPrevious.ProcessId) did not exit; new scan withheld."
    }
}
Set-Location -LiteralPath $scanRoot
try {
    $scanArguments = @($scanGames[0].Id, $scanDirectory)
    if ($VerifyReadFrom) { $scanArguments += @('--verify-read-from', $VerifyReadFrom) }
    & python -u -B $scanScript @scanArguments 2>&1 |
        Tee-Object -FilePath $scanLog
    $scanResult = $LASTEXITCODE
    "Python exit code: $scanResult" | Tee-Object -FilePath $scanLog -Append
    if ($scanResult -ne 0) { throw "Health/stamina capture failed. Completed phases remain saved; inspect $scanLog" }
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $scanLog -Append
    throw
}

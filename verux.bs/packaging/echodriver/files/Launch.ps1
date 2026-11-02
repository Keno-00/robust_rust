param([ValidateSet('Install', 'Uninstall')][string]$Action)
$ErrorActionPreference = 'Stop'
try {
    $setup = Join-Path $PSScriptRoot 'Setup.ps1'
    $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $process = Start-Process -FilePath $powershell -Verb RunAs -ArgumentList ('-NoProfile -File "' + $setup + '" -Action ' + $Action + ' -PauseOnExit') -PassThru -Wait
    exit $process.ExitCode
} catch {
    Write-Host "Setup could not launch: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

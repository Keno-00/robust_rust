param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$expectedRoot = 'C:\Users\Admin\Documents\source_repos\robust_rust'
if ($root -ne $expectedRoot -or (Get-Location).Path -ne $expectedRoot) {
    throw "Run the core scan from the main checkout: $expectedRoot"
}
if (-not [IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $root $OutputDirectory
}
$directory = [IO.Path]::GetFullPath($OutputDirectory)
if (-not $directory.StartsWith("$root\", [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Core scan output must stay inside the main checkout.'
}
$log = "$directory.log"
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output already exists: $directory"
}
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) {
    throw 'Output parent is missing.'
}
Start-Transcript -LiteralPath $log -NoClobber
try {
    $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'EchoDriver core scanning requires an elevated PowerShell.'
    }
    $games = @(Get-Process -Name APB -ErrorAction SilentlyContinue)
    if ($games.Count -ne 1) { throw "Expected one APB process; found $($games.Count)." }
    $driver = Get-Service -Name EchoDriver
    Write-Host "EchoDriver initial state: $($driver.Status)"
    if ($driver.Status -ne 'Running') {
        Write-Host 'Starting the installed EchoDriver service.'
        Start-Service -Name EchoDriver
    }
    Write-Host "EchoDriver state: $((Get-Service -Name EchoDriver).Status); APB PID: $($games[0].Id)"
    $previousOverride = $env:APB_SCAN_ALLOW_UNVERIFIED_BUILD
    try {
        $env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
        $ErrorActionPreference = 'Continue'
        & python -u -B (Join-Path $PSScriptRoot 'apb_build_candidates.py') $games[0].Id $directory --wait-f8 2>&1 |
            ForEach-Object { Write-Host $_.ToString() }
        $scanExitCode = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($scanExitCode -ne 0) { throw "Core scan failed with exit $scanExitCode. Inspect $log" }
    } finally {
        $env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = $previousOverride
    }
} catch {
    Write-Host "CORE SCAN FAILED: $($_.Exception.Message)"
    throw
} finally {
    Stop-Transcript
}

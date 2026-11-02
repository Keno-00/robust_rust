param([ValidateSet('Install', 'Uninstall', 'Validate')][string]$Action = 'Install', [switch]$PauseOnExit)
$ErrorActionPreference = 'Stop'
$destination = Join-Path $env:ProgramFiles 'VeruxAPB\Driver'
$driverPath = Join-Path $destination 'EchoDriver.sys'
$logDirectory = Join-Path $env:ProgramData 'VeruxAPB\Logs'
$thumbprint = '18537D3D8E5D05E87E25FC7238C6AD8A92065407'
function Invoke-ServiceCommand([string[]]$Arguments) {
    $result = & "$env:SystemRoot\System32\sc.exe" @Arguments 2>&1
    $code = $LASTEXITCODE
    $result | ForEach-Object { Write-Host $_; Add-Content -LiteralPath $logPath -Value $_ }
    if ($code -ne 0) { throw "Service command failed ($code). See $logPath." }
}
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not ([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this setup from an Administrator PowerShell window.'
    }
    New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null
    $logPath = Join-Path $logDirectory 'EchoDriver-setup.log'
    Add-Content -LiteralPath $logPath -Value "$(Get-Date -Format o) $Action requested"
    $service = Get-CimInstance Win32_SystemDriver -Filter "Name='EchoDriver'"
    $registeredPath=if($service){$service.PathName.Trim('"')}else{$null}
    if($registeredPath -and $registeredPath.StartsWith('\??\')) {$registeredPath=$registeredPath.Substring(4)}
    if ($Action -ne 'Validate' -and $service -and $registeredPath -ine $driverPath) {
        throw "EchoDriver is registered at $($service.PathName). Remove that installation explicitly before using this package."
    }
    if ($Action -eq 'Uninstall') {
        if ($service) {
            $controller = Get-Service EchoDriver
            if ($controller.Status -ne 'Stopped') {
                Invoke-ServiceCommand -Arguments @('stop', 'EchoDriver')
                $controller.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(15))
            }
            Invoke-ServiceCommand -Arguments @('delete', 'EchoDriver')
        }
        if (Test-Path -LiteralPath $driverPath) { Remove-Item -LiteralPath $driverPath }
        Write-Host 'Driver removed. The shared test certificate remains trusted; Secure Boot and test-signing settings were not changed.'
    } else {
        if (-not [Environment]::Is64BitOperatingSystem) { throw 'This package requires 64-bit Windows.' }
        $source = Join-Path $PSScriptRoot 'EchoDriver.sys'
        $certificatePath = Join-Path $PSScriptRoot 'EchoDriver.cer'
        if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne '9D265D82C29C371368759141DBFA730E2A87071ADB5C0518A8B4C66471D5390B') { throw 'Driver integrity check failed. Obtain the original package.' }
        if ((Get-FileHash -LiteralPath $certificatePath -Algorithm SHA256).Hash -ne '84F0BD6EECFE766D82423025EE148A0597E24EEED3E73C3C7CD46AA4B1552F67') { throw 'Certificate integrity check failed. Obtain the original package.' }
        $keySetup = Join-Path $PSScriptRoot 'Initialize-TransportKey.ps1'
        if ((Get-FileHash -LiteralPath $keySetup -Algorithm SHA256).Hash -ne 'D39B4DCC0312B4E05CD00DE55D11306EF8EE98E55186540F986C71B7C1C3D613') { throw 'Transport key setup integrity check failed. Obtain the original package.' }
        $certificate = Get-PfxCertificate -FilePath $certificatePath
        if ($certificate.Thumbprint -ne $thumbprint -or $certificate.HasPrivateKey) { throw 'Incorrect public test certificate.' }
        if ($certificate.NotAfter -lt (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) { throw 'Test certificate is outside its validity period.' }
        if ($Action -eq 'Validate') { Write-Output 'Driver payload validated.'; return }
        if ($service -and (Get-Service EchoDriver).Status -ne 'Stopped') { throw 'Close VERUX.APB and stop EchoDriver before installing.' }
        Write-Host "Installing public test certificate $thumbprint into Local Machine Root and Trusted Publishers."
        Import-Certificate -FilePath $certificatePath -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
        Import-Certificate -FilePath $certificatePath -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
        $signature = Get-AuthenticodeSignature -LiteralPath $source
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Thumbprint -ne $thumbprint) { throw 'Driver signature verification failed.' }
        New-Item -ItemType Directory -Path $destination -Force | Out-Null
        Copy-Item -LiteralPath $source -Destination $driverPath -Force
        if (-not $service) { Invoke-ServiceCommand -Arguments @('create', 'EchoDriver', 'type=', 'kernel', 'start=', 'demand', 'binPath=', ('\??\' + $driverPath)) }
        else { Invoke-ServiceCommand -Arguments @('config', 'EchoDriver', 'binPath=', ('\??\' + $driverPath)) }
        & $keySetup | ForEach-Object { Write-Host $_; Add-Content -LiteralPath $logPath -Value $_ }
        Invoke-ServiceCommand -Arguments @('start', 'EchoDriver')
        (Get-Service EchoDriver).WaitForStatus('Running', [TimeSpan]::FromSeconds(15))
        Write-Host 'Driver installed and running. Open VERUX.APB and use Start EchoDriver in Memory to verify the process-read interface.'
    }
    Add-Content -LiteralPath $logPath -Value "$(Get-Date -Format o) $Action completed"
} catch {
    $message = "FAILED: $($_.Exception.Message)"
    if ($logPath) { Add-Content -LiteralPath $logPath -Value "$(Get-Date -Format o) $message" }
    Write-Host $message -ForegroundColor Red
    exit 1
} finally {
    if ($PauseOnExit) { Read-Host 'Press Enter to close' | Out-Null }
}


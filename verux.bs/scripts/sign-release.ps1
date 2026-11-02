param(
    [Parameter(Mandatory=$true)][string]$CertificateThumbprint,
    [string]$SignToolPath = 'signtool.exe',
    [string]$ExecutablePath = (Join-Path $PSScriptRoot '../target/release/verux_apb.exe')
)
$ErrorActionPreference = 'Stop'
$resolved = (Resolve-Path -LiteralPath $ExecutablePath).Path
$certificateId = $CertificateThumbprint.Replace(' ', '')
if ($certificateId -notmatch '^[a-fA-F0-9]{40}$') { throw 'Supply the SHA-1 thumbprint of your approved code-signing certificate.' }
$certificate = Get-Item -LiteralPath "Cert:/CurrentUser/My/$certificateId"
if (-not $certificate.HasPrivateKey -or $certificate.NotAfter -le (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) {
    throw 'The code-signing certificate is invalid or its private key is unavailable.'
}
if (-not ($certificate.EnhancedKeyUsageList.ObjectId -contains '1.3.6.1.5.5.7.3.3')) { throw 'The selected certificate is not authorized for code signing.' }
$tool = (Get-Command $SignToolPath -ErrorAction Stop).Source
& $tool sign /sha1 $certificateId /fd SHA256 /tr 'https://timestamp.digicert.com' /td SHA256 $resolved
if ($LASTEXITCODE -ne 0) { throw 'Authenticode signing failed. Do not publish the executable.' }
& $tool verify /pa /all $resolved
if ($LASTEXITCODE -ne 0) { throw 'Authenticode verification failed. Do not publish the executable.' }
$signature = Get-AuthenticodeSignature -LiteralPath $resolved
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Thumbprint -ne $certificateId) { throw 'Release signature does not match the approved signer.' }
Write-Output 'Release signature verified. Generate distribution checksums after signing.'

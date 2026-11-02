$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'VeruxSecrets.ps1')
Add-Type -AssemblyName System.Security
$privateRoot = Join-Path (Get-VeruxSecretsRoot) 'VeruxServerSecurity\surface-labels'
$originalPath = Join-Path $privateRoot 'catalog.dpapi'
$objectsPath = Join-Path $privateRoot 'catalog.objects.dpapi'
$keyPath = Join-Path $privateRoot 'surface-key.dpapi'
if (-not (Test-Path -LiteralPath $originalPath) -or -not (Test-Path -LiteralPath $keyPath)) {
    throw 'Recover the existing private APB display catalog and key before preparing object controls.'
}
$bytes = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($originalPath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
$encoded = $null
$labels = $null
try {
    $labels = [Text.Encoding]::UTF8.GetString($bytes) | ConvertFrom-Json -AsHashtable
    if ($labels.Count -ne 132) { throw 'Expected the existing 132-row catalog; inspect the current catalog revision.' }
    $labels['u133'] = 'Pumpkins'
    $labels['u134'] = 'Projectiles'
    $encoded = [Text.Encoding]::UTF8.GetBytes(($labels | ConvertTo-Json -Compress))
    $protected = [Security.Cryptography.ProtectedData]::Protect($encoded, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    [IO.File]::WriteAllBytes($objectsPath, $protected)
} finally {
    [Array]::Clear($bytes, 0, $bytes.Length)
    if ($encoded) { [Array]::Clear($encoded, 0, $encoded.Length) }
    $labels = $null
}
& (Join-Path $PSScriptRoot 'publish-surface.ps1') -Catalog 'surface.objects'

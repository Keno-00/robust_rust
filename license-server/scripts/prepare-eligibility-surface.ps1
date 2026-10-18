$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$privateRoot = Join-Path $env:LOCALAPPDATA 'VeruxServerSecurity\surface-labels'
$originalPath = Join-Path $privateRoot 'catalog.dpapi'
$objectsPath = Join-Path $privateRoot 'catalog.eligibility.dpapi'
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
    $labels['u135'] = 'Memory Features'
    $labels['u136'] = 'Vertical tracking'
    $labels['u137'] = 'Current'
    $labels['u138'] = 'No Y'
    $labels['u139'] = 'Head'
    $labels['u140'] = 'Neck'
    $labels['u141'] = 'Torso'
    $labels['u142'] = 'Feet'
    $labels['u143'] = 'Closest Point'
    $encoded = [Text.Encoding]::UTF8.GetBytes(($labels | ConvertTo-Json -Compress))
    $protected = [Security.Cryptography.ProtectedData]::Protect($encoded, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    [IO.File]::WriteAllBytes($objectsPath, $protected)
} finally {
    [Array]::Clear($bytes, 0, $bytes.Length)
    if ($encoded) { [Array]::Clear($encoded, 0, $encoded.Length) }
    $labels = $null
}
& (Join-Path $PSScriptRoot 'publish-surface.ps1') -Catalog 'surface.eligibility'

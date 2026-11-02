$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$privateRoot = Join-Path $env:LOCALAPPDATA 'VeruxServerSecurity\surface-labels'
$sourcePath = Join-Path $privateRoot 'catalog.eligibility.dpapi'
$targetPath = Join-Path $privateRoot 'catalog.hitsound.dpapi'
$bytes = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($sourcePath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
$encoded = $null
try {
    $labels = [Text.Encoding]::UTF8.GetString($bytes) | ConvertFrom-Json -AsHashtable
    if ($labels.Count -ne 143) { throw 'Expected the existing 143-row eligibility catalog.' }
    $labels['u137'] = 'Default'
    $labels['u144'] = 'Hit sound'
    $labels['u145'] = 'Sound'
    $encoded = [Text.Encoding]::UTF8.GetBytes(($labels | ConvertTo-Json -Compress))
    $protected = [Security.Cryptography.ProtectedData]::Protect($encoded, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    [IO.File]::WriteAllBytes($targetPath, $protected)
} finally {
    [Array]::Clear($bytes, 0, $bytes.Length)
    if ($encoded) { [Array]::Clear($encoded, 0, $encoded.Length) }
    $labels = $null
}
& (Join-Path $PSScriptRoot 'publish-surface.ps1') -Catalog surface.hitsound

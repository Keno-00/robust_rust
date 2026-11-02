param([ValidateSet('verux.apb','verux.apb.cb')][string]$Product = 'verux.apb')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$privateRoot = Join-Path $env:LOCALAPPDATA 'VeruxServerSecurity\surface-labels'
if ($Product -eq 'verux.apb.cb') { $privateRoot = Join-Path $privateRoot $Product }
$sourcePath = Join-Path $privateRoot $(if ($Product -eq 'verux.apb') { 'catalog.hitsound.dpapi' } else { 'catalog.eligibility.dpapi' })
$targetPath = Join-Path $privateRoot 'catalog.visuals.dpapi'
if (-not (Test-Path -LiteralPath $sourcePath)) { throw 'Recover the existing product display catalog before preparing Visuals. No embedded substitute is available.' }
$bytes = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($sourcePath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
$encoded = $null
try {
    $labels = [Text.Encoding]::UTF8.GetString($bytes) | ConvertFrom-Json -AsHashtable
    $expected = if ($Product -eq 'verux.apb') { 145 } else { 143 }
    if ($labels.Count -ne $expected) { throw 'Unexpected source catalog revision. Inspect the product catalog before preparing Visuals.' }
    $labels['u137'] = 'Default'
    $labels['u034'] = 'FOV'
    $labels['u096'] = 'Player ESP'
    $newLabels = @(
        'SB Protect', 'Risk', 'This setting increases risk.', 'OK', 'Aim', 'ESP',
        'Activations', 'Appearance', 'Configure', 'Color mode', 'Solid', 'Rainbow',
        'Breathing', 'Color', 'Opacity', 'Line width', 'Outline', 'Off', 'Black',
        'Custom', 'Outline width', 'Outline color', 'Fill', 'Fill opacity', 'Speed',
        'Saturation', 'Brightness', 'Minimum opacity', 'Shape', 'Full box', 'Circle',
        'Corners', 'Corner length', 'Marker size', 'Max distance (m, 0 = unlimited)',
        'Colors', 'Single color', 'Player relation', 'Endpoint', 'Center', 'Top',
        'Bottom', 'Copy appearance from', 'Aim FOV (px)', 'Range (m)', 'Object state'
    )
    for ($index = 0; $index -lt $newLabels.Count; $index++) { $labels[('u{0:000}' -f ($index + 146))] = $newLabels[$index] }
    $encoded = [Text.Encoding]::UTF8.GetBytes(($labels | ConvertTo-Json -Compress))
    $protected = [Security.Cryptography.ProtectedData]::Protect($encoded, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    [IO.File]::WriteAllBytes($targetPath, $protected)
} finally {
    [Array]::Clear($bytes, 0, $bytes.Length)
    if ($encoded) { [Array]::Clear($encoded, 0, $encoded.Length) }
    $labels = $null
}
& (Join-Path $PSScriptRoot 'publish-surface.ps1') -Product $Product -Catalog surface.visuals

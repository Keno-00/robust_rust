param(
    [switch]$Upload,
    [ValidateSet('verux.apb','verux.apb.cb')][string]$Product = 'verux.apb',
    [ValidateSet('surface','surface.objects','surface.eligibility','surface.hitsound','surface.visuals')][string]$Catalog = 'surface'
)
$ErrorActionPreference = 'Stop'
if ($Product -eq 'verux.apb.cb' -and $Catalog -notin @('surface.eligibility','surface.visuals')) { throw 'Color supports only its enrolled eligibility and visuals catalogs.' }
Add-Type -AssemblyName System.Security
$serverRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$privateRoot = Join-Path $env:LOCALAPPDATA 'VeruxServerSecurity\surface-labels'
if ($Product -eq 'verux.apb.cb') { $privateRoot = Join-Path $privateRoot 'verux.apb.cb' }
$catalogPath = Join-Path $privateRoot $(switch ($Catalog) { 'surface.visuals' { 'catalog.visuals.dpapi' } 'surface.hitsound' { 'catalog.hitsound.dpapi' } 'surface.objects' { 'catalog.objects.dpapi' } 'surface.eligibility' { 'catalog.eligibility.dpapi' } default { 'catalog.dpapi' } })
$keyPath = Join-Path $privateRoot 'surface-key.dpapi'
if (-not (Test-Path -LiteralPath $catalogPath)) { throw 'Recover the private display catalog vault before publishing. No embedded catalog is available.' }
$catalogBytes = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($catalogPath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
$labels = $null
$keyBytes = $null
try {
    $labels = [Text.Encoding]::UTF8.GetString($catalogBytes) | ConvertFrom-Json -AsHashtable
    $idsPath = Join-Path $serverRoot '..\src\petal_ids.rs'
    $required = @([regex]::Matches([IO.File]::ReadAllText($idsPath), '"(u\d{3})"') | ForEach-Object { $_.Groups[1].Value })
    if ($Catalog -eq 'surface.hitsound') { $required = @($required | Where-Object { [int]$_.Substring(1) -le 145 }) }
    if ($Product -eq 'verux.apb.cb') { $required = @($required | Where-Object { $_ -notin @('u144','u145') }) }
    if ($Catalog -eq 'surface.objects') { $required = @($required | Where-Object { [int]$_.Substring(1) -le 134 }) }
    if ($Catalog -eq 'surface.eligibility') { $required = @($required | Where-Object { [int]$_.Substring(1) -le 143 }) }
    if ($Catalog -eq 'surface') { $required = @($required | Where-Object { [int]$_.Substring(1) -le 132 }) }
    if ($required.Count -ne $labels.Count -or @($required | Where-Object { -not $labels.ContainsKey($_) }).Count -ne 0) { throw 'The private catalog does not match the client identifiers.' }
    if (-not (Test-Path -LiteralPath $keyPath)) {
        if ($Catalog -ne 'surface') { throw 'Recover the existing APB surface key; the object catalog must not replace it.' }
        $keyBytes = [byte[]]::new(32)
        [Security.Cryptography.RandomNumberGenerator]::Fill($keyBytes)
        $protected = [Security.Cryptography.ProtectedData]::Protect($keyBytes, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
        [IO.File]::WriteAllBytes($keyPath, $protected)
    } else {
        $keyBytes = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($keyPath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    }
    if ($keyBytes.Length -ne 32) { throw 'Private catalog key is invalid. Recover the correct vault.' }
    $encodedKey = [Convert]::ToBase64String($keyBytes).TrimEnd('=').Replace('+','-').Replace('/','_')
    $privateInput = @{product=$Product; key=$encodedKey; labels=$labels} | ConvertTo-Json -Depth 6 -Compress
    $encrypted = $privateInput | node (Join-Path $PSScriptRoot 'encrypt-surface.cjs')
    $privateInput = $null
    if ($LASTEXITCODE -ne 0) { throw 'Display catalog encryption failed.' }
    $prepared = $encrypted | ConvertFrom-Json
    $sqlPath = Join-Path $serverRoot $(switch ($Catalog) { 'surface.visuals' { 'security\apb-surface-visuals-catalog.sql' } 'surface.hitsound' { 'security\apb-surface-hitsound-catalog.sql' } 'surface.objects' { 'security\apb-surface-objects-catalog.sql' } 'surface.eligibility' { 'security\apb-surface-eligibility-catalog.sql' } default { 'security\apb-surface-catalog.sql' } })
    if ($Product -eq 'verux.apb.cb') { $sqlPath = Join-Path $serverRoot $(if ($Catalog -eq 'surface.visuals') { 'security\apb-cb-surface-visuals-catalog.sql' } else { 'security\apb-cb-surface-eligibility-catalog.sql' }) }
    $rowsSql = $prepared.encryptedRows.Replace("'", "''")
    $sql = "INSERT INTO surface_catalogs (product_id,catalog_id,version,encrypted_rows,digest,updated_at) VALUES ('$Product','$Catalog','2.0.0','$rowsSql','$($prepared.digest)',unixepoch()) ON CONFLICT(product_id,catalog_id,version) DO UPDATE SET encrypted_rows=excluded.encrypted_rows,digest=excluded.digest,updated_at=excluded.updated_at;`n"
    [IO.File]::WriteAllText($sqlPath, $sql, [Text.UTF8Encoding]::new($false))
    $configPath = Join-Path $serverRoot 'wrangler.toml'
    $config = [IO.File]::ReadAllText($configPath)
    $digestName = switch ($Catalog) { 'surface.visuals' { 'APB_SURFACE_VISUALS_DIGEST' } 'surface.hitsound' { 'APB_SURFACE_HITSOUND_DIGEST' } 'surface.objects' { 'APB_SURFACE_OBJECTS_DIGEST' } 'surface.eligibility' { 'APB_SURFACE_ELIGIBILITY_DIGEST' } default { 'APB_SURFACE_DIGEST' } }
    if ($Product -eq 'verux.apb.cb') { $digestName = if ($Catalog -eq 'surface.visuals') { 'APB_CB_SURFACE_VISUALS_DIGEST' } else { 'APB_CB_SURFACE_ELIGIBILITY_DIGEST' } }
    $pattern = '(?m)^'+$digestName+' = "[a-f0-9]*"'
    $setting = $digestName+' = "'+$prepared.digest+'"'
    $matches = [regex]::Matches($config, $pattern).Count
    if ($matches -eq 0) { $config = $config.Replace('[vars]', "[vars]`n$setting") }
    elseif ($matches -eq 1) { $config = [regex]::Replace($config, $pattern, $setting) }
    else { throw 'Expected only one APB_SURFACE_DIGEST setting.' }
    [IO.File]::WriteAllText($configPath, $config, [Text.UTF8Encoding]::new($false))
    if ($Upload) {
        Push-Location $serverRoot
        try {
            $env:WRANGLER_SEND_METRICS = 'false'
            $keyName = if ($Product -eq 'verux.apb.cb') { 'APB_CB_SURFACE_KEY' } else { 'APB_SURFACE_KEY' }
            $encodedKey | npx wrangler secret put $keyName
            if ($LASTEXITCODE -ne 0) { throw 'Catalog key provisioning failed.' }
            npx wrangler d1 execute LICENSE_DB --remote --file (Join-Path $serverRoot 'migrations\0004_surface_catalog.sql')
            if ($LASTEXITCODE -ne 0) { throw 'Catalog schema provisioning failed.' }
            npx wrangler d1 execute LICENSE_DB --remote --file $sqlPath
            if ($LASTEXITCODE -ne 0) { throw 'Encrypted catalog publication failed.' }
        } finally { Pop-Location }
    }
    Write-Output "Encrypted display catalog prepared: $($prepared.count) rows, SHA-256 $($prepared.digest). Worker deployment is a separate release step."
} finally {
    [Array]::Clear($catalogBytes, 0, $catalogBytes.Length)
    if ($keyBytes) { [Array]::Clear($keyBytes, 0, $keyBytes.Length) }
    $labels = $null; $encodedKey = $null; $privateInput = $null
}

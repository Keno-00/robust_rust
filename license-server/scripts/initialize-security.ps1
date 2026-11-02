param([switch]$Upload)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'VeruxSecrets.ps1')
Add-Type -AssemblyName System.Security
$keyDirectory = Join-Path (Get-VeruxSecretsRoot) 'VeruxServerSecurity'
$keyPath = Join-Path $keyDirectory 'verux-20261003.pkcs8.dpapi'
$publicPath = Join-Path $PSScriptRoot '../security/public-keys.json'
if (-not (Test-Path -LiteralPath $keyPath)) {
    if ((Get-Content -LiteralPath $publicPath -Raw).Trim() -ne '{}') {
        throw 'Public pins already exist, but the private key is unavailable. Recover it or follow documented key rotation; do not silently replace the trust root.'
    }
    $generated = @'
const {generateKeyPairSync} = require('node:crypto');
const pair = generateKeyPairSync('ed25519');
const rawPublic = pair.publicKey.export({format:'jwk'}).x;
const privateKey = pair.privateKey.export({format:'der',type:'pkcs8'}).toString('base64url');
process.stdout.write(JSON.stringify({publicKey:rawPublic,privateKey}));
'@ | node
    if ($LASTEXITCODE -ne 0) { throw 'Signing key generation failed.' }
    $pair = $generated | ConvertFrom-Json
    $privateBytes = [Text.Encoding]::UTF8.GetBytes($pair.privateKey)
    $wrapped = [Security.Cryptography.ProtectedData]::Protect($privateBytes, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    New-Item -ItemType Directory -Path $keyDirectory -Force | Out-Null
    [IO.File]::WriteAllBytes($keyPath, $wrapped)
    [Array]::Clear($privateBytes, 0, $privateBytes.Length)
    $publicJson = @{ 'verux-20261003' = $pair.publicKey } | ConvertTo-Json
    [IO.File]::WriteAllText([IO.Path]::GetFullPath($publicPath), $publicJson, [Text.UTF8Encoding]::new($false))
    $pair = $null
    $generated = $null
}
if ($Upload) {
    $wrapped = [IO.File]::ReadAllBytes($keyPath)
    $privateBytes = [Security.Cryptography.ProtectedData]::Unprotect($wrapped, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    try {
        $env:WRANGLER_SEND_METRICS = 'false'
        Push-Location (Join-Path $PSScriptRoot '..')
        try {
            [Text.Encoding]::UTF8.GetString($privateBytes) | npx wrangler secret put SERVER_SIGNING_KEY_PKCS8
            if ($LASTEXITCODE -ne 0) { throw 'Worker secret upload failed.' }
        } finally { Pop-Location }
    } finally { [Array]::Clear($privateBytes, 0, $privateBytes.Length) }
}
Write-Output 'Public signing pin is ready. Private key is protected outside the repository.'

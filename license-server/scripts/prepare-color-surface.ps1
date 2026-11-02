param([switch]$InitializeKey)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'VeruxSecrets.ps1')
Add-Type -AssemblyName System.Security
$surfaceRoot=Join-Path (Get-VeruxSecretsRoot) 'VeruxServerSecurity/surface-labels'
$colorRoot=Join-Path $surfaceRoot 'verux.apb.cb'
[void][IO.Directory]::CreateDirectory($colorRoot)
$source=Join-Path $surfaceRoot 'catalog.eligibility.dpapi'
if (-not (Test-Path -LiteralPath $source)) { throw 'Prepare the Full eligibility catalog before deriving the Color display catalog.' }
Copy-Item -LiteralPath $source -Destination (Join-Path $colorRoot 'catalog.eligibility.dpapi')
$keyPath=Join-Path $colorRoot 'surface-key.dpapi'
if (-not (Test-Path -LiteralPath $keyPath)) {
 if (-not $InitializeKey) { throw 'Recover the existing Color surface key. Use -InitializeKey only for an explicitly authorized first enrollment.' }
 $key=[byte[]]::new(32)
 try { [Security.Cryptography.RandomNumberGenerator]::Fill($key); [IO.File]::WriteAllBytes($keyPath,[Security.Cryptography.ProtectedData]::Protect($key,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)) } finally { [Array]::Clear($key,0,$key.Length) }
}
& (Join-Path $PSScriptRoot 'publish-surface.ps1') -Product verux.apb.cb -Catalog surface.eligibility

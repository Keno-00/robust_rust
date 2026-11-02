[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PreparedDirectory,
    [Parameter(Mandatory)][string]$CredentialsFile,
    [string]$RenewInto,
    [switch]$Library,
    [string]$PrivateKeyFile=(Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Verux/ReleasePublisher/release-ed25519.pem'),
    [ValidatePattern('^[a-f0-9]{32}$')][string]$AccountId='8b35956ff018a988ed0a6c84df4f4807',
    [string]$Bucket='verux-release-artifacts'
)
$ErrorActionPreference='Stop'
$raw=Get-Content -Raw -LiteralPath $CredentialsFile
$access=[regex]::Match($raw,'(?im)Access\s+Key\s+ID\s*[:=]?\s*\r?\n?\s*([a-f0-9]{32})')
$secret=[regex]::Match($raw,'(?im)Secret\s+Access\s+Key\s*[:=]?\s*\r?\n?\s*([a-f0-9]{64})')
if(-not $access.Success -or -not $secret.Success){throw 'Could not identify S3 credentials by their labels.'}
$values=@{R2_ACCESS_KEY_ID=$access.Groups[1].Value;R2_SECRET_ACCESS_KEY=$secret.Groups[1].Value;R2_ACCOUNT_ID=$AccountId;R2_BUCKET=$Bucket;VERUX_RELEASE_PRIVATE_KEY_FILE=(Resolve-Path -LiteralPath $PrivateKeyFile).Path}
$previous=@{}
try {
    foreach($name in $values.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name,'Process');[Environment]::SetEnvironmentVariable($name,$values[$name],'Process')}
    $script=Join-Path $PSScriptRoot 'release.mjs'
    if($Library -and $RenewInto){throw 'Choose library publication or catalog renewal.'}
    if($Library){& node $script publish-library $PreparedDirectory}elseif($RenewInto){& node $script renew $PreparedDirectory $RenewInto}else{& node $script publish $PreparedDirectory}
    if($LASTEXITCODE -ne 0){throw 'Release publication failed; inspect the reported error.'}
} finally {
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name],'Process')}
    $raw=$null;$values=$null;$secret=$null;$access=$null
}

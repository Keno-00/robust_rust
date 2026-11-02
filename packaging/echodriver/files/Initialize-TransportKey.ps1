#Requires -RunAsAdministrator
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$service = Get-Service -Name EchoDriver
if ($service.Status -ne 'Stopped') { throw 'Stop EchoDriver before provisioning its transport key.' }
$path = 'HKLM:\SYSTEM\CurrentControlSet\Services\EchoDriver\Parameters'
if (-not (Test-Path -LiteralPath $path)) { New-Item -Path $path -Force | Out-Null }
# Apply ACL before generating/writing secret material. No inherited user access.
$acl = [System.Security.AccessControl.RegistrySecurity]::new()
$acl.SetAccessRuleProtection($true, $false)
foreach ($sidText in @('S-1-5-18', 'S-1-5-32-544')) {
    $sid = [System.Security.Principal.SecurityIdentifier]::new($sidText)
    $rule = [System.Security.AccessControl.RegistryAccessRule]::new($sid,
        [System.Security.AccessControl.RegistryRights]::FullControl,
        [System.Security.AccessControl.InheritanceFlags]::ContainerInherit,
        [System.Security.AccessControl.PropagationFlags]::None,
        [System.Security.AccessControl.AccessControlType]::Allow)
    $acl.AddAccessRule($rule)
}
Set-Acl -LiteralPath $path -AclObject $acl
$existing = Get-ItemProperty -LiteralPath $path -Name TransportKey -ErrorAction SilentlyContinue
if ($null -ne $existing) {
    if ($existing.TransportKey -isnot [byte[]] -or $existing.TransportKey.Length -ne 32) {
        throw 'Existing TransportKey is invalid. Remove it explicitly while the driver is stopped, then rerun provisioning.'
    }
    Write-Output 'Existing 256-bit transport key retained; SYSTEM/Administrators ACL applied.'
    return
}
$secret = [byte[]]::new(32)
$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
try {
    $rng.GetBytes($secret)
    New-ItemProperty -LiteralPath $path -Name TransportKey -PropertyType Binary -Value $secret -Force | Out-Null
    Write-Output 'Provisioned 256-bit transport key. Load the matching v3 driver. Key material was not logged.'
} finally {
    [Array]::Clear($secret, 0, $secret.Length)
    $rng.Dispose()
}

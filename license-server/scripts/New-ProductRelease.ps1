[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9][a-z0-9.-]{0,63}$')][string]$ProductId,
    [Parameter(Mandatory)][string]$Title,
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [Parameter(Mandatory)][string]$ClientExe,
    [string]$Thumbnail,
    [string]$Banner,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$InstallDirectory,
    [string[]]$Requisites = @(),
    [string]$DriverFolder,
    [string]$WixPath = 'wix.exe',
    [switch]$DesktopShortcut
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'VeruxWorkspace.ps1')
Assert-VeruxCanonicalWorkspace -OutputPath $OutputDirectory
function Xml([string]$Value) { [Security.SecurityElement]::Escape($Value) }
function StableGuid([string]$Value) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $bytes = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes('verux-release-msi:' + $Value)); [byte[]]$guidBytes = $bytes[0..15]; '{' + ([Guid]::new($guidBytes)).ToString().ToUpperInvariant() + '}' }
    finally { $sha.Dispose() }
}
$segments = $InstallDirectory.Split('/')
if ($segments.Count -eq 0 -or @($segments | Where-Object { $_ -notmatch '^[a-zA-Z0-9_-][a-zA-Z0-9_.-]*$' -or $_.EndsWith('.') }).Count -gt 0) { throw 'Use an ordinary relative installation directory under Program Files.' }
$versionParts = $Version.Split('.') | ForEach-Object { [int]$_ }
if ($versionParts.Count -ne 3 -or $versionParts[0] -gt 255 -or $versionParts[1] -gt 255 -or $versionParts[2] -gt 65535) { throw 'Version exceeds MSI limits.' }
$client = (Resolve-Path -LiteralPath $ClientExe).Path
if ([IO.Path]::GetExtension($client) -ine '.exe') { throw 'ClientExe must be an EXE.' }
$thumbnailPath = if ($Thumbnail) { (Resolve-Path -LiteralPath $Thumbnail).Path } else { $null }
$bannerPath = if ($Banner) { (Resolve-Path -LiteralPath $Banner).Path } else { $null }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'OutputDirectory already exists. Use a new release directory.' }
$compilerCmd = Get-Command $WixPath -ErrorAction SilentlyContinue
$defaultWixShim = Join-Path $env:LOCALAPPDATA 'Verux/BuildTools/wix-4.0.6/wix.cmd'
$compiler = if ($compilerCmd) {
    $compilerCmd.Source
} elseif (Test-Path -LiteralPath $defaultWixShim) {
    (Resolve-Path -LiteralPath $defaultWixShim).Path
} else {
    (Get-Command $WixPath -ErrorAction Stop).Source
}
$toolVersion = (& $compiler --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $toolVersion -notmatch '^[4-9]\.') { throw 'A WiX 4+ build compiler is required. It is an authoring tool only; the customer launcher uses Windows Installer.' }
$root = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $root, (Join-Path $root 'payload'), (Join-Path $root 'artwork'), (Join-Path $root 'packages') | Out-Null
$exeName = [IO.Path]::GetFileName($client)
Copy-Item -LiteralPath $client -Destination (Join-Path $root "payload/$exeName")
if ($thumbnailPath) { Copy-Item -LiteralPath $thumbnailPath -Destination (Join-Path $root 'artwork/thumbnail.png') }
if ($bannerPath) { Copy-Item -LiteralPath $bannerPath -Destination (Join-Path $root 'artwork/banner.png') }
$files = [Collections.Generic.List[object]]::new()
$files.Add(@{ path = $exeName; source = "payload/$exeName" })
$components = [Collections.Generic.List[string]]::new()
$refs = [Collections.Generic.List[string]]::new()
$clientGuid = StableGuid "$ProductId/file/$exeName"
$hasDesktopShortcut = $DesktopShortcut.IsPresent -or ($ProductId -eq 'verux.launcher')
$shortcutXml = if ($hasDesktopShortcut) { '<Shortcut Id="ApplicationDesktopShortcut" Directory="DesktopFolder" Name="' + (Xml $Title) + '" WorkingDirectory="INSTALLFOLDER" Advertise="no" />' } else { '' }
$components.Add('<Component Id="Client" Guid="' + $clientGuid + '"><File Id="ClientExe" Source="' + (Xml (Join-Path $root "payload/$exeName")) + '" KeyPath="yes">' + $shortcutXml + '</File></Component>')
$refs.Add('<ComponentRef Id="Client"/>')
$extraDirectories = ''
$driverActions = ''
$driverSequence = ''
$driverProperties = ''
if ($DriverFolder) {
    # This adapter intentionally preserves the repository's selected APB setup.
    # Other driver formats require their own reviewed package adapter, not a guessed command.
    if ($InstallDirectory -cne 'VeruxAPB') { throw 'The existing APB Setup.ps1 owns Program Files/VeruxAPB/Driver. Use that exact installation root.' }
    $driver = (Resolve-Path -LiteralPath $DriverFolder).Path
    foreach ($name in @('EchoDriver.sys','EchoDriver.cer','Setup.ps1','Initialize-TransportKey.ps1')) {
        if (-not (Test-Path -LiteralPath (Join-Path $driver $name) -PathType Leaf)) { throw "Missing real driver payload: $name" }
    }
    $selected = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../packaging/echodriver/files/Setup.ps1')).Path
    if ((Get-FileHash -LiteralPath (Join-Path $driver 'Setup.ps1')).Hash -ne (Get-FileHash -LiteralPath $selected).Hash) { throw 'Driver folder Setup.ps1 differs from the reviewed repository implementation. Review that change explicitly.' }
    $setupText = Get-Content -Raw -LiteralPath $selected
    $expectedNames = @('EchoDriver.sys','EchoDriver.cer','Initialize-TransportKey.ps1')
    $recorded = [regex]::Matches($setupText, "Hash -ne '([A-Fa-f0-9]{64})'")
    if ($recorded.Count -ne 3) { throw 'Cannot read the selected driver integrity contract.' }
    for ($i=0; $i -lt 3; $i++) {
        if ((Get-FileHash -LiteralPath (Join-Path $driver $expectedNames[$i])).Hash -ine $recorded[$i].Groups[1].Value) { throw "Selected driver integrity check failed: $($expectedNames[$i]). Rebuild the matching package; checks were not weakened." }
    }
    New-Item -ItemType Directory -Path (Join-Path $root 'payload/DriverPackage') | Out-Null
    $payloadComponents = ''
    $index = 0
    foreach ($name in @('EchoDriver.sys','EchoDriver.cer','Setup.ps1','Initialize-TransportKey.ps1')) {
        Copy-Item -LiteralPath (Join-Path $driver $name) -Destination (Join-Path $root "payload/DriverPackage/$name")
        $fileId = if ($name -eq 'Setup.ps1') { 'DriverSetupFile' } else { "DriverPayloadFile$index" }
        $componentId = "DriverPayload$index"
        $payloadComponents += '<Component Id="' + $componentId + '" Guid="' + (StableGuid "$ProductId/file/DriverPackage/$name") + '"><File Id="' + $fileId + '" Source="' + (Xml (Join-Path $root "payload/DriverPackage/$name")) + '" KeyPath="yes"/></Component>'
        $refs.Add('<ComponentRef Id="' + $componentId + '"/>')
        $files.Add(@{path="DriverPackage/$name";source="payload/DriverPackage/$name"})
        $index++
    }
    $driverComponent = '<Component Id="DriverInstalled" Guid="' + (StableGuid "$ProductId/file/Driver/EchoDriver.sys") + '"><File Id="DriverInstalledFile" Source="' + (Xml (Join-Path $root 'payload/DriverPackage/EchoDriver.sys')) + '" KeyPath="yes"/><ServiceControl Id="DriverControl" Name="EchoDriver" Stop="both" Wait="yes"/></Component>'
    $extraDirectories = '<Directory Id="DRIVERPACKAGE" Name="DriverPackage">' + $payloadComponents + '</Directory><Directory Id="DRIVER" Name="Driver">' + $driverComponent + '</Directory>'
    $refs.Add('<ComponentRef Id="DriverInstalled"/>')
    $files.Add(@{path='Driver/EchoDriver.sys';source='payload/DriverPackage/EchoDriver.sys'})
    $driverProperties = '<Property Id="EXISTINGDRIVERPATH"><RegistrySearch Id="DriverOwner" Root="HKLM" Key="SYSTEM\CurrentControlSet\Services\EchoDriver" Name="ImagePath" Type="raw" Bitness="always64"/></Property><CustomAction Id="ExpectedDriver" Property="ExpectedDriverPath" Value="[INSTALLFOLDER]Driver\EchoDriver.sys"/><CustomAction Id="QuotedDriver" Property="QuotedDriverPath" Value="&quot;[INSTALLFOLDER]Driver\EchoDriver.sys&quot;"/><CustomAction Id="RawExpectedDriver" Property="RawExpectedDriverPath" Value="#%[INSTALLFOLDER]Driver\EchoDriver.sys"/><CustomAction Id="RawQuotedDriver" Property="RawQuotedDriverPath" Value="#%&quot;[INSTALLFOLDER]Driver\EchoDriver.sys&quot;"/><Launch Condition="NOT EXISTINGDRIVERPATH OR EXISTINGDRIVERPATH ~= ExpectedDriverPath OR EXISTINGDRIVERPATH ~= QuotedDriverPath OR EXISTINGDRIVERPATH ~= RawExpectedDriverPath OR EXISTINGDRIVERPATH ~= RawQuotedDriverPath" Message="EchoDriver belongs to another installation. Remove that installation explicitly before continuing."/>'
    $driverProperties='<Property Id="VERUX_REPLACE_EXISTING" Value="0" Secure="yes"/><Property Id="VERUX_REPLACE_WAS_RUNNING" Value="0" Secure="yes"/>'+$driverProperties.Replace('NOT EXISTINGDRIVERPATH OR','VERUX_REPLACE_EXISTING = &quot;1&quot; OR NOT EXISTINGDRIVERPATH OR')
    $driverProperties=$driverProperties.Replace('OR EXISTINGDRIVERPATH ~= ExpectedDriverPath','OR EXISTINGDRIVERPATH ~= NtExpectedDriverPath OR EXISTINGDRIVERPATH ~= RawNtExpectedDriverPath OR EXISTINGDRIVERPATH ~= ExpectedDriverPath')
    $driverProperties+='<CustomAction Id="NtExpectedDriver" Property="NtExpectedDriverPath" Value="\??\[INSTALLFOLDER]Driver\EchoDriver.sys"/><CustomAction Id="RawNtExpectedDriver" Property="RawNtExpectedDriverPath" Value="#%\??\[INSTALLFOLDER]Driver\EchoDriver.sys"/>'
    $thumbprint=[regex]::Match($setupText,'\$thumbprint\s*=\s*''([A-Fa-f0-9]{40})''')
    if (-not $thumbprint.Success) {throw 'Cannot read the selected driver certificate identity.'}
    $contract=@{service='EchoDriver';destination='VeruxAPB\Driver\EchoDriver.sys';driver_sha256=$recorded[0].Groups[1].Value;certificate_sha256=$recorded[1].Groups[1].Value;key_script_sha256=$recorded[2].Groups[1].Value;certificate_thumbprint=$thumbprint.Groups[1].Value}
    $contractPath=Join-Path $root 'payload/DriverPackage/driver.json'
    [IO.File]::WriteAllText($contractPath,($contract|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    $native=Join-Path $root 'payload/DriverPackage/DriverInstaller.exe'
    $csc=Join-Path ([Environment]::GetFolderPath('Windows')) 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    & $csc /nologo /target:exe /platform:x64 /optimize+ /r:System.ServiceProcess.dll /r:System.Web.Extensions.dll /r:System.Security.dll "/out:$native" (Join-Path $PSScriptRoot '../../packaging/echodriver/DriverInstaller.cs')
    if ($LASTEXITCODE -ne 0) {throw 'Compiled driver helper build failed.'}
    foreach($name in @('DriverInstaller.exe','driver.json')) {
        $id=if($name -eq 'DriverInstaller.exe'){'DriverNativeFile'}else{'DriverContractFile'}
        $component='<Component Id="'+$id+'" Guid="'+(StableGuid "$ProductId/file/DriverPackage/$name")+'"><File Id="'+$id+'" Source="'+(Xml (Join-Path $root "payload/DriverPackage/$name"))+'" KeyPath="yes"/></Component>'
        $extraDirectories=$extraDirectories.Replace('</Directory><Directory Id="DRIVER"',$component+'</Directory><Directory Id="DRIVER"')
        $refs.Add('<ComponentRef Id="'+$id+'"/>')
        $files.Add(@{path="DriverPackage/$name";source="payload/DriverPackage/$name"})
    }
    $driverActions='<Binary Id="DriverNativeBinary" SourceFile="'+(Xml $native)+'"/>'
    foreach($action in @(@('InstallDriver','Install','deferred'),@('UninstallDriver','Uninstall','deferred'),@('RollbackNewDriver','RemoveNew','rollback'),@('RestoreUninstalledDriver','RestoreUninstall','rollback'),@('RollbackMigration','Rollback','rollback'),@('CommitMigration','Commit','commit'),@('CommitUninstall','CommitUninstall','commit'))) {
        $arguments=$action[1]+' &quot;[DRIVERPACKAGE].&quot; [VERUX_REPLACE_EXISTING] [VERUX_REPLACE_WAS_RUNNING]'
        $driverActions+='<CustomAction Id="'+$action[0]+'" BinaryRef="DriverNativeBinary" ExeCommand="'+$arguments+'" Execute="'+$action[2]+'" Impersonate="no" Return="check"/>'
    }
    $driverSequence='<Custom Action="ExpectedDriver" After="CostFinalize"/><Custom Action="QuotedDriver" After="ExpectedDriver"/><Custom Action="RawExpectedDriver" After="QuotedDriver"/><Custom Action="RawQuotedDriver" After="RawExpectedDriver"/><LaunchConditions After="RawQuotedDriver"/><Custom Action="RestoreUninstalledDriver" After="StopServices" Condition="REMOVE~=&quot;ALL&quot; AND NOT UPGRADINGPRODUCTCODE AND EXISTINGDRIVERPATH"/><Custom Action="UninstallDriver" After="RestoreUninstalledDriver" Condition="REMOVE~=&quot;ALL&quot; AND NOT UPGRADINGPRODUCTCODE"/><Custom Action="CommitUninstall" After="UninstallDriver" Condition="REMOVE~=&quot;ALL&quot; AND NOT UPGRADINGPRODUCTCODE"/><Custom Action="RollbackNewDriver" After="InstallFiles" Condition="NOT REMOVE~=&quot;ALL&quot; AND NOT EXISTINGDRIVERPATH"/><Custom Action="RollbackMigration" After="InstallServices" Condition="NOT REMOVE~=&quot;ALL&quot;"/><Custom Action="InstallDriver" After="RollbackMigration" Condition="NOT REMOVE~=&quot;ALL&quot;"/><Custom Action="CommitMigration" After="InstallDriver" Condition="NOT REMOVE~=&quot;ALL&quot;"/>'
    $driverSequence=$driverSequence.Replace('<LaunchConditions After="RawQuotedDriver"/>','<Custom Action="NtExpectedDriver" After="RawQuotedDriver"/><Custom Action="RawNtExpectedDriver" After="NtExpectedDriver"/><LaunchConditions After="RawNtExpectedDriver"/>')
}
$inner = '<Directory Id="INSTALLFOLDER" Name="' + (Xml $segments[-1]) + '">' + ($components -join '') + $extraDirectories + '</Directory>'
for ($i=$segments.Count-2; $i -ge 0; $i--) { $inner = '<Directory Id="Parent' + $i + '" Name="' + (Xml $segments[$i]) + '">' + $inner + '</Directory>' }
$productCode = StableGuid "$ProductId/version/$Version"
$upgradeCode = StableGuid "$ProductId/upgrade"
$desktopFolder = if ($hasDesktopShortcut) { '<StandardDirectory Id="DesktopFolder"/>' } else { '' }
$xml = '<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"><Package Name="' + (Xml $Title) + '" Manufacturer="Verux" Version="' + $Version + '" ProductCode="' + $productCode + '" UpgradeCode="' + $upgradeCode + '" Scope="perMachine" InstallerVersion="500"><MajorUpgrade Schedule="afterInstallInitialize" DowngradeErrorMessage="A newer release is installed."/><MediaTemplate EmbedCab="yes"/><Property Id="ARPNOMODIFY" Value="1"/><CustomAction Id="PublishInstallLocation" Property="ARPINSTALLLOCATION" Value="[INSTALLFOLDER]"/><StandardDirectory Id="ProgramFiles64Folder">' + $inner + '</StandardDirectory>' + $desktopFolder + '<Feature Id="Product" Title="' + (Xml $Title) + '" Level="1">' + ($refs -join '') + '</Feature>' + $driverProperties + $driverActions + '<InstallExecuteSequence><Custom Action="PublishInstallLocation" After="CostFinalize"/>' + $driverSequence + '</InstallExecuteSequence></Package></Wix>'
$wxs = Join-Path $root 'package.wxs'
[IO.File]::WriteAllText($wxs,$xml,[Text.UTF8Encoding]::new($false))
$packageRelative = "packages/$ProductId-$Version.msi"
& $compiler build $wxs -arch x64 -o (Join-Path $root $packageRelative)
if ($LASTEXITCODE -ne 0) { throw 'MSI build failed. Output preserved for diagnosis.' }
$product = @{
    id=$ProductId; title=$Title; requisites=@($Requisites); thumbnail=$null; banner=$null
    release=@{version=$Version;package=$packageRelative;product_code=$productCode;upgrade_code=$upgradeCode;directory=$InstallDirectory;executable=$exeName;files=@($files);min_windows_build=17763;architecture='x64'}
}
if ($thumbnailPath) { $product.thumbnail = 'artwork/thumbnail.png' }
if ($bannerPath) { $product.banner = 'artwork/banner.png' }
if ($DriverFolder) { $product.replacement_service = 'EchoDriver'; $product.service_file = 'Driver/EchoDriver.sys' }
[IO.File]::WriteAllText((Join-Path $root 'product.source.json'),($product|ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
Write-Output "Prepared $ProductId $Version. Add this product source record to your catalog, preserving its relative files under the catalog folder. Qualify installation/rollback before publishing."






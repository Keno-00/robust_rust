param([switch]$Upload)
$ErrorActionPreference='Stop'
$serverRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$configPath=Join-Path $serverRoot 'wrangler.toml'
$config=[IO.File]::ReadAllText($configPath)
$privateRoot=Join-Path $env:LOCALAPPDATA 'VeruxServerSecurity/color-artifacts'
[void][IO.Directory]::CreateDirectory($privateRoot)
$artifacts=@()
foreach ($name in @('smoothing')) {
 $variable='APB_'+$name.ToUpperInvariant()+'_DIGEST'
 $matches=[regex]::Matches($config,'(?m)^'+$variable+' = "([a-f0-9]{64})"')
 if ($matches.Count -ne 1) { throw "Expected one $variable." }
 $digest=$matches[0].Groups[1].Value
 $artifacts+=@{Name=$name;Digest=$digest;Relative="$name/abi1/$digest.wasm";Variable='APB_CB_'+$name.ToUpperInvariant()+'_DIGEST';File=(Join-Path $privateRoot "$name.wasm")}
}
$matches=[regex]::Matches($config,"(?m)^LEONARDO_FIRMWARE_TEMPLATE = '(.+)'\r?$")
if ($matches.Count -ne 1) { throw 'Expected original firmware template.' }
$template=$matches[0].Groups[1].Value | ConvertFrom-Json
$artifacts+=@{Name='firmware';Digest=$template.digest;Relative="firmware/leonardo/template/$($template.version)/$($template.digest).hex";File=(Join-Path $privateRoot 'leonardo.hex')}
Push-Location $serverRoot
try {
 foreach ($artifact in $artifacts) {
  & npx --no-install wrangler r2 object get "verux-algorithm-artifacts/verux.apb/$($artifact.Relative)" --file $artifact.File --remote
  if ($LASTEXITCODE -ne 0) { throw "Download failed: $($artifact.Name)." }
  if ((Get-FileHash -LiteralPath $artifact.File -Algorithm SHA256).Hash.ToLowerInvariant() -ne $artifact.Digest) { throw "Artifact integrity failed: $($artifact.Name)." }
 }
 if ($Upload) {
  foreach ($artifact in $artifacts) {
   & npx --no-install wrangler r2 object put "verux-algorithm-artifacts/verux.apb.cb/$($artifact.Relative)" --file $artifact.File --remote
   if ($LASTEXITCODE -ne 0) { throw "Color upload failed: $($artifact.Name). Do not deploy Color enrollment until publication completes." }
  }
  $config=[IO.File]::ReadAllText($configPath)
  foreach ($artifact in $artifacts | Where-Object {$_.Variable}) {
   $pattern='(?m)^'+$artifact.Variable+' = "[a-f0-9]*"'
   $setting=$artifact.Variable+' = "'+$artifact.Digest+'"'
   if ([regex]::IsMatch($config,$pattern)) { $config=[regex]::Replace($config,$pattern,$setting) } else { $config=$config.Replace('[vars]',"[vars]`n$setting") }
  }
  [IO.File]::WriteAllText($configPath,$config,[Text.UTF8Encoding]::new($false))
 }
} finally { Pop-Location }
Write-Output "Color artifacts verified; uploaded=$Upload. Private staging files remain under LocalAppData."

param(
    [switch]$Upload,
    [ValidateSet('smoothing','dataset','training','inference')][string]$Algorithm='smoothing'
)
$ErrorActionPreference='Stop'
$serverRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifest=Join-Path $serverRoot "algorithms/apb-$Algorithm/Cargo.toml"
& cargo build --locked --manifest-path $manifest --target wasm32-unknown-unknown --release
if ($LASTEXITCODE -ne 0) {throw 'Algorithm compilation failed. Install the wasm32-unknown-unknown Rust target and retry.'}
$artifact=Join-Path $serverRoot "algorithms/apb-$Algorithm/target/wasm32-unknown-unknown/release/verux_apb_$Algorithm.wasm"
$size=(Get-Item -LiteralPath $artifact).Length
$maximum=if ($Algorithm -in @('training','inference')) {1048576} else {131072}
if ($size -le 0 -or $size -gt $maximum) {throw "Algorithm artifact exceeds its $maximum byte limit."}
$digest=(Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$configPath=Join-Path $serverRoot 'wrangler.toml'
$config=[IO.File]::ReadAllText($configPath)
$variable='APB_'+$Algorithm.ToUpperInvariant()+'_DIGEST'
$pattern='(?m)^'+$variable+' = "[a-f0-9]*"'
if ([regex]::Matches($config,$pattern).Count -ne 1) {throw "Expected exactly one $variable setting."}
$config=[regex]::Replace($config,$pattern,($variable+' = "'+$digest+'"'))
[IO.File]::WriteAllText($configPath,$config,[Text.UTF8Encoding]::new($false))
if ($Upload) {
    Push-Location $serverRoot
    try {
        & npx wrangler r2 object put "verux-algorithm-artifacts/verux.apb/$Algorithm/abi1/$digest.wasm" --file $artifact --remote --content-type application/wasm
        if ($LASTEXITCODE -ne 0) {throw 'Private artifact upload failed. Do not deploy this digest until upload succeeds.'}
    } finally {Pop-Location}
}
Write-Output "Algorithm $Algorithm prepared: $digest ($size bytes). Worker deployment is a separate release step."

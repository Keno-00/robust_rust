param([switch]$Upload)
$ErrorActionPreference = 'Stop'
$serverRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$prepared = @()
foreach ($algorithm in @('dataset', 'training', 'inference')) {
    $crateRoot = Join-Path $serverRoot "algorithms/apb-$algorithm"
    $targetRoot = Join-Path $crateRoot 'target/color-only'
    & cargo build --locked --manifest-path (Join-Path $crateRoot 'Cargo.toml') --target wasm32-unknown-unknown --release --features color-only --target-dir $targetRoot
    if ($LASTEXITCODE -ne 0) { throw "Color $algorithm compilation failed. No digest was changed." }
    $artifact = Join-Path $targetRoot "wasm32-unknown-unknown/release/verux_apb_$algorithm.wasm"
    $size = (Get-Item -LiteralPath $artifact).Length
    $maximum = if ($algorithm -eq 'dataset') { 131072 } else { 1048576 }
    if ($size -le 0 -or $size -gt $maximum) { throw "Color $algorithm exceeds the artifact size limit." }
    $digest = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
    $prepared += @{ Name = $algorithm; Artifact = $artifact; Digest = $digest; Size = $size }
}
if ($Upload) {
    Push-Location $serverRoot
    try {
        foreach ($item in $prepared) {
            & npx --no-install wrangler r2 object put "verux-algorithm-artifacts/verux.apb.cb/$($item.Name)/abi1/$($item.Digest).wasm" --file $item.Artifact --remote --content-type application/wasm
            if ($LASTEXITCODE -ne 0) { throw "Color $($item.Name) upload failed. No digest was changed." }
        }
        $configPath = Join-Path $serverRoot 'wrangler.toml'
        $config = [IO.File]::ReadAllText($configPath)
        foreach ($item in $prepared) {
            $variable = 'APB_CB_' + $item.Name.ToUpperInvariant() + '_DIGEST'
            $pattern = '(?m)^' + $variable + ' = "[a-f0-9]*"'
            if ([regex]::Matches($config, $pattern).Count -ne 1) { throw "Expected exactly one $variable setting." }
            $config = [regex]::Replace($config, $pattern, ($variable + ' = "' + $item.Digest + '"'))
        }
        [IO.File]::WriteAllText($configPath, $config, [Text.UTF8Encoding]::new($false))
    }
    finally { Pop-Location }
}
foreach ($item in $prepared) { Write-Output "Color $($item.Name): $($item.Digest) ($($item.Size) bytes); uploaded=$Upload" }
Write-Output 'Worker deployment is a separate release step. Full product digests and artifacts were not changed.'

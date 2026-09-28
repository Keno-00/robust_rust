$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$probe = Join-Path $root 'target\debug\examples\apb_memory_probe.exe'
$outputDir = [System.IO.Path]::GetFullPath((Join-Path $root '..\drayebr\build-rpm\snapshots'))
if (-not (Test-Path -LiteralPath $probe -PathType Leaf)) {
    throw "Build the read-only probe first: cargo build --offline --example apb_memory_probe"
}
if (-not (Test-Path -LiteralPath $outputDir -PathType Container)) {
    throw "EchoDriver snapshot directory is missing: $outputDir"
}
$status = Join-Path $outputDir 'apb-native-probe-status.txt'
$ErrorActionPreference = 'Continue'
$result = & $probe 2>&1
$code = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
$result | Out-String | Set-Content -LiteralPath $status
exit $code

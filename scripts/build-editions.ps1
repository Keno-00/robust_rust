param(
    [ValidateSet('Both', 'Color', 'ColorMemory')]
    [string]$Edition = 'Both'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    $selected = if ($Edition -eq 'Both') { @('Color', 'ColorMemory') } else { @($Edition) }
    foreach ($item in $selected) {
        $slug = if ($item -eq 'Color') { 'color' } else { 'color-memory' }
        $target = Join-Path $repo "target/editions/$slug"
        $cargoArgs = @('build', '--locked', '--release', '--bin', 'verux_apb', '--no-default-features', '--target-dir', $target)
        if ($item -eq 'ColorMemory') { $cargoArgs += @('--features', 'memory') }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw "Build failed for $item (exit $LASTEXITCODE). No release was published." }
        $executable = Join-Path $target 'release/verux_apb.exe'
        $hash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
        Write-Output "$item : $executable"
        Write-Output "SHA256 : $hash"
    }
}
finally { Pop-Location }

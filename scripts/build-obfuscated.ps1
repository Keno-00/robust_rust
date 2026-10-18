param(
    [string]$TargetDirectory = (Join-Path $PSScriptRoot '../target/obfuscated')
)
$ErrorActionPreference = 'Stop'
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$target = [IO.Path]::GetFullPath($TargetDirectory)
$previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
$previousCppFlags = $env:CXXFLAGS_x86_64_pc_windows_msvc
$previousShellEscaping = $env:CC_SHELL_ESCAPED_FLAGS
$previousDirectory = Get-Location
try {
    Set-Location -LiteralPath $repository
    $separator = [string][char]31
    $flags = @()
    if ($previousFlags) {
        $flags += $previousFlags.Split([char]31)
    } elseif ($env:RUSTFLAGS) {
        throw 'Use CARGO_ENCODED_RUSTFLAGS instead of RUSTFLAGS for this build so argument boundaries are preserved.'
    }
    $cargoRoot = if ($env:CARGO_HOME) { [IO.Path]::GetFullPath($env:CARGO_HOME) } else { Join-Path $env:USERPROFILE '.cargo' }
    $sysroot = & rustc --print sysroot
    if ($LASTEXITCODE -ne 0) { throw 'Cannot locate the Rust toolchain.' }
    $flags += "--remap-path-prefix=$repository=/verux"
    $flags += "--remap-path-prefix=$cargoRoot=/cargo"
    $flags += "--remap-path-prefix=$sysroot=/rust"
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join $separator
    # Rust remapping does not reach __FILE__ in ImGui's native C++ assertions.
    # Preserve those assertions while remapping their diagnostic paths in MSVC.
    $cppMappings = @(
        '/experimental:deterministic',
        "`"/pathmap:$repository=/verux`"",
        "`"/pathmap:$cargoRoot=/cargo`"",
        "`"/pathmap:$sysroot=/rust`""
    ) -join ' '
    $env:CXXFLAGS_x86_64_pc_windows_msvc = "$previousCppFlags $cppMappings".Trim()
    $env:CC_SHELL_ESCAPED_FLAGS = '1'
    & cargo build --locked --release --bin verux_apb --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'Obfuscated release build failed.' }
    $executable = Join-Path $target 'release/verux_apb.exe'
    if (-not (Test-Path -LiteralPath $executable)) { throw 'Release executable is missing.' }
    Write-Output "Executable: $executable"
    Get-FileHash -LiteralPath $executable -Algorithm SHA256
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags
    $env:CXXFLAGS_x86_64_pc_windows_msvc = $previousCppFlags
    $env:CC_SHELL_ESCAPED_FLAGS = $previousShellEscaping
    Set-Location -LiteralPath $previousDirectory.Path
}

param(
    [Parameter(Mandatory = $true)][string]$ActionContext,
    [Parameter(Mandatory = $true)][string]$SocialContext,
    [Parameter(Mandatory = $true)][string]$PageBase,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$action = [System.IO.Path]::GetFullPath($ActionContext)
$social = [System.IO.Path]::GetFullPath($SocialContext)
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if (-not (Test-Path -LiteralPath $action -PathType Leaf)) { throw "Action context missing: $action" }
if (-not (Test-Path -LiteralPath $social -PathType Leaf)) { throw "Social context missing: $social" }
if (-not (Test-Path -LiteralPath (Split-Path -Parent $directory) -PathType Container)) {
    throw "Output parent directory does not exist: $(Split-Path -Parent $directory)"
}
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $log)) {
    throw "Output directory or log already exists: $directory"
}
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) { throw "Expected one APB.exe process; found $($game.Count)." }
Set-Location -LiteralPath $root
try {
    & python -u -B (Join-Path $PSScriptRoot 'apb_camera_container_refs_f8.py') $game[0].Id $action $social $PageBase $directory 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "Camera-container reference scan failed with exit $result. Inspect $log" }
    Write-Host "Camera-container reference scan complete. Log: $log"
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

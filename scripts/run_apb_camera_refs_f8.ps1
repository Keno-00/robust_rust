param(
    [Parameter(Mandatory = $true)][string]$CandidateCapture,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [string]$Address
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = [System.IO.Path]::GetFullPath($CandidateCapture)
$directory = [System.IO.Path]::GetFullPath($OutputDirectory)
$log = "$directory.log"
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Candidate capture is missing: $source" }
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
    $pythonArguments = @($game[0].Id, $source, $directory)
    if ($Address) { $pythonArguments += $Address }
    & python -u -B (Join-Path $PSScriptRoot 'apb_camera_refs_f8.py') @pythonArguments 2>&1 | Tee-Object -FilePath $log
    $result = $LASTEXITCODE
    "Python exit code: $result" | Tee-Object -FilePath $log -Append
    if ($result -ne 0) { throw "Camera reference scan failed with exit $result. Inspect $log" }
    Write-Host "Camera reference scan complete. Log: $log"
} catch {
    "Terminal error: $_" | Tee-Object -FilePath $log -Append
    throw
}

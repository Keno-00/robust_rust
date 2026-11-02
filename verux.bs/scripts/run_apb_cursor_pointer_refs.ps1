param(
    [Parameter(Mandatory = $true)][string]$SnapshotDirectory,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$Output,
    [string]$Radius = '0x1000'
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = [System.IO.Path]::GetFullPath($SnapshotDirectory)
$destination = [System.IO.Path]::GetFullPath($Output)
$log = "$destination.log"
if ((Test-Path -LiteralPath $destination) -or (Test-Path -LiteralPath $log)) {
    throw "Output report or log already exists: $destination"
}
if (-not (Test-Path -LiteralPath (Join-Path $source 'white1.pages') -PathType Leaf)) {
    throw "Cursor baseline page snapshot is missing: $source"
}
Set-Location -LiteralPath $root
& python -u -B (Join-Path $PSScriptRoot 'apb_cursor_pointer_refs.py') $source $Target $destination --radius $Radius 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Cursor pointer scan failed with exit $LASTEXITCODE. Inspect $log" }
Write-Host "Cursor pointer references saved. Log: $log"

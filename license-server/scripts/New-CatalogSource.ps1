[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Brand,
    [string[]]$ProductSource = @(),
    [string]$LauncherSource,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][ValidateRange(1,9007199254740991)][long]$Revision,
    [ValidateRange(1,604800)][int]$TtlSeconds = 86400
)
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $OutputDirectory) {throw 'Use a new output directory.'}
$root=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $root,(Join-Path $root '_shell') | Out-Null
Copy-Item -LiteralPath (Resolve-Path -LiteralPath $Brand).Path -Destination (Join-Path $root '_shell/banner.png')
$ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
function Import-Product([string]$Path) {
    $full=(Resolve-Path -LiteralPath $Path).Path
    $sourceRoot=Split-Path -Parent $full
    $product=Get-Content -Raw -LiteralPath $full | ConvertFrom-Json
    if ($product.id -cnotmatch '^[a-z0-9][a-z0-9.-]{0,63}$' -or -not $ids.Add($product.id)) {throw 'Invalid or duplicate product ID.'}
    if (-not $product.thumbnail -or -not $product.banner) {throw "Supply approved thumbnail and banner paths in $full before creating a catalog."}
    function Copy-Source([string]$Relative) {
        if (-not $Relative -or $Relative -match '[\:]' -or @($Relative.Split('/') | Where-Object {$_ -in @('','.', '..')}).Count) {throw "Unsafe source path: $Relative"}
        $source=[IO.Path]::GetFullPath((Join-Path $sourceRoot $Relative))
        if (-not $source.StartsWith($sourceRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'Source escaped product folder.'}
        $cursor=Get-Item -LiteralPath $source
        while ($cursor -and $cursor.FullName.StartsWith($sourceRoot,[StringComparison]::OrdinalIgnoreCase)) {
            if ($cursor.Attributes -band [IO.FileAttributes]::ReparsePoint) {throw 'Reparse source paths are not allowed.'}
            if ($cursor -is [IO.FileInfo]) { $cursor=$cursor.Directory } else { $cursor=$cursor.Parent }
        }
        $targetRelative=$product.id+'/'+$Relative
        $target=Join-Path $root $targetRelative
        New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
        if (-not (Test-Path -LiteralPath $target)) {Copy-Item -LiteralPath $source -Destination $target}
        return $targetRelative
    }
    $product.thumbnail=Copy-Source $product.thumbnail
    $product.banner=Copy-Source $product.banner
    $product.release.package=Copy-Source $product.release.package
    foreach ($file in $product.release.files) {$file.source=Copy-Source $file.source}
    return $product
}
$products=@(foreach ($path in $ProductSource) {Import-Product $path})
$launcher=if ($LauncherSource) {Import-Product $LauncherSource} else {$null}
$catalog=@{revision=$Revision;ttl_seconds=$TtlSeconds;banner='_shell/banner.png';products=$products;launcher=$launcher}
[IO.File]::WriteAllText((Join-Path $root 'catalog.source.json'),($catalog|ConvertTo-Json -Depth 12),[Text.UTF8Encoding]::new($false))
Write-Output (Join-Path $root 'catalog.source.json')


# Packaging and publication run only from the canonical checkout, and release
# output stays in its target folder; see AGENTS.md "Workspace and sandbox rule".
function Assert-VeruxCanonicalWorkspace([string[]]$OutputPath = @()) {
    $root = if ($env:VERUX_SOURCE_ROOT) { $env:VERUX_SOURCE_ROOT } else { 'C:\Users\Admin\Documents\source_repos' }
    $repo = [IO.Path]::GetFullPath((Join-Path $root 'robust_rust')).TrimEnd('\')
    $here = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..')).TrimEnd('\')
    if ($here -ine $repo) {
        throw "Workspace rule: run from $repo, not $here. Move the work into the main repo instead of using a copy or sandbox."
    }
    if (-not (Test-Path -LiteralPath (Join-Path $repo '.git') -PathType Container)) {
        throw "Workspace rule: $repo is a git worktree or has no repository; use the main checkout."
    }
    $target = (Join-Path $repo 'target') + '\'
    foreach ($path in $OutputPath) {
        $full = [IO.Path]::GetFullPath($path)
        if (-not $full.StartsWith($target, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Workspace rule: release output $full must be under $target"
        }
    }
}

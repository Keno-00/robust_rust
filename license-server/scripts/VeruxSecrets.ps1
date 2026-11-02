# Single location for operator secrets (release signing key, server signing key,
# surface keys, firmware backups). See docs/OPERATOR_SECRETS.md.
#
# Default: Desktop\deploy\Verux Secrets. Override with VERUX_SECRETS_ROOT.
# %LOCALAPPDATA% is deliberately not used: packaged apps (for example the Codex
# desktop app) silently redirect it into a private sandbox, which hid the
# release key outside Codex and would delete it when the app is uninstalled.
function Get-VeruxSecretsRoot {
    $root = if ($env:VERUX_SECRETS_ROOT) { $env:VERUX_SECRETS_ROOT }
        else { Join-Path ([Environment]::GetFolderPath('Desktop')) 'deploy\Verux Secrets' }
    $root = [IO.Path]::GetFullPath($root)
    if ($root -match '\\AppData\\Local\\Packages\\') {
        throw "Verux secrets root is inside an app sandbox: $root. Use a visible folder and update docs/OPERATOR_SECRETS.md."
    }
    if (-not (Test-Path -LiteralPath $root -PathType Container)) {
        throw "Verux secrets folder not found: $root. Restore it from backup or set VERUX_SECRETS_ROOT (see docs/OPERATOR_SECRETS.md)."
    }
    $root
}

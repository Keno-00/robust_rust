param(
    [Parameter(Mandatory = $true)][string]$Leads,
    [string]$PointerRva,
    [string]$FieldOffset,
    [string]$LogPath,
    [switch]$AllowNewSession
)

$ErrorActionPreference = 'Stop'
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$leadsPath = [System.IO.Path]::GetFullPath($Leads)
$log = if ($LogPath) { [System.IO.Path]::GetFullPath($LogPath) } elseif ($AllowNewSession) { [System.IO.Path]::ChangeExtension($leadsPath, '.restart.live.log') } elseif ($PointerRva) { [System.IO.Path]::ChangeExtension($leadsPath, '.pointer.live.log') } else { [System.IO.Path]::ChangeExtension($leadsPath, '.live.log') }
$game = @(Get-Process -Name APB -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'APB' })
if ($game.Count -ne 1) {
    throw "Expected one APB.exe process; found $($game.Count)."
}
Set-Location -LiteralPath $root
$arguments = @('-u', '-B', (Join-Path $PSScriptRoot 'apb_cursor_live.py'), $game[0].Id, $leadsPath)
if ($PointerRva -or $FieldOffset) {
    if (-not $PointerRva -or -not $FieldOffset) { throw 'Both pointer RVA and field offset are required.' }
    $arguments += @('--pointer-rva', $PointerRva, '--field-offset', $FieldOffset)
}
if ($AllowNewSession) { $arguments += '--allow-new-session' }
& python @arguments 2>&1 | Tee-Object -FilePath $log
Write-Host "Live cursor reader exited. Log: $log"

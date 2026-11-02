@echo off
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -File "%~dp0files\Launch.ps1" -Action Uninstall
if errorlevel 1 echo Uninstallation failed. Review the error above.
pause


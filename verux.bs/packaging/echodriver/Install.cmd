@echo off
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -File "%~dp0files\Launch.ps1" -Action Install
if errorlevel 1 echo Installation failed. Review the error above.
pause


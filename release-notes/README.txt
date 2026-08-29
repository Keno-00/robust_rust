VERUX APB 1.5
=============

Included files
--------------
- verux_apb.exe: Verux APB Version 1.5
- User Guide and Setup.pdf: installation and Arduino instructions
- Manual And Features.pdf: interface and feature reference
- Requirements\setup.ino: Arduino Leonardo sketch
- CHECKSUMS.txt: SHA-256 integrity values for this package

Requirements
------------
- Windows 10 or Windows 11, 64-bit
- Administrator permission when starting the application
- An active Verux APB license
- A stable internet connection while the application is running
- Arduino Leonardo and USB cable
- Arduino IDE when initially flashing setup.ino

Quick start
-----------
1. Flash Requirements\setup.ino to the Arduino Leonardo using Arduino IDE.
2. Connect the Arduino Leonardo.
3. Start verux_apb.exe and approve the administrator prompt.
4. Select Login, enter the license key, and click Submit.
5. Open Misc, click Refresh COM ports, and select the Arduino port.
6. Configure the application and click Save.

Troubleshooting
---------------
- The application requires an internet connection for activation and periodic verification.
- If the Arduino is unavailable, close other software using its COM port, reconnect it, and refresh the port list.
- Diagnostic logs are stored at Documents\VeruxAPB\verux_apb.log.
- Logs rotate automatically and exclude known sensitive fields.
- Windows may display a SmartScreen warning because Version 1.5 is not yet digitally signed.

Privacy
-------
The licensing service processes the supplied license key and limited installation/session information required to validate access, enforce the license policy, measure licensed runtime, and investigate service abuse. License and installation identifiers are stored by the service as keyed hashes. Do not share your license key or diagnostic files publicly.

Support
-------
Official Verux Discord: https://discord.com/invite/DJ4THpewg5

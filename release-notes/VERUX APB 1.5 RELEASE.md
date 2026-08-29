# **VERUX APB**

VERSION 1.5 RELEASED TO PUBLIC

- Added a new online license system:

> Licenses are now verified through the Verux service and require an internet connection while the software is running.

- Added live license information under **Misc**:

> View your license status, product, variation, used time, and remaining time directly inside the application.

- Added automatic session verification:

> The application now keeps the active session synchronized while it is running and safely stops when a license expires or the connection can no longer be verified.

- Improved **Arduino Leonardo** support:

> COM-port selection, port refresh, and adjustable mouse polling rates are available under the Misc tab.

- Added clearer startup and fatal-error messages.
- Added automatic diagnostic-log rotation for easier troubleshooting.
- Updated the application name to `verux_apb.exe`.
- Optimized the release build and reduced the application to one compact executable.
- Updated the User Guide, setup instructions, and feature manual for Version 1.5.
- Removed obsolete ViGEm requirements from the current package.

### **IMPORTANT CONNECTION REQUIREMENT**

VERUX APB now requires an active internet connection for license verification. If the connection is interrupted, reconnect and launch the application again.

### **ARDUINO SETUP**

- Connect your prepared Arduino Leonardo.
- Open **Misc**.
- Click **Refresh COM ports**.
- Select the Arduino port shown in Windows Device Manager.
- Choose a stable mouse polling rate for your board and system.

For troubleshooting, review the included manuals or contact support through the official Verux Discord.

Enjoy!

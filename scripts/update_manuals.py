from pathlib import Path
from copy import deepcopy

from docx import Document


SOURCE = Path(r"C:\Users\Admin\Downloads\VERUX MANUALS.docx")
OUTPUT = Path(r"C:\Users\Admin\Desktop\verux.apb.1.5\VERUX MANUALS.docx")


REPLACEMENTS = {
    "This document provides a comprehensive guide for the setup and usage of the verux.apb.exe application. It includes instructions for driver installation and the necessary preparation steps for the Arduino Leonardo microcontroller.":
        "This document provides a comprehensive guide for installing, licensing, and using verux_apb.exe with an Arduino Leonardo.",
    "verux.apb.exe is an application designed to interact with an Arduino Leonardo, enabling precise control over mouse movements and actions. To use this application, you must first ensure that the required drivers are installed and the Arduino is properly configured with the provided code.":
        "verux_apb.exe works with an Arduino Leonardo to provide configurable mouse movement and actions. The application requires an active license, an internet connection, and a correctly prepared Arduino.",
    "verux.apb.exe file.": "verux_apb.exe file.",
    "ViGEmBus Driver installer.": "An active Verux APB license and a stable internet connection.",
    "3. Driver Installation: ViGEmBus Driver": "3. Windows Preparation",
    "The ViGEmBus Driver is essential for creating virtual input devices that the verux.apb.exe application may utilize.":
        "The current release uses the Arduino Leonardo input method and does not require ViGEmBus.",
    "Installation Steps": "Preparation Steps",
    "Locate the ViGEmBus Driver installer file.": "Keep verux_apb.exe in a folder where your Windows account has write access.",
    "Run the installer and follow the on-screen prompts.": "Keep the computer connected to the internet while launching and using the application.",
    "Accept the license agreement.": "Launch verux_apb.exe and approve the Windows administrator prompt.",
    "Choose the installation location (default is usually recommended).": "If Windows displays a security warning, confirm that the file came from the official Verux release package.",
    "Complete the installation. A system restart might be required after the installation is complete.": "No separate input driver installation is required for this release.",
    "The Arduino Leonardo is now configured and ready to communicate with verux.apb.exe.":
        "The Arduino Leonardo is now configured and ready to communicate with verux_apb.exe.",
    "5. Running VERUX.APB.EXE": "5. Running VERUX_APB.EXE",
    "Once the ViGEmBus Driver is installed and the Arduino Leonardo is programmed:":
        "Once the Arduino Leonardo is programmed and connected:",
    "Double-click the verux.apb.exe file to launch the application.": "Double-click verux_apb.exe and approve the administrator prompt.",
    "The application should automatically detect the Arduino's serial port and begin operation.":
        "Enter your license key when prompted. After verification, open the Misc tab, refresh the COM-port list, and select the Arduino Leonardo port.",
    "If connection issues occur, ensure no other applications are using the Arduino's COM port, and check the port selection within any configuration settings of verux.apb.exe if available.":
        "If connection issues occur, ensure no other application is using the Arduino COM port, click Refresh COM ports, and select the correct port again.",
    "For troubleshooting, consult the application's documentation or contact technical support at discord.com/invite/DJ4THpewg5":
        "For troubleshooting, review the current manuals or contact support at discord.com/invite/DJ4THpewg5. Diagnostic logs are stored in Documents\\VeruxAPB\\verux_apb.log and automatically rotate when they become large.",
    "This manual provides a detailed breakdown of the features within verux.apb.exe application. Use this as a reference to fine-tune your configuration for the best performance and \"closet\" (legit-looking) gameplay.":
        "This manual provides a detailed breakdown of the features in verux_apb.exe. Use it as a reference when configuring the application for your system and preferred response.",
    "Upon launching verux.apb.exe, you will be prompted with the Login screen.":
        "Upon launching verux_apb.exe, approve the administrator prompt and select Login to open the license screen.",
    "License Key: Enter your unique product key.": "License Key: Enter your unique product key. The application requires an internet connection for activation and periodic verification.",
    "Submission: Click Submit to verify your license. Once verified, the software will unlock the full control panel.":
        "Submission: Click Submit to verify your license. Once verified, the main control panel becomes available.",
    "Profile Info: Your license status, registered email, and remaining time can be viewed at any time under the Misc tab.":
        "Profile Info: Your license status, product, variation, used time, and remaining time are shown under the Misc tab.",
    "The Triggerbot automates your shots when a target is detected within your crosshair.":
        "The Triggerbot sends a click after the configured activation condition is met.",
    "Double Crouch: An experimental feature to assist with recoil management or hitbox manipulation during firing.":
        "Double Crouch: Pressing the crouch key twice within the configured interval temporarily activates Triggerbot.",
    "Delay Slider: Add a millisecond delay (e.g., 37ms) to simulate human reaction time and bypass server-side detection.":
        "Delay Slider: Adds a configurable millisecond delay before Triggerbot sends a click.",
    "This section handles how the software communicates with your computer.":
        "This section controls Arduino communication, application settings, and the interface color.",
    "Mouse Method: Select your preferred input bridge:": "Mouse Method: The current release supports Arduino Leonardo output.",
    "Vigem: Virtual Controller emulation.": "Arduino: Select the connected Leonardo COM port after refreshing the port list.",
    "Arduino: (Recommended) Physical hardware movement via the Arduino Leonardo.":
        "Mouse Polling Rate: Select the output rate that is stable for your Arduino and system.",
    "COM Ports: If using Arduino, click Refresh COM ports and select the port identified in your Device Manager.":
        "COM Ports: Click Refresh COM ports and select the Arduino port identified in Windows Device Manager.",
}


def replace_paragraph_text(paragraph, replacement: str) -> None:
    run_properties = None
    if paragraph.runs and paragraph.runs[0]._r.rPr is not None:
        run_properties = deepcopy(paragraph.runs[0]._r.rPr)
    for child in list(paragraph._p):
        if not child.tag.endswith("}pPr"):
            paragraph._p.remove(child)
    run = paragraph.add_run(replacement)
    if run_properties is not None:
        run._r.insert(0, run_properties)


def main() -> None:
    document = Document(SOURCE)
    replaced = set()
    for paragraph in document.paragraphs:
        original = paragraph.text
        if original in REPLACEMENTS:
            replace_paragraph_text(paragraph, REPLACEMENTS[original])
            replaced.add(original)

    for table in document.tables:
        for row in table.rows:
            for cell in row.cells:
                for paragraph in cell.paragraphs:
                    if "making it virtually undetectable by standard anti-cheats" in paragraph.text:
                        replacement = paragraph.text.replace(
                            ", making it virtually undetectable by standard anti-cheats", ""
                        )
                        replace_paragraph_text(paragraph, replacement)

    missing = set(REPLACEMENTS) - replaced
    if missing:
        raise RuntimeError(f"Manual source changed; replacements not found: {sorted(missing)}")

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    document.save(OUTPUT)
    print(OUTPUT)


if __name__ == "__main__":
    main()

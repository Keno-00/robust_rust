# verux.leonardo feature contract

Status: setup executable implemented and personalized signed firmware delivery deployed; physical installation remains unqualified. Updated 2026-10-04.

## Distribution requirements

The customer package contains the manual, verux.apb.exe, the original EchoDriver distribution, and a Leonardo setup tool. Do not add an unsigned-app status note or SHA256SUMS.txt to the customer package. Internal artifact digest verification remains required; omitting a distributed checksum file does not remove server/client integrity checks. Do not claim publisher signing when none exists. Preserve existing Windows trust enforcement and the original driver files.

## Owned-device readback

COM10 enumerated as Arduino Leonardo, USB VID 2341 / PID 8036. A 1200-baud bootloader touch temporarily enumerated the Caterina bootloader on COM11, VID 2341 / PID 0036. The board returned to COM10 afterward. Official Arduino avrdude 6.3.0-arduino17 was obtained from the Arduino package index and its archive SHA-256 verified before execution.

Readback used avr109, ATmega32U4, 57600 baud, `-n -D -U flash:r:<private-backup>:i`. No erase, flash write, EEPROM write, fuse write, or lock-bit write was requested. avrdude reported device signature 0x1e9587. The Intel HEX output was checked for record lengths and checksums; it represents 32,730 flash bytes, including a nonblank application and bootloader region. Application strings include Arduino Leonardo and shoot. These observations do not establish the entire firmware protocol or reconstruct its source.

The private backup resides outside source control under `%LOCALAPPDATA%/VeruxFirmwareBackups/20261004`. Keep the complete readback as recovery evidence, not an approved customer firmware artifact. Do not redistribute or flash it until the application region, bootloader boundary, licensing/provenance, and behavior are qualified. Compiled machine code does not contain the original sketch, comments, or original variable names.

## Setup tool behavior

- Name the executable verux.leonardo.exe. Reuse the existing Verux software shell, banner, icon, colors, and control style. Use terse labels such as Login, Detect, Install, Cancel, and Exit, with no routine expository text. Use explicit board selection, bounded background work, and a percentage-only progress bar. Actionable error dialogs remain required.
- Reuse the authoritative server in license-server and explicit product identity verux.apb. Do not duplicate the license authority.
- Require signed protocol 2 authorization with fresh nonces. Define a separate firmware-delivery endpoint and allowlist; do not disguise firmware as a smoothing algorithm.
- Use a signed manifest binding product, board model, MCU signature, immutable firmware version, application address range, size, digest, request nonce, recipient, and authorization window. Reuse reviewed encrypted delivery primitives with a distinct documented firmware context.
- Keep vendor firmware artifacts private in product-scoped R2 storage. Verify the complete delivery before permitting a device write. Never embed Payhip, server signing, or shared licensing secrets in the setup executable or firmware.
- Validate the physical MCU signature and authorized address range. Never erase or overwrite the bootloader, EEPROM, fuses, or lock bits as part of routine setup.
- Back up readable existing firmware before an explicitly selected installation. Present failures visibly, with sanitized logs. Do not silently flash an alternate firmware, retry through another transport, or restore firmware automatically.
- Confirm post-write verification and device re-enumeration before declaring success. A recovery action must be explicit and based on a verified backup and device identity.
- Keep high-value algorithms in the server-delivered app runtime. Leonardo firmware should contain only the narrow transport/HID function required by the device.

## Protection boundary

Server authorization controls obtaining the approved firmware through the setup tool. Firmware must persist on the Leonardo to execute without an internet connection. Standard Leonardo hardware cannot guarantee firmware secrecy, permanent license enforcement, or resistance to a device owner extracting/modifying firmware. A licensed flasher alone does not prevent copying an already obtained image. Any stronger hardware policy requires a separate documented hardware design, not a hidden shared secret.

## Qualification before implementation/release

Recover or reconstruct and review the firmware behavior without overwriting the current board. The app currently uses 115200 baud, `dx:dy\n` movement lines, and `shoot\n`; observing shoot strings in the dump is not proof of complete compatibility. Define serial bounds, malformed-message behavior, no unsolicited HID output, disconnect behavior, and a non-destructive identification command before approving a replacement firmware. Verify these manually on an owned spare board, including interrupted programming and recovery. No new automated tests or fixtures without user authorization.


## Personalized firmware delivery deployed — 2026-10-04

User authorized publication and deployment. The initial two-identity catalog deployment (`1105fb3e-8f3d-457f-b5d1-36047ea6822c`) was too narrow for other users and was immediately superseded. Current Worker: `ec22fadd-840c-42fc-bdc2-c75e558c200d`. The server accepts the selected USB mouse VID/PID as integers 1–65535 and personalizes a verified private template; no per-mouse catalog or compilation is required. This does not assert Windows device merging or physical compatibility with every mouse driver.

Template version `1.0.0-candidate.1`, SHA-256 `f9d35d821fa7396516cad9035e10c11d470f3e104f4ac4d56021d771aa21286e`, 78,860 HEX bytes, private R2 path `verux.apb/firmware/leonardo/template/1.0.0-candidate.1/<digest>.hex`. Configuration: LEONARDO_FIRMWARE_TEMPLATE with descriptor_offset 242. The source descriptor's entire 18-byte sequence is checked before only bytes 250–253 (VID/PID) are changed. Input HEX checksums, EOF, uniqueness and complete 28,672-byte application coverage are mandatory. No bootloader bytes are allowed. Generated HEX checksums/digest are recalculated and bound into the signed/encrypted session/recipient/nonce/identity manifest. Other application bytes stay identical. Replacing the compiled template requires reviewing its descriptor offset, expected bytes, digest, version and compatibility; never search/patch arbitrary matching machine-code bytes.

TypeScript and Wrangler dry run passed. Local inline diagnostics checked IDs 0001:0001, FFFF:FFFF, 1234:5678, 046D:C08B and 4C4A:9002, verifying decrypted image identity and every other application byte unchanged. Live owned signed activation/delivery/decryption/digest/HEX/descriptor checks passed for 1234:5678 and both detected mice. Invalid identity bounds received a verified signed 400. Temporary sessions were signed-deactivated; credentials were read from existing DPAPI storage, never logged or written plaintext. These activations may replace an existing app session, so Login may be needed again. No serial ports were opened and no physical flash occurred. Interactive Install/physical readback/re-enumeration qualification remains pending. Existing setup executable understands the same manifest and needs no rebuild for this correction.

Historical per-identity immutable objects remain private but are unused by the current delivery configuration. Server scope, signing secrets and algorithm digests were preserved. No new automated tests, fixtures or test-only dependencies were added. One TypeScript invocation ran from the repository root and found no local compiler; rerunning in license-server used the pinned compiler and passed without installing a dependency.


## Post-write repair — 2026-10-04

Owned full-flash readback diagnosed 488 old application bytes at 0x1980–0x1BA9 where the padded signed image expected 0xFF. New compiled code ended at byte 6469; all mismatches equaled the old backup. The bootloader matches the pre-write backup exactly. AVRDUDE 6.3 trimmed erased trailing input despite -D. No verification check was relaxed.

Installer now pins official Arduino AVRDUDE 8.0-arduino.1 (archive SHA-256 833aa1a66a8e70cd597fcfdbd7e559a91a00eca1d7aa3be2ce9bcadf7ccb987c, verified against the existing official Arduino package index). It explicitly passes -A as well as -D so the full 28672-byte input, including erased padding, is programmed without chip erase or bootloader writes. The MCU signature parser matches the pinned version's output. A read-only run confirmed the physical signature and identical flash readback; original backup digest was verified.

Repair is an explicit UI action for a previously personalized board at the same physical USB location with matching prior installation metadata and a valid original backup digest. It still uses the selected serial programming implementation, server-authorized signed firmware, pre-write backup, physical MCU check, full application verification and bootloader comparison. Install retains the original Arduino first-install requirement. Failed post-write evidence is now retained, with mismatch count/location and bootloader status logged. No automatic recovery or flash was performed during diagnosis.

Release build and format check passed; executable and new pinned tool files were copied directly to outputs and package and verified. No ZIP or new tests were added. Physical Repair must still be run explicitly by the user to qualify the write. A combined packaging/deletion command was rejected by automatic approval review with only blocked by policy; explicit non-deleting copies succeeded, and the obsolete unused libusb0.dll was left in place.


## COM readiness and identification correction — 2026-10-04

The user's updated writer programmed and verified all 28672 application bytes, and the independent full-flash comparison passed application and bootloader preservation. It then failed opening COM12 for the non-moving identification command. A subsequent Repair also saw a stale COM12 node during bootloader re-entry. Device enumeration can expose the COM node before the Windows handle is usable.

Added a single bounded application-port readiness helper, retaining the selected serial backend, physical USB location and exact VID/PID. It re-enumerates and attempts the same device for at most 15 seconds, rejects ambiguous identities, honors cancellation, and logs elapsed time/attempt counts; persistent failure remains actionable. Both 1200-baud bootloader entry and 115200-baud identification use this helper. No alternate transport, weakened identity matching or automatic flash/recovery was added.

Added an explicit Verify button: authenticated identification-only query with a live-session check before/after, no bootloader touch or flash write, and a distinct Verified notice. Existing Install/Repair retain their full signed firmware, physical signature, image and bootloader checks.

An owned diagnostic opened COM12 at 115200 and sent only id\n. It received LEONARDO_MOUSE:1:1.0.0-candidate.1; the expected firmware is running. No additional flash was performed by the agent. Initial compilation after adding the Verify dispatch failed because an old repair variable remained in the worker; corrected the dispatch explicitly without weakening behavior. Final validation/build/delivery are recorded in the current task. No new tests or fixtures were added.

Final release build and cargo fmt --check passed. Updated the existing Desktop/verux.apb 2.0 executable, outputs and package directly and verified their hashes and pinned tool files. The user can now click Verify without another flash. Interactive bounded-transition/Verify UI qualification remains separate from the observed successful hardware identification.

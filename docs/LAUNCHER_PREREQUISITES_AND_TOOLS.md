# Launcher prerequisites, tools and shared access (agreed 2026-11-22)

This is the contract between the license server and catalog in `robust_rust` and the launcher in `verux_launcher`. It covers three things the operator approved: EchoDriver as an invisible prerequisite, Leonardo as a tool reached from product cards, and Songmaker as a paid product. Code changes are approved. Signing, catalog or library publication, Worker deployment, launcher self-update and firmware template publication each still need the operator's explicit go-ahead.

## Products

| id | Shown as | Access | Notes |
| --- | --- | --- | --- |
| `verux.apb` | product card | paid (unchanged) | MSI rebuilt **without** the driver; `prerequisites: ["verux.echodriver"]`; `tools: [{"id": "verux.leonardo", "label": "LEONARDO"}]`. |
| `verux.sm` | product card | paid: `https://payhip.com/b/NfvjB`, directory `VeruxSM`, entropy `veruxsm-license-ledger-v2` (key file `Documents\VeruxSM\license.key`, DPAPI) | MSI built: `robust_rust/target/packages/verux.sm-2.0.0` (banner from apbsongmaker `assets/launcher/banner.png`). Requisites `EchoDriver`, `Leonardo 2.0`, `Runs as administrator`. `prerequisites: ["verux.echodriver"]`, `tools: [{"id": "verux.leonardo", "label": "LEONARDO"}]`. |
| `verux.echodriver` | **never shown** | any of `verux.apb`, `verux.sm` | MSI with only the existing reviewed EchoDriver payload and DriverInstaller flow; same service identity as today. |
| `verux.leonardo` | **no card**; a LEONARDO button on the cards that list it in `tools` | any of `verux.apb`, `verux.sm` | MSI with the verux.leonardo installer (exe and pinned AVRDUDE). Not a product, so no banner or artwork. |

## Catalog additions (signed product records)

- `visibility`: `"card"` (default), `"hidden"` (installed only as a prerequisite, never listed), or `"tool"` (no card; reached only through `tools`).
- `prerequisites`: product ids. Installing or repairing a product first installs each missing prerequisite **silently in the same flow**, through the same verified MSI path. A prerequisite stays installed while any installed product lists it. It is removed with the last one, including on managed expiry uninstall.
- `tools`: `[{ "id", "label" }]`, buttons on the product card. A click installs the tool if missing, after the same verification, and launches it.

## Library additions

- `access: "any_of"` with `any_of: ["verux.apb", "verux.sm"]`: entitled when the protected key of **any** listed paid product validates. The server issues download grants for these products on any listed license. `purchase_url`, `directory` and `entropy` stay empty. There is no key storage of its own.
- The Leonardo firmware template delivery (`personalize`) accepts an APB **or** Songmaker license.

## Firmware

- One firmware for every board. The public name is **Leonardo 2.0**; internally it is `1.0.0-candidate.4` (keyboard HID and mouse wheel). Internal candidate numbers never appear in customer-facing text, requisites or catalog fields.
- The server template becomes candidate.4 (built in verux.leonardo `target/firmware-build/leonardo-mouse-1.0.0-candidate.4-template.hex`, SHA-256 `f1588ab9…eecfb36`, descriptor offset 370).
- APB keeps accepting candidate.1 and newer, since it uses no candidate.4 features yet. Songmaker requires candidate.4 (apbsongmaker `b1d52ac`).

## Migration

Existing APB installs own EchoDriver under `Program Files\VeruxAPB\Driver`. Moving ownership to `verux.echodriver` must keep the running service working. Use the existing `replacement_service` / `service_file` metadata, or a reviewed migration; never leave a machine without the driver between steps.

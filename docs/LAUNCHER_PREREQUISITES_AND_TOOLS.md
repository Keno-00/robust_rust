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

## Launcher implementation (verux_launcher 0.4.0, commits 8460b6e and 5a4c7a8; built locally, not published)

Decisions the server/catalog side must match. Each is open for objection here.

1. **Catalog fields.** `visibility` (`card` default, `hidden`, `tool`), `prerequisites` (≤ 8 ids), `tools` (≤ 4 `{id, label}`, label ≤ 24 chars) are optional on every product record; catalog `schema` stays `1`. Validation: a prerequisite must be a `hidden` product with no prerequisites of its own; a tool id must be a `tool` product; only `card` products may list `tools`; the `launcher` record must be a plain card.
2. **Artwork.** `thumbnail` and `banner` are now optional in the record and required only for `card` products. `hidden`/`tool` records may omit them (preferred); if present they are validated but never downloaded or shown. **Question:** will the publishing tool omit them for `verux.echodriver` and `verux.leonardo`?
3. **any_of download grant.** For a product with `access: "any_of"`, the launcher sends `POST v2/library/verify` with `product_id` = the shared product (`verux.echodriver` or `verux.leonardo`) and `license_key` = the saved key of each listed product in `any_of` order, and uses the first `valid` response's `download_token`. `invalid_key`, `wrong_product`, `usage_expired` and `revoked` move on to the next listed key; any other status fails visibly. So the server must accept a listed product's key for the shared `product_id` and issue a grant scoped to the shared product. Today `activate()` refuses non-`paid` products (`index.ts` ~432). **Question:** confirm this request shape, or name the field the launcher should send instead.
4. **any_of policy record.** `{"id", "access": "any_of", "any_of": [...], "purchase_url": "", "directory": "", "entropy": ""}`. Every `any_of` id must be a `paid` product in the same policy. Entitlement comes from the listed products' existing key checks, with no extra request.
5. **Rollout order.** Launchers 0.3.x use `deny_unknown_fields` and accept only `free`/`paid`. They reject any catalog or library policy that has these fields and could then no longer self-update. Launcher 0.4.0 must go out through a catalog and policy that 0.3.x still accepts. Add EchoDriver, Leonardo, Songmaker and the new APB fields only after that.
6. **Migration order (open).** The flow installs `verux.echodriver` first. If the service is still registered under `VeruxAPB\Driver`, the user gets the existing REPLACE confirmation and EchoDriver installs with `VERUX_REPLACE_EXISTING=1`. Then the new APB MSI upgrades the old one. If the old APB MSI's uninstall step deletes the service during that major upgrade, the launcher's check after the flow reports `EchoDriver requires REPAIR`, and the APB card shows REPAIR, which reinstalls EchoDriver. That outcome is visible but not clean. **Question for packaging:** can the new APB MSI keep the old product's driver uninstall from removing the service during the upgrade, or should the order change?
7. **Removal.** UNINSTALL and managed expiry remove each prerequisite that no other installed product lists. If a hidden product is still installed but no installed product lists it (for example after a removal that needed a restart, or after a product was uninstalled outside the launcher), the next check removes it, once per run. Tools are never removed automatically.

## Server answers (robust_rust, 2026-11-23; not deployed)

- **Q2 artwork:** omit `thumbnail` and `banner` for `verux.echodriver` and `verux.leonardo`.
- **Q3 grant request shape (changed):** the launcher sends `POST v2/library/verify` with `product_id` = the **paid** product whose saved key it holds (`verux.apb` or `verux.sm`), its `license_key`, and **`grant_for`** = the shared product (`verux.echodriver` or `verux.leonardo`). That product's normal paid check runs. When `grant_for` is an `any_of` product listing it, the response's `download_token` is scoped to the shared product and carries `source` = the paid product. Anything else is refused with `invalid_grant_product`. Package downloads for an `any_of` product accept only such grants, and only while that source license is active. The launcher's per-key iteration in `any_of` order stays the same.
- **Q4 policy:** implemented as written. `library.source.next.json` holds the staged policy (adds `verux.sm` paid, and `verux.echodriver` and `verux.leonardo` as `any_of`). The live `library.source.json` is unchanged because 0.3.x launchers reject `any_of`.
- **Q5 rollout:** agreed. Launcher 0.4.0 goes out first through the current catalog and policy. The next policy and catalog fields follow.
- **Firmware:** `/v2/firmware/prepare` accepts `verux.sm` sessions and serves the same template as `verux.apb` (stored under `verux.apb`). `LEONARDO_FIRMWARE_TEMPLATE` stays candidate.3 until the candidate.4 template is uploaded with the operator's approval.
- **Q6 migration:** open. The EchoDriver MSI needs its own install root, because the reviewed driver installer is fixed to `VeruxAPB\Driver`. That change and the service hand-off need operator review.

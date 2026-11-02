# Launcher library implementation contract

Source-internal implementation contract. Do not package this document in customer artifacts. The user approved this behavior and subsequently requested server-controlled startup/product notices. Launcher 0.3.0 implements it; consult the release guide for actual qualification and deployment status.

## Requested layout

Preserve the existing launcher banner, width, CHECK / UPDATE row, PRODUCTS tab/grid, and STATUS console. Show free products and paid products added with a valid license. Include one + grid card. Its popup copies the existing APB `draw_login` modal in `src/ui.rs`: `License Key`, `Submit`, disabled pending state, inline errors, existing styling. Add a close control. Do not replace this with a new visual design.

On confirmed expiry, automatically uninstall only the launcher-managed product and its owned driver. Keep the product card/banner visible for five days. The top launcher banner remains unchanged. The expired product modal exposes its purchase link and the same license popup. A valid renewed/replacement key restores entitlement and permits installation again. After five days, remove that expired card; the customer can use + to add it again. Preserve customer profiles and saved settings.

Explicit purchase URL supplied by the user for `verux.apb`: https://payhip.com/b/J3Omz . This is server-provided product metadata, not a client hardcoded link. Future products supply their own access policy, purchase URL and compatible protected storage metadata.

## Authorization and expiry

Existing APB protected license storage is `Documents/VeruxAPB/license.key`, using user DPAPI with the existing product entropy. Read and validate an existing key before adding the paid product. Save a newly verified key compatibly so APB can load it. Keep raw keys out of logs, command lines, URLs and unprotected state. Existing APB login and runtime licensing remain authoritative; a launcher check must not create a metered usage session or consume usage time.

Only an authenticated, fresh, product-bound server response confirming expiry can initiate automatic uninstall. Transport failures, unavailable Payhip, rate limits, stale metadata, invalid local storage, or a local clock change are not expiry. Bind the five-day retention deadline to server-recorded confirmed expiry and the applicable license. A restored license cancels pending expiry cleanup; revalidate immediately before executing a queued uninstall.

Automatic uninstall is possible while the launcher runs, not while it is closed. Use the existing MSI ownership, journal, rollback and restart flow. Windows still controls elevation approval. Do not kill customer applications or development services silently. Cancellation, files-in-use, ownership conflicts and restart requirements remain visible and logged. Never report successful removal without verifying it.

Card visibility is not authorization. Paid MSI downloads need server-side authorization bound to the exact product; free downloads and launcher self-update must continue to work. Preserve existing signed catalog verification and package/file integrity checks. Never interpret missing product access metadata as free access.

## Implementation boundaries

Coordinate the launcher UI/state/storage/networking, authoritative license server validation and delivery routes, product metadata, release tooling and operator documentation. Reuse the existing APB signed response protocol and license validation/accounting under its coordinator. Do not duplicate Payhip verification or weaken checks to make launcher discovery work. Maintain explicit product enrollment and server-only product secrets.

Qualify a valid saved key, manual enrollment, invalid/wrong-product key, expired entitlement, renewal during the five-day interval, card removal, unauthorized paid download, network failure, cancelled/failed uninstall and restart handling. Follow repository restrictions: no new automated tests or fixtures without the user's explicit request. Do not claim a destructive expiry scenario has been exercised merely because a build passes.

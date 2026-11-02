# Memory Triggerbot cursor-unavailable suspension — 2026-10-19

Source-internal; do not package or embed.

The runtime log at 13:22:52 Asia/Manila records `Memory pegasus source stopped: APB cursor value 0 (0x0) is not classified for this build.` The user reported aiming down sights with the HUD hidden using F12. Application event-log inspection found no corresponding Verux application crash entry; the client log shows its controlled runtime error path and cleanup. The observation associates this occurrence with HUD-off ADS; it does not establish an enemy/neutral classification for zero.

The user explicitly selected: “Suspend Triggerbot only; keep other features running.” Category, master, source, edition/entitlement, and profile preferences remain unchanged. This is runtime signal availability under the existing Memory Triggerbot consumer, without a substitute source or revised target classification.

`ApbCursorSource::should_fire` now returns an unavailable observation for zero and records suspension. A recognized red/green/white value clears suspension; the existing red/green selection still determines whether a shot is permitted. The cursor pointer is reread after the value read, so a changed binding cannot restore firing from that discarded sample. Genuine read, process, pointer, and authentication errors propagate. Other unclassified nonzero values keep their existing 500 ms fail-visible stop behavior. The previously retained red mapping is not reinterpreted by this change.

The Trigger worker clears the pending shot before publishing the suspension notice. While suspended it continues reading the same source even if the activation key is released, allowing availability recovery; the activation and block-key guards still prevent firing while inactive or blocked. Neither the global run flags nor Aim, ESP, radar, or audio consumers are stopped by a zero observation. Normal pause, focus, World/binding availability, and server/session revocation guards retain their existing behavior.

Suspension/resumption transitions are logged once. The existing main window's Start section shows a persistent yellow diagnostic, `Triggerbot suspended. Restore HUD (F12).`, while suspended. It appears whenever the main window is visible and does not open a focus-stealing popup or a new in-game overlay. Cleanup clears the notice. No catalog, saved profile, feature eligibility, or server deployment changes are needed.

Validation: Full `cargo check --locked --quiet` and Color `cargo check --locked --quiet --no-default-features` pass. Color retains the existing unused `encode_fingerprint` warning. No automated tests, fixtures, or dependencies were added or run. Live HUD-off suspension/resumption and gameplay acceptance remain pending.

Release receipt: Full release build passed in 3m 47s using target/v3-release. Copied to target/editions/color-memory/release/verux_apb.exe and verified matching SHA-256: 42020BD833011DF92835EAAF804FA8433F0E564E44F6DA0EAF6D1641D4029FA1 (42,864,640 bytes). No server deployment was required. Live HUD-off recovery remains unverified.

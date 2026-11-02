# Memory red hit sound

Evidence: `captures/hitmarker-cursor-9844-20261019-095620` captured 101 idle, 75 red and 100 white samples. Cursor-adjacent timer +0x14C was zero in idle, rose three times with red RGBA (1,0,0,1), and ten times with white RGBA (1,1,1,1). The four color floats begin at +0x150. Six diagnostic image frames contained red markers; all three red timer rises aligned with red-marker frames. These are observations for the audited build, not general engine field definitions.

Live confirmation: `captures/hitmarker-sound-9844-20261019-095937/events.jsonl` recorded 13 red WAV plays and 6 silent white events. User reported “It works Nice” and authorized integration. Runtime reads memory only; images were diagnostic ground truth.

Implementation: `ApbCursorSource::red_hit_event` reuses the existing build-gated EchoDriver cursor binding. It checks pointer/header stability, finite timer/color values, a timer rise above 0.05 and red color. White events are excluded. Initialization, focus loss, pause and binding replacement reset the baseline. The dedicated worker polls every 10 ms, checks runner state, and joins on shutdown. Multiple hits between reads and asynchronous WAV interruption can limit audible event counts.

General contains the Memory-dependent Hit sound toggle and supplied-WAV selector. Profiles default disabled and preserve preferences while effective eligibility disables the consumer. No Aim, Trigger, DXGI, overlay or Arduino dependency is added. Color compiles without the reader, sound assets, worker or new UI controls.

Preview is an action beside the WAV selector, available under the same eligibility. Each click calls asynchronous embedded-WAV playback anew, restarting any current sound. It has no saved toggle, debounce or remembered completed state and requires no APB memory connection. Playback failures produce the existing visible error dialog plus a diagnostic log entry. It reuses authenticated display label u128, so no additional server catalog revision is needed.

The Full client requests the new authenticated `surface.hitsound` catalog (145 rows); previous Full catalog routes and Color's 143-row catalog remain intact. Prepare locally with `license-server/scripts/prepare-hitsound-surface.ps1`. Publication requires `publish-surface.ps1 -Catalog surface.hitsound -Upload` and Worker deployment. No secrets are embedded in source.

Build checks: `cargo check --locked --bin verux_apb`; `cargo check --locked --bin verux_apb --no-default-features`; `npx tsc --noEmit` from license-server. These are compilation checks, not automated tests or integrated-client live acceptance.

All three compilation checks passed on 2026-10-19. Color retained the existing unused `encode_fingerprint` warning. The prepared 145-row encrypted Full catalog digest is `aa0dd1b09a39f1789104577370625f36488e20c49fdbef0bdf0dc6773ac1468b`; it has not been published by this implementation step. Existing 143-row eligibility catalogs were not modified.

Full release build passed using the existing dependency cache: `cargo build --locked --release --bin verux_apb --features memory --target-dir target/v3-release`. The executable was copied to `target/editions/color-memory/release/verux_apb.exe`; SHA-256 `A6A28E8206D4C307D69947D36494156FE5CF9C0A1ADF4D0BDC78C8A897B3A385`, 14,794,240 bytes. The initial build in the separate edition target was canceled during dependency compilation; the copied artifact comes from the successful Full build. Publication and integrated-client live acceptance remain pending.

User authorized publication and launch afterward. The catalog was published via `publish-surface.ps1 -Catalog surface.hitsound -Upload`; encryption regenerated its digest as `e01724a558f279c8d21fb80b38dd51a66084b7127e52fdeb254aea3c54050133`, with the matching Worker variable deployed. Production Worker version: `3955b9d6-9b29-4c60-8365-ccef453e9bd6`. Existing catalog routes and APB minimum client version 1.5.0 were preserved. The Full executable hash was rechecked and launched elevated, PID 17232. Integrated-client gameplay acceptance remains pending.

Preview-button build: Full release compilation passed on 2026-10-19 in 7m09s. Packaged executable: `target/editions/color-memory-preview/release/verux_apb.exe`, SHA-256 `0932EE1D6D4C7CD7C51CADE97C6D60D86A049E581A50E944CBF150390BCBCB24`, 14,795,776 bytes. The Preview action reuses the published u128 label; no server deployment was required. This build was not launched or live-tested by the build step.

# Object markers and stable recency — October 18, 2026

The later [green availability and continuity update](APB_PUMPKIN_AVAILABILITY_20261018.md)
supersedes the possible-location-only green behavior described in the initial
release below. It uses an operator-labelled intact/empty comparison and a
recorded destruction transition, while retaining red fixed-location markers.

## Automatic pumpkin identity

`captures/pumpkin-prefab-identity-probe.json` follows each captured typed prefab's `+0x308` reference. All four user-tagged locations reference an asset named `PumpkinGreen_prop`, vtable RVA `0x31517A8`. Ammo distributors reference `AmmoVendingMachine_Crim_Prop`; drains, dumpsters, ticket machines and electrical boxes have different asset names. The native classifier requires the exact prefab type and exact resolved asset name, rather than generic audio-emitter/prefab proximity.

The same capture also found `PumpkinRed_prop`, whose outer-name chain included `Materials` and `Collectable_Halloween`. The user explicitly requested projecting that asset even before visual confirmation. Red asset identity is therefore included, with availability and visual correspondence still unconfirmed.

### Restart evidence

APB restarted as PID 22492 with creation time 134367853775096276 and the same gated executable hash. The fresh read-only capture `captures/pumpkin-restart-22492.json` inspected 43,197 actor slots in 258 levels, recording 24 green assets and two red assets; 20 matching prefabs were within 8,000 world units of the sampled camera. No read failures were recorded. This is a bounded discovery snapshot, not a native feature acceptance run.

The previous green name index 152580 now read `Pumpkin.Mailbox_Prop_Arc0`, and previous red index 223683 read `financialdistrict_holiday_halloween_greenpumpkin.BulkData`. Therefore asset name indices are **not** treated as build constants. The implementation resolves each typed asset's actual name through the current GNames binding, checking entry flags, slot identity and root bounds. It never reuses the previous process's object addresses.

The first native acquisition attempt then exposed an invalid alignment assumption: GNames entries are byte-packed. `captures/apb-name-entry-alignment-22492.json` records valid entries at `0x33E5CDD` (projectile), `0x3C3A9693` (green asset) and `0x300CCD41` (red asset), all with matching name-entry flags and text. Name-entry addresses therefore use user-address bounds without UObject alignment rules. UObject alignment checks, entry identity flags, exact text and binding checks remain enforced. The initial object build failed visibly on these entries; its native read guards were corrected before the replacement build.

`apb_jade` incrementally inspects eight loaded levels per pass and caches membership. Runtime positions and exact instance identities are reread for markers. Normal array changes discard affected captures and are logged; failed reads of still-current objects, invalid bounds and incompatible types remain errors. The marker means a matching persistent location, not a pumpkin currently available to collect. Smashed locations may continue to appear.

## Projectiles

Two earlier observations establish `cAPBProjectile`, type `(0x2B68D90, 6770)`. Held `cGrenadeWeapon` objects use a different type and are excluded. `apb_wisp` surveys the current persistent level automatically and records first/last observations; no F6 tagging is needed. Unobserved subclasses are outside its coverage. No fuse or detonation inference is implemented.

Independent `apb_glow` workers keep the pumpkin sweep away from projectile discovery and the aim loop. Projectile passes pause 75 ms and pumpkin passes pause 100 ms. Freshness limits are five seconds for pumpkin batches and one second for projectile batches; displayed positions are reread in the rendering frame. Cached discovery positions restrict hot reads to 8,000 world units (80 m), then fresh positions are checked against the same limit. Foreground/ownership loss clears published marker batches; generation guards prevent an old World from publishing into a new context. Worker infrastructure failures are latched and reported through the existing runtime error path.

## Controls and drawing

Visuals adds independent **Pumpkins** and **Projectiles** checkboxes, both defaulting off. They use the existing DPAPI profile vault and overlay. Green-asset pumpkins draw orange squares, red-asset pumpkins red squares, and projectiles cyan circles. Existing Display Distance controls their distance labels. Object controls work independently from the player ESP filter and use the verified camera; they do not become aim targets.

## Stable Visible Only eligibility

With the existing `prism_only` option enabled, a pawn must sustain age at most 100 ms for 250 ms and produce at least three distinct advances of its render timestamp. A sample gap greater than 150 ms, stale timestamp, backward timestamp, absent candidate, changed World/instance, withheld frame or foreground loss resets acquisition. Leaving the option off preserves the ordinary aim path. Raw render recency remains separate from stable aim eligibility.

Times above use the verified game World clock. This is a stricter heuristic, not obstruction proof: a moving player behind a wall can still qualify if its render timestamp updates continuously. The previous one-second raw recency threshold is not silently relabelled as geometry visibility.

## Encrypted interface release

The new native build requests signed resource `verux.apb.surface.objects` and requires the original 132 rows plus `u133` and `u134`. New catalog ID `surface.objects`, version `2.0.0`, uses the existing server-only APB surface key, signed session delivery and a separate `APB_SURFACE_OBJECTS_DIGEST`. The old `surface` catalog, digest, resource and 132-row clients remain supported unchanged.

`license-server/scripts/prepare-object-surface.ps1` derives a separate CurrentUser-DPAPI private catalog, preserving the legacy private catalog and key, and generates encrypted `security/apb-surface-objects-catalog.sql`. Preparation does not publish to Cloudflare. No secret is embedded in native source or copied to another product.

Release publication requires uploading that encrypted SQL and deploying the new resource handler/digest. The new build cannot use the old catalog and fails visibly if the matching catalog is unavailable. No local embedded-label fallback is provided.

## Validation status

TypeScript `tsc --noEmit` and Wrangler's local deployment preview completed successfully. Release compilation completed in 6m 29s without the hash-bypass feature, producing `target/v3-release/release/verux_apb.exe` (14,327,296 bytes), SHA-256 `4152D445076BA536DBF3443D6577687A24B7274421AAF6A79FA87BE2E68889A8`, at approximately 17:20 Manila time on October 18.

No automated tests were added or run. The live restart scan confirms name-based discovery on the current launch; projectile trajectories, red marker appearance and the stable-recency feel still need operator acceptance in the rebuilt client.

After the user approved **Publish and launch**, the encrypted `surface.objects` SQL was uploaded successfully and the license Worker was deployed as version `94bb9fea-3cbd-4da4-9110-fc89ba2f271b`. The rebuilt client was launched through the approved administrator/UAC path as PID 22136. This confirms publication and launch, not completion of an in-game feature acceptance run.

That first native run exposed the byte-packed name-entry issue documented above and stopped acquisition visibly. After correcting only the name-entry address checks, the replacement optimized build completed in 6m 47s at approximately 17:30 Manila time. Replacement artifact: 14,327,296 bytes, SHA-256 `724CDD7F2AD1CC89292B4B6D0366285C33F7962E0A3BCE666E16D1C45B98A168`; administrator launch PID 17704. The earlier object binary hash is superseded by this replacement. The server catalog did not require another publication for the native read correction.

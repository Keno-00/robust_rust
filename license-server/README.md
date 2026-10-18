# Verux license server

Encrypted APB display catalog: see [storage, publishing and release status](../docs/ENCRYPTED_SURFACE.md).

This Cloudflare Worker is the authoritative online license service for Verux products. It verifies Payhip licenses server-side, stores product-scoped usage in D1, permits one active session per product license, and requires replay-resistant heartbeats.

## Request hardening (October 2, 2026)

The current source adds bounded JSON parsing (8 KiB licensing requests; 64 KiB Discord and Payhip bodies), 10-second body/upstream timeouts, canonical 32-byte URL-safe base64 installation/session identities, positive safe-integer heartbeat sequences, and numeric three-component client versions. HTTP redirects are rejected for Payhip verification. Exceptions return sanitized JSON failures and diagnostic codes rather than raw provider errors.

Rate limits have two independent dimensions, per Cloudflare location:

| Endpoint | Edge-IP bucket per minute | Independent identity bucket per minute |
| --- | ---: | ---: |
| Activation | 120 | 10 per product-scoped license HMAC |
| Heartbeat and deactivation, combined | 1200 | 180 per product-scoped session HMAC |
| Discord interactions | 120 | 60 per verified actor |

The edge-IP bucket runs before parsing or upstream/database work. Changing installation IDs or session prefixes does not reset that bucket. Cloudflare's limiter is approximate and per location; it is not global accounting or full distributed denial-of-service protection. Shared-IP users share the network ceiling. Missing edge IPs share a limited bucket. Deploy all new bindings in `wrangler.toml` with this source; missing bindings fail visibly.

Payhip verification requires the documented `data.enabled === true` and matching `data.license_key`. Tier selection uses only `data.variant_name`, accepting exactly one recognized 1-day, 7-day, 30-day, or lifetime marker. Customer names/emails and unrelated JSON strings cannot influence entitlement. Missing, ambiguous, or unsupported variants fail with `unknown_tier`; no tier is guessed. Before deployment, verify every enrolled variant against real authorized Payhip responses privately. Existing license usage rows and APB legacy license hashes are retained. See [Payhip's v2 reference](https://payhip.com/api-reference).

The Rust client rejects non-JSON or oversized responses, missing required status fields, success on non-200 HTTP responses, malformed activation sessions, unsupported heartbeat intervals, and contradictory usage values. It does not silently clamp heartbeat intervals. Session shutdown failures are logged without session credentials.

The 2.0 source now adds per-license Durable Object coordination, single-session D1 enforcement, fractional usage settlement, expiry alarms, signed protocol 2 responses, private encrypted algorithm delivery, and the client runtime/progress lifecycle. Protocol 1 licensing compatibility is explicit for enrolled clients; APB 2.0 does not downgrade. See [the full protocol and cutover procedure](../docs/SECURITY_PROTOCOL_V2.md). Production server deployment completed on 2026-10-03. Publisher signing, installation proof-of-possession, broad algorithm migration, and positive licensed performance/lifecycle qualification remain pending.

Implementation and validation record: [security hardening progress](../docs/SECURITY_HARDENING_PROGRESS.md).

## Required secrets

- `PAYHIP_PRODUCT_SECRET_APB`: Verux APB Payhip product secret. During migration, the existing `PAYHIP_PRODUCT_SECRET` is still accepted for APB only.
- `PAYHIP_PRODUCT_SECRET_SM`: Songmaker Payhip product secret. Do not create or configure this until Songmaker has its official Payhip listing.
- `LICENSE_HASH_SECRET`: At least 32 random bytes, used to key hashes of licenses, installs, and sessions.
- `DISCORD_WEBHOOK_URL`: Optional Discord webhook used for activation and expiry status messages identifying customers by their Payhip email. Missing emails are shown as "Email unavailable". Rotate exposed webhook URLs before configuring this secret.

Set them using `npx wrangler secret put NAME`. Payhip secrets must never be placed in a client, committed file, migration, or `wrangler.toml`.

## Product protocol

Every `/v1/activate`, `/v1/heartbeat`, and `/v1/deactivate` JSON request must include an explicit `product_id`. Supported identities are `verux.apb` and `verux.sm`. License, installation, and session hashes include that identity, and D1 rows carry it explicitly, preventing one product from acquiring or consuming another product's entitlement.

Existing APB license rows are retained with `product_id = "verux.apb"`. Activation recognizes their legacy hash so accumulated usage and revocation state remain authoritative; newly seen licenses and all new installation/session hashes use the product-scoped format.

The server selects Payhip credentials from `product_id`; clients cannot submit or select a secret. APB continues to use its existing policy. Songmaker is recognized by the protocol but activation safely returns `product_not_configured` while either `SM_PAYHIP_PRODUCT_NAMES` or `PAYHIP_PRODUCT_SECRET_SM` is absent.

### Enrolling Songmaker

1. Create the official Songmaker Payhip listing and copy its exact product name.
2. Set `SM_PAYHIP_PRODUCT_NAMES` in `wrangler.toml` to that exact name (comma-separated aliases are supported only when Payhip legitimately reports more than one name).
3. Run `npx wrangler secret put PAYHIP_PRODUCT_SECRET_SM` and paste the product secret at the private prompt.
4. Deploy the Worker and verify activation using a real Songmaker test license from a client sending `"product_id": "verux.sm"`.

Do not send the secret through chat, source control, command-line arguments, build logs, or client configuration.

## Deployment

1. Run `npm install`.
2. Authenticate with `npx wrangler login`.
3. Create D1 with `npx wrangler d1 create verux-license`.
4. Put the returned database ID in `wrangler.toml`.
5. Release the APB client that sends `"product_id": "verux.apb"` while the existing Worker is still deployed. The old Worker safely ignores this additional JSON field.
6. Allow that APB release to reach users, or deliberately enforce its minimum version before changing the Worker protocol.
7. Run `npm run db:migrate:remote`.
8. Configure the required secrets.
9. Run `npm run deploy`.
10. Compile each Rust application with `VERUX_LICENSE_SERVER_URL` set to the deployed Worker origin.

The multi-product Worker intentionally requires `product_id`; it does not accept legacy requests without one. APB sessions created by the previous Worker use legacy session hashes and must log in again after cutover. Existing APB license usage and revocation state remain authoritative through the documented legacy license-hash lookup.

`APP_ENABLED`, `APB_MIN_CLIENT_VERSION`, `SM_MIN_CLIENT_VERSION`, heartbeat interval, and session TTL are server-controlled values in `wrangler.toml`. The committed defaults are APB `1.5.0` and SM `1.0.0`.

## APB 2.0 release

Use [SECURITY_PROTOCOL_V2.md](../docs/SECURITY_PROTOCOL_V2.md) before distributing the 2.0 client. The new required bindings are `LICENSE_COORDINATOR`, private `ALGORITHM_ARTIFACTS`, and `ALGORITHM_RATE_LIMITER` (30/minute per product/license). New signing secret: `SERVER_SIGNING_KEY_PKCS8`; pin ID: `SERVER_SIGNING_KEY_ID`; immutable artifact selector: `APB_SMOOTHING_DIGEST`. Prepare the key and artifact through the maintainer scripts, never through the client. Do not release the signed-response-only client against the old unsigned Worker.

Current production version (2026-10-03): `c631ca22-c3bc-466c-9edb-4ce8ffa74327`. Signing key, private R2 artifact, coordinator bindings, and migration 0003 are live; `/health` and pinned signed-refusal checks passed. See the rollout record in [hardening progress](../docs/SECURITY_HARDENING_PROGRESS.md).

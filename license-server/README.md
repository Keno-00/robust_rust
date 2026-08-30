# Verux license server

This Cloudflare Worker is the authoritative online license service for Verux products. It verifies Payhip licenses server-side, stores product-scoped usage in D1, permits one active session per product license, and requires replay-resistant heartbeats.

## Required secrets

- `PAYHIP_PRODUCT_SECRET_APB`: Verux APB Payhip product secret. During migration, the existing `PAYHIP_PRODUCT_SECRET` is still accepted for APB only.
- `PAYHIP_PRODUCT_SECRET_SM`: Songmaker Payhip product secret. Do not create or configure this until Songmaker has its official Payhip listing.
- `LICENSE_HASH_SECRET`: At least 32 random bytes, used to key hashes of licenses, installs, and sessions.
- `DISCORD_WEBHOOK_URL`: Optional Discord webhook used for privacy-safe activation and expiry status messages. Rotate exposed webhook URLs before configuring this secret.

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
5. Run `npm run db:migrate:remote`.
6. Configure both required secrets.
7. Run `npm run deploy`.
8. Compile the Rust application with `VERUX_LICENSE_SERVER_URL` set to the deployed Worker origin.

`APP_ENABLED`, `MIN_CLIENT_VERSION`, heartbeat interval, and session TTL are server-controlled values in `wrangler.toml`.

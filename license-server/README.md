# Verux license server

This Cloudflare Worker is the authoritative online license service. It verifies Payhip licenses server-side, stores usage in D1, permits one active session per license, and requires replay-resistant heartbeats.

## Required secrets

- `PAYHIP_PRODUCT_SECRET`: Payhip product secret. Never place it in the client or in `wrangler.toml`.
- `LICENSE_HASH_SECRET`: At least 32 random bytes, used to key hashes of licenses, installs, and sessions.
- `DISCORD_WEBHOOK_URL`: Optional Discord webhook used for privacy-safe activation and expiry status messages. Rotate exposed webhook URLs before configuring this secret.

Set them using `npx wrangler secret put NAME`.

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

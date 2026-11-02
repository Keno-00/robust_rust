// This Worker has no HTTP handler or public route. Software publication remains operator-only.
interface RenewalEnv {
  RELEASE_ARTIFACTS: R2Bucket;
  RELEASE_PUBLIC_KEY: string;
  RELEASE_SIGNING_KEY_PKCS8: string;
  RENEWAL_DISCORD_WEBHOOK_URL?: string;
}

const encoder = new TextEncoder();
const prefix = encoder.encode("VERUX-RELEASE-CATALOG-V1\n");
const currentKey = "catalog/current.json";

function bytes(value: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(atob(value), c => c.charCodeAt(0));
}

function base64(value: Uint8Array): string {
  let result = "";
  for (const byte of value) result += String.fromCharCode(byte);
  return btoa(result);
}

function signedBytes(payload: Uint8Array): Uint8Array<ArrayBuffer> {
  const result = new Uint8Array(prefix.length + payload.length);
  result.set(prefix);
  result.set(payload, prefix.length);
  return result;
}

async function renew(env: RenewalEnv): Promise<void> {
  const object = await env.RELEASE_ARTIFACTS.get(currentKey);
  if (!object || object.size > 1024 * 1024) throw new Error("Current catalog missing or oversized");
  const envelope = JSON.parse(await object.text());
  if (!envelope || Object.keys(envelope).sort().join(",") !== "payload,signature" ||
      typeof envelope.payload !== "string" || typeof envelope.signature !== "string") {
    throw new Error("Invalid catalog envelope");
  }
  const publicKey = await crypto.subtle.importKey("raw", bytes(env.RELEASE_PUBLIC_KEY), "Ed25519", false, ["verify"]);
  const payload = bytes(envelope.payload);
  if (!await crypto.subtle.verify("Ed25519", publicKey, bytes(envelope.signature), signedBytes(payload))) {
    throw new Error("Current catalog signature rejected");
  }
  // Only a previously signed release is accepted. Keep its product/package metadata opaque and unchanged.
  const catalog = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(payload));
  const now = Math.floor(Date.now() / 1000);
  const ttl = catalog.expires_at - catalog.issued_at;
  if (Object.keys(catalog).sort().join(",") !== "banner,expires_at,issued_at,launcher,products,revision,schema" ||
      catalog.schema !== 1 || !Number.isSafeInteger(catalog.revision) || catalog.revision < 1 ||
      !Number.isSafeInteger(catalog.revision + 1) || !Number.isSafeInteger(catalog.issued_at) ||
      !Number.isSafeInteger(catalog.expires_at) || catalog.issued_at > now + 300 ||
      ttl < 86400 || ttl > 604800 || !Array.isArray(catalog.products)) {
    throw new Error("Catalog renewal contract rejected; operator intervention required");
  }
  if (now <= catalog.issued_at + 60) {
    console.log(JSON.stringify({ event: "CATALOG_RENEWAL_SKIPPED", revision: catalog.revision, reason: "recent_publication" }));
    return;
  }
  catalog.revision += 1;
  catalog.issued_at = now;
  catalog.expires_at = now + ttl;
  const fresh = encoder.encode(JSON.stringify(catalog));
  const privateKey = await crypto.subtle.importKey("pkcs8", bytes(env.RELEASE_SIGNING_KEY_PKCS8), "Ed25519", false, ["sign"]);
  const signature = await crypto.subtle.sign("Ed25519", privateKey, signedBytes(fresh));
  if (!await crypto.subtle.verify("Ed25519", publicKey, signature, signedBytes(fresh))) {
    throw new Error("Renewal key does not match installed launcher trust anchor");
  }
  const renewed = JSON.stringify({ payload: base64(fresh), signature: base64(new Uint8Array(signature)) });
  const digest = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", encoder.encode(renewed))), b => b.toString(16).padStart(2, "0")).join("");
  // A separate content-addressed archive avoids claiming a publisher's numeric history slot during a race.
  const archived = await env.RELEASE_ARTIFACTS.put(`catalog/renewals/${catalog.revision}-${digest}.json`, renewed, {
    onlyIf: { etagDoesNotMatch: "*" }, httpMetadata: { contentType: "application/json" },
  });
  if (!archived) throw new Error("Renewal archive already exists; inspect concurrent execution");
  const committed = await env.RELEASE_ARTIFACTS.put(currentKey, renewed, {
    onlyIf: { etagMatches: object.etag }, httpMetadata: { contentType: "application/json", cacheControl: "no-store" },
  });
  if (!committed) throw new Error("Catalog changed during renewal; newer publication retained");
  console.log(JSON.stringify({ event: "CATALOG_RENEWED", revision: catalog.revision, expires_at: catalog.expires_at }));
}

export default {
  async scheduled(_controller: ScheduledController, env: RenewalEnv): Promise<void> {
    try {
      await renew(env);
    } catch (error) {
      console.error("CATALOG_RENEWAL_FAILED", error instanceof Error ? error.message : "Unknown failure");
      if (env.RENEWAL_DISCORD_WEBHOOK_URL) {
        try {
          if (!/^https:\/\/discord\.com\/api\/webhooks\/\d{17,20}\/[A-Za-z0-9_-]+$/.test(env.RENEWAL_DISCORD_WEBHOOK_URL)) {
            throw new Error("Renewal Discord webhook configuration rejected");
          }
          const response = await fetch(env.RENEWAL_DISCORD_WEBHOOK_URL, {
            method: "POST", headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              content: `CATALOG RENEWAL FAILED\nVerux catalog needs attention. Check verux-catalog-renewal logs before the current catalog expires.\n${new Date().toISOString()}`,
              allowed_mentions: { parse: [] },
            }),
            signal: AbortSignal.timeout(5000),
          });
          await response.body?.cancel();
          if (!response.ok) throw new Error(`Discord rejected renewal alert (${response.status})`);
        } catch {
          console.error("CATALOG_RENEWAL_ALERT_FAILED: inspect Discord secret/configuration and delivery availability");
        }
      }
      throw error;
    }
  },
} satisfies ExportedHandler<RenewalEnv>;

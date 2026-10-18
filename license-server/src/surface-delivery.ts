import { CipherSuite, DhkemP256HkdfSha256, HkdfSha256, Aes256Gcm } from "@hpke/core";
import { decode64, encode64 } from "./signed-protocol";
import { RequestFailure } from "./request-security";

export interface SurfaceEnv {
  LICENSE_DB: D1Database;
  APB_SURFACE_KEY?: string;
  APB_SURFACE_DIGEST?: string;
  APB_SURFACE_OBJECTS_DIGEST?: string;
}

export async function deliverSurface(body: Record<string, unknown>, env: SurfaceEnv, sessionHash: string, expiresAt: number): Promise<Response> {
  const objects = body.algorithm_id === "verux.apb.surface.objects";
  const catalogId = objects ? "surface.objects" : "surface";
  const expectedDigest = objects ? env.APB_SURFACE_OBJECTS_DIGEST : env.APB_SURFACE_DIGEST;
  if (body.product_id !== "verux.apb" || (!objects && body.algorithm_id !== "verux.apb.surface") || body.abi_version !== 1
    || typeof body.recipient_key !== "string" || body.recipient_key.length > 100) {
    throw new RequestFailure(400, "invalid_surface", "Update the application and retry.");
  }
  if (!env.APB_SURFACE_KEY || !/^[a-f0-9]{64}$/.test(expectedDigest || "")) {
    throw new RequestFailure(503, "surface_not_configured", "Interface preparation is unavailable. Contact support.");
  }
  let key: Uint8Array;
  try { key = decode64(env.APB_SURFACE_KEY); }
  catch { throw new RequestFailure(503, "surface_key_invalid", "Interface preparation is unavailable. Contact support."); }
  try {
  let recipient: Uint8Array;
  try { recipient = decode64(body.recipient_key); }
  catch { throw new RequestFailure(400, "invalid_recipient", "Restart the application and retry."); }
  if (key.length !== 32) throw new RequestFailure(503, "surface_key_invalid", "Interface preparation is unavailable. Contact support.");
  if (recipient.length !== 65 || recipient[0] !== 4) throw new RequestFailure(400, "invalid_recipient", "Restart the application and retry.");
  const row = await env.LICENSE_DB.prepare("SELECT encrypted_rows, digest FROM surface_catalogs WHERE product_id=? AND catalog_id=? AND version=?")
    .bind("verux.apb", catalogId, "2.0.0").first<{encrypted_rows: string; digest: string}>();
  if (!row || row.encrypted_rows.length === 0 || row.encrypted_rows.length > 96 * 1024) {
    throw new RequestFailure(503, "surface_unavailable", "Interface preparation is unavailable. Contact support.");
  }
  const rowsBytes = new TextEncoder().encode(row.encrypted_rows);
  const digest = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", rowsBytes)), b => b.toString(16).padStart(2, "0")).join("");
  if (digest !== row.digest || digest !== expectedDigest) {
    throw new RequestFailure(503, "surface_integrity", "Interface preparation is unavailable. Contact support.");
  }
  let rows: unknown;
  try { rows = JSON.parse(row.encrypted_rows); }
  catch { throw new RequestFailure(503, "surface_schema", "Interface preparation is unavailable. Contact support."); }
  if (rows === null || typeof rows !== "object" || Array.isArray(rows)) throw new RequestFailure(503, "surface_schema", "Interface preparation is unavailable. Contact support.");
  // The server forwards encrypted rows; it never decrypts their display text.
  const bytes = new TextEncoder().encode(JSON.stringify({version: 1, key: encode64(key), rows}));
  try {
    const payloadDigest = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), b => b.toString(16).padStart(2, "0")).join("");
    const manifest = JSON.stringify({product_id: "verux.apb", algorithm_id: body.algorithm_id, artifact_version: "2.0.0",
      abi_version: 1, digest: payloadDigest, size: bytes.length, session_hash: sessionHash, recipient_key: body.recipient_key,
      request_nonce: body.request_nonce, expires_at: expiresAt});
    const suite = new CipherSuite({kem: new DhkemP256HkdfSha256(), kdf: new HkdfSha256(), aead: new Aes256Gcm()});
    const publicKey = await suite.kem.deserializePublicKey(recipient);
    const sender = await suite.createSenderContext({recipientPublicKey: publicKey, info: new TextEncoder().encode("verux-algorithm-v2")});
    const ciphertext = await sender.seal(bytes, new TextEncoder().encode(manifest));
    return Response.json({valid: true, status: "valid", message: "Ready.", manifest,
      encapsulated_key: encode64(new Uint8Array(sender.enc)), ciphertext: encode64(new Uint8Array(ciphertext))}, {headers: {"Cache-Control": "no-store"}});
  } finally { bytes.fill(0); }
  } finally { key.fill(0); }
}

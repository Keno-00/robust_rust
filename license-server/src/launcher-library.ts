import { boundedText, RequestFailure } from "./request-security";
import { decode64, encode64 } from "./signed-protocol";

export interface LibraryEnv {
  RELEASE_ARTIFACTS?: R2Bucket;
  LICENSE_DB: D1Database;
  LICENSE_HASH_SECRET: string;
  APP_ENABLED: string;
}
export interface LibraryProduct {
  id: string;
  access: "free" | "paid";
  purchase_url: string;
  directory: string;
  entropy: string;
}
export interface Notice { id: string; product_id: string | null; title: string; text: string }
export interface LibraryConfig { schema: number; products: LibraryProduct[]; notices: Notice[] }
export async function libraryConfig(env: LibraryEnv): Promise<LibraryConfig> {
  const object = await env.RELEASE_ARTIFACTS?.get("library/current.json");
  if (!object || object.size > 65536) throw new RequestFailure(503, "library_not_configured", "Library configuration is unavailable.");
  const c: LibraryConfig = JSON.parse(await boundedText(new Response(object.body), 65536));
  const ids = new Set<string>(), notices = new Set<string>();
  if (c.schema !== 1 || !Array.isArray(c.products) || c.products.length > 65 || !Array.isArray(c.notices) || c.notices.length > 32) throw new Error("Invalid library configuration");
  for (const p of c.products) {
    if (!/^[a-z0-9][a-z0-9.-]{0,63}$/.test(p.id) || ids.has(p.id) || !["free", "paid"].includes(p.access)) throw new Error("Invalid library product");
    ids.add(p.id);
    if (p.access === "paid" && (!/^https:\/\/[^\s]+$/.test(p.purchase_url) || !/^[A-Za-z0-9_-]{1,64}$/.test(p.directory) || !/^[a-z0-9-]{1,100}$/.test(p.entropy))) throw new Error("Invalid library license metadata");
    if (p.access === "paid") { const url = new URL(p.purchase_url); if (url.username || url.password || url.hash) throw new Error("Invalid purchase URL"); }
  }
  if (!c.products.some(p => p.id === "verux.launcher" && p.access === "free")) throw new Error("Launcher must remain freely updateable");
  for (const n of c.notices) {
    if (!/^[a-z0-9][a-z0-9.-]{0,63}$/.test(n.id) || notices.has(n.id) ||
        (n.product_id !== null && !ids.has(n.product_id)) || typeof n.title !== "string" || !n.title.trim() || n.title.length > 80 ||
        typeof n.text !== "string" || !n.text.trim() || n.text.length > 4000 || /[\x00-\x08\x0b\x0c\x0e-\x1f]/.test(n.text)) throw new Error("Invalid library notice");
    notices.add(n.id);
  }
  return c;
}

async function tokenKey(env: LibraryEnv, usage: KeyUsage[]): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", new TextEncoder().encode(env.LICENSE_HASH_SECRET), { name: "HMAC", hash: "SHA-256" }, false, usage);
}
export async function downloadGrant(env: LibraryEnv, product: string, license: string): Promise<string> {
  const bytes = new TextEncoder().encode(JSON.stringify({ product, license, expires: Math.floor(Date.now() / 1000) + 300 }));
  return encode64(bytes) + "." + encode64(new Uint8Array(await crypto.subtle.sign("HMAC", await tokenKey(env, ["sign"]), bytes)));
}
export async function authorizePackage(request: Request, env: LibraryEnv, product: string): Promise<boolean> {
  const p = (await libraryConfig(env)).products.find(p => p.id === product);
  if (!p) throw new RequestFailure(503, "product_access_not_configured", "Product access is unavailable.");
  if (p.access === "free") return true;
  if (env.APP_ENABLED !== "true") return false;
  const authorization = request.headers.get("Authorization") || "";
  if (authorization.length > 2048 || !authorization.startsWith("Bearer ")) return false;
  try {
    const parts = authorization.slice(7).split(".");
    if (parts.length !== 2) return false;
    const bytes = decode64(parts[0]);
    if (!await crypto.subtle.verify("HMAC", await tokenKey(env, ["verify"]), decode64(parts[1]), bytes)) return false;
    const grant = JSON.parse(new TextDecoder().decode(bytes)), now = Math.floor(Date.now() / 1000);
    if (grant.product !== product || !/^[a-f0-9]{64}$/.test(grant.license) || !Number.isSafeInteger(grant.expires) || grant.expires <= now || grant.expires > now + 300) return false;
    const row = await env.LICENSE_DB.prepare("SELECT status, used_seconds, entitlement_seconds FROM licenses WHERE product_id=? AND license_hash=?")
      .bind(product, grant.license).first<{ status: string; used_seconds: number; entitlement_seconds: number | null }>();
    return !!row && row.status === "active" && (row.entitlement_seconds === null || row.used_seconds < row.entitlement_seconds);
  } catch { return false; }
}

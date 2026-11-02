import { launcherRequest, LauncherEnv } from "./launcher-delivery";
import { libraryConfig, downloadGrant } from "./launcher-library";
import {
  boundedText, diagnostic, DISCORD_BODY_MAX_BYTES, isJsonContentType,
  readLicenseBody, RequestFailure, UPSTREAM_BODY_MAX_BYTES, UPSTREAM_TIMEOUT_MS,
  validClientVersion, validSessionToken,
} from "./request-security";
import { settleSessions, settleOne } from "./session-accounting";
import { requireSignedRequest, signResponse, SigningEnv } from "./signed-protocol";
import { DurableObject } from "cloudflare:workers";
import { AlgorithmEnv, deliverAlgorithm } from "./algorithm-delivery";
import { FirmwareEnv, deliverFirmware } from "./firmware-delivery";

export interface Env extends SigningEnv, AlgorithmEnv, FirmwareEnv, LauncherEnv {
  LICENSE_COORDINATOR: DurableObjectNamespace;
  LICENSE_DB: D1Database;
  ACTIVATION_RATE_LIMITER: RateLimit;
  SESSION_RATE_LIMITER: RateLimit;
  ALGORITHM_RATE_LIMITER: RateLimit;
  FIRMWARE_RATE_LIMITER: RateLimit;
  DISCORD_RATE_LIMITER: RateLimit;
  ACTIVATION_NETWORK_RATE_LIMITER: RateLimit;
  SESSION_NETWORK_RATE_LIMITER: RateLimit;
  DISCORD_NETWORK_RATE_LIMITER: RateLimit;
  PAYHIP_PRODUCT_SECRET?: string;
  PAYHIP_PRODUCT_SECRET_APB?: string;
  PAYHIP_PRODUCT_SECRET_APB_CB?: string;
  PAYHIP_PRODUCT_SECRET_SM?: string;
  LICENSE_HASH_SECRET: string;
  DISCORD_WEBHOOK_URL?: string;
  DISCORD_PUBLIC_KEY: string;
  DISCORD_APPLICATION_ID: string;
  APP_ENABLED: string;
  APB_PAYHIP_PRODUCT_NAMES: string;
  APB_CB_PAYHIP_PRODUCT_NAMES: string;
  SM_PAYHIP_PRODUCT_NAMES: string;
  APB_MIN_CLIENT_VERSION: string;
  APB_CB_MIN_CLIENT_VERSION: string;
  SM_MIN_CLIENT_VERSION: string;
  HEARTBEAT_INTERVAL_SECONDS: string;
  SESSION_TTL_SECONDS: string;
  DISCORD_ADMIN_USER_ID: string;
}

type JsonObject = Record<string, unknown>;

type Product = {
  id: "verux.apb" | "verux.apb.cb" | "verux.sm";
  displayName: string;
  payhipSecret: string;
  payhipProductNames: string;
  minimumClientVersion: string;
};

function requestedProduct(body: JsonObject, env: Env): Product | null {
  const id = stringAt(body, "product_id").toLowerCase();
  if (id === "verux.apb") return { id, displayName: "Verux APB", payhipSecret: env.PAYHIP_PRODUCT_SECRET_APB || env.PAYHIP_PRODUCT_SECRET || "", payhipProductNames: env.APB_PAYHIP_PRODUCT_NAMES, minimumClientVersion: env.APB_MIN_CLIENT_VERSION };
  if (id === "verux.apb.cb") return { id, displayName: "Verux APB Color", payhipSecret: env.PAYHIP_PRODUCT_SECRET_APB_CB || "", payhipProductNames: env.APB_CB_PAYHIP_PRODUCT_NAMES, minimumClientVersion: env.APB_CB_MIN_CLIENT_VERSION };
  if (id === "verux.sm") return { id, displayName: "Songmaker", payhipSecret: env.PAYHIP_PRODUCT_SECRET_SM || "", payhipProductNames: env.SM_PAYHIP_PRODUCT_NAMES, minimumClientVersion: env.SM_MIN_CLIENT_VERSION };
  return null;
}

const json = (body: JsonObject, status = 200): Response =>
  Response.json(body, {
    status,
    headers: {
      "Cache-Control": "no-store",
      "Content-Security-Policy": "default-src 'none'",
      "X-Content-Type-Options": "nosniff",
    },
  });

const nowSeconds = (): number => Math.floor(Date.now() / 1000);

async function hmacHex(secret: string, value: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(value));
  return [...new Uint8Array(signature)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function randomToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return btoa(String.fromCharCode(...bytes)).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

function hexBytes(value: string): Uint8Array | null {
  if (!/^[0-9a-f]+$/i.test(value) || value.length % 2 !== 0) return null;
  const bytes = new Uint8Array(value.length / 2);
  for (let index = 0; index < bytes.length; index++) {
    bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function asObject(value: unknown): JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as JsonObject : {};
}

function stringAt(source: JsonObject, ...keys: string[]): string {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return "";
}

function customerName(payload: JsonObject, source: JsonObject): string {
  const direct = stringAt(source, "customer_name", "buyer_name", "name", "full_name", "customer")
    || stringAt(payload, "customer_name", "buyer_name", "name", "full_name", "customer");
  if (direct) return direct;
  for (const value of [source.customer, source.buyer, source.user, payload.customer, payload.buyer, payload.user]) {
    const nested = stringAt(asObject(value), "name", "full_name", "display_name", "username");
    if (nested) return nested;
  }
  return "";
}

function customerEmail(payload: JsonObject, source: JsonObject): string {
  const direct = stringAt(source, "customer_email", "buyer_email", "email")
    || stringAt(payload, "customer_email", "buyer_email", "email");
  if (direct) return direct;
  for (const value of [source.customer, source.buyer, source.user, payload.customer, payload.buyer, payload.user]) {
    const nested = stringAt(asObject(value), "email");
    if (nested) return nested;
  }
  return "";
}

function normalizedProductName(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

function productName(payload: JsonObject, source: JsonObject): string {
  return stringAt(source, "product_name", "product", "plan_name")
    || stringAt(payload, "product_name", "product", "plan_name")
    || stringAt(asObject(source.product), "name", "title")
    || stringAt(asObject(payload.product), "name", "title");
}

function productAllowed(actual: string, configured: string): boolean {
  const normalizedActual = normalizedProductName(actual);
  if (!normalizedActual) return false;
  return configured
    .split(",")
    .map(normalizedProductName)
    .filter(Boolean)
    .includes(normalizedActual);
}

function inferTier(payload: JsonObject): { tier: string; seconds: number | null } | null {
  // Payhip v2's documented entitlement field. Customer names, emails, and
  // unrelated strings must never select a tier or increase entitlement.
  const text = stringAt(asObject(payload.data), "variant_name").toLowerCase();
  if (!text || text.length > 256) return null;
  const matches = [
    { pattern: /\b30[\s._-]*days?\b/, tier: "30.days", seconds: 30 * 86400 },
    { pattern: /\b7[\s._-]*days?\b/, tier: "7.days", seconds: 7 * 86400 },
    { pattern: /\b1[\s._-]*days?\b/, tier: "1.day", seconds: 86400 },
    { pattern: /\blifetime\b/, tier: "lifetime", seconds: null },
  ].filter((rule) => rule.pattern.test(text));
  if (matches.length !== 1) return null;
  const { tier, seconds } = matches[0];
  return { tier, seconds };
}

function versionAtLeast(actual: string, minimum: string): boolean {
  if (!validClientVersion(actual) || !validClientVersion(minimum)) return false;
  const parse = (version: string): number[] => version.split(".").map(Number);
  const left = parse(actual);
  const right = parse(minimum);
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    if ((left[i] || 0) !== (right[i] || 0)) return (left[i] || 0) > (right[i] || 0);
  }
  return true;
}

async function verifyDiscordRequest(request: Request, env: Env, rawBody: string): Promise<boolean> {
  if (!env.DISCORD_PUBLIC_KEY) return false;
  const signature = hexBytes(request.headers.get("X-Signature-Ed25519") || "");
  const timestamp = request.headers.get("X-Signature-Timestamp") || "";
  if (!signature || !/^\d{10,13}$/.test(timestamp)) return false;
  const timestampSeconds = Number(timestamp.length > 10 ? timestamp.slice(0, 10) : timestamp);
  if (!Number.isFinite(timestampSeconds) || Math.abs(nowSeconds() - timestampSeconds) > 300) return false;
  const publicKeyBytes = hexBytes(env.DISCORD_PUBLIC_KEY.trim());
  if (!publicKeyBytes || publicKeyBytes.length !== 32) return false;
  try {
    const key = await crypto.subtle.importKey("raw", publicKeyBytes, { name: "Ed25519" }, false, ["verify"]);
    return await crypto.subtle.verify(
      { name: "Ed25519" },
      key,
      signature,
      new TextEncoder().encode(timestamp + rawBody),
    );
  } catch {
    return false;
  }
}

function discordResponse(content: string, ephemeral = true): Response {
  return json({
    type: 4,
    data: {
      content: content.slice(0, 2000),
      flags: ephemeral ? 64 : 0,
      allowed_mentions: { parse: [] },
    },
  });
}

function interactionOption(interaction: JsonObject, name: string): string {
  const data = asObject(interaction.data);
  const options = Array.isArray(data.options) ? data.options : [];
  for (const option of options) {
    const item = asObject(option);
    if (item.name === name && typeof item.value === "string") return item.value.trim();
  }
  return "";
}

function utcTime(epoch: unknown): string {
  const seconds = Number(epoch);
  return Number.isFinite(seconds) ? new Date(seconds * 1000).toISOString().replace(".000Z", "Z") : "Unknown";
}

async function discordCommand(interaction: JsonObject, env: Env): Promise<string> {
  const command = stringAt(asObject(interaction.data), "name").toLowerCase();
  const now = nowSeconds();
  if (command === "service") {
    await env.LICENSE_DB.prepare("SELECT 1").first();
    return [
      "**License service**",
      `Status: ${env.APP_ENABLED === "true" ? "Online" : "Maintenance"}`,
      "Database: Online",
      `APB minimum client: ${env.APB_MIN_CLIENT_VERSION}`,
      `SM minimum client: ${env.SM_MIN_CLIENT_VERSION}`,
      `Heartbeat interval: ${env.HEARTBEAT_INTERVAL_SECONDS}s`,
      `Session TTL: ${env.SESSION_TTL_SECONDS}s`,
      `Checked: ${new Date().toISOString()}`,
    ].join("\n");
  }
  if (command === "security") {
    const events = await env.LICENSE_DB.prepare(`
      SELECT event_type,
        SUM(CASE WHEN created_at >= ? THEN 1 ELSE 0 END) AS hour_count,
        SUM(CASE WHEN created_at >= ? THEN 1 ELSE 0 END) AS day_count,
        COUNT(*) AS week_count
      FROM audit_events WHERE created_at >= ? GROUP BY event_type ORDER BY week_count DESC
    `).bind(now - 3600, now - 86400, now - 604800).all<Record<string, unknown>>();
    const unauthorized = await env.LICENSE_DB.prepare(`
      SELECT
        SUM(CASE WHEN created_at >= ? THEN 1 ELSE 0 END) AS hour_count,
        SUM(CASE WHEN created_at >= ? THEN 1 ELSE 0 END) AS day_count,
        COUNT(*) AS week_count
      FROM discord_interactions WHERE outcome = 'unauthorized' AND created_at >= ?
    `).bind(now - 3600, now - 86400, now - 604800).first<Record<string, unknown>>();
    const lines = ["**Security summary**", "Event â€” 1h / 24h / 7d"];
    for (const row of events.results || []) {
      lines.push(`${row.event_type} â€” ${row.hour_count || 0} / ${row.day_count || 0} / ${row.week_count || 0}`);
    }
    lines.push(`discord_unauthorized â€” ${unauthorized?.hour_count || 0} / ${unauthorized?.day_count || 0} / ${unauthorized?.week_count || 0}`);
    return lines.join("\n");
  }
  if (command === "audit") {
    const rows = await env.LICENSE_DB.prepare(`
      SELECT event_type, detail, created_at FROM audit_events ORDER BY created_at DESC LIMIT 15
    `).all<Record<string, unknown>>();
    const lines = ["**Recent license audit events**"];
    for (const row of rows.results || []) {
      lines.push(`${utcTime(row.created_at)} â€” ${row.event_type}${row.detail ? ` â€” ${String(row.detail).slice(0, 80)}` : ""}`);
    }
    return lines.length === 1 ? `${lines[0]}\nNo events recorded.` : lines.join("\n");
  }
  if (command === "licenses") {
    const rows = await env.LICENSE_DB.prepare(`
      SELECT tier, status, COUNT(*) AS count, SUM(used_seconds) AS used_seconds
      FROM licenses GROUP BY tier, status ORDER BY tier, status
    `).all<Record<string, unknown>>();
    const lines = ["**License summary**"];
    for (const row of rows.results || []) {
      lines.push(`${row.tier} / ${row.status}: ${row.count} â€” used ${formatDuration(Number(row.used_seconds || 0))}`);
    }
    return lines.length === 1 ? `${lines[0]}\nNo licenses recorded.` : lines.join("\n");
  }
  if (command === "license") {
    const query = interactionOption(interaction, "customer");
    if (query.length < 3 || query.length > 100) return "Provide at least 3 characters of a customer name or email.";
    const row = await env.LICENSE_DB.prepare(`
      SELECT product_id, customer_name, customer_email, product_name, tier, status, used_seconds, entitlement_seconds,
        last_verified_at, updated_at
      FROM licenses WHERE customer_name LIKE ? OR customer_email LIKE ?
      ORDER BY updated_at DESC LIMIT 1
    `).bind(`%${query}%`, `%${query}%`).first<Record<string, unknown>>();
    if (!row) return "No matching customer was found.";
    const entitlement = row.entitlement_seconds === null ? null : Number(row.entitlement_seconds);
    const used = Number(row.used_seconds || 0);
    return [
      "**License information**",
      `Product ID: ${row.product_id || "verux.apb"}`,
      `Customer: ${row.customer_name || "Unknown"}`,
      `Email: ${row.customer_email || "Unavailable"}`,
      `Product: ${row.product_name || "Unknown"}`,
      `Tier: ${row.tier}`,
      `Status: ${row.status}`,
      `Used: ${formatDuration(used)}`,
      `Remaining: ${formatDuration(entitlement === null ? null : entitlement - used)}`,
      `Last verified: ${utcTime(row.last_verified_at)}`,
      "License key: never displayed",
    ].join("\n");
  }
  if (command === "sessions") {
    const rows = await env.LICENSE_DB.prepare(`
      SELECT s.product_id, l.customer_name, l.tier, s.client_version, s.sequence, s.started_at, s.last_seen_at, s.expires_at
      FROM sessions s JOIN licenses l ON l.product_id = s.product_id AND l.license_hash = s.license_hash
      WHERE s.revoked_at IS NULL AND s.expires_at >= ? ORDER BY s.last_seen_at DESC LIMIT 20
    `).bind(now).all<Record<string, unknown>>();
    const lines = [`**Active sessions (${rows.results?.length || 0})**`];
    for (const row of rows.results || []) {
      lines.push(`${row.product_id || "verux.apb"} â€” ${row.customer_name || "Unknown"} â€” ${row.tier} â€” v${row.client_version} â€” heartbeat #${row.sequence} â€” seen ${utcTime(row.last_seen_at)}`);
    }
    return lines.join("\n");
  }
  if (command === "version") {
    const rows = await env.LICENSE_DB.prepare(`
      SELECT client_version, COUNT(*) AS count FROM sessions
      WHERE revoked_at IS NULL AND expires_at >= ? GROUP BY client_version ORDER BY count DESC
    `).bind(now).all<Record<string, unknown>>();
    const lines = [
      `**Client versions**`,
      `APB minimum supported: ${env.APB_MIN_CLIENT_VERSION}`,
      `SM minimum supported: ${env.SM_MIN_CLIENT_VERSION}`,
    ];
    for (const row of rows.results || []) lines.push(`v${row.client_version}: ${row.count} active`);
    return lines.join("\n");
  }
  return "Unknown command.";
}

async function handleDiscord(request: Request, env: Env): Promise<Response> {
  const rawBody = await boundedText(request, DISCORD_BODY_MAX_BYTES);
  if (!(await verifyDiscordRequest(request, env, rawBody))) return new Response("Invalid request signature", { status: 401 });
  let interaction: JsonObject;
  try {
    interaction = asObject(JSON.parse(rawBody));
  } catch {
    return new Response("Invalid JSON", { status: 400 });
  }
  if (interaction.type === 1) return json({ type: 1 });
  if (interaction.type !== 2) return discordResponse("Unsupported interaction.");
  const member = asObject(interaction.member);
  const actor = stringAt(asObject(member.user), "id") || stringAt(asObject(interaction.user), "id");
  const command = stringAt(asObject(interaction.data), "name").toLowerCase();
  const interactionId = stringAt(interaction, "id");
  if (!actor || !interactionId || !command) return discordResponse("Malformed interaction.");
  if (!(await env.DISCORD_RATE_LIMITER.limit({ key: actor })).success) return discordResponse("Command rate limit exceeded.");
  const actorHash = await hmacHex(env.LICENSE_HASH_SECRET, `discord:${actor}`);
  const authorized = actor === env.DISCORD_ADMIN_USER_ID;
  const inserted = await env.LICENSE_DB.prepare(`
    INSERT OR IGNORE INTO discord_interactions (interaction_id, actor_hash, command_name, outcome, created_at)
    VALUES (?, ?, ?, ?, ?)
  `).bind(interactionId, actorHash, command, authorized ? "accepted" : "unauthorized", nowSeconds()).run();
  if ((inserted.meta.changes || 0) === 0) return discordResponse("This interaction was already processed.");
  if (!authorized) return discordResponse("You are not authorized to use this application.");
  try {
    return discordResponse(await discordCommand(interaction, env));
  } catch {
    await env.LICENSE_DB.prepare("UPDATE discord_interactions SET outcome = 'error' WHERE interaction_id = ?").bind(interactionId).run();
    return discordResponse("The command failed safely. Check the Worker logs.");
  }
}

function networkPrefix(request: Request): string {
  const address = request.headers.get("CF-Connecting-IP") || "";
  if (address.includes(":")) return address.split(":").slice(0, 4).join(":");
  return address.split(".").slice(0, 3).join(".");
}

async function audit(env: Env, request: Request, productId: string, type: string, licenseHash = "", installHash = "", detail = ""): Promise<void> {
  await env.LICENSE_DB.prepare(
    "INSERT INTO audit_events (product_id, license_hash, install_hash, event_type, ip_prefix, detail, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
  ).bind(productId, licenseHash || null, installHash || null, type, networkPrefix(request), detail.slice(0, 500), nowSeconds()).run();
}

function formatDuration(totalSeconds: number | null): string {
  if (totalSeconds === null) return "Lifetime";
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const parts: string[] = [];
  if (days) parts.push(`${days}d`);
  if (hours) parts.push(`${hours}h`);
  if (minutes || parts.length === 0) parts.push(`${minutes}m`);
  return parts.join(" ");
}

async function notifyDiscord(env: Env, lines: string[]): Promise<void> {
  if (!env.DISCORD_WEBHOOK_URL) return;
  const safe = (value: string): string => value.replaceAll("@", "ï¼ ").slice(0, 1000);
  const response = await fetch(env.DISCORD_WEBHOOK_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      username: "Verux License Service",
      allowed_mentions: { parse: [] },
      content: lines.map(safe).join("\n").slice(0, 2000),
    }),
  });
  if (!response.ok) throw new Error(`Discord webhook returned HTTP ${response.status}`);
}

function publicLicense(row: Record<string, unknown>) {
  const entitlement = row.entitlement_seconds === null ? null : Number(row.entitlement_seconds);
  const used = Number(row.used_seconds || 0);
  return {
    tier: String(row.tier),
    used_seconds: used,
    entitlement_seconds: entitlement,
    remaining_seconds: entitlement === null ? null : Math.max(0, entitlement - used),
  };
}

async function activate(request: Request, env: Env, ctx: ExecutionContext, library = false): Promise<Response> {
  const body = await readLicenseBody(request);
  const product = requestedProduct(body, env);
  if (!product) return json({ valid: false, status: "invalid_product", message: "A supported product_id is required." }, 400);
  if (library && !(await libraryConfig(env)).products.some(p => p.id === product.id && p.access === "paid")) {
    throw new RequestFailure(400,"invalid_library_product","This product is not enrolled in the library.");
  }
  const licenseKey = stringAt(body, "license_key").trim();
  const installId = stringAt(body, "install_id");
  const clientVersion = stringAt(body, "client_version");
  if (!licenseKey || licenseKey.length > 512 || !validSessionToken(installId) || !validClientVersion(clientVersion)) {
    return json({ valid: false, status: "invalid_request", message: "Missing or invalid activation fields." }, 400);
  }
  const activationHash = await hmacHex(env.LICENSE_HASH_SECRET, `activation:${product.id}:${licenseKey}`);
  if (!(await env.ACTIVATION_RATE_LIMITER.limit({ key: activationHash })).success) {
    diagnostic("activation_rate_limited", "/v1/activate");
    return json({ valid: false, status: "rate_limited", message: "Too many activation attempts." }, 429);
  }
  if (env.APP_ENABLED !== "true") return json({ valid: false, status: "maintenance", message: "Service is temporarily unavailable." }, 503);
  if (!versionAtLeast(clientVersion, product.minimumClientVersion)) {
    return json({ valid: false, status: "update_required", message: "This application version is no longer supported." }, 426);
  }
  if (!product.payhipSecret || !product.payhipProductNames) {
    return json({ valid: false, status: "product_not_configured", message: "Licensing is not configured for this product." }, 503);
  }

  const payhipUrl = new URL("https://payhip.com/api/v2/license/verify");
  payhipUrl.searchParams.set("license_key", licenseKey);
  let payhipResponse: Response;
  let payhipPayload: JsonObject;
  let upstreamStage: "request" | "body" | "json" = "request";
  const upstreamAbort = new AbortController();
  const upstreamTimer = setTimeout(() => upstreamAbort.abort(), UPSTREAM_TIMEOUT_MS);
  try {
    payhipResponse = await fetch(payhipUrl, {
      headers: { Accept: "application/json", "product-secret-key": product.payhipSecret },
      redirect: "manual",
      signal: upstreamAbort.signal,
    });
    if (payhipResponse.status >= 300 && payhipResponse.status < 400) {
      throw new RequestFailure(503, "upstream_redirect_rejected", "License provider is unavailable.");
    }
    if (payhipResponse.status === 429 || payhipResponse.status >= 500) {
      throw new RequestFailure(503, `upstream_http_${payhipResponse.status}`, "License provider is unavailable.");
    }
    if (!payhipResponse.ok) {
      await audit(env, request, product.id, "activation_denied");
      return json({ valid: false, status: "invalid_key", message: "Invalid or disabled license key." }, 401);
    }
    if (!isJsonContentType(payhipResponse.headers)) throw new RequestFailure(503, "upstream_content_type", "License provider is unavailable.");
    upstreamStage = "body";
    const upstreamText = await boundedText(payhipResponse, UPSTREAM_BODY_MAX_BYTES);
    upstreamStage = "json";
    const raw: unknown = JSON.parse(upstreamText);
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) throw new RequestFailure(503, "upstream_shape", "License provider is unavailable.");
    payhipPayload = raw as JsonObject;
  } catch (error) {
    const code = upstreamAbort.signal.aborted ? "upstream_timeout"
      : error instanceof RequestFailure ? error.code
      : upstreamStage === "request" ? "upstream_transport_failed"
      : upstreamStage === "json" ? "upstream_invalid_json" : "upstream_body_failed";
    diagnostic(code, "/v1/activate");
    if (code === "upstream_transport_failed") {
      // Never log an exception verbatim: fetch errors may contain the license URL.
      const message = error instanceof Error ? error.message.toLowerCase() : "";
      const reason = message.includes("redirect") ? "redirect"
        : message.includes("header") ? "header"
        : message.includes("signal") ? "signal"
        : message.includes("dns") ? "dns"
        : message.includes("ssl") || message.includes("tls") ? "tls"
        : message.includes("network") ? "network" : "unclassified";
      diagnostic(`upstream_transport_${reason}`, "/v1/activate");
    }
    return json({ valid: false, status: "upstream_unavailable", message: "License provider is unavailable." }, 503);
  } finally {
    clearTimeout(upstreamTimer);
  }
  const data = asObject(payhipPayload.data);
  if (data.enabled !== true || stringAt(data, "license_key") !== licenseKey) {
    await audit(env, request, product.id, "activation_denied");
    return json({ valid: false, status: "invalid_key", message: "Invalid or disabled license key." }, 401);
  }
  const tier = inferTier(payhipPayload);
  if (!tier) {
    diagnostic("unknown_payhip_variant", "/v1/activate");
    return json({ valid: false, status: "unknown_tier", message: "This license has an unsupported product variation. Contact support." }, 403);
  }

  let licenseHash = await hmacHex(env.LICENSE_HASH_SECRET, `license:${product.id}:${licenseKey.trim()}`);
  const installHash = await hmacHex(env.LICENSE_HASH_SECRET, `install:${product.id}:${installId}`);
  const now = Date.now() / 1000;
  let existing = await env.LICENSE_DB.prepare("SELECT * FROM licenses WHERE product_id = ? AND license_hash = ?").bind(product.id, licenseHash).first<Record<string, unknown>>();
  if (!existing && product.id === "verux.apb") {
    const legacyLicenseHash = await hmacHex(env.LICENSE_HASH_SECRET, `license:${licenseKey.trim()}`);
    existing = await env.LICENSE_DB.prepare("SELECT * FROM licenses WHERE product_id = ? AND license_hash = ?").bind(product.id, legacyLicenseHash).first<Record<string, unknown>>();
    if (existing) licenseHash = legacyLicenseHash;
  }
  if (existing?.status === "revoked") return json({ valid: false, status: "revoked", message: "This license has been revoked." }, 403);

  const source = Object.keys(data).length ? data : payhipPayload;
  const verifiedProductName = productName(payhipPayload, source);
  if (!productAllowed(verifiedProductName, product.payhipProductNames)) {
    await audit(env, request, product.id, "activation_wrong_product");
    return json({ valid: false, status: "wrong_product", message: "This license is not valid for the requested product." }, 403);
  }
  // A launcher library check creates no session, so it must not revoke the
  // product's live session; it settles only sessions that already expired.
  await settleSessions(env.LICENSE_DB, product.id, licenseHash, now, library);
  const purchasedAt = existing ? Number(existing.purchased_at) : now;
  await env.LICENSE_DB.prepare(`
    INSERT INTO licenses (product_id, license_hash, payhip_license_id, tier, purchased_at, entitlement_seconds, used_seconds, status,
      customer_name, customer_email, product_name, last_verified_at, created_at, updated_at)
    VALUES (?, ?, ?, ?, ?, ?, 0, 'active', ?, ?, ?, ?, ?, ?)
    ON CONFLICT(license_hash) DO UPDATE SET payhip_license_id=excluded.payhip_license_id, tier=excluded.tier,
      entitlement_seconds=excluded.entitlement_seconds, customer_name=excluded.customer_name,
      customer_email=excluded.customer_email, product_name=excluded.product_name,
      last_verified_at=excluded.last_verified_at, updated_at=excluded.updated_at
  `).bind(
    product.id, licenseHash, stringAt(source, "id", "license_id"), tier.tier, purchasedAt, tier.seconds,
    customerName(payhipPayload, source), customerEmail(payhipPayload, source),
    verifiedProductName, now, now, now,
  ).run();
  const license = await env.LICENSE_DB.prepare("SELECT * FROM licenses WHERE product_id = ? AND license_hash = ?").bind(product.id, licenseHash).first<Record<string, unknown>>();
  if (!license) return json({ valid: false, status: "server_error", message: "Unable to create license state." }, 500);
  if (license.status !== "active") return json({ valid: false, status: "revoked", message: "This license has been revoked." }, 403);
  const usage = publicLicense(license);
  if (usage.remaining_seconds !== null && usage.remaining_seconds < 1) {
    if (library) {
      await env.LICENSE_DB.prepare("INSERT OR IGNORE INTO launcher_license_expiry (product_id, license_hash, expired_at) VALUES (?, ?, ?)")
        .bind(product.id, licenseHash, Math.floor(now)).run();
      const expiry = await env.LICENSE_DB.prepare("SELECT expired_at FROM launcher_license_expiry WHERE product_id=? AND license_hash=?")
        .bind(product.id, licenseHash).first<{expired_at: number}>();
      if (!expiry) throw new Error("License expiry record unavailable");
      return json({valid:false, status:"usage_expired", message:"EXPIRED", expired_at:expiry.expired_at, hide_at:expiry.expired_at + 432000},403);
    }
    return json({ valid: false, status: "usage_expired", message: "License usage time has been fully consumed.", usage }, 403);
  }

  if (library) {
    const policy = (await libraryConfig(env)).products.find(p => p.id === product.id);
    if (!policy || policy.access !== "paid") throw new Error("Paid library product not configured");
    await env.LICENSE_DB.prepare("DELETE FROM launcher_license_expiry WHERE product_id=? AND license_hash=?").bind(product.id,licenseHash).run();
    await audit(env, request, product.id, "library_verified", licenseHash, installHash);
    return json({valid:true,status:"valid",message:"VERIFIED",download_token:await downloadGrant(env,product.id,licenseHash)});
  }

  const sessionToken = randomToken();
  const sessionHash = await hmacHex(env.LICENSE_HASH_SECRET, `session:${product.id}:${sessionToken}`);
  const ttl = Math.min(Number.parseInt(env.SESSION_TTL_SECONDS, 10), usage.remaining_seconds === null ? Infinity : Math.floor(usage.remaining_seconds));
  await env.LICENSE_DB.prepare(`
    INSERT INTO sessions (product_id, session_hash, license_hash, install_hash, client_version, sequence, started_at, last_seen_at, expires_at)
    VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)
  `).bind(product.id, sessionHash, licenseHash, installHash, clientVersion, now, now, now + ttl).run();
  await audit(env, request, product.id, "activation_granted", licenseHash, installHash);
  ctx.waitUntil(notifyDiscord(env, [
    `Software user ${String(license.customer_email || "Email unavailable")} logged in`,
    `Software: ${product.displayName}`,
    `Tier: ${tier.tier}`,
    `Time used: ${formatDuration(usage.used_seconds)}`,
    `Time remaining: ${formatDuration(usage.remaining_seconds)}`,
    `Client: ${clientVersion}`,
    `Country (IP-based): ${request.cf?.country ? String(request.cf.country) : "Unavailable"}`,
    `Region (IP-based): ${request.cf?.region ? String(request.cf.region) : "Unavailable"}`,
    `Event time: ${new Date().toISOString()}`,
  ]).catch(() => undefined));

  return json({
    valid: true,
    status: "valid",
    message: "License verified.",
    session_token: sessionToken,
    session_hash: sessionHash,
    heartbeat_interval_seconds: Number.parseInt(env.HEARTBEAT_INTERVAL_SECONDS, 10),
    session_ttl_seconds: ttl,
    user: {
      name: String(license.customer_name || ""), email: String(license.customer_email || ""), expiry: "",
      status: "active", product_name: String(license.product_name || ""), bought_at: "", license_key: "",
    },
    usage,
  });
}

async function heartbeat(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  const body = await readLicenseBody(request);
  const product = requestedProduct(body, env);
  if (!product) return json({ valid: false, status: "invalid_product", message: "A supported product_id is required." }, 400);
  const token = stringAt(body, "session_token");
  const sequence = typeof body.sequence === "number" ? body.sequence : -1;
  if (!validSessionToken(token) || !Number.isSafeInteger(sequence) || sequence < 1) return json({ valid: false, status: "invalid_request", message: "Invalid heartbeat." }, 400);
  const sessionHash = await hmacHex(env.LICENSE_HASH_SECRET, `session:${product.id}:${token}`);
  if (!(await env.SESSION_RATE_LIMITER.limit({ key: sessionHash })).success) {
    diagnostic("session_rate_limited", "/v1/heartbeat");
    return json({ valid: false, status: "rate_limited", message: "Too many session requests." }, 429);
  }
  if (env.APP_ENABLED !== "true") return json({ valid: false, status: "maintenance", message: "Service is temporarily unavailable." }, 503);

  const session = await env.LICENSE_DB.prepare(`
    SELECT s.*, l.tier, l.used_seconds, l.entitlement_seconds, l.status AS license_status,
      l.customer_name, l.customer_email, l.product_name
    FROM sessions s JOIN licenses l ON l.product_id = s.product_id AND l.license_hash = s.license_hash
    WHERE s.product_id = ? AND s.session_hash = ?
  `).bind(product.id, sessionHash).first<Record<string, unknown>>();
  const now = Date.now() / 1000;
  if (!session || session.revoked_at !== null || Number(session.expires_at) <= now || session.license_status !== "active") {
    if (session && session.revoked_at === null) await settleOne(env.LICENSE_DB, product.id, sessionHash, now);
    return json({ valid: false, status: "session_expired", message: "Session expired. Please log in again." }, 401);
  }
  if (sequence !== Number(session.sequence) + 1) {
    await audit(env, request, product.id, "heartbeat_replay", String(session.license_hash), String(session.install_hash));
    return json({ valid: false, status: "replay_detected", message: "Invalid session sequence." }, 409);
  }
  const elapsed = Math.max(0, Math.min(now - Number(session.last_seen_at), Number.parseInt(env.SESSION_TTL_SECONDS, 10)));
  const entitlement = session.entitlement_seconds === null ? null : Number(session.entitlement_seconds);
  const newUsed = Number(session.used_seconds) + elapsed;
  const remaining = entitlement === null ? Infinity : Math.max(0, entitlement - newUsed);
  const ttl = Math.min(Number.parseInt(env.SESSION_TTL_SECONDS, 10), Math.floor(remaining));
  await env.LICENSE_DB.batch([
    env.LICENSE_DB.prepare("UPDATE licenses SET used_seconds = used_seconds + ?, updated_at = ? WHERE product_id = ? AND license_hash = ?").bind(elapsed, now, product.id, session.license_hash),
    env.LICENSE_DB.prepare("UPDATE sessions SET sequence = ?, last_seen_at = ?, expires_at = ? WHERE product_id = ? AND session_hash = ?").bind(sequence, now, now + ttl, product.id, sessionHash),
  ]);
  const usage = {
    tier: String(session.tier), used_seconds: newUsed, entitlement_seconds: entitlement,
    remaining_seconds: entitlement === null ? null : Math.max(0, entitlement - newUsed),
  };
  if (usage.remaining_seconds !== null && usage.remaining_seconds < 1) {
    await env.LICENSE_DB.prepare("UPDATE sessions SET revoked_at = ? WHERE product_id = ? AND session_hash = ?").bind(now, product.id, sessionHash).run();
    ctx.waitUntil(notifyDiscord(env, [
      `Software user ${String(session.customer_email || "Email unavailable")} license expired`,
      `Software: ${product.displayName}`,
      `Tier: ${String(session.tier)}`,
      `Time used: ${formatDuration(newUsed)}`,
      "Time remaining: 0m",
      `Event time: ${new Date().toISOString()}`,
    ]).catch(() => undefined));
    return json({ valid: false, status: "usage_expired", message: "License usage time has been fully consumed.", usage }, 403);
  }
  return json({ valid: true, status: "valid", message: "License active.", usage, session_ttl_seconds: ttl, session_hash: sessionHash });
}

async function deactivate(request: Request, env: Env): Promise<Response> {
  const body = await readLicenseBody(request);
  const product = requestedProduct(body, env);
  if (!product) return json({ valid: false, status: "invalid_product", message: "A supported product_id is required." }, 400);
  const token = stringAt(body, "session_token");
  if (!validSessionToken(token)) return json({ valid: false, status: "invalid_request", message: "Invalid session token." }, 400);
  const sessionHash = await hmacHex(env.LICENSE_HASH_SECRET, `session:${product.id}:${token}`);
  if (!(await env.SESSION_RATE_LIMITER.limit({ key: sessionHash })).success) {
    diagnostic("session_rate_limited", "/v1/deactivate");
    return json({ valid: false, status: "rate_limited", message: "Too many session requests." }, 429);
  }
  await settleOne(env.LICENSE_DB, product.id, sessionHash, Date.now() / 1000);
  return json({ valid: true, status: "deactivated", message: "Session closed." });
}

export class LicenseCoordinator extends DurableObject<Env> {
  async fetch(request: Request): Promise<Response> {
    return this.ctx.blockConcurrencyWhile(async () => {
      try {
        const body = await readLicenseBody(request.clone());
        const product = requestedProduct(body, this.env);
        if (!product) throw new RequestFailure(400, "invalid_product", "A supported product_id is required.");
        const route = new URL(request.url).pathname;
        let licenseHash = "";
        if (route === "/v1/activate" || route === "/v2/library/verify") {
          const key = stringAt(body, "license_key").trim();
          licenseHash = await hmacHex(this.env.LICENSE_HASH_SECRET, `license:${product.id}:${key}`);
          if (product.id === "verux.apb") {
            const legacy = await hmacHex(this.env.LICENSE_HASH_SECRET, `license:${key}`);
            const row = await this.env.LICENSE_DB.prepare("SELECT license_hash FROM licenses WHERE product_id = ? AND license_hash IN (?, ?) ORDER BY license_hash = ? DESC LIMIT 1")
              .bind(product.id, licenseHash, legacy, licenseHash).first<{ license_hash: string }>();
            if (row) licenseHash = row.license_hash;
          }
        } else {
          const hash = await hmacHex(this.env.LICENSE_HASH_SECRET, `session:${product.id}:${stringAt(body, "session_token")}`);
          const session = await this.env.LICENSE_DB.prepare("SELECT license_hash FROM sessions WHERE product_id = ? AND session_hash = ?")
            .bind(product.id, hash).first<{ license_hash: string }>();
          licenseHash = session?.license_hash || "";
        }
        if (licenseHash) await this.ctx.storage.put("account", { product: product.id, license: licenseHash });
        // Establish the expiry alarm before mutation; a later failure must not
        // leave a successfully-created session without an expiry settlement.
        const proposedAlarm = Date.now() + Number(this.env.SESSION_TTL_SECONDS) * 1000 + 1000;
        const existingAlarm = await this.ctx.storage.getAlarm();
        await this.ctx.storage.setAlarm(existingAlarm === null ? proposedAlarm : Math.min(existingAlarm, proposedAlarm));
        let response: Response;
        if (route === "/v2/algorithms/prepare" || route === "/v2/firmware/prepare") {
          requireSignedRequest(body, this.env);
          const deliveryLimiter = route === "/v2/firmware/prepare" ? this.env.FIRMWARE_RATE_LIMITER : this.env.ALGORITHM_RATE_LIMITER;
          if (!(await deliveryLimiter.limit({ key: `${product.id}:${licenseHash}` })).success) {
            return json({ valid: false, status: "rate_limited", message: route === "/v2/firmware/prepare" ? "Too many Install attempts. Wait a minute and retry." : "Too many Start attempts. Wait a minute and retry." }, 429);
          }
          const tokenHash = await hmacHex(this.env.LICENSE_HASH_SECRET, `session:${product.id}:${stringAt(body, "session_token")}`);
          const session = await this.env.LICENSE_DB.prepare(`SELECT s.*, l.status AS license_status, l.entitlement_seconds, l.used_seconds
            FROM sessions s JOIN licenses l ON l.product_id=s.product_id AND l.license_hash=s.license_hash
            WHERE s.product_id=? AND s.session_hash=?`).bind(product.id, tokenHash).first<Record<string, unknown>>();
          const now = Date.now() / 1000;
          if (!session || session.revoked_at !== null || Number(session.expires_at) <= now || session.license_status !== "active"
            || this.env.APP_ENABLED !== "true" || (session.entitlement_seconds !== null
              && Number(session.used_seconds) + Math.max(0, now - Number(session.last_seen_at)) >= Number(session.entitlement_seconds))) {
            response = json({ valid: false, status: "session_expired", message: "Session expired. Please log in again." }, 401);
          } else {
            response = route === "/v2/firmware/prepare"
              ? await deliverFirmware(body, this.env, tokenHash, Number(session.expires_at))
              : await deliverAlgorithm(body, this.env, tokenHash, Number(session.expires_at));
          }
        } else response = route === "/v2/library/verify" ? await activate(request, this.env, this.ctx as unknown as ExecutionContext, true)
          : route === "/v1/activate" ? await activate(request, this.env, this.ctx as unknown as ExecutionContext)
          : route === "/v1/heartbeat" ? await heartbeat(request, this.env, this.ctx as unknown as ExecutionContext)
          : await deactivate(request, this.env);
        return response;
      } catch (error) {
        const failure = error instanceof RequestFailure ? error
          : new RequestFailure(503, "coordinator_error", "License service is unavailable. Please retry or contact support.");
        diagnostic(failure.code, new URL(request.url).pathname);
        return json({ valid: false, status: failure.code, message: failure.message }, failure.status);
      }
    });
  }

  async alarm(): Promise<void> {
    await this.ctx.blockConcurrencyWhile(async () => {
      const account = await this.ctx.storage.get<{ product: string; license: string }>("account");
      if (!account) return;
      await settleSessions(this.env.LICENSE_DB, account.product, account.license, Date.now() / 1000, true);
      const next = await this.env.LICENSE_DB.prepare("SELECT MIN(expires_at) AS expiry FROM sessions WHERE product_id = ? AND license_hash = ? AND revoked_at IS NULL")
        .bind(account.product, account.license).first<{ expiry: number | null }>();
      if (next?.expiry != null) await this.ctx.storage.setAlarm(Math.max(Date.now() + 1000, next.expiry * 1000 + 1000));
    });
  }
}

async function coordinated(request: Request, env: Env): Promise<Response> {
  const body = await readLicenseBody(request.clone());
  const product = requestedProduct(body, env);
  if (!product) return json({ valid: false, status: "invalid_product", message: "A supported product_id is required." }, 400);
  const route = new URL(request.url).pathname;
  const signed = body.protocol_version === 2;
  if (route === "/v2/library/verify") requireSignedRequest(body, env);
  if (signed) requireSignedRequest(body, env);
  // Validate before hashing/lookup/coordination so untrusted tokens cannot create
  // arbitrarily many Durable Objects. IP ceilings have already run at the edge.
  let licenseHash: string;
  if (route === "/v1/activate" || route === "/v2/library/verify") {
    const key = stringAt(body, "license_key").trim();
    if (!key || key.length > 512 || !validSessionToken(stringAt(body, "install_id")) || !validClientVersion(stringAt(body, "client_version"))) {
      throw new RequestFailure(400, "invalid_request", "Missing or invalid activation fields.");
    }
    // Verify Payhip before allocating new objects is a later operational gate;
    // current object allocation is bounded by the edge activation ceiling.
    licenseHash = await hmacHex(env.LICENSE_HASH_SECRET, `license:${product.id}:${key}`);
    if (product.id === "verux.apb") {
      const legacy = await hmacHex(env.LICENSE_HASH_SECRET, `license:${key}`);
      const row = await env.LICENSE_DB.prepare("SELECT license_hash FROM licenses WHERE product_id = ? AND license_hash IN (?, ?) ORDER BY license_hash = ? DESC LIMIT 1")
        .bind(product.id, licenseHash, legacy, licenseHash).first<{ license_hash: string }>();
      if (row) licenseHash = row.license_hash;
    }
  } else {
    const token = stringAt(body, "session_token");
    if (!validSessionToken(token)) throw new RequestFailure(400, "invalid_request", "Invalid session token.");
    const hash = await hmacHex(env.LICENSE_HASH_SECRET, `session:${product.id}:${token}`);
    const row = await env.LICENSE_DB.prepare("SELECT license_hash FROM sessions WHERE product_id = ? AND session_hash = ?")
      .bind(product.id, hash).first<{ license_hash: string }>();
    if (!row) {
      const failure = json({ valid: false, status: "session_expired", message: "Session expired. Please log in again." }, 401);
      return signed ? signResponse(failure, body, env, route) : failure;
    }
    licenseHash = row.license_hash;
  }
  const id = env.LICENSE_COORDINATOR.idFromName(`${product.id}:${licenseHash}`);
  const response = await env.LICENSE_COORDINATOR.get(id).fetch(request);
  return signed ? signResponse(response, body, env, route) : response;
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/launcher/")) return launcherRequest(request, env);
    if (request.method === "GET" && url.pathname === "/health") return json({ status: "ok" });
    if (request.method !== "POST") return json({ valid: false, status: "not_found", message: "Not found." }, 404);
    try {
      const limiter = url.pathname === "/v1/activate" || url.pathname === "/v2/library/verify" || url.pathname === "/v2/library/policy" ? env.ACTIVATION_NETWORK_RATE_LIMITER
        : url.pathname === "/v1/heartbeat" || url.pathname === "/v1/deactivate" || url.pathname === "/v2/algorithms/prepare" || url.pathname === "/v2/firmware/prepare" ? env.SESSION_NETWORK_RATE_LIMITER
        : url.pathname === "/discord/interactions" ? env.DISCORD_NETWORK_RATE_LIMITER : null;
      if (!limiter) return json({ valid: false, status: "not_found", message: "Not found." }, 404);
      // Cloudflare overwrites this header at the edge. Never trust forwarded-IP
      // headers supplied by callers. A shared missing-IP bucket is fail-closed.
      const networkKey = request.headers.get("CF-Connecting-IP") || "missing-edge-ip";
      if (!(await limiter.limit({ key: networkKey })).success) {
        diagnostic("network_rate_limited", url.pathname);
        return json({ valid: false, status: "rate_limited", message: "Too many requests. Please try again later." }, 429);
      }
      if (url.pathname === "/discord/interactions") return await handleDiscord(request, env);
      if (url.pathname === "/v2/library/policy") {
        const body = await readLicenseBody(request);
        requireSignedRequest(body, env);
        if (body.product_id !== "verux.launcher") throw new RequestFailure(400,"invalid_product","Invalid library request.");
        return signResponse(json({valid:true,status:"valid",library:await libraryConfig(env)}),body,env,url.pathname);
      }
      const interval = Number(env.HEARTBEAT_INTERVAL_SECONDS);
      const ttl = Number(env.SESSION_TTL_SECONDS);
      if (!Number.isSafeInteger(interval) || interval < 15 || interval > 300
        || !Number.isSafeInteger(ttl) || ttl <= interval || ttl > 3600
        || !env.LICENSE_HASH_SECRET || env.LICENSE_HASH_SECRET.length < 32
        || !validClientVersion(env.APB_MIN_CLIENT_VERSION) || !validClientVersion(env.SM_MIN_CLIENT_VERSION)) {
        throw new RequestFailure(503, "invalid_server_configuration", "License service is unavailable. Contact support.");
      }
      return await coordinated(request, env);
    } catch (error) {
      const failure = error instanceof RequestFailure ? error
        : new RequestFailure(503, "server_error", "License service is unavailable. Please try again later.");
      diagnostic(failure.code, url.pathname);
      return json({ valid: false, status: failure.code, message: failure.message }, failure.status);
    }
  },
} satisfies ExportedHandler<Env>;

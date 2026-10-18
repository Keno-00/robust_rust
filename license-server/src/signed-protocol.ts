import { RequestFailure, validSessionToken } from "./request-security";

export interface SigningEnv {
  SERVER_SIGNING_KEY_PKCS8?: string;
  SERVER_SIGNING_KEY_ID: string;
}

export function encode64(bytes: Uint8Array): string {
  let text = "";
  for (const byte of bytes) text += String.fromCharCode(byte);
  return btoa(text).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

export function decode64(text: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]+$/.test(text)) throw new Error("invalid_base64");
  const raw = atob(text.replaceAll("-", "+").replaceAll("_", "/"));
  const bytes = Uint8Array.from(raw, (c) => c.charCodeAt(0));
  if (encode64(bytes) !== text) throw new Error("noncanonical_base64");
  return bytes;
}

export function requireSignedRequest(body: Record<string, unknown>, env: SigningEnv): string {
  if (body.protocol_version !== 2 || typeof body.request_nonce !== "string" || !validSessionToken(body.request_nonce)) {
    throw new RequestFailure(400, "invalid_protocol", "Update the application and try again.");
  }
  if (!env.SERVER_SIGNING_KEY_PKCS8 || !/^[a-z0-9-]{1,64}$/.test(env.SERVER_SIGNING_KEY_ID)) {
    throw new RequestFailure(503, "signing_not_configured", "License service requires configuration. Contact support.");
  }
  return body.request_nonce;
}

export async function signResponse(response: Response, body: Record<string, unknown>, env: SigningEnv, route: string): Promise<Response> {
  const nonce = requireSignedRequest(body, env);
  const now = Math.floor(Date.now() / 1000);
  const result: unknown = await response.json();
  const payload = new TextEncoder().encode(JSON.stringify({
    protocol_version: 2, product_id: body.product_id, request_nonce: nonce,
    route, http_status: response.status, issued_at: now, expires_at: now + 120, result,
  }));
  const key = await crypto.subtle.importKey("pkcs8", decode64(env.SERVER_SIGNING_KEY_PKCS8!), "Ed25519", false, ["sign"]);
  const signature = await crypto.subtle.sign("Ed25519", key, payload);
  return Response.json({ key_id: env.SERVER_SIGNING_KEY_ID, payload: encode64(payload), signature: encode64(new Uint8Array(signature)) }, {
    status: response.status,
    headers: { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" },
  });
}

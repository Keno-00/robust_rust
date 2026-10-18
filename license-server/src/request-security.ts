// Shared limits for public licensing and upstream responses. Never log payloads.
export const LICENSE_BODY_MAX_BYTES = 8 * 1024;
export const UPSTREAM_BODY_MAX_BYTES = 64 * 1024;
export const DISCORD_BODY_MAX_BYTES = 64 * 1024;
export const BODY_TIMEOUT_MS = 10_000;
export const UPSTREAM_TIMEOUT_MS = 10_000;

export class RequestFailure extends Error {
  constructor(public readonly status: number, public readonly code: string, message: string) {
    super(message);
  }
}

export function diagnostic(code: string, route: string): void {
  console.warn(JSON.stringify({ event: "license_request_failure", code, route }));
}

export function isJsonContentType(headers: Headers): boolean {
  return headers.get("content-type")?.split(";", 1)[0].trim().toLowerCase() === "application/json";
}

export async function boundedText(
  source: Request | Response,
  maxBytes: number,
): Promise<string> {
  const declared = source.headers.get("content-length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > maxBytes)) {
    throw new RequestFailure(413, "body_too_large", "Request exceeds the permitted size.");
  }
  if (!source.body) return "";
  const reader = source.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new RequestFailure(408, "body_timeout", "Request timed out. Please try again."));
      void reader.cancel().catch(() => undefined);
    }, BODY_TIMEOUT_MS);
  });
  try {
    while (true) {
      const { done, value } = await Promise.race([reader.read(), timeout]);
      if (done) break;
      size += value.byteLength;
      if (size > maxBytes) {
        throw new RequestFailure(413, "body_too_large", "Request exceeds the permitted size.");
      }
      chunks.push(value);
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    try {
      return new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(bytes);
    } catch {
      throw new RequestFailure(400, "invalid_encoding", "Expected UTF-8 JSON.");
    }
  } finally {
    if (timer !== undefined) clearTimeout(timer);
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}

export async function readLicenseBody(request: Request): Promise<Record<string, unknown>> {
  if (!isJsonContentType(request.headers)) {
    throw new RequestFailure(415, "invalid_content_type", "Expected application/json.");
  }
  const text = await boundedText(request, LICENSE_BODY_MAX_BYTES);
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new RequestFailure(400, "invalid_json", "Expected a valid JSON request.");
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new RequestFailure(400, "invalid_request", "Expected a JSON object.");
  }
  return value as Record<string, unknown>;
}

export function validSessionToken(token: string): boolean {
  // Canonical URL-safe base64 encoding of the server's 32 random bytes.
  return /^[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]$/.test(token);
}

export function validClientVersion(version: string): boolean {
  return /^\d{1,6}\.\d{1,6}\.\d{1,6}$/.test(version);
}

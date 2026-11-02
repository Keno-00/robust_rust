import { diagnostic } from "./request-security";
import { authorizePackage, LibraryEnv } from "./launcher-library";

export interface LauncherEnv extends LibraryEnv {
  RELEASE_ARTIFACTS?: R2Bucket;
  RELEASE_RATE_LIMITER?: RateLimit;
}

// Publication occurs with a private, bucket-scoped operator credential.
// The public Worker intentionally has no upload, delete, or signing route.
export async function launcherRequest(request: Request, env: LauncherEnv): Promise<Response> {
  const path = new URL(request.url).pathname;
  const fail = (status: number, code: string): Response => {
    diagnostic(code, path);
    return Response.json({ status: code }, { status, headers: { "Cache-Control": "no-store" } });
  };
  try {
  if (request.method !== "GET") return fail(405, "method_not_allowed");
  if (!env.RELEASE_ARTIFACTS || !env.RELEASE_RATE_LIMITER) return fail(503, "releases_not_configured");
  if (!(await env.RELEASE_RATE_LIMITER.limit({ key: request.headers.get("CF-Connecting-IP") || "missing-edge-ip" })).success) {
    return fail(429, "rate_limited");
  }
    if (path === "/launcher/v1/catalog") {
      const catalog = await env.RELEASE_ARTIFACTS.get("catalog/current.json");
      if (!catalog) return fail(503, "catalog_not_published");
      if (catalog.size > 1024 * 1024) return fail(503, "catalog_oversized");
      return new Response(catalog.body, {
        headers: {
          "Content-Type": "application/json",
          "Cache-Control": "no-store",
          "X-Content-Type-Options": "nosniff",
        },
      });
    }
    const match = /^\/launcher\/v1\/objects\/([a-z0-9][a-z0-9.-]{0,63}|_shell)\/([a-f0-9]{64})\.(png|msi)$/.exec(path);
    if (!match) return fail(404, "not_found");
    if (match[3] === "msi" && !await authorizePackage(request, env, match[1])) return fail(403, "license_required");
    const key = `objects/${match[1]}/${match[2]}.${match[3]}`;
    const object = await env.RELEASE_ARTIFACTS.get(key);
    if (!object) return fail(404, "artifact_not_published");
    const limit = match[3] === "png" ? 8 * 1024 * 1024 : 2 * 1024 * 1024 * 1024;
    if (object.size === 0 || object.size > limit) return fail(503, "artifact_oversized");
    return new Response(object.body, {
      headers: {
        "Content-Type": match[3] === "png" ? "image/png" : "application/octet-stream",
        "Content-Length": String(object.size),
        "Cache-Control": match[3] === "msi" ? "private, no-store" : "public, max-age=31536000, immutable",
        "ETag": object.httpEtag,
        "X-Content-Type-Options": "nosniff",
      },
    });
  } catch {
    return fail(503, "release_storage_error");
  }
}


import { CipherSuite, DhkemP256HkdfSha256, HkdfSha256, Aes256Gcm } from "@hpke/core";
import { decode64, encode64 } from "./signed-protocol";
import { RequestFailure } from "./request-security";
import { SurfaceEnv, deliverSurface } from "./surface-delivery";

export interface AlgorithmEnv extends SurfaceEnv {
  ALGORITHM_ARTIFACTS: R2Bucket;
  APB_SMOOTHING_DIGEST: string;
  APB_TRAINING_DIGEST: string;
  APB_DATASET_DIGEST: string;
  APB_INFERENCE_DIGEST: string;
  APB_CB_SMOOTHING_DIGEST: string;
  APB_CB_TRAINING_DIGEST: string;
  APB_CB_DATASET_DIGEST: string;
  APB_CB_INFERENCE_DIGEST: string;
}

export async function deliverAlgorithm(body: Record<string, unknown>, env: AlgorithmEnv, sessionHash: string, expiresAt: number): Promise<Response> {
  const product = body.product_id;
  if (product !== "verux.apb" && product !== "verux.apb.cb") throw new RequestFailure(400, "invalid_product", "This product is unsupported.");
  const color = product === "verux.apb.cb";
  const resource = typeof body.algorithm_id === "string" && body.algorithm_id.startsWith(`${product}.`) ? body.algorithm_id.slice(product.length + 1) : "";
  if (["surface", "surface.objects", "surface.eligibility"].includes(resource)) return deliverSurface(body, env, sessionHash, expiresAt);
  const selected = resource === "smoothing" ? { name: "smoothing", digest: color ? env.APB_CB_SMOOTHING_DIGEST : env.APB_SMOOTHING_DIGEST, maximum: 128 * 1024 }
    : resource === "training" ? { name: "training", digest: color ? env.APB_CB_TRAINING_DIGEST : env.APB_TRAINING_DIGEST, maximum: 1024 * 1024 }
    : resource === "inference" ? { name: "inference", digest: color ? env.APB_CB_INFERENCE_DIGEST : env.APB_INFERENCE_DIGEST, maximum: 1024 * 1024 }
    : resource === "dataset" ? { name: "dataset", digest: color ? env.APB_CB_DATASET_DIGEST : env.APB_DATASET_DIGEST, maximum: 128 * 1024 } : null;
  if (!selected || body.abi_version !== 1 || typeof body.recipient_key !== "string" || body.recipient_key.length > 100) {
    throw new RequestFailure(400, "invalid_algorithm", "This configuration is unsupported. Update the application.");
  }
  const recipient = decode64(body.recipient_key);
  if (recipient.length !== 65 || recipient[0] !== 4) throw new RequestFailure(400, "invalid_recipient", "Unable to prepare this session. Restart the application.");
  const digest = selected.digest;
  if (!/^[a-f0-9]{64}$/.test(digest)) throw new RequestFailure(503, "artifact_not_configured", "Preparation is unavailable. Contact support.");
  const artifact = await env.ALGORITHM_ARTIFACTS.get(`${product}/${selected.name}/abi1/${digest}.wasm`);
  if (!artifact || artifact.size === 0 || artifact.size > selected.maximum) throw new RequestFailure(503, "artifact_unavailable", "Preparation is unavailable. Contact support.");
  const bytes = new Uint8Array(await artifact.arrayBuffer());
  try {
    const actual = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), (b) => b.toString(16).padStart(2, "0")).join("");
    if (actual !== digest) throw new RequestFailure(503, "artifact_integrity", "Preparation is unavailable. Contact support.");
    const manifest = JSON.stringify({ product_id: product, algorithm_id: body.algorithm_id, artifact_version: "2.0.0",
      abi_version: 1, digest, size: bytes.length, session_hash: sessionHash, recipient_key: body.recipient_key,
      request_nonce: body.request_nonce, expires_at: expiresAt });
    const suite = new CipherSuite({ kem: new DhkemP256HkdfSha256(), kdf: new HkdfSha256(), aead: new Aes256Gcm() });
    const publicKey = await suite.kem.deserializePublicKey(recipient);
    const sender = await suite.createSenderContext({ recipientPublicKey: publicKey, info: new TextEncoder().encode("verux-algorithm-v2") });
    const ciphertext = await sender.seal(bytes, new TextEncoder().encode(manifest));
    return Response.json({ valid: true, status: "valid", message: "Ready.", manifest,
      encapsulated_key: encode64(new Uint8Array(sender.enc)), ciphertext: encode64(new Uint8Array(ciphertext)) }, { headers: { "Cache-Control": "no-store" } });
  } finally { bytes.fill(0); }
}

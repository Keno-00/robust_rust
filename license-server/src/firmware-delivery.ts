import { CipherSuite, DhkemP256HkdfSha256, HkdfSha256, Aes256Gcm } from "@hpke/core";
import { decode64, encode64 } from "./signed-protocol";
import { RequestFailure } from "./request-security";

export interface FirmwareEnv {
  ALGORITHM_ARTIFACTS: R2Bucket;
  LEONARDO_FIRMWARE_TEMPLATE: string;
}
interface Template { version: string; digest: string; descriptor_offset: number }
const fail = (code: string): never => { throw new RequestFailure(503, code, "Firmware installation is unavailable. Contact support."); };

// Validate the actual image range, independent of catalog and client checks.
function applicationImage(bytes: Uint8Array): Uint8Array {
  const image = new Uint8Array(28672);
  const text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(bytes);
  const seen = new Set<number>();
  let base = 0, eof = false;
  for (const line of text.trim().split(/\r?\n/)) {
    if (eof || !/^:[0-9a-f]+$/i.test(line) || (line.length - 1) % 2) fail("firmware_hex");
    const record = Uint8Array.from(line.slice(1).match(/../g)!, pair => Number.parseInt(pair, 16));
    if (record.length !== record[0] + 5 || record.reduce((a,b) => a + b, 0) % 256) fail("firmware_hex");
    const address = (record[1] << 8) | record[2], type = record[3];
    if (type === 0) {
      if (!record[0] || base + address + record[0] > 28672) fail("firmware_address");
      for (let i=0; i<record[0]; ++i) {
        const target = base + address + i;
        if (seen.has(target)) fail("firmware_overlap");
        seen.add(target);
        image[target] = record[4 + i];
      }
    } else if (type === 1 && record[0] === 0 && address === 0) eof = true;
    else if (type === 4 && record[0] === 2 && address === 0) base = ((record[4] << 8) | record[5]) * 65536;
    else fail("firmware_hex_record");
  }
  if (!eof || seen.size !== 28672) fail("firmware_application_range");
  return image;
}

// Personalize only the reviewed USB descriptor's VID/PID. Executable logic and
// all other application bytes remain identical to the verified private template.
function personalize(image: Uint8Array, offset: number, vid: number, pid: number): Uint8Array {
  const descriptor = [18,1,0,2,239,2,1,64,109,4,139,192,0,1,1,2,3,1];
  if (!Number.isSafeInteger(offset) || offset < 0 || offset + descriptor.length > image.length
    || descriptor.some((byte,i) => image[offset+i] !== byte)) fail("firmware_template_descriptor");
  image[offset+8] = vid & 255; image[offset+9] = vid >>> 8;
  image[offset+10] = pid & 255; image[offset+11] = pid >>> 8;
  const lines: string[] = [];
  const hex = (b: number) => b.toString(16).padStart(2,"0").toUpperCase();
  for (let address=0; address<image.length; address+=16) {
    const record = [16,address>>>8,address&255,0,...image.subarray(address,address+16)];
    const checksum = (-record.reduce((a,b)=>a+b,0)) & 255;
    lines.push(":" + [...record,checksum].map(hex).join(""));
  }
  lines.push(":00000001FF");
  return new TextEncoder().encode(lines.join("\n")+"\n");
}

export async function deliverFirmware(body: Record<string, unknown>, env: FirmwareEnv, sessionHash: string, expiresAt: number): Promise<Response> {
  if (body.product_id !== "verux.apb" || body.firmware_id !== "leonardo-mouse"
    || body.board_model !== "arduino-leonardo" || body.mcu_signature !== "1e9587"
    || !Number.isInteger(body.target_vid) || !Number.isInteger(body.target_pid)
    || (body.target_vid as number) < 1 || (body.target_vid as number) > 65535
    || (body.target_pid as number) < 1 || (body.target_pid as number) > 65535
    || typeof body.recipient_key !== "string" || body.recipient_key.length > 100) {
    throw new RequestFailure(400, "invalid_firmware", "Select a supported Leonardo and mouse.");
  }
  const recipient = decode64(body.recipient_key);
  if (recipient.length !== 65 || recipient[0] !== 4) throw new RequestFailure(400, "invalid_recipient", "Restart the application and retry Install.");
  let selected: Template;
  try { selected = JSON.parse(env.LEONARDO_FIRMWARE_TEMPLATE) as Template; }
  catch { fail("firmware_template"); }
  if (!selected! || typeof selected! !== "object" || typeof selected!.digest !== "string"
    || typeof selected!.version !== "string" || !/^[a-f0-9]{64}$/.test(selected!.digest)
    || !/^[0-9A-Za-z.-]{1,64}$/.test(selected!.version) || !Number.isSafeInteger(selected!.descriptor_offset)) fail("firmware_template");
  const object = await env.ALGORITHM_ARTIFACTS.get(`verux.apb/firmware/leonardo/template/${selected!.version}/${selected!.digest}.hex`);
  if (!object || object.size === 0 || object.size > 128 * 1024) fail("firmware_unavailable");
  const bytes = new Uint8Array(await object!.arrayBuffer());
  let image: Uint8Array | undefined, personalized: Uint8Array | undefined;
  try {
    const actual = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256",bytes)), b => b.toString(16).padStart(2,"0")).join("");
    if (actual !== selected!.digest) fail("firmware_integrity");
    image = applicationImage(bytes);
    personalized = personalize(image,selected!.descriptor_offset,body.target_vid as number,body.target_pid as number);
    const digest = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256",personalized)),b=>b.toString(16).padStart(2,"0")).join("");
    const manifest = JSON.stringify({ product_id:"verux.apb", firmware_id:"leonardo-mouse", firmware_version:selected!.version,
      board_model:"arduino-leonardo", mcu_signature:"1e9587", application_start:0, application_end:28672,
      target_vid:body.target_vid, target_pid:body.target_pid, digest, size:personalized.length,
      session_hash:sessionHash, recipient_key:body.recipient_key, request_nonce:body.request_nonce, expires_at:expiresAt });
    const suite = new CipherSuite({ kem:new DhkemP256HkdfSha256(), kdf:new HkdfSha256(), aead:new Aes256Gcm() });
    const publicKey = await suite.kem.deserializePublicKey(recipient);
    const sender = await suite.createSenderContext({recipientPublicKey:publicKey, info:new TextEncoder().encode("verux-firmware-v2")});
    const ciphertext = await sender.seal(personalized,new TextEncoder().encode(manifest));
    return Response.json({valid:true,status:"valid",message:"Ready.",manifest,
      encapsulated_key:encode64(new Uint8Array(sender.enc)),ciphertext:encode64(new Uint8Array(ciphertext))}, {headers:{"Cache-Control":"no-store"}});
  } finally { bytes.fill(0); image?.fill(0); personalized?.fill(0); }
}


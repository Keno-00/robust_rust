// Standard WebCrypto names used by HPKE that Cloudflare names differently.
// Keep strict library checking; do not import the conflicting browser DOM lib.
type KeyUsage = "decrypt" | "deriveBits" | "deriveKey" | "encrypt" | "sign" | "unwrapKey" | "verify" | "wrapKey";
interface KeyAlgorithm { name: string; }
interface HmacKeyGenParams { name: string; hash: string | { name: string }; length?: number; }

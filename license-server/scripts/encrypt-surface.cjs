// Publisher input is private stdin. Output contains ciphertext and its digest only.
const { createCipheriv, randomBytes, createHash } = require('node:crypto');
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { input += chunk; if (input.length > 256 * 1024) throw new Error('Publisher input exceeds its limit.'); });
process.stdin.on('end', () => {
  const packet = JSON.parse(input);
  input = '';
  const product = packet.product || 'verux.apb';
  if (!['verux.apb', 'verux.apb.cb'].includes(product)) throw new Error('Unsupported product.');
  const key = Buffer.from(packet.key, 'base64url');
  packet.key = '';
  try {
    if (key.length !== 32 || !packet.labels || Array.isArray(packet.labels)) throw new Error('Invalid publisher input.');
    const rows = {};
    for (const id of Object.keys(packet.labels).sort()) {
      const text = packet.labels[id];
      if (!/^u\d{3}$/.test(id) || typeof text !== 'string' || !text.length || text.includes('\0')) throw new Error('Invalid display row.');
      const bytes = Buffer.from(text, 'utf8');
      if (bytes.length > 4080) throw new Error('Display row exceeds its limit.');
      const nonce = randomBytes(12);
      const cipher = createCipheriv('aes-256-gcm', key, nonce);
      cipher.setAAD(Buffer.from(`${product}|surface|1|${id}`, 'utf8'));
      try {
        const encrypted = Buffer.concat([cipher.update(bytes), cipher.final(), cipher.getAuthTag()]);
        rows[id] = {nonce: nonce.toString('base64url'), ciphertext: encrypted.toString('base64url')};
      } finally { bytes.fill(0); packet.labels[id] = ''; }
    }
    const encryptedRows = JSON.stringify(rows);
    if (Buffer.byteLength(encryptedRows) > 96 * 1024) throw new Error('Catalog exceeds its limit.');
    process.stdout.write(JSON.stringify({encryptedRows, digest: createHash('sha256').update(encryptedRows).digest('hex'), count: Object.keys(rows).length}));
  } finally { key.fill(0); }
});

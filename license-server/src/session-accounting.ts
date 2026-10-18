// Called only inside the per-license Durable Object's concurrency gate.
export async function settleSessions(db: D1Database, product: string, license: string, now: number, expiredOnly = false): Promise<void> {
  const condition = expiredOnly ? " AND expires_at <= ?" : "";
  const values = expiredOnly ? [product, license, now] : [product, license];
  await db.batch([
    db.prepare(`UPDATE licenses SET used_seconds = used_seconds + COALESCE((
      SELECT SUM(MAX(0, MIN(?, expires_at) - last_seen_at)) FROM sessions
      WHERE product_id = ? AND license_hash = ? AND revoked_at IS NULL${condition}
    ), 0), updated_at = ? WHERE product_id = ? AND license_hash = ?`)
      .bind(now, ...values, now, product, license),
    db.prepare(`UPDATE sessions SET last_seen_at = MIN(?, expires_at), revoked_at = ?
      WHERE product_id = ? AND license_hash = ? AND revoked_at IS NULL${condition}`)
      .bind(now, now, ...values),
  ]);
}

export async function settleOne(db: D1Database, product: string, tokenHash: string, now: number): Promise<void> {
  await db.batch([
    db.prepare(`UPDATE licenses SET used_seconds = used_seconds + COALESCE((
      SELECT MAX(0, MIN(?, expires_at) - last_seen_at) FROM sessions
      WHERE product_id = ? AND session_hash = ? AND revoked_at IS NULL
    ), 0), updated_at = ? WHERE product_id = ? AND license_hash = (
      SELECT license_hash FROM sessions WHERE product_id = ? AND session_hash = ?
    )`).bind(now, product, tokenHash, now, product, product, tokenHash),
    db.prepare(`UPDATE sessions SET last_seen_at = MIN(?, expires_at), revoked_at = ?
      WHERE product_id = ? AND session_hash = ? AND revoked_at IS NULL`)
      .bind(now, now, product, tokenHash),
  ]);
}

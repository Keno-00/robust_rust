-- Settle and revoke duplicate legacy sessions before enforcing the invariant.
-- Keep the newest session, with the hash breaking timestamp ties deterministically.
UPDATE licenses SET used_seconds = used_seconds + COALESCE((
  SELECT SUM(MAX(0, MIN(unixepoch('now'), s.expires_at) - s.last_seen_at))
  FROM sessions s WHERE s.product_id = licenses.product_id
    AND s.license_hash = licenses.license_hash AND s.revoked_at IS NULL
    AND EXISTS (SELECT 1 FROM sessions newer WHERE newer.product_id = s.product_id
      AND newer.license_hash = s.license_hash AND newer.revoked_at IS NULL
      AND (newer.started_at > s.started_at OR (newer.started_at = s.started_at AND newer.session_hash > s.session_hash)))
), 0);

UPDATE sessions AS s SET revoked_at = unixepoch('now')
WHERE s.revoked_at IS NULL AND EXISTS (
  SELECT 1 FROM sessions newer WHERE newer.product_id = s.product_id
    AND newer.license_hash = s.license_hash AND newer.revoked_at IS NULL
    AND (newer.started_at > s.started_at OR (newer.started_at = s.started_at AND newer.session_hash > s.session_hash))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_product_license
  ON sessions(product_id, license_hash) WHERE revoked_at IS NULL;

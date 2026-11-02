CREATE TABLE IF NOT EXISTS launcher_license_expiry (
  product_id TEXT NOT NULL,
  license_hash TEXT NOT NULL,
  expired_at INTEGER NOT NULL,
  PRIMARY KEY (product_id, license_hash)
);

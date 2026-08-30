ALTER TABLE licenses ADD COLUMN product_id TEXT NOT NULL DEFAULT 'verux.apb';
ALTER TABLE sessions ADD COLUMN product_id TEXT NOT NULL DEFAULT 'verux.apb';
ALTER TABLE audit_events ADD COLUMN product_id TEXT NOT NULL DEFAULT 'verux.apb';

CREATE UNIQUE INDEX IF NOT EXISTS idx_licenses_product_hash
    ON licenses(product_id, license_hash);

CREATE INDEX IF NOT EXISTS idx_sessions_product_license_active
    ON sessions(product_id, license_hash, revoked_at, expires_at);

CREATE INDEX IF NOT EXISTS idx_sessions_product_session
    ON sessions(product_id, session_hash);

CREATE INDEX IF NOT EXISTS idx_audit_product_created
    ON audit_events(product_id, created_at);

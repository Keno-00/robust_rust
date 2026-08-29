CREATE TABLE IF NOT EXISTS licenses (
    license_hash TEXT PRIMARY KEY,
    payhip_license_id TEXT,
    tier TEXT NOT NULL,
    purchased_at INTEGER NOT NULL,
    entitlement_seconds INTEGER,
    used_seconds INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',
    customer_name TEXT NOT NULL DEFAULT '',
    customer_email TEXT NOT NULL DEFAULT '',
    product_name TEXT NOT NULL DEFAULT '',
    last_verified_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    session_hash TEXT PRIMARY KEY,
    license_hash TEXT NOT NULL,
    install_hash TEXT NOT NULL,
    client_version TEXT NOT NULL,
    sequence INTEGER NOT NULL DEFAULT 0,
    started_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    FOREIGN KEY (license_hash) REFERENCES licenses(license_hash)
);

CREATE INDEX IF NOT EXISTS idx_sessions_license_active
    ON sessions(license_hash, revoked_at, expires_at);

CREATE TABLE IF NOT EXISTS audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    license_hash TEXT,
    install_hash TEXT,
    event_type TEXT NOT NULL,
    ip_prefix TEXT NOT NULL DEFAULT '',
    detail TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_license_created
    ON audit_events(license_hash, created_at);

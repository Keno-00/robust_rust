CREATE TABLE IF NOT EXISTS surface_catalogs (
  product_id TEXT NOT NULL,
  catalog_id TEXT NOT NULL,
  version TEXT NOT NULL,
  encrypted_rows TEXT NOT NULL,
  digest TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (product_id, catalog_id, version)
);

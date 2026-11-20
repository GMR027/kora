-- Identidad local y dispositivos.
-- `secret_ref` apunta al Keyring/Keystore; nunca contiene la clave privada.

CREATE TABLE accounts (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    email        TEXT,
    is_local     INTEGER NOT NULL DEFAULT 0 CHECK (is_local IN (0, 1)),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- Solo puede existir una cuenta local por base de datos.
CREATE UNIQUE INDEX idx_accounts_local ON accounts (is_local) WHERE is_local = 1;

CREATE TABLE devices (
    id           TEXT PRIMARY KEY,
    account_id   TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    platform     TEXT NOT NULL,
    public_key   TEXT,
    last_seen_at TEXT,
    is_local     INTEGER NOT NULL DEFAULT 0 CHECK (is_local IN (0, 1)),
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_devices_account ON devices (account_id);

CREATE TABLE connection_keys (
    id         TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    device_id  TEXT REFERENCES devices (id) ON DELETE CASCADE,
    kind       TEXT NOT NULL CHECK (kind IN ('ed25519', 'x25519', 'pairing')),
    public_key TEXT NOT NULL,
    secret_ref TEXT,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    revoked_at TEXT
);

CREATE INDEX idx_connection_keys_account ON connection_keys (account_id);
CREATE INDEX idx_connection_keys_device ON connection_keys (device_id);

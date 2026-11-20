-- Recursos compartidos entre cuentas y claves envueltas para cada receptor.

CREATE TABLE shares (
    id               TEXT PRIMARY KEY,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    resource_type    TEXT NOT NULL,
    resource_id      TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    revoked_at       TEXT
);

CREATE INDEX idx_shares_owner ON shares (owner_account_id);
CREATE INDEX idx_shares_resource ON shares (resource_type, resource_id);

CREATE TABLE share_keys (
    id                   TEXT PRIMARY KEY,
    share_id             TEXT NOT NULL REFERENCES shares (id) ON DELETE CASCADE,
    recipient_account_id TEXT REFERENCES accounts (id) ON DELETE SET NULL,
    wrapped_key          TEXT NOT NULL,
    created_at           TEXT NOT NULL,
    expires_at           TEXT,
    revoked_at           TEXT
);

CREATE INDEX idx_share_keys_share ON share_keys (share_id);
CREATE INDEX idx_share_keys_recipient ON share_keys (recipient_account_id);

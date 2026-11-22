-- Fase 10: cuentas locales, SPAKE2, QR y claves de compartir.

ALTER TABLE accounts ADD COLUMN username TEXT;
ALTER TABLE accounts ADD COLUMN pin_hash TEXT;
ALTER TABLE accounts ADD COLUMN public_key TEXT;
ALTER TABLE accounts ADD COLUMN account_key_ciphertext TEXT;
ALTER TABLE accounts ADD COLUMN account_key_nonce TEXT;
ALTER TABLE accounts ADD COLUMN connection_key_hash TEXT;
ALTER TABLE accounts ADD COLUMN connection_key_ciphertext TEXT;
ALTER TABLE accounts ADD COLUMN connection_key_nonce TEXT;
ALTER TABLE accounts ADD COLUMN connection_key_rotated_at TEXT;

CREATE UNIQUE INDEX idx_accounts_username ON accounts (username)
    WHERE username IS NOT NULL;

ALTER TABLE devices ADD COLUMN peer_account_id TEXT;
ALTER TABLE devices ADD COLUMN peer_username TEXT;
ALTER TABLE devices ADD COLUMN peer_account_public_key TEXT;
ALTER TABLE devices ADD COLUMN account_signature TEXT;
ALTER TABLE devices ADD COLUMN peer_account_signature TEXT;
ALTER TABLE devices ADD COLUMN revoked_at TEXT;

CREATE INDEX idx_devices_peer_account ON devices (peer_account_id, trusted);

ALTER TABLE shares ADD COLUMN scope_type TEXT NOT NULL DEFAULT 'library'
    CHECK (scope_type IN ('library', 'book', 'voice'));
ALTER TABLE shares ADD COLUMN permission TEXT NOT NULL DEFAULT 'read'
    CHECK (permission IN ('read', 'copy'));
ALTER TABLE shares ADD COLUMN expires_at TEXT;
ALTER TABLE shares ADD COLUMN max_uses INTEGER CHECK (max_uses IS NULL OR max_uses > 0);
ALTER TABLE shares ADD COLUMN uses INTEGER NOT NULL DEFAULT 0 CHECK (uses >= 0);
ALTER TABLE shares ADD COLUMN share_key_hash TEXT;
ALTER TABLE shares ADD COLUMN share_key_ciphertext TEXT;
ALTER TABLE shares ADD COLUMN share_key_nonce TEXT;

CREATE UNIQUE INDEX idx_shares_key_hash ON shares (share_key_hash)
    WHERE share_key_hash IS NOT NULL;

-- Copias vinculadas de solo lectura. `copy` no las usa.
ALTER TABLE books ADD COLUMN read_only INTEGER NOT NULL DEFAULT 0
    CHECK (read_only IN (0, 1));
ALTER TABLE books ADD COLUMN received_share_id TEXT;

CREATE INDEX idx_books_received_share ON books (received_share_id);

CREATE TABLE received_shares (
    share_id             TEXT PRIMARY KEY,
    owner_account_id     TEXT NOT NULL,
    owner_username       TEXT,
    owner_public_key     TEXT,
    owner_account_hint   TEXT,
    owner_device_id      TEXT,
    scope_type           TEXT NOT NULL
                         CHECK (scope_type IN ('library', 'book', 'voice')),
    permission           TEXT NOT NULL CHECK (permission IN ('read', 'copy')),
    expires_at           TEXT,
    max_uses             INTEGER,
    uses                 INTEGER NOT NULL DEFAULT 1,
    revoked_at           TEXT,
    accepted_at          TEXT NOT NULL,
    owner_signature      TEXT,
    peer_account_signature TEXT,
    last_checked_at      TEXT,
    owner_account_signature TEXT
);

CREATE INDEX idx_received_shares_owner ON received_shares (owner_account_id);
CREATE INDEX idx_received_shares_revoked ON received_shares (revoked_at);

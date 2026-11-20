-- Estado de sincronización, propiedad polimórfica de recursos y auditoría.
-- `peer_device_id` no tiene FK: un par puede aparecer antes de registrarse.

CREATE TABLE sync_state (
    resource_type  TEXT NOT NULL,
    resource_id    TEXT NOT NULL,
    peer_device_id TEXT NOT NULL,
    version        INTEGER NOT NULL DEFAULT 0,
    last_synced_at TEXT,
    state          TEXT NOT NULL DEFAULT 'pending'
                   CHECK (state IN ('pending', 'syncing', 'synced', 'conflict', 'error')),
    PRIMARY KEY (resource_type, resource_id, peer_device_id)
);

CREATE INDEX idx_sync_state_resource ON sync_state (resource_type, resource_id);
CREATE INDEX idx_sync_state_peer ON sync_state (peer_device_id);

CREATE TABLE resource_owners (
    resource_type    TEXT NOT NULL,
    resource_id      TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    updated_at       TEXT NOT NULL,
    PRIMARY KEY (resource_type, resource_id)
);

CREATE INDEX idx_resource_owners_owner ON resource_owners (owner_account_id);

CREATE TABLE audit_log (
    id               TEXT PRIMARY KEY,
    actor_account_id TEXT REFERENCES accounts (id) ON DELETE SET NULL,
    action           TEXT NOT NULL,
    resource_type    TEXT,
    resource_id      TEXT,
    metadata         TEXT NOT NULL DEFAULT '{}',
    created_at       TEXT NOT NULL
);

CREATE INDEX idx_audit_log_created ON audit_log (created_at);
CREATE INDEX idx_audit_log_resource ON audit_log (resource_type, resource_id);

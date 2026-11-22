-- Fase 11: búsqueda semántica local, diario automático, logros,
-- Pomodoro y selección de sync por dispositivo.

-- Índice semántico por libro. `model` permite invalidar el índice cuando
-- cambie el modelo de embeddings sin tocar el contenido original.
CREATE TABLE semantic_book_index (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    model            TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running', 'done', 'failed', 'skipped')),
    chunks           INTEGER NOT NULL DEFAULT 0 CHECK (chunks >= 0),
    error            TEXT,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_semantic_book_index_owner
    ON semantic_book_index (owner_account_id, status);

-- Chunks de ~500 palabras. El vector se guarda como f32 little-endian para
-- no depender de sqlite-vec ni de una extensión nativa de SQLite.
CREATE TABLE semantic_chunks (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    section          INTEGER NOT NULL CHECK (section >= 0),
    chunk_index      INTEGER NOT NULL CHECK (chunk_index >= 0),
    section_title    TEXT,
    content          TEXT NOT NULL,
    offset_start     INTEGER NOT NULL DEFAULT 0 CHECK (offset_start >= 0),
    offset_end       INTEGER NOT NULL DEFAULT 0 CHECK (offset_end >= offset_start),
    content_hash     TEXT NOT NULL,
    model            TEXT NOT NULL,
    dimensions       INTEGER NOT NULL CHECK (dimensions > 0),
    embedding        BLOB NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    UNIQUE (book_id, model, section, chunk_index)
);

CREATE INDEX idx_semantic_chunks_owner
    ON semantic_chunks (owner_account_id, model);
CREATE INDEX idx_semantic_chunks_book
    ON semantic_chunks (book_id, model);

-- El diario existente sigue siendo válido: las entradas manuales usan los
-- valores por defecto. Las sesiones automáticas rellenan los campos nuevos.
ALTER TABLE diary_entries ADD COLUMN kind TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE diary_entries ADD COLUMN session_start TEXT;
ALTER TABLE diary_entries ADD COLUMN session_end TEXT;
ALTER TABLE diary_entries ADD COLUMN duration_minutes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE diary_entries ADD COLUMN pages_read INTEGER NOT NULL DEFAULT 0;
ALTER TABLE diary_entries ADD COLUMN extracts_created INTEGER NOT NULL DEFAULT 0;
ALTER TABLE diary_entries ADD COLUMN notes_created INTEGER NOT NULL DEFAULT 0;
ALTER TABLE diary_entries ADD COLUMN pomodoro_round INTEGER;

CREATE INDEX idx_diary_entries_kind
    ON diary_entries (owner_account_id, kind, entry_date);

-- Selección por dispositivo emparejado. Si no hay filas para un peer, se
-- comparte todo con él; si hay filas, solo las categorías habilitadas.
CREATE TABLE sync_device_collections (
    device_id        TEXT NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    category_id      TEXT NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    enabled          INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    PRIMARY KEY (device_id, category_id)
);

CREATE INDEX idx_sync_device_collections_owner
    ON sync_device_collections (owner_account_id, device_id);

-- Conserva la selección anterior para todos los dispositivos ya conocidos.
INSERT INTO sync_device_collections
    (device_id, category_id, enabled, updated_at, owner_account_id)
SELECT d.id, sc.category_id, sc.enabled, sc.updated_at, sc.owner_account_id
FROM sync_collections sc
JOIN devices d ON d.account_id = sc.owner_account_id;

-- Conserva también el modo global anterior como modo por dispositivo. Así una
-- selección vacía en modo "selected" sigue significando "no compartir nada".
INSERT INTO settings (key, value, updated_at)
SELECT 'sync.collections.mode.' || d.id, s.value, s.updated_at
FROM devices d
CROSS JOIN settings s
WHERE s.key = 'sync.collections.mode'
ON CONFLICT (key) DO UPDATE SET
    value = excluded.value,
    updated_at = excluded.updated_at;

DROP TABLE sync_collections;

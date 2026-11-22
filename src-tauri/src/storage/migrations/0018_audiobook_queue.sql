-- Fase 8: cola persistente de generación de audiolibros.
-- Cada fila sobrevive al cierre de la app y se procesa en orden de posición.

CREATE TABLE audiobook_queue (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    scope            TEXT NOT NULL,
    voice_id         TEXT NOT NULL,
    narration        TEXT,
    origin           TEXT NOT NULL DEFAULT 'generated'
                     CHECK (origin IN ('generated', 'tejido')),
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running')),
    position         INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE
);

CREATE INDEX idx_audiobook_queue_owner
    ON audiobook_queue (owner_account_id, position, created_at);

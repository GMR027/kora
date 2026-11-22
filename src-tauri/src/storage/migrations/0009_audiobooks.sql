-- Fase 7: audiolibros generados, importados o nacidos de Tejido.
-- El audio final vive en assets/audiobooks/<hash>.<ext>; el manifiesto de
-- capítulos y su sincronización con el texto se guarda como asset JSON.

ALTER TABLE books ADD COLUMN origin TEXT NOT NULL DEFAULT 'imported'
    CHECK (origin IN ('imported', 'generated', 'transcribed', 'tejido'));

ALTER TABLE audiobooks ADD COLUMN origin TEXT NOT NULL DEFAULT 'generated'
    CHECK (origin IN ('generated', 'external', 'tejido'));

ALTER TABLE audiobooks ADD COLUMN scope TEXT;

ALTER TABLE audiobooks ADD COLUMN audio_hash TEXT;
ALTER TABLE audiobooks ADD COLUMN audio_ext TEXT;

CREATE INDEX idx_audiobooks_origin ON audiobooks (origin, created_at);

-- Posición de escucha por audiolibro. Es independiente del progreso de lectura
-- porque un mismo libro puede tener varias locuciones.
CREATE TABLE audiobook_progress (
    audiobook_id     TEXT PRIMARY KEY REFERENCES audiobooks (id) ON DELETE CASCADE,
    position_ms      INTEGER NOT NULL DEFAULT 0 CHECK (position_ms >= 0),
    chapter_index    INTEGER NOT NULL DEFAULT 0 CHECK (chapter_index >= 0),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE
);

CREATE INDEX idx_audiobook_progress_owner ON audiobook_progress (owner_account_id, updated_at);

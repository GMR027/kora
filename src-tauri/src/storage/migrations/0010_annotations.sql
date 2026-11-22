-- Fase 8: anotaciones superpuestas, notas ricas, notas de voz y reacciones.
-- Todas las filas conservan owner_account_id y usan borrado suave para que la
-- sincronización de la fase 9 pueda propagar tumbas (deleted = 1).

ALTER TABLE annotations ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;

ALTER TABLE extracts ADD COLUMN chapter_index INTEGER;
ALTER TABLE extracts ADD COLUMN offset_start INTEGER;
ALTER TABLE extracts ADD COLUMN offset_end INTEGER;
ALTER TABLE extracts ADD COLUMN citation TEXT;
ALTER TABLE extracts ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;

ALTER TABLE notes ADD COLUMN content_json TEXT;
ALTER TABLE notes ADD COLUMN chapter_index INTEGER;
ALTER TABLE notes ADD COLUMN offset_start INTEGER;
ALTER TABLE notes ADD COLUMN offset_end INTEGER;
ALTER TABLE notes ADD COLUMN text TEXT;
ALTER TABLE notes ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_annotations_book_active ON annotations (book_id, deleted);
CREATE INDEX idx_extracts_book_active ON extracts (book_id, deleted);
CREATE INDEX idx_notes_book_active ON notes (book_id, deleted);

-- Nota de voz anclada a un pasaje. El audio vive en
-- assets/notes_audio/<id>.opus y se deriva del id.
CREATE TABLE voice_notes (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    chapter_index    INTEGER NOT NULL CHECK (chapter_index >= 0),
    offset_start     INTEGER NOT NULL CHECK (offset_start >= 0),
    offset_end       INTEGER NOT NULL CHECK (offset_end >= offset_start),
    text             TEXT NOT NULL DEFAULT '',
    duration_ms      INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_voice_notes_book ON voice_notes (book_id, deleted);
CREATE INDEX idx_voice_notes_owner ON voice_notes (owner_account_id);

-- Reacciones rápidas sobre un pasaje. Varias reacciones del mismo tipo pueden
-- convivir en el mismo rango; el mapa emocional las agrega por sección.
CREATE TABLE reactions (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    chapter_index    INTEGER NOT NULL CHECK (chapter_index >= 0),
    offset_start     INTEGER NOT NULL CHECK (offset_start >= 0),
    offset_end       INTEGER NOT NULL CHECK (offset_end >= offset_start),
    kind             TEXT NOT NULL
                     CHECK (kind IN ('like', 'surprise', 'tension', 'boredom', 'emotion')),
    text             TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_reactions_book ON reactions (book_id, chapter_index, deleted);
CREATE INDEX idx_reactions_owner ON reactions (owner_account_id);

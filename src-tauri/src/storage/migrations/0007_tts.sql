-- Fase 5: secuencias expresivas, voces por personaje, preferencias y ensayo.
-- La secuencia cacheada es texto derivado; el libro original nunca cambia.

CREATE TABLE tts_sequences (
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    section          INTEGER NOT NULL CHECK (section >= 0),
    content_hash     TEXT NOT NULL,
    unit             TEXT NOT NULL CHECK (unit IN ('utf16', 'word')),
    sequence         TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    PRIMARY KEY (book_id, section, content_hash)
);

CREATE INDEX idx_tts_sequences_book ON tts_sequences (book_id, section);

CREATE TABLE tts_character_voices (
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    character        TEXT NOT NULL,
    voice_id         TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    PRIMARY KEY (book_id, character)
);

CREATE TABLE tts_preferences (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    voice_id         TEXT,
    rate             REAL NOT NULL DEFAULT 1.0 CHECK (rate BETWEEN 0.5 AND 3.0),
    pitch            REAL NOT NULL DEFAULT 1.0 CHECK (pitch BETWEEN 0.5 AND 1.5),
    pause_scale      REAL NOT NULL DEFAULT 1.0 CHECK (pause_scale BETWEEN 0.0 AND 3.0),
    expressive       INTEGER NOT NULL DEFAULT 1 CHECK (expressive IN (0, 1)),
    multi_voice      INTEGER NOT NULL DEFAULT 1 CHECK (multi_voice IN (0, 1)),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE TABLE tts_practice_attempts (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    section          INTEGER NOT NULL CHECK (section >= 0),
    offset_start     INTEGER NOT NULL CHECK (offset_start >= 0),
    offset_end       INTEGER NOT NULL CHECK (offset_end >= offset_start),
    expected         TEXT NOT NULL,
    transcript       TEXT NOT NULL,
    accuracy         REAL NOT NULL CHECK (accuracy BETWEEN 0.0 AND 1.0),
    diff             TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_tts_practice_book ON tts_practice_attempts (book_id, created_at);

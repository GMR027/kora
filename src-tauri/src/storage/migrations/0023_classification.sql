-- Fase 12: clasificación asistida de libros y audiolibros.
-- Las sugerencias son locales del dispositivo; las asignaciones aceptadas
-- usan book_categories y por tanto siguen el flujo normal de sincronización.

ALTER TABLE book_categories ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'
    CHECK (source IN ('manual', 'metadata', 'embedding', 'ai', 'sync'));
ALTER TABLE book_categories ADD COLUMN confidence REAL;
ALTER TABLE book_categories ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;

-- Las asignaciones que ya existían se hicieron a mano: no deben ser pisadas
-- por la clasificación automática.
UPDATE book_categories SET locked = 1;

CREATE TABLE category_aliases (
    id               TEXT PRIMARY KEY,
    category_id      TEXT NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    alias            TEXT NOT NULL,
    alias_normalized TEXT NOT NULL,
    source           TEXT NOT NULL DEFAULT 'seed'
                     CHECK (source IN ('seed', 'learned', 'manual')),
    created_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    UNIQUE (category_id, alias_normalized)
);

CREATE INDEX idx_category_aliases_owner
    ON category_aliases (owner_account_id, alias_normalized);

CREATE TABLE category_suggestions (
    id                       TEXT PRIMARY KEY,
    book_id                  TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    category_id              TEXT REFERENCES categories (id) ON DELETE CASCADE,
    proposed_name            TEXT,
    proposed_name_normalized TEXT,
    target_key               TEXT NOT NULL,
    score                    REAL NOT NULL DEFAULT 0,
    source                   TEXT NOT NULL CHECK (source IN ('metadata', 'embedding', 'ai')),
    reason                   TEXT,
    status                   TEXT NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending', 'accepted', 'dismissed')),
    created_at               TEXT NOT NULL,
    resolved_at              TEXT,
    owner_account_id         TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id          TEXT REFERENCES shares (id) ON DELETE SET NULL,
    CHECK ((category_id IS NOT NULL) <> (proposed_name IS NOT NULL))
);

CREATE UNIQUE INDEX idx_category_suggestions_pending
    ON category_suggestions (book_id, target_key) WHERE status = 'pending';

CREATE INDEX idx_category_suggestions_owner
    ON category_suggestions (owner_account_id, status);

CREATE TABLE book_classification (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    input_hash       TEXT NOT NULL,
    model            TEXT,
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running', 'done', 'failed')),
    error            TEXT,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE
);

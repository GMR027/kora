-- Fase 10: favoritos de audiolibros y tipo de recurso en shares recibidos.
-- Los favoritos de audiolibros se guardan aparte de `book_favorites` porque un
-- mismo libro puede tener varias locuciones y el usuario puede querer marcar
-- solo una. `received_shares` necesita el tipo de recurso para distinguir un
-- share de audiolibro (que usa el alcance `book` por compatibilidad con el
-- CHECK de SQLite) de un share de libro normal.

CREATE TABLE audiobook_favorites (
    audiobook_id     TEXT PRIMARY KEY REFERENCES audiobooks (id) ON DELETE CASCADE,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL
);

CREATE INDEX idx_audiobook_favorites_owner ON audiobook_favorites (owner_account_id);

ALTER TABLE received_shares ADD COLUMN resource_type TEXT NOT NULL DEFAULT 'book';

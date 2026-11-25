-- Fase 13: álbumes de audiolibros.
-- Un álbum es una colección ordenada y local de audiolibros. No sustituye a
-- las categorías: la categoría es una etiqueta temática y el álbum define un
-- orden de escucha y una reproducción continua. Los audiolibros no se borran
-- al borrar un álbum; solo se elimina la agrupación.

CREATE TABLE audiobook_albums (
    id               TEXT PRIMARY KEY,
    title            TEXT NOT NULL,
    description      TEXT,
    sort_order       INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE
);

CREATE INDEX idx_audiobook_albums_owner
    ON audiobook_albums (owner_account_id, sort_order, title COLLATE NOCASE);

CREATE TABLE audiobook_album_items (
    album_id         TEXT NOT NULL REFERENCES audiobook_albums (id) ON DELETE CASCADE,
    audiobook_id     TEXT NOT NULL REFERENCES audiobooks (id) ON DELETE CASCADE,
    position         INTEGER NOT NULL DEFAULT 0,
    added_at         TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    PRIMARY KEY (album_id, audiobook_id)
);

CREATE INDEX idx_audiobook_album_items_album
    ON audiobook_album_items (album_id, position, added_at);
CREATE INDEX idx_audiobook_album_items_audiobook
    ON audiobook_album_items (audiobook_id);

-- Fase 9: sincronización LAN, confianza de dispositivos, cola de cambios,
-- documentos CRDT por anotación y filtros por colecciones.

ALTER TABLE devices ADD COLUMN certificate_fingerprint TEXT;
ALTER TABLE devices ADD COLUMN trusted INTEGER NOT NULL DEFAULT 0
    CHECK (trusted IN (0, 1));
ALTER TABLE devices ADD COLUMN sas_verified_at TEXT;
ALTER TABLE devices ADD COLUMN last_address TEXT;
ALTER TABLE devices ADD COLUMN last_port INTEGER;

-- El dispositivo local ya es de confianza para sí mismo.
UPDATE devices SET trusted = 1 WHERE is_local = 1;

ALTER TABLE sync_state ADD COLUMN hash TEXT;
ALTER TABLE voice_notes ADD COLUMN audio_hash TEXT;

-- Cola local de cambios. Los triggers de SQLite la alimentan para no tener que
-- tocar cada comando cuando se añade una entidad sincronizable.
CREATE TABLE sync_pending (
    resource_type TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    changed_at    TEXT NOT NULL,
    state         TEXT NOT NULL DEFAULT 'pending'
                  CHECK (state IN ('pending', 'syncing')),
    PRIMARY KEY (resource_type, resource_id)
);

CREATE INDEX idx_sync_pending_changed ON sync_pending (changed_at);

-- Snapshot CRDT por anotación. El id de la anotación es el id del documento.
CREATE TABLE crdt_documents (
    resource_type TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    snapshot      BLOB NOT NULL,
    hash          TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    PRIMARY KEY (resource_type, resource_id)
);

CREATE INDEX idx_crdt_documents_hash ON crdt_documents (hash);

-- Selección de colecciones por dispositivo. Si no hay filas, se sincroniza
-- todo; si hay filas, solo las categorías con enabled = 1.
CREATE TABLE sync_collections (
    category_id      TEXT PRIMARY KEY REFERENCES categories (id) ON DELETE CASCADE,
    enabled          INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE
);

-- Traducción de ids remotos a locales cuando el mismo libro ya existe por
-- content_hash en ambos dispositivos.
CREATE TABLE sync_aliases (
    peer_device_id TEXT NOT NULL,
    resource_type  TEXT NOT NULL,
    remote_id      TEXT NOT NULL,
    local_id       TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    PRIMARY KEY (peer_device_id, resource_type, remote_id)
);

-- Triggers de cola. Las tumbas también cuentan como cambios.
CREATE TRIGGER sync_pending_annotations_insert AFTER INSERT ON annotations BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('annotation', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_annotations_update AFTER UPDATE ON annotations BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('annotation', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_annotations_delete AFTER DELETE ON annotations BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('annotation', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_extracts_insert AFTER INSERT ON extracts BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('extract', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_extracts_update AFTER UPDATE ON extracts BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('extract', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_extracts_delete AFTER DELETE ON extracts BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('extract', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_notes_insert AFTER INSERT ON notes BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('note', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_notes_update AFTER UPDATE ON notes BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('note', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_notes_delete AFTER DELETE ON notes BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('note', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_voice_notes_insert AFTER INSERT ON voice_notes BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('voice_note', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_voice_notes_update AFTER UPDATE ON voice_notes BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('voice_note', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_voice_notes_delete AFTER DELETE ON voice_notes BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('voice_note', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_reactions_insert AFTER INSERT ON reactions BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('reaction', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_reactions_update AFTER UPDATE ON reactions BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('reaction', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_reactions_delete AFTER DELETE ON reactions BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('reaction', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_progress_insert AFTER INSERT ON reading_progress BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('reading_progress', NEW.book_id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_progress_update AFTER UPDATE ON reading_progress BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('reading_progress', NEW.book_id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_progress_delete AFTER DELETE ON reading_progress BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('reading_progress', OLD.book_id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_books_insert AFTER INSERT ON books BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('book', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_books_update AFTER UPDATE ON books BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('book', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_books_delete AFTER DELETE ON books BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('book', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_categories_insert AFTER INSERT ON categories BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('category', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_categories_update AFTER UPDATE ON categories BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('category', NEW.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_categories_delete AFTER DELETE ON categories BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('category', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

CREATE TRIGGER sync_pending_book_categories_insert AFTER INSERT ON book_categories BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('book', NEW.book_id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;
CREATE TRIGGER sync_pending_book_categories_delete AFTER DELETE ON book_categories BEGIN
    INSERT INTO sync_pending (resource_type, resource_id, changed_at, state)
    VALUES ('book', OLD.book_id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'pending')
    ON CONFLICT (resource_type, resource_id) DO UPDATE SET
        changed_at = excluded.changed_at, state = 'pending';
END;

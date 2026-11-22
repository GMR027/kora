-- Fase 4: metadatos de PDF, estado del OCR por página y auto-índice.
-- El PDF original nunca se modifica; la capa de texto vive en assets/ocr.

CREATE TABLE pdf_document_info (
    book_id             TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    page_count          INTEGER NOT NULL CHECK (page_count >= 0),
    is_scanned          INTEGER NOT NULL DEFAULT 0 CHECK (is_scanned IN (0, 1)),
    has_native_outline  INTEGER NOT NULL DEFAULT 0 CHECK (has_native_outline IN (0, 1)),
    inspected_at        TEXT NOT NULL,
    owner_account_id    TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id     TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE TABLE pdf_ocr_status (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running', 'done', 'failed', 'skipped')),
    pages_done       INTEGER NOT NULL DEFAULT 0 CHECK (pages_done >= 0),
    pages_total      INTEGER NOT NULL DEFAULT 0 CHECK (pages_total >= 0),
    language         TEXT NOT NULL DEFAULT 'spa+eng',
    error            TEXT,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE TABLE pdf_auto_index (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    source           TEXT NOT NULL CHECK (source IN ('outline', 'heuristic', 'ocr')),
    entries          TEXT NOT NULL,
    generated_at     TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

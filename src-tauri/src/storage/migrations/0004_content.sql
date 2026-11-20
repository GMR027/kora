-- Contenido local-first. Todas las tablas de contenido guardan el propietario
-- y, si el recurso llegó por una compartición, el share de origen.

CREATE TABLE books (
    id               TEXT PRIMARY KEY,
    title            TEXT NOT NULL,
    author           TEXT,
    format           TEXT NOT NULL CHECK (format IN ('epub', 'pdf')),
    content_hash     TEXT NOT NULL,
    content_ext      TEXT NOT NULL CHECK (content_ext IN ('epub', 'pdf')),
    original_name    TEXT NOT NULL,
    cover_hash       TEXT,
    cover_ext        TEXT,
    size_bytes       INTEGER NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
    language         TEXT,
    publisher        TEXT,
    description      TEXT,
    added_at         TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    UNIQUE (owner_account_id, content_hash)
);

CREATE INDEX idx_books_title ON books (title);
CREATE INDEX idx_books_added_at ON books (added_at);
CREATE INDEX idx_books_owner ON books (owner_account_id);
CREATE INDEX idx_books_content_hash ON books (content_hash);

CREATE TABLE reading_progress (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    locator          TEXT NOT NULL,
    percent          REAL NOT NULL DEFAULT 0 CHECK (percent >= 0 AND percent <= 1),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE TABLE categories (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    color            TEXT,
    sort_order       INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    UNIQUE (owner_account_id, name)
);

CREATE INDEX idx_categories_owner ON categories (owner_account_id);

CREATE TABLE book_categories (
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    category_id      TEXT NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    PRIMARY KEY (book_id, category_id)
);

CREATE INDEX idx_book_categories_category ON book_categories (category_id);

CREATE TABLE annotations (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    kind             TEXT NOT NULL CHECK (kind IN ('highlight', 'underline', 'bookmark', 'strike')),
    locator          TEXT NOT NULL,
    text             TEXT,
    note             TEXT,
    color            TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_annotations_book ON annotations (book_id);

CREATE TABLE extracts (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    locator          TEXT NOT NULL,
    text             TEXT NOT NULL,
    note             TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_extracts_book ON extracts (book_id);

CREATE TABLE notes (
    id               TEXT PRIMARY KEY,
    book_id          TEXT REFERENCES books (id) ON DELETE SET NULL,
    title            TEXT,
    body             TEXT NOT NULL,
    audio_hash       TEXT,
    audio_ext        TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_notes_book ON notes (book_id);

CREATE TABLE voices (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    engine           TEXT NOT NULL,
    language         TEXT NOT NULL,
    profile_hash     TEXT,
    profile_ext      TEXT,
    sample_hash      TEXT,
    sample_ext       TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_voices_owner ON voices (owner_account_id);

CREATE TABLE audiobooks (
    id               TEXT PRIMARY KEY,
    book_id          TEXT NOT NULL REFERENCES books (id) ON DELETE CASCADE,
    voice_id         TEXT REFERENCES voices (id) ON DELETE SET NULL,
    status           TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running', 'done', 'failed', 'cancelled')),
    manifest_hash    TEXT,
    manifest_ext     TEXT,
    duration_ms      INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_audiobooks_book ON audiobooks (book_id);

CREATE TABLE diary_entries (
    id               TEXT PRIMARY KEY,
    book_id          TEXT REFERENCES books (id) ON DELETE SET NULL,
    entry_date       TEXT NOT NULL,
    body             TEXT NOT NULL,
    mood             TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

CREATE INDEX idx_diary_entries_date ON diary_entries (entry_date);
CREATE INDEX idx_diary_entries_owner ON diary_entries (owner_account_id);

CREATE TABLE achievements (
    id               TEXT PRIMARY KEY,
    key              TEXT NOT NULL,
    title            TEXT NOT NULL,
    description      TEXT,
    progress         REAL NOT NULL DEFAULT 0,
    unlocked_at      TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL,
    UNIQUE (owner_account_id, key)
);

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

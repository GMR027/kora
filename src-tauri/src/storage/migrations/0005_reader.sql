-- Lector: posiciones de anotación por capítulo y ajustes tipográficos por libro.

ALTER TABLE annotations ADD COLUMN chapter_index INTEGER;
ALTER TABLE annotations ADD COLUMN offset_start INTEGER;
ALTER TABLE annotations ADD COLUMN offset_end INTEGER;

CREATE INDEX idx_annotations_book_chapter ON annotations (book_id, chapter_index);

CREATE TABLE reader_settings (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    font_size        REAL NOT NULL DEFAULT 18.0 CHECK (font_size BETWEEN 12 AND 36),
    line_height      REAL NOT NULL DEFAULT 1.7 CHECK (line_height BETWEEN 1.0 AND 2.5),
    margin           INTEGER NOT NULL DEFAULT 32 CHECK (margin BETWEEN 0 AND 160),
    font_family      TEXT NOT NULL DEFAULT 'serif'
                     CHECK (font_family IN ('serif', 'sans', 'mono')),
    theme            TEXT NOT NULL DEFAULT 'light'
                     CHECK (theme IN ('light', 'dark', 'sepia')),
    texture          TEXT NOT NULL DEFAULT 'none'
                     CHECK (texture IN ('none', 'paper', 'linen', 'grid')),
    mode             TEXT NOT NULL DEFAULT 'scroll'
                     CHECK (mode IN ('scroll', 'paged')),
    zoom             REAL NOT NULL DEFAULT 1.0 CHECK (zoom BETWEEN 0.6 AND 2.5),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

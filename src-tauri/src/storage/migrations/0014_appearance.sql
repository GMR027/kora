-- Fase 12: apariencia global, temas personalizados, idioma y onboarding.

-- El lector necesita aceptar el tema/textura heredados y los nuevos valores.
ALTER TABLE reader_settings RENAME TO reader_settings_old;

CREATE TABLE reader_settings (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    font_size        REAL NOT NULL DEFAULT 18.0 CHECK (font_size BETWEEN 12 AND 36),
    line_height      REAL NOT NULL DEFAULT 1.7 CHECK (line_height BETWEEN 1.0 AND 2.5),
    margin           INTEGER NOT NULL DEFAULT 32 CHECK (margin BETWEEN 0 AND 160),
    font_family      TEXT NOT NULL DEFAULT 'serif'
                     CHECK (font_family IN ('serif', 'sans', 'mono')),
    theme            TEXT NOT NULL DEFAULT 'inherit'
                     CHECK (theme IN ('light', 'dark', 'sepia', 'contrast', 'inherit')),
    texture          TEXT NOT NULL DEFAULT 'inherit'
                     CHECK (texture IN ('none', 'paper', 'parchment', 'soft-dark', 'grid', 'linen', 'inherit')),
    mode             TEXT NOT NULL DEFAULT 'scroll'
                     CHECK (mode IN ('scroll', 'paged')),
    zoom             REAL NOT NULL DEFAULT 1.0 CHECK (zoom BETWEEN 0.6 AND 2.5),
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    origin_share_id  TEXT REFERENCES shares (id) ON DELETE SET NULL
);

INSERT INTO reader_settings
    (book_id, font_size, line_height, margin, font_family, theme, texture,
     mode, zoom, updated_at, owner_account_id, origin_share_id)
SELECT book_id, font_size, line_height, margin, font_family, theme, texture,
       mode, zoom, updated_at, owner_account_id, origin_share_id
FROM reader_settings_old;

DROP TABLE reader_settings_old;

CREATE TABLE app_preferences (
    owner_account_id TEXT PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
    theme_id          TEXT NOT NULL DEFAULT 'light',
    texture           TEXT NOT NULL DEFAULT 'paper',
    locale            TEXT NOT NULL DEFAULT 'es' CHECK (locale IN ('es', 'en')),
    onboarding_done   INTEGER NOT NULL DEFAULT 0 CHECK (onboarding_done IN (0, 1)),
    library_layout    TEXT NOT NULL DEFAULT 'grid'
                      CHECK (library_layout IN ('grid', 'list')),
    updated_at        TEXT NOT NULL
);

CREATE TABLE custom_themes (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    background       TEXT NOT NULL,
    foreground       TEXT NOT NULL,
    accent           TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE
);

CREATE INDEX idx_custom_themes_owner ON custom_themes (owner_account_id, name);

-- Las instalaciones que ya existían no deben ver el onboarding otra vez.
INSERT INTO app_preferences
    (owner_account_id, theme_id, texture, locale, onboarding_done, library_layout, updated_at)
SELECT id, 'light', 'paper', 'es', 1, 'grid', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM accounts
WHERE is_local = 1;

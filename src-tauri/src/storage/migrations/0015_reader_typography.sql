-- Tipografía editorial avanzada por libro. No se edita la migración 0014:
-- estas columnas se añaden de forma aditiva para conservar los ajustes.
ALTER TABLE reader_settings
    ADD COLUMN font_face TEXT NOT NULL DEFAULT 'newsreader';

ALTER TABLE reader_settings
    ADD COLUMN column_width INTEGER NOT NULL DEFAULT 840;

ALTER TABLE reader_settings
    ADD COLUMN text_align TEXT NOT NULL DEFAULT 'left';

ALTER TABLE reader_settings
    ADD COLUMN paragraph_indent INTEGER NOT NULL DEFAULT 0;

ALTER TABLE reader_settings
    ADD COLUMN hyphenate INTEGER NOT NULL DEFAULT 0;

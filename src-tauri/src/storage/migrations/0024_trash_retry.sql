-- Papelera lógica de libros: el registro y sus anotaciones se conservan para
-- poder restaurarlo; los assets siguen referenciados hasta el borrado
-- definitivo. Las consultas de biblioteca filtran `deleted_at IS NULL`.
ALTER TABLE books ADD COLUMN deleted_at TEXT;

CREATE INDEX idx_books_deleted_at ON books (owner_account_id, deleted_at);

-- Ajustes de narración usados en la generación. Permiten reintentar un
-- audiolibro fallido con la misma voz, estilo y reparto.
ALTER TABLE audiobooks ADD COLUMN narration TEXT;
ALTER TABLE audiobooks ADD COLUMN cast TEXT;

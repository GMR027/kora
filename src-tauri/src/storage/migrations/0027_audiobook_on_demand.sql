-- Audiolibros bajo demanda.
-- Una ficha `on_demand` existe en la sección de audiolibros y conserva
-- título, portada y favoritos, pero no guarda audio ni manifiesto. La narración
-- se sintetiza en tiempo real con el motor TTS al elegir un capítulo.

ALTER TABLE audiobooks ADD COLUMN delivery TEXT NOT NULL DEFAULT 'file'
    CHECK (delivery IN ('file', 'on_demand'));

CREATE UNIQUE INDEX idx_audiobooks_on_demand_book
    ON audiobooks (book_id, owner_account_id)
    WHERE delivery = 'on_demand';

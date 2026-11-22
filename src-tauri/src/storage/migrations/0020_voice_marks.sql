-- Fase 9: marcadores de voz durante la reproducción.
-- Reutilizamos `voice_notes` para no duplicar el almacenamiento de audio, la
-- sincronización y el panel de anotaciones. Los campos nuevos solo se rellenan
-- en los marcadores creados desde el reproductor de audiolibros.

ALTER TABLE voice_notes ADD COLUMN audiobook_id TEXT REFERENCES audiobooks (id) ON DELETE CASCADE;
ALTER TABLE voice_notes ADD COLUMN position_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE voice_notes ADD COLUMN quote TEXT;
ALTER TABLE voice_notes ADD COLUMN transcript TEXT;
ALTER TABLE voice_notes ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

CREATE INDEX idx_voice_notes_audiobook ON voice_notes (audiobook_id, position_ms);

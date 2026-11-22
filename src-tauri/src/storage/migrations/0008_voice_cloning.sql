-- Fase 6: perfiles de voz clonados. Los archivos viven en
-- assets/voices/<voice_id>/; aquí queda la metadata consultable.
-- `system` son las voces del catálogo (Piper/Android) y `cloned` los perfiles
-- creados por el usuario.

ALTER TABLE voices ADD COLUMN kind TEXT NOT NULL DEFAULT 'system'
    CHECK (kind IN ('system', 'cloned'));

ALTER TABLE voices ADD COLUMN reference_duration_ms INTEGER NOT NULL DEFAULT 0
    CHECK (reference_duration_ms >= 0);

CREATE INDEX idx_voices_kind ON voices (kind, created_at);

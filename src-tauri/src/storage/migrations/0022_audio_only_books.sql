-- Fase 11: importación de audiolibros sin transcripción.
-- El libro asociado es un EPUB mínimo que no aparece en la biblioteca normal;
-- existe para conservar título, portada, shares y compatibilidad con las
-- claves foráneas de audiolibros. Al transcribir después se pone a 0.

ALTER TABLE books ADD COLUMN audio_only INTEGER NOT NULL DEFAULT 0;

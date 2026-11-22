-- Fase 8: reparto de voces por personaje en la cola de audiolibros.
-- El JSON guarda las asignaciones manuales; al generar se resuelven las
-- voces automáticas y el resultado queda en el manifiesto.

ALTER TABLE audiobook_queue ADD COLUMN "cast" TEXT;

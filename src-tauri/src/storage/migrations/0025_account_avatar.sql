-- Foto de perfil local. Se guarda como asset en `assets/avatars/` y la cuenta
-- conserva su hash y extensión para reconstruir la ruta.
ALTER TABLE accounts ADD COLUMN avatar_hash TEXT;
ALTER TABLE accounts ADD COLUMN avatar_ext TEXT;

# Kora 0.1.6

- Sincronización: las categorías con el mismo nombre en varios dispositivos se
  reutilizan en lugar de duplicarse y las asignaciones de libros se remapean al
  id local, evitando el error `UNIQUE constraint failed` al emparejar.
- Icono Android adaptativo regenerado con fondo propio y margen de seguridad;
  el launcher ya no recorta el libro ni los audífonos.
- APK Android de 64 bits (`arm64` y `x86_64`) con `versionCode` 7.
- Instaladores Linux `.deb` y `.rpm` actualizados con el mismo arreglo de
  sincronización.

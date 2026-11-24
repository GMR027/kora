# Kora 0.1.5

- Importación de audiolibros en Android desde el selector del sistema, con
  lectura de metadatos y portada incrustada.
- Transcripción local con Whisper en el teléfono: modelos Base (147 MB) y Tiny
  (78 MB), progreso continuo, karaoke por palabra y funcionamiento con la
  pantalla apagada mediante un servicio en primer plano.
- Reproductor de audiolibros a pantalla completa con portada, capítulo,
  progreso, controles de transporte y gesto para minimizar.
- Controles de pantalla de bloqueo y notificación con título, portada, capítulo
  y progreso; las acciones de capítulo actualizan los metadatos sin reiniciar
  la reproducción.
- Estudio de Voz en Android: vista previa real con el motor TTS del teléfono,
  texto propio y velocidad 1.0×/1.2×/1.5×. La importación, exportación,
  renombrado y borrado de perfiles `.koravoice` funciona desde el selector del
  sistema.
- Lector: voces de lectura solo en español e inglés, botón de cierre en la
  ventana «Escuchar», modo ensayo circular y panel de lectura rediseñado.
- Las actualizaciones y descargas se publican únicamente en
  <https://kora.iguzman.com.mx>; la app ya no permite cambiar la URL del
  manifiesto desde Ajustes.
- APK Android de 64 bits (`arm64` y `x86_64`) con `versionCode` 6 e instaladores Linux actualizados.

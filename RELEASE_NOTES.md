# Kora 0.2.1

- **Enviar a audiolibro**: el menú de cada libro añade una ficha a Audiolibros
  sin generar un M4B. El audio se narra en tiempo real al elegir un capítulo,
  usando la voz TTS configurada para ese libro.
- Los audiolibros **bajo demanda** muestran su propia etiqueta y filtro, no
  ocupan espacio de audio y recuerdan el último capítulo escuchado.
- La lista de capítulos usa las secciones reales del EPUB o las páginas del PDF.
  Se puede pausar, reanudar y continuar desde el mini-reproductor global.
- Los audiolibros bajo demanda no se añaden a álbumes ni se comparten, porque no
  tienen un archivo que transferir. Borrar la ficha no borra el libro.
- Se mantienen las mejoras de Kora 0.2.0: lectura continua con Supertonic,
  limpieza de voz, reproducción en segundo plano y mini-reproductor global.
- APK Android `arm64` con `versionCode` 12. Los paquetes de Linux se actualizan
  con los mismos cambios.

# Kora 0.1.9

- Audiolibros: nuevos álbumes locales para agrupar por sagas, cursos o temas,
  ordenar la escucha y reproducirlos en cola continua.
- La base de datos añade la migración 26 (álbumes) sin borrar ni modificar los
  datos existentes.
- Conversión desde la biblioteca: **Crear audiolibro** en el menú de un EPUB o
  PDF y **Convertir a EPUB** en un audiolibro sin texto, con Whisper Tiny/Base
  y progreso cancelable. Los audiolibros con texto se pueden exportar como EPUB.
- Generación M4B en Android con `MediaCodec`/`MediaMuxer` y un servicio en
  primer plano; los capítulos se conservan en el manifiesto de Kora.
- Estudio de Voz más limpio: el catálogo nativo de Android se reduce a las
  voces locales que merece la pena usar y oculta variantes técnicas, de red y
  duplicadas. El botón «Todas las variantes» sigue disponible para quien
  necesite el listado completo del sistema.
- Nombres legibles para las voces nativas: «Español (Estados Unidos)»,
  «Inglés (Estados Unidos)», sin códigos internos.
- Nuevo pack opcional Supertonic 3: seis voces neuronales (tres por idioma)
  para español e inglés, con descarga y activación desde Ajustes → Voz.
- El pack no va incluido en el APK ni en los instaladores: se descarga una
  sola vez (~383 MB) y se ejecuta en el dispositivo con ONNX Runtime.
- Carga perezosa: el modelo se carga al narrar y se libera tras 90 segundos
  de inactividad. En el móvil usa unos 470 MB adicionales mientras está activo.
- Licencia OpenRAIL-M visible y aceptación explícita antes de descargar.
- Lectura real en Android con las voces mejoradas, con pausa, reanudación y
  parada integradas en la sesión de medios.
- Piper y TextToSpeech siguen siendo los motores de respaldo cuando el pack
  no está instalado o está desactivado.
- APK Android para `arm64` con `versionCode` 10. El emulador `x86_64` se puede compilar desde el código.

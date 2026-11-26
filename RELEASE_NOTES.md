# Kora 0.2.0

- Lectura en voz alta continua con Supertonic: la frase siguiente se sintetiza
  mientras suena la actual, sin esperar a generar un audiolibro completo.
- Android reproduce la cola de Supertonic con una playlist persistente de
  Media3. Las frases dejan de recrear el reproductor y desaparecen los silencios
  de casi un segundo entre frases.
- El lector muestra «Preparando voz…» mientras carga el modelo y el primer
  fragmento.
- El resaltado de la frase se sincroniza con el inicio real del audio.
- La lectura sigue sonando al cambiar de pantalla. Un mini-reproductor global
  permite pausar, reanudar, detener y volver al libro.
- El autoavance de capítulos funciona también fuera del lector.
- El TTS se coordina con el reproductor de audiolibros: solo suena una fuente a
  la vez y se conserva la posición del audiolibro al cambiar.
- Las voces Supertonic se usan por defecto cuando el pack está instalado, sin
  pisar una voz elegida explícitamente.
- El texto narrado limpia símbolos y ruido tipográfico (`??`, `____`, `$`, `%`,
  `^`, emojis, marcas de diálogo, etc.). Se conserva el texto visible y la
  puntuación que aporta prosodia; los audiolibros también usan esta limpieza.
- Se mantienen Piper, NeuTTS y el TTS nativo como motores de respaldo.
- APK Android `arm64` con `versionCode` 11. Los paquetes de Linux se actualizan
  con los mismos cambios de lectura y limpieza de voz.

# Migración visual y funcional a Lumina Reader

Fuente de diseño: `/home/edgar/Descargas/layout` (Lumina Editorial Reader, Lumina Nocturne, pantallas de escritorio/móvil y prototipos HTML).
Logo: `/home/edgar/Descargas/layout/logo.png`.

La migración ya no es solo cromática: se trasladó la estructura editorial de Lumina a la shell, la biblioteca, el lector, los ajustes y las pantallas de cuenta. También se replicaron las herramientas locales que no dependen de servicios de pago.

## Implementado

### Shell global
- Barra lateral fija de 288 px color marfil (`#f5f3ef`) con logo de 36 px, nombre Kora, badge `LOCAL`, navegación de 7 secciones y tarjeta de perfil al pie.
- Estado activo con degradado cobre/ámbar y sombra cálida, como en Lumina.
- Barra superior fija de 64 px con píldora de lectura actual, contador de libros en curso, búsqueda, sincronización y avatar. Cuando hay un audiolibro en reproducción, la píldora se sustituye por un mini-reproductor con portada, capítulo, progreso y controles.
- Cabecera móvil con logo, `KORA READER`, título de la sección activa, contador y avatar.
- Navegación inferior de 5 destinos con iconos y estado ámbar.
- Tipografía: **Newsreader** para títulos y lectura; **Plus Jakarta Sans** para interfaz. Licencias OFL incluidas.
- Paleta marfil/ámbar/cobre/tinta, superficies cálidas, radios 8–24 px y sombras suaves.

### Biblioteca
- Cabecera «Catálogo personal / Mi Biblioteca» con acciones **Importar (PDF / EPUB)**, **Generar Audiolibro**, selector de orden y conmutador cuadrícula/lista.
- Búsqueda semántica con chips de filtro: Todos, Leyendo ahora, **Favoritos**, **Audiolibros**, EPUB, PDF, Compartidos y categorías.
- Tira de 4 estadísticas (libros, en curso, terminados, categorías).
- Sección «Continuar leyendo» independiente, con portada en perspectiva, lomo, sello de progreso, porcentaje, barra y acciones.
- Estantería principal de tarjetas blancas con portada, lomo, formato, estado, porcentaje, categoría, título editorial, autor, progreso, estrella de favoritos y menú.
- Favoritos persistentes en SQLite (`book_favorites`).
- La sección de audiolibros replica las tarjetas con favorito por audiolibro (`audiobook_favorites`), share `SHARE-AUDIOBOOK` y un mini-reproductor persistente en la barra superior de escritorio.
- Panel lateral **Detalles del Libro** con portada, metadatos, última lectura, abrir en el lector, **escuchar en voz alta**, **exportar notas** y cambiar portada.
- Menú contextual convertido en hoja modal centrada / bottom sheet móvil.

### Lector
- Shell de Lumina visible: barra lateral y barra superior permanecen.
- Sub-barra con regreso a Biblioteca, título, autor/capítulo, progreso, **página actual/total del capítulo**, búsqueda, Aa, TTS, marcador, pantalla completa, IA local, anotaciones y espejo.
- Página editorial de **una sola columna** centrada y configurable (520–1000 px), sin tarjeta con borde; capítulos con Newsreader itálica y filete cobre. En ventanas anchas pasa a dos columnas.
- **Vista de dos columnas / hoja doble automática** en modo paginado: EPUB reparte el texto en dos columnas a partir de ~880 px de área de lectura y PDF muestra dos páginas enfrentadas desde ~700 px (portada sola y después 2–3, 4–5…).
- **Buscador dentro de todo el libro** con resultados por capítulo, fragmento y salto a la posición.
- **Marcador rápido** en la posición actual.
- **Copiar cita** y **Escuchar selección** desde el menú de selección; el audio
  se detiene al terminar el extracto.
- **Temporizador de sueño** para TTS: 15, 30 o 60 minutos.
- **Pantalla completa** desde la barra del lector.
- **Tipografía real por libro**: Newsreader, Literata, EB Garamond, Atkinson Hyperlegible, Plus Jakarta Sans y OpenDyslexic; tamaño, interlineado, márgenes, ancho de columna, alineación izquierda/justificada, sangría tradicional, guionado, atenuación de ilustraciones y guía de lectura focalizada.
- Botón **Aplicar a toda la biblioteca** para guardar la tipografía como base global y aplicarla a todos los libros.
- Panel TTS con velocidad, tono, voz, voces por personaje, modo ensayo y auto-avance.
- Paneles de ajustes, TOC, anotaciones y espejo reestilizados como hojas blancas redondeadas.

### IA local (gratuita, sin nube)
- **Panel «IA local & Notas»** en el lector: resumen de escena, términos clave, personajes y chat sobre el capítulo actual.
- El resumen se puede guardar como nota local.
- **Ollama en PC** y **llama.cpp en Android/LAN** mediante APIs locales compatibles con OpenAI.
- Configuración en **Ajustes → IA local**: proveedor, URL de Ollama, URL de llama.cpp, modelo, temperatura y límite de tokens.
- Detección automática de modelos, comprobación de conexión y guía de instalación integrada.
- Android permite HTTP local en producción para hablar con `llama-server` o con Ollama del PC en la LAN.
- Documentación: `docs/local-ai.md`.
- No hay cuentas, cuotas, claves de API ni envío de datos a Internet.

### Ajustes
- Vista previa editorial viva en Apariencia: refleja el tema activo, tipografía, texto y métricas.
- Galería de temas Lumina (claro, oscuro, sepia, alto contraste, **menta velada**) y temas personalizados.
- **Tipografía base global** con vista previa: familia, tamaño, interlineado, ancho, márgenes, alineación, sangría y guionado; botón para aplicar a toda la biblioteca.
- **Bienestar**: filtro de luz ámbar, atenuación de pantalla, escala de interfaz 80/90/100/110/125 %, pausa 20-20-20 con recordatorio, límite nocturno y modo cama.
- **Almacenamiento**: desglose de espacio (base de datos, libros, audiolibros, voces, modelos, papelera, logs, exportaciones, temporales), limpieza de temporales y vaciado de papelera.
- **IA local** y **Sync** con pestañas propias.

### Cuenta, Diario, Audiolibros, Voz y Sincronización
- Cabeceras y paneles alineados con la retícula de Lumina.
- Cuenta con tarjeta de perfil editorial ampliada, avatar de 84 px y edición del nombre visible.
- **Privacidad y notas**: exportación completa a Markdown o JSON de subrayados, notas, extractos y notas de voz.
- Diario con «Diario de Lectura & Hábitos», estadísticas, calendario de intensidad, bitácora, logros (incluidos Lector Nocturno, Bimodal Master, Centinela del Enfoque y Devorador de Libros), Pomodoro con presets 25/5, 50/10 y 90/15 y **sonido ambiente** sintetizado (silencio, lluvia, ruido, chimenea).
- Audiolibros con cabecera editorial, motor local real, pestañas Todos / Generados / Importados / Tejido / En proceso, búsqueda por título, autor o voz y orden por fecha, título, duración o progreso.
- Tarjetas con portada, origen, voz, duración, progreso, última escucha y acciones de abrir texto, descargar el M4B, revisar capítulos y borrar.
- Generador en modal con alcance libro/capítulo/selección, estilos de narración (neutro, calmado, dramático, enérgico), velocidad base de 0.75× a 1.5×, selección de voz, muestra de voz adaptada al estilo y resumen del M4B.
- Importación en modal con estado de ffmpeg y Whisper Base, descarga del modelo desde la propia vista y extracción de la portada embebida.
- Detección de capítulos al importar: metadatos embebidos, pausas largas con `silencedetect`, marcas acústicas de 440 Hz/1 kHz y bloques fijos de diez minutos como respaldo. La transcripción se agrupa por capítulo detectado y el método se muestra en el modal de capítulos.
- Cola de generación persistente en SQLite, con panel de pendientes, reordenación por llegada, eliminación individual y vaciado.
- Controles de pausa, reanudación y cancelación de la generación activa, con progreso por capítulo y limpieza del audio parcial al cancelar.
- Normalización de volumen LUFS en dos pasadas con `loudnorm`, presets de ecualización (neutro, voz clara, cálido, brillante) y re-codificación que conserva capítulos y metadatos.
- Forma de onda real del audiolibro en el reproductor, calculada desde PCM a 8 kHz y cacheada por hash de audio.
- Sincronización texto-audio palabra a palabra: el manifiesto guarda líneas por frase con sus tiempos, Whisper aporta timestamps de palabra en los audiolibros importados y el reproductor resalta la palabra actual. Incluye una vista de lectura sincronizada con salto por frase.
- Multivot por personaje: detección de diálogos, reparto manual o automático de voces, rotación entre voces instaladas y reparto resuelto en el manifiesto y en la cola persistente.
- Marcadores de voz durante la reproducción: comando «Kora, marca esto», segmentación por silencios, transcripción local con Whisper Tiny/Base, cita anclada al pasaje, comentario dictado y salto al punto de escucha.
- Favoritos por audiolibro y shares `SHARE-AUDIOBOOK` con transferencia de M4B por bloques; mini-reproductor persistente en la barra superior de escritorio.
- Reproductor de audiolibros con velocidad, temporizador de sueño, saltos ±30 s, capítulos y apertura del texto.
- **Estudio de Voz** con cabecera editorial, latencia real de la última prueba y media de las últimas diez, pestañas con contadores, buscadores independientes, filtros por idioma, género y estado, tarjetas con lomo ámbar, onda siempre visible, velocidad de reproducción 1.0×/1.2×/1.5×, texto propio de prueba, detalles por voz, voz y perfil destacados, y rail del motor NeuTTS con componentes, progreso y compromiso de privacidad. La descarga de voces del sistema muestra progreso real y avisa si faltan modelos antes de grabar.
- **Gestión de modelos de clonación** por idioma y por archivo: descarga individual con progreso, borrado y reparación con confirmación.
- **Grabación de muestra de voz** en la pestaña de clonadas: selección de micrófono, medidor de nivel, aviso si se oye bajo, máximo 30 s, recorte de silencios, normalización, avisos de saturación, escucha antes de usar y borrado de la muestra temporal al crear el perfil. Los perfiles se pueden renombrar, su muestra se puede reemplazar y desde los detalles se puede escuchar la muestra original. En Android se mantiene la importación de archivo y se ocultan las acciones que no están disponibles (grabación, descarga de voces y gestión de modelos).
- Sincronización con **registro de actividad** y exportación de auditoría JSON.
- Sincronización mantiene pestañas tipo pill y tarjetas de superficie cálida.

### Sistema
- Temas claro/oscuro/sepia/contraste/menta actualizados a los valores de Lumina.
- Variables derivadas (`surface-3`, `surface-4`, `accent-3`, `border-warm`) calculadas según el tema activo.
- Iconos de app, favicon y logo regenerados desde el logo de Lumina.
- Tema de lectura por defecto: paginado, 19 px y margen 48.

## Pendiente / futuras mejoras




Estas piezas aparecen en los prototipos de Lumina y todavía no se han implementado, o requieren un trabajo nativo que no se simula:

1. **llama.cpp embebido en el APK de Android**: hoy Kora se conecta a `llama-server` local; embeberlo exige compilar por ABI y empaquetar el modelo GGUF.
2. **Descarga y gestión de modelos desde la app**: hoy se descargan con Ollama/llama.cpp.
3. **Panel de detalles enriquecido**: tamaño de archivo, última sincronización, voz de lectura y recuento de notas/subrayados.
4. **Estadísticas globales de lectura**: tiempo leído, racha, audiolibros totales y tiempo restante estimado.
5. **Reproductor mini persistente** en la barra superior con controles de audio reales.
6. **Perfiles de lectura guardados por nombre** (la base global y los ajustes por libro ya existen).
7. **Vista semanal/anual del Diario**: el calendario sigue siendo mensual; la exportación anual ya existe.
8. **Plan PRO / membresía, pagos y facturas**: Kora es local-first y no implementa cobros.
9. **Sincronización en la nube**: Kora usa P2P local; no se añade una nube de pago.
11. **Control explícito del spread**: hoy la hoja doble es automática según el ancho; falta un ajuste para forzarla o desactivarla.
12. **Efectos de estantería 3D** más elaborados; se añadió lomo, brillo y sombra.
13. **Mover la biblioteca a un SSD externo** desde la interfaz; es una operación de riesgo que requiere migración y reinicio.
14. **Buscar actualizaciones**: no hay servidor de actualizaciones configurado.
15. **Grabación de muestra de voz en Android**: por ahora solo está disponible la importación de archivo.
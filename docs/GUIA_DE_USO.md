# Guía de uso de Kora

## Navegación

En escritorio verás una barra lateral con todas las secciones. En móvil hay una
barra inferior con Biblioteca, Lector, Audiolibros, Diario y **Más**; desde
**Más** se accede a Voz, Cuenta, Sync y Ajustes. Esas cuatro pantallas se abren
como una pila con botón **Volver**.

Si hay un audiolibro sonando, un mini-reproductor aparece sobre la barra
inferior con portada, capítulo, tiempo, retroceso de 15 s y pausa. En la
pantalla de Audiolibros se sustituye por el reproductor completo. En escritorio,
fuera de la pantalla de Audiolibros, el mismo reproductor aparece en la barra
superior con la portada, el capítulo, el progreso y los controles de pausa y
retroceso.

Cada cambio de pantalla tiene una transición suave y respeta
`prefers-reduced-motion`.

## Biblioteca

- Arrastra uno o varios EPUB/PDF o pulsa **Importar**.
- Filtra por categoría, **Sin categoría**, **Terminados**, **Sin empezar**,
  **Favoritos**, **Compartidos** o por formato.
- Cambia entre cuadrícula y lista; la preferencia se guarda en SQLite. Puedes
  ordenar por fecha, última lectura, título o progreso.
- La barra de búsqueda combina coincidencias por título/autor con el índice
  semántico local. Si el modelo no está preparado, Kora ofrece descargarlo.
- **Gestionar categorías** permite crear, renombrar, recolorear y borrar
  categorías. Desde las opciones de un libro puedes añadirlo o quitarlo de una
  categoría.
- **Detectar categorías** analiza metadatos, similitud e IA local y deja las
  propuestas en la bandeja de **Sugerencias**. Nada se asigna hasta que aceptas
  la sugerencia; puedes aceptar todas, descartarlas o crear la categoría.
- En el menú de un PDF, **Convertir PDF a EPUB** abre un asistente con vista
  previa del texto, modo de capítulos y la opción de reemplazar el PDF o crear
  un EPUB nuevo. Si reemplazas, se conservan progreso, categorías, anotaciones
  y audiolibros.
- **Editar metadatos** permite corregir título, autor, idioma, editorial y
  descripción. Los metadatos del EPUB/PDF se leen al importar y al abrir.
- **Papelera** guarda los libros borrados con sus notas, marcadores, progreso y
  archivos. Puedes restaurarlos o borrarlos definitivamente. Si vuelves a
  importar el mismo archivo mientras está en la papelera, se restaura en lugar
  de duplicarse.
- Los libros de shares en modo `read` muestran la etiqueta **Solo lectura**.

## Notas

- **Notas** reúne las notas, citas, marcadores y notas de voz de todos los
  libros en una sola libreta.
- La búsqueda filtra por texto, libro, cita o nota; las pestañas separan notas,
  citas, marcadores, notas de voz y reacciones.
- **Abrir en el libro** salta al pasaje exacto. Desde la libreta también puedes
  borrar una entrada.
- Las citas permiten editar su nota y referencia. Las notas de voz permiten
  editar texto, transcripción y cita sin perder el audio. Las reacciones
  permiten cambiar el tipo y el comentario.

## Lector

### EPUB

- Selecciona texto y usa la barra de selección: aparecen **Nota**, **Resaltar**
  (con paleta de colores), **Subrayar**, **Escuchar selección**, **Copiar cita**
  y **Consultar**. **Escuchar selección** reproduce solo ese extracto y se
  detiene al terminar; **Anotaciones** abre el panel con el pasaje seleccionado.
- **Índice** abre la tabla de contenidos.
- **Ajustes de lectura** permite fuente, tamaño, interlineado, márgenes, tema,
  textura y modo paginado/scroll.
- El tema del lector puede **heredar** el tema global o sobrescribirlo para ese
  libro.
- Los controles TTS permiten reproducir, pausar, cambiar velocidad/tono y elegir
  voz. Las frases se resaltan sincronizadas con el audio.

### Ventanas del lector

- **Índice y navegación** reúne el índice, la búsqueda en todo el libro y el
  salto directo a una página del capítulo.
- **Marcadores y citas** agrupa marcadores, notas, citas y reacciones, con
  búsqueda local y un botón para marcar la página actual.
- La pestaña **Marcadores** muestra el pasaje seleccionado con la paleta del
  resaltador y las acciones Resaltar, Subrayar, Escuchar, Copiar cita y
  Consultar. **Nueva nota** abre el editor de la nota marginal con el pasaje
  citado.
- **Consultar** abre una consulta de términos con la IA local: definición
  contextual, etimología cuando el modelo la conoce, sinónimos y uso en el
  pasaje. Se puede copiar o guardar como nota.
- **Métricas de lectura** (botón de gráfico en la barra de progreso móvil y
  **Modo ensayo** en el reproductor TTS) muestra precisión, ritmo e historial de
  los últimos intentos a partir de la grabación comparada con Whisper.

### PDF

- Usa `pdf.js` para renderizar el documento.
- Puedes navegar por páginas, hacer zoom, buscar texto en todo el PDF y usar
  anotaciones.
- Selecciona texto y abre **Anotaciones**: el pasaje seleccionado incluye
  **Escuchar** para reproducir solo ese extracto de la página.
- Si hay PDFium/Tesseract disponibles, el OCR y el auto-índice añaden texto
  buscable.
- **Convertir a EPUB** genera un EPUB reflowable con portada, capítulos e
  índice. Puedes elegir la detección de capítulos (automática, índice del PDF,
  encabezados o bloques de páginas) y reemplazar el PDF conservando el mismo
  libro, o crear una copia nueva. Las anotaciones, notas, extractos y
  reacciones se reubican por texto; el progreso se ajusta a los capítulos
  nuevos. Las imágenes, tablas y fórmulas no se conservan.

## Audiolibros

- Genera un M4B completo, por capítulo o por selección desde el botón
  **Generar audiolibro**; el modal permite elegir libro, alcance, estilo de
  narración (neutro, calmado, dramático o enérgico), velocidad base y voz.
- La muestra de voz aplica el estilo y la velocidad elegidos, y cada audiolibro
  guarda esos ajustes en su manifiesto para mostrarlos en la tarjeta.
- Al iniciar la síntesis, el trabajo entra en una **cola persistente**. Si no
  hay otra tarea activa empieza de inmediato; si la hay, espera su turno y se
  puede reordenar con las flechas, quitar o vaciar desde el panel «Cola de
  generación».
- Durante la generación puedes **pausar**, **reanudar** o **cancelar** desde la
  franja de tarea activa. La cancelación descarta el audio parcial. Si cierras
  la app, la cola pendiente se conserva; el trabajo que estaba en curso vuelve
  a la cola y se reintenta al volver a abrirla.
- Filtra por **Todos**, **Generados**, **Importados**, **Tejido** o **En proceso**,
  busca por título, autor o voz y ordena por fecha, título, duración o progreso.
- Cada tarjeta muestra portada, origen, voz, duración, porcentaje completado y
  última escucha. Los iconos permiten abrir el texto, descargar el audio,
  revisar capítulos o borrar el audiolibro.
- Si una generación falla o se cancela, la tarjeta muestra **Reintentar
  generación**. El reintento reutiliza la voz, el alcance, el estilo y el
  reparto guardados, y conserva los marcadores de voz del audiolibro.
- La importación acepta M4B, M4A, MP3, AAC, FLAC y OGG. En escritorio comprueba
  ffmpeg y Whisper Base, permite descargar el modelo y conserva la portada
  embebida cuando existe. En Android usa `MediaMetadataRetriever` para leer
  metadatos y portada, y reparte el audio en bloques de diez minutos. Puedes
  pausar, reanudar o cancelar la importación desde la franja de tarea activa; al
  cancelar se elimina el audio copiado y los archivos temporales.
- Los capítulos se toman de los metadatos del archivo. Si no los trae, Kora
  detecta pausas largas con ffmpeg y marcas acústicas de 440 Hz o 1 kHz; como
  último recurso divide el audio en bloques de diez minutos. El origen se
  muestra en el modal de capítulos.
- En el reproductor puedes pausar, buscar posición, cambiar la velocidad
  (0,75×–2×), saltar por capítulos y pulsar **Abrir texto** para ir al pasaje
  exacto en el lector. Sobre la barra de progreso se dibuja la **forma de onda**
  real del audio, con la parte reproducida en color de acento.
- En **Android**, la reproducción continúa en segundo plano y con la pantalla
  apagada. La notificación y la pantalla de bloqueo muestran los controles de
  reproducción, el temporizador de sueño sigue funcionando aunque el WebView esté
  suspendido y el botón del sol del reproductor permite mantener la pantalla
  encendida mientras escuchas. La importación de audio está disponible en el
  móvil; la generación y la transcripción automática siguen siendo de escritorio.
- La **lectura sincronizada** muestra la frase que suena bajo el reproductor y
  resalta la palabra actual. **Leer y escuchar** abre el capítulo completo con
  todas las frases sincronizadas: toca una frase para saltar a ese punto. En
  audiolibros generados la sincronización usa los límites reales de cada
  fragmento; en los importados, Whisper alinea las palabras. Si el audiolibro
  es anterior a esta función, la sincronización se estima por longitud de
  frase y la insignia lo indica.
- En el generador puedes activar **Multivot por personaje**. Kora detecta los
  personajes con diálogo en el alcance elegido y muestra cuántas líneas tiene
  cada uno. Asigna una voz concreta o deja «Voz automática»: las voces se
  reparten en rotación entre los personajes, mientras el narrador mantiene la
  voz principal. Se necesitan al menos dos voces instaladas. El reparto final
  queda visible en el modal de capítulos de cada audiolibro.
- **Marcadores de voz**: activa la escucha en el reproductor y di «Kora, marca
  esto» (o el comando que guardes en el modal). Kora transcribe la frase con
  Whisper local y guarda la cita que estaba sonando, tu comentario y la posición
  exacta. Puedes cambiar el comando, descargar Whisper Base, escuchar el audio
  del marcador y volver a ese punto desde el propio panel. Usa auriculares para
  que la narración no se cuele por el micrófono. Requiere escritorio.
- **Normalización y timbre** abre un tratamiento de audio: normaliza el volumen
  a un objetivo LUFS (−16, −18, −20 o −23) y aplica un preset de ecualización
  (neutro, voz clara, cálido o brillante). El audio se vuelve a codificar y
  conserva sus capítulos; puede tardar unos minutos.
- **Favoritos**: pulsa la estrella de una tarjeta para guardarla en el filtro
  **Favoritos**. La preferencia se guarda por audiolibro, no por libro.
- **Compartir**: abre el botón de compartir de una tarjeta o del reproductor.
  Kora pide tu PIN y crea una clave `SHARE-AUDIOBOOK` con permiso de copia o
  solo lectura, caducidad y usos máximos. Se transfieren el M4B, sus capítulos,
  la portada y el texto del libro. La clave y el QR se muestran una vez y luego
  se pueden recuperar desde **Cuenta**. Para que el otro dispositivo te
  encuentre, deja activo el anuncio de **Sync → Compartir**.
- **Tejido** convierte extractos en un mini-libro EPUB + audiolibro.

## Voz

- **Voces del sistema**: instala/descarga voces y pruébalas.
- **Voces clonadas**: crea un perfil con un clip de 3–30 s, una transcripción y
  consentimiento; importa/exporta `.koravoice`; prueba o borra perfiles.
- **Modelos**: descarga los modelos NeuTTS/NeuCodec por idioma.

## Sync

- Inicia Sync para anunciar el dispositivo en la LAN.
- Empareja desde **Cuenta → Emparejar** con SPAKE2 + SAS.
- En **Sync** puedes ver peers, sincronizar, detener el anuncio y elegir qué
  categorías compartes con cada dispositivo.
- El modo compartir anuncia el dispositivo para handoff, lectura en espejo y
  transferencia de shares.

## Cuenta

- Crea la cuenta local con PIN (Argon2id).
- Edita el **nombre visible**. El cambio se refleja en la barra superior y en
  la tarjeta de perfil lateral.
- Pulsa la foto de perfil para subir una imagen PNG, JPG o WebP. Kora la
  recorta en cuadrado, la guarda en `assets/avatars/` y la muestra en círculo.
  **Quitar foto** vuelve a las iniciales del nombre.
- Muestra/rota la clave de conexión y su QR.
- Cambia el PIN.
- **¿Olvidaste tu PIN?** permite restablecerlo sin el anterior. Se regenera la
  identidad de la cuenta y la clave de conexión; la biblioteca y las notas se
  conservan, pero los dispositivos y shares anteriores dejan de ser válidos.
- **Zona peligrosa → Eliminar cuenta y datos** borra cuenta, biblioteca, notas,
  audiolibros y voces clonadas al reiniciar Kora. Los modelos de IA descargables
  se conservan.
- Consulta dispositivos, revoca dispositivos y gestiona shares creados o
  recibidos. Los shares de audiolibro aparecen como **Audiolibro** y su clave
  puede recuperarse con el PIN.

## Diario, logros y Pomodoro

- El diario registra automáticamente sesiones de lectura/escucha y Pomodoros.
- **Añadir nota** crea entradas manuales con fecha, texto y estado de ánimo;
  las notas manuales se pueden editar y cualquier entrada se puede borrar.
- El heatmap muestra la actividad por día; al seleccionar un día ves sus
  entradas.
- Los logros se desbloquean con libros, extractos, notas, minutos, páginas,
  rachas, Pomodoros, audiolibros, shares y notas de voz.
- El Pomodoro es configurable (foco/descanso/descanso largo/rondas). Al
  completar un foco se registra en el diario y cuenta para logros.
- Exporta el mes o el año a EPUB; opcionalmente genera un audiolibro M4B con la
  voz elegida.

## Ajustes

### Apariencia

- Temas integrados: claro, oscuro, sepia y alto contraste.
- Temas personalizados con fondo, texto y acento. Kora calcula superficies,
  bordes y contraste, avisa si el contraste es bajo y permite editar o borrar
  los temas creados.
- Texturas de lectura: papel, pergamino, oscuro suave, cuadrícula, lino y lisa.
- Idioma de interfaz: español o inglés.

### Lectura

- Textura por defecto para el lector.
- Acceso rápido al lector para ajustar tipografía por libro.

### IA local

- Elige proveedor (**Automático**, **Ollama** o **llama.cpp**) y la URL del
  servidor. Kora comprueba la conexión y lista los modelos de chat disponibles.
- Ajusta modelo, temperatura y máximo de tokens.
- Todo se ejecuta en tu equipo o en la red local: no hay cuentas, cuotas ni
  envío de libros a Internet. Guía en `docs/local-ai.md`.

### Organización automática

- Activa o desactiva las sugerencias de categoría y el análisis al importar.
- Permite usar la IA local, proponer categorías nuevas y ajustar los umbrales
  de similitud e IA.
- **Clasificar libros sin categoría** y **Reanalizar toda la biblioteca**
  lanzan el trabajo en segundo plano.
- **Añadir categorías base que falten** instala el catálogo inicial y sus alias.
- Las propuestas se revisan en **Biblioteca → Sugerencias**; nada se asigna sin
  confirmación.

### Bienestar

- Accesos a Diario, Logros y Pomodoro.

### Sync

- Selección de categorías por dispositivo emparejado.

### Almacenamiento

- Rutas privadas, base de datos, assets, exportaciones, papelera y logs.
- **Vaciar archivos borrados** elimina definitivamente los archivos que ya se
  habían movido a la carpeta `trash/` (por ejemplo, tras borrar un libro desde
  **Biblioteca → Papelera**).
- **Crear copia de seguridad** genera un ZIP con la base de datos, libros,
  portadas, fotos de perfil, audiolibros, voces y notas. Los modelos de IA
  descargables no se incluyen.
- **Restaurar copia** valida el ZIP y prepara la restauración; se aplica al
  cerrar y volver a abrir Kora. La base anterior se conserva como
  `app.sqlite.before-restore-*` en la carpeta `db/`.
- Estado y preparación del modelo semántico.

## Accesibilidad

- Objetivos táctiles mínimos de 48dp.
- Foco visible y enlace **Saltar al contenido**.
- Etiquetas ARIA en navegación, progresos y controles principales.
- Región `aria-live` para toasts y estados.
- El tema de alto contraste y el aviso de contraste ayudan con baja visión.
- Las animaciones se reducen automáticamente con `prefers-reduced-motion`.

## Atajos y detalles

- `Enter` en los campos de búsqueda/importación ejecuta la acción.
- `Esc` cierra paneles y diálogos del lector cuando están abiertos.
- Los cambios de tema/idioma se guardan por cuenta en SQLite.
- El modelo semántico y las voces se descargan una vez y quedan en la carpeta
  privada de datos.

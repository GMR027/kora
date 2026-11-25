# Kora

Lector de EPUB/PDF local-first con síntesis de voz, clonación de voz y
sincronización peer-to-peer en red local. Construido con **Rust + Tauri 2** y
**Svelte 5**, con objetivos Linux y Android.

## Descarga

Kora se descarga únicamente desde el clúster oficial:

- Landing y descargas: <https://kora.iguzman.com.mx/#descargas>
- Android: <https://kora.iguzman.com.mx/downloads/Kora-0.1.9-android.apk>
- Linux Debian/Ubuntu: <https://kora.iguzman.com.mx/downloads/Kora_0.1.9_amd64.deb>
- Linux Fedora/RPM: <https://kora.iguzman.com.mx/downloads/Kora-0.1.9-1.x86_64.rpm>

La app consulta `https://kora.iguzman.com.mx/downloads/latest.json` y descarga
la actualización desde ese mismo host. GitHub se usa solo para el código
fuente: no se publican binarios, releases ni GitHub Pages.

> **Estado: fase 12 + utilidades de biblioteca.** La biblioteca
> importa EPUB/PDF por SHA-256. El lector abre EPUB con capítulos saneados y PDF
> con `pdf.js`, incluyendo progreso por página, selección, OCR local para PDFs
> escaneados y auto-índice. El motor TTS añade lectura por frases con resaltado
> sincronizado, pausa/reanudación, velocidad 0.5×–3×, preprocesador expresivo,
> voces por personaje, modo ensayo con Whisper y TTS nativo en Android, además
> de transcripción local de audiolibros en el propio teléfono. Las
> fases 6–8 añadieron clonación de voz, audiolibros M4B, Tejido y anotaciones
> superpuestas. La fase 9 añadió descubrimiento mDNS, transporte QUIC con TLS
> mutuo Ed25519, diff por hash, CRDT con `yrs`, handoff y lectura en espejo. La
> fase 10 añadió cuenta local con PIN Argon2id, claves Ed25519, vinculación de
> dispositivos con código temporal y QR, y shares con permisos, caducidad y usos. La fase 11
> añade búsqueda semántica offline con `fastembed` y `multilingual-e5-small`,
> diario automático por sesión con heatmap, 22 logros, Pomodoro configurable,
> sync selectivo por dispositivo y exportación del diario a EPUB/M4B. La fase
> 12 añade temas globales y personalizados por cuenta, diez texturas de
> lectura, onboarding, navegación lateral/inferior con transiciones, panel de
> voz, reproductor con velocidad/capítulos/sincronización con texto/karaoke,
> multivot por personaje y marcadores de voz «Kora, marca esto», favoritos de
> audiolibros, shares de M4B y mini-reproductor persistente en la barra
> superior, accesibilidad 48dp, i18n español/inglés y empaquetado Linux +
> validación del APK núcleo por CI.
>
> Sobre esa base se añadieron la **IA local** (asistente Qwen2.5 en Android,
> con Ollama/llama.cpp opcionales), la
> **clasificación asistida** de libros y audiolibros con sugerencias de
> categoría, la **conversión PDF→EPUB** con reubicación de anotaciones, la
> **conversión entre texto y audio** (EPUB/PDF→M4B y audiolibro sin texto→EPUB
> con Whisper), los **álbumes de audiolibros**, la **papelera con restauración**,
> las **copias de seguridad** y la optimización de consultas de la biblioteca.
> La cuenta local permite editar el nombre y subir una foto de perfil que se
> recorta en círculo. El escritorio se empaqueta como `.deb`/`rpm`/AppImage; el
> APK núcleo se valida en CI.

### Verificación de esta fase

| Comprobación | Resultado |
|---|---|
| `pnpm build` (tsc + Vite) | ✅ |
| `pnpm check` (svelte-check) | ✅ 0 errores, 0 warnings |
| `cargo fmt --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ 226 unitarios + 5 M4B + 5 flujo de audiolibros + 4 PDF/OCR + 2 TTS + 1 clonación (E2E opcional); 3 ignorados |
| Temas y texturas | ✅ 4 temas integrados + temas por cuenta, 10 texturas y contraste |
| i18n | ✅ español/inglés tipado con paridad de claves |
| Accesibilidad | ✅ 48dp, foco visible, skip link, ARIA y reduced-motion |
| Foto de perfil | ✅ subida local, recorte circular, asset SHA-256 y borrado |
| Linux `.deb` | ✅ paquete generado y documentado |
| Android APK núcleo | ✅ workflow x86_64 y build local x86_64; en aarch64 no es nativo |
| APK debug universal | ✅ compilado localmente para las cuatro ABI |
| Audiolibros Android | ✅ Media3 ExoPlayer, `MediaSessionService`, notificación, segundo plano y pantalla apagada |
| TTS Android | ✅ `TextToSpeech` + `MediaSessionService`; pack opcional Supertonic 3 con 6 voces neuronales |
| APK release firmado | ✅ 64 MB, `arm64`, firma V2 verificada con la llave de Kora |
| Icono Android adaptativo | ✅ fondo propio y primer plano con margen seguro; sin esquinas negras ni recorte del arte |
| Búsqueda semántica | ✅ chunking UTF-16, coseno y ranking con proveedor determinista |
| IA local | ✅ asistente Qwen2.5 0.5B en Android con activación de un botón y modo avanzado para Ollama/llama.cpp |
| Voces soportadas | ✅ solo español e inglés; catálogo nativo curado y opción «Todas las variantes» |
| Modo avanzado | ✅ código de acceso oculta proveedor, URL, modelo y parámetros técnicos |
| Clasificación automática | ✅ reglas, metadatos, embeddings, IA y sugerencias |
| Conversión PDF→EPUB | ✅ capítulos, OCR, reubicación de anotaciones y portada |
| Conversión texto↔audio | ✅ EPUB/PDF→M4B desde la biblioteca y audiolibro sin texto→EPUB con Whisper |
| Álbumes de audiolibros | ✅ colecciones ordenadas, mosaico de portadas y reproducción en cola |
| Generación M4B en Android | ✅ MediaCodec/MediaMuxer, servicio en primer plano y capítulos en el manifiesto |
| Papelera y copias | ✅ restauración, ZIP verificado y purga |
| CI de escritorio | ✅ workflow Linux añadido; mismas comprobaciones ejecutadas localmente |
| Arranque de escritorio | ✅ `pnpm tauri dev` inicia la app y la base SQLite |
| Diario y logros | ✅ agregación mensual, rachas, 22 logros y Pomodoro con entrada automática |
| Sync por dispositivo | ✅ filtro por peer aplicado al manifiesto |
| Exportación del diario | ✅ EPUB mensual/anual; audiolibro opcional por el pipeline M4B |
| Opus real | ✅ Ogg/Opus con `OpusHead`/`OpusTags` y 0,5 s de audio |
| PDFium + Tesseract reales | ✅ con `KORA_PDFIUM_PATH` y `tesseract` instalados |
| Piper real | ✅ con una voz en `assets/tts` |
| NeuTTS real | ✅ español e inglés con `encoder.onnx`, `decoder.onnx` y GGUF Q4 |
| ffmpeg + M4B real | ✅ dos capítulos con `ffprobe` y un M4B generado desde Piper |
| Sync real entre dos instancias | ✅ QUIC local, libro+portada, anotación CRDT, borrado suave y convergencia |
| Categorías entre dispositivos | ✅ las categorías con el mismo nombre se reutilizan por alias y las asignaciones se reescriben al id local |
| Handoff y espejo | ✅ eventos `sync::handoff`, `mirror::position` y `mirror::tts_state` |
| Cuenta y vinculación | ✅ PIN Argon2id, clave Ed25519, código temporal + QR y confianza por huella TLS |
| Shares | ✅ copia, solo lectura, caducidad, `max_uses` y revocación |
| `tauri android init --ci` | ✅ `src-tauri/gen/android` generado |
| `tauri android build --apk` | ✅ workflow x86_64 en `.github/workflows/android.yml`, builds debug y release firmado locales; el host aarch64 no lo compila de forma nativa |

El lado Rust se compiló en un contenedor rootless `fedora:42` con
`webkit2gtk4.1-devel`, `tesseract-devel`, `leptonica-devel`, `clang-devel`,
`cmake`, `alsa-lib-devel`, `opus-devel`, `ffmpeg` y `git`. Los tests cubren migraciones y
checksums, CRUD por entidad, cascadas, identidad local, assets SHA-256,
papelera, importación, lector EPUB, lector PDF (texto nativo, rasterizado,
detección de escaneados), parser TSV de Tesseract, heurísticas de auto-índice,
WAV con marca de agua, offsets UTF-16, diálogo, pausas expresivas, diff de
palabras, síntesis Piper, perfiles `.koravoice`, capítulos M4B, EPUB generados,
portadas PNG, UUIDv7, borrado suave, notas ricas, notas de voz Opus,
vinculación con código temporal, QUIC/TLS, CRDT, diff por hash, filtros de colecciones por
peer, handoff, espejo, PIN Argon2id, SPAKE2, QR, auto-sync, shares, chunking semántico,
ranking por coseno, diario automático, heatmap, 22 logros, Pomodoro,
exportación EPUB, el pipeline real de clonación, IA local, clasificación
asistida, conversión PDF→EPUB, papelera y copias de seguridad. El workflow
`.github/workflows/linux.yml` reproduce las comprobaciones de escritorio en
Ubuntu 24.04.

## Stack

| Capa | Tecnología |
|---|---|
| Shell nativo | Tauri 2.12 |
| Backend | Rust 1.90+ (edition 2024) |
| Frontend | Svelte 5 + Vite 8 + TypeScript 5.9 |
| Base de datos | SQLite embebido (`sqlx` 0.8, feature `sqlite` con `libsqlite3-sys` bundled) |
| Hash de contenido | `sha2` + `hex` (SHA-256 en streaming) |
| Sync | mDNS (`mdns-sd`), QUIC (`quinn` + `rustls`), claves y certificados Ed25519 (`rcgen`/`ring`), CRDT (`yrs`) |
| EPUB | `rbook` + `ammonia`/`scraper` (saneado) |
| PDF (visor) | `pdf.js` (`pdfjs-dist`) en el WebView |
| PDF (texto/OCR) | `pdfium-render` + `libpdfium` del sistema (`lopdf` como plan B) |
| OCR | Tesseract 5 vía `leptess` (solo escritorio) |
| TTS escritorio | Piper (`piper-rs` + ONNX Runtime), Supertonic 3 opcional y reproducción con `rodio`/`cpal` |
| TTS Android | `TextToSpeech` nativo, Supertonic 3 opcional (ONNX Runtime estático), plugin Kotlin propio y `MediaSessionService` de Media3 |
| Transcripción Android | `MediaExtractor`/`MediaCodec` + `whisper-rs` compilado con el NDK |
| Modo ensayo | `cpal` para grabar y `whisper-rs` (`tiny`/`base`) para transcribir |
| Notas ricas | Quill 2 (Delta JSON en `notes.content_json`) |
| Notas de voz | `cpal` + `opus`/`ogg` (Ogg/Opus en `assets/notes_audio/`) |
| Clonación | NeuTTS (GGUF + llama.cpp) y NeuCodec (ONNX Runtime) en un proceso auxiliar |
| Embeddings | `fastembed` 7 + ONNX Runtime, modelo `multilingual-e5-small` (384 dims) |
| Búsqueda semántica | Chunks de ~500 palabras en SQLite (BLOB) y similitud coseno en Rust |
| Logs | `tracing` + `tracing-appender` |

**¿Por qué Svelte 5?** El compilador genera JavaScript pequeño y sin virtual
DOM, lo que importa en WebViews de Android de gama baja; las runes dan estado
reactivo explícito y `svelte-check` valida los componentes con TypeScript.
Solid es una alternativa válida, pero Svelte tiene mejor tooling de chequeo y
más ejemplos de integración con Tauri.

## Estructura

```
Kora/
├─ src/                         # Frontend Svelte (SPA, sin SSR)
│  ├─ lib/api.ts                # Wrappers tipados de invoke()
│  ├─ lib/types.ts              # DTOs espejo de Rust
│  └─ views/                    # Biblioteca, Lector, Ajustes
├─ src-tauri/
│  ├─ src/
│  │  ├─ storage/               # SQLite + migraciones, repos CRUD, assets SHA-256, papelera
│  │  ├─ reader/                # Parsers EPUB/PDF y renderizado
│  │  ├─ tts/                   # Motores, perfiles y sinks de audio
│  │  ├─ sync/                  # mDNS, QUIC y CRDT
│  │  ├─ accounts/              # Identidad, pairing y sharing
│  │  ├─ ai/                    # Asistente local Candle y cliente Ollama/llama.cpp
│  │  ├─ classify/              # Reglas, embeddings e IA para categorías
│  │  ├─ features/              # OCR, embeddings, diario, logros
│  │  ├─ commands/              # Comandos #[tauri::command]
│  │  ├─ state.rs               # AppState compartido
│  │  └─ error.rs               # KoraError { code, message }
│  ├─ vendor/piper-rs/          # Copia MIT con ort rc.15 para fastembed
│  ├─ capabilities/default.json
│  ├─ tauri.conf.json
│  └─ gen/android/              # Generado por `tauri android init`
└─ Cargo.toml                   # Workspace (miembro: src-tauri)
```

Cada módulo de negocio sigue el mismo patrón: un `mod.rs` con los traits y un
archivo por implementación prevista. El almacenamiento vive en
`storage/` con `model/` (DTOs), `repos/` (CRUD por entidad), `migrations/`
(SQL versionado) y utilidades de assets, hash y papelera.

## Almacenamiento privado

Todo se resuelve con `tauri::path::BaseDirectory::AppLocalData`:

| Plataforma | Ruta base |
|---|---|
| Linux | `~/.local/share/dev.kora.reader/` |
| Android | `/data/user/0/dev.kora.reader/files/` |

```
db/app.sqlite            SQLite en modo WAL (+ -wal / -shm)
assets/books/            contenido de libros, <sha256>.<epub|pdf>
assets/covers/           portadas, <sha256>.<jpg|png|webp>
assets/avatars/          fotos de perfil, <sha256>.<jpg|png|webp>
assets/voices/           perfiles clonados: <voice_id>/embedding.bin, reference.opus, reference.wav, reference.txt, profile.json
assets/voices/models/    modelos de clonación: encoder.onnx(+.data), decoder.onnx, backbone Q4
assets/notes_audio/      audio de notas
assets/audiobooks/       manifiestos y audio generado
assets/generated_books/  exportaciones generadas
assets/ocr/              capas de texto por página: <book_id>/page_<n>.json
assets/tts/              voces Piper: <voice_id>/model.onnx + config.json
assets/tts/supertonic/   pack opcional Supertonic 3: onnx/, voice_styles/, state.json
assets/tts/practice/     grabaciones del modo ensayo: <book_id>/*.wav
assets/whisper/          modelos GGML de Whisper: ggml-tiny.bin, ggml-base.bin
assets/models/embeddings/ modelo ONNX/tokenizer de búsqueda semántica
exports/                 archivos generados por el usuario (diario EPUB/M4B)
trash/                   borrado diferido (assets huérfanos)
logs/                    kora.log con rotación diaria
```

El árbol se crea al arrancar (`StorageLayout::ensure`) y se expone al frontend
con el comando `storage_layout`. Los assets se deduplican por SHA-256: guardar
dos veces el mismo archivo reutiliza el mismo blob. Al borrar un libro, sus
assets se mueven a `trash/` solo si ningún otro registro los referencia.

## Base de datos y migraciones

El esquema se aplica con migraciones embebidas (`include_str!`) y la tabla
`schema_version` guarda versión, nombre, checksum SHA-256 y fecha. Si un archivo
ya aplicado cambia, el arranque falla en vez de corromper la base. Las
migraciones corren dentro de una transacción por versión.

| Archivo | Contenido |
|---|---|
| `0001_identity.sql` | `accounts`, `devices`, `connection_keys` |
| `0002_sharing.sql` | `shares`, `share_keys` |
| `0003_sync.sql` | `sync_state`, `resource_owners`, `audit_log` |
| `0004_content.sql` | `books`, `reading_progress`, `categories`, `book_categories`, `annotations`, `extracts`, `notes`, `voices`, `audiobooks`, `diary_entries`, `achievements`, `settings` |
| `0005_reader.sql` | Posiciones por capítulo en `annotations` y tabla `reader_settings` por libro |
| `0006_pdf.sql` | `pdf_document_info`, `pdf_ocr_status` y `pdf_auto_index` |
| `0007_tts.sql` | Secuencias expresivas, voces por personaje, preferencias y ensayo |
| `0008_voice_cloning.sql` | `voices.kind` (`system`/`cloned`) y `reference_duration_ms` |
| `0009_audiobooks.sql` | `books.origin`, `audiobooks.origin/scope/audio_hash` y `audiobook_progress` |
| `0010_annotations.sql` | `deleted`, `notes.content_json`, anclaje de extractos/notas, `voice_notes` y `reactions` |
| `0011_sync.sql` | Trust/fingerprint de dispositivos, `sync_state.hash`, cola `sync_pending`, `crdt_documents`, `sync_collections` y alias de libros |
| `0012_accounts.sql` | PIN/Ed25519 de cuenta, clave de conexión, permisos de shares, `received_shares` y libros de solo lectura |
| `0013_features.sql` | `semantic_book_index`, `semantic_chunks`, columnas de sesión en `diary_entries`, `sync_device_collections` por peer y migración del modo global |
| `0014_appearance.sql` | `app_preferences`, `custom_themes`, nuevos valores de tema/textura en `reader_settings` |
| `0015_reader_typography.sql` | Ajustes tipográficos por libro |
| `0016_favorites.sql` | `book_favorites` |
| `0017_font_face_compat.sql` | Compatibilidad de fuentes del lector |
| `0018_audiobook_queue.sql` | `audiobook_queue` persistente |
| `0019_audiobook_cast.sql` | Reparto de voces en la cola |
| `0020_voice_marks.sql` | Marcadores de voz en `voice_notes` |
| `0021_audiobook_favorites.sql` | `audiobook_favorites` y `received_shares.resource_type` |
| `0022_audio_only_books.sql` | `books.audio_only` para audiolibros importados sin transcripción |
| `0023_classification.sql` | Origen/confianza de categorías, alias, sugerencias y estado de clasificación |
| `0024_trash_retry.sql` | Papelera lógica (`books.deleted_at`) y ajustes de narración para reintentos |
| `0025_account_avatar.sql` | `avatar_hash`/`avatar_ext` de la foto de perfil local |

Todas las tablas de contenido llevan `owner_account_id` (FK a `accounts`) y
`origin_share_id` (FK opcional a `shares`). `foreign_keys=ON` se activa en cada
conexión del pool y `audit_log` es append-only.

> La base de la fase 1 (`db/kora.sqlite3`) no se migra: el esquema se creó de
> cero en `db/app.sqlite`. Si tienes una instalación anterior, borra el archivo
> antiguo; no contenía datos de usuario reales.

## Comandos IPC

| Comando | Estado |
|---|---|
| `app_info` | Devuelve nombre, versión, plataforma y rutas |
| `storage_layout` | Rutas privadas ya creadas |
| `library_list` | Lista libros con portada y progreso |
| `library_overview` | Libro en curso y contador para la barra superior |
| `library_import` | Copia un EPUB/PDF por SHA-256 y crea el libro |
| `export_book` | Copia el EPUB de un libro al destino elegido |
| `library_update` | Corrige título, autor, idioma, editorial y descripción |
| `library_delete` | Mueve el libro a la papelera lógica conservando notas y assets |
| `library_trash` / `library_restore` / `library_purge_book` / `library_empty_trash` | Papelera, restauración y borrado definitivo |
| `library_ensure_cover` | Genera una portada si el libro no tiene |
| `library_set_favorite` / `library_assign_category` / `library_unassign_category` | Favoritos y categorías |
| `library_replace_cover` | Guarda una portada nueva y limpia la anterior |
| `category_list` / `category_create` / `category_update` / `category_delete` | CRUD de categorías de la cuenta local |
| `storage_usage` / `backup_library` / `restore_library` / `purge_storage` | Espacio, copia ZIP, restauración y limpieza de papelera/temporales |
| `classification_get_config` / `classification_save_config` / `classification_status` | Configuración y estado de la clasificación asistida |
| `library_classify` / `library_classify_pending` | Clasifican un libro o la biblioteca pendiente |
| `category_suggestions` / `category_suggestion_list_pending` | Sugerencias pendientes por libro o globales |
| `category_suggestion_accept` / `category_suggestion_dismiss` / `category_suggestion_accept_all` | Aceptan, descartan o aplican todas las sugerencias |
| `category_seed_apply` | Crea el catálogo base de categorías y alias |
| `pdf_conversion_preview` | Analiza un PDF, capítulos, OCR y datos que se conservarán |
| `convert_pdf_to_epub` | Convierte el PDF a EPUB y reubica anotaciones y progreso |
| `pdf_conversion_status` | Progreso de la conversión activa |
| `ai_get_config` / `ai_save_config` / `ai_status` / `ai_chat` | Asistente local en Android, o Ollama/llama.cpp en el modo avanzado |
| `ai_local_status` / `ai_local_download` / `ai_local_delete` / `ai_activate_local` / `ai_deactivate_local` | Descarga, borrado y activación del modelo Qwen2.5 en el teléfono |
| `reader_get_defaults` / `reader_apply_to_all` | Tipografía base global aplicada a la biblioteca |
| `reader_open` | Inspecciona EPUB/PDF, completa metadatos/portada y devuelve sesión, progreso y ajustes |
| `reader_get_chapter` | Devuelve un capítulo saneado con imágenes incrustadas (EPUB) |
| `reader_update_progress` | Guarda capítulo/página, offset y porcentaje del libro |
| `reader_add_annotation` / `reader_update_annotation` / `reader_delete_annotation` | Anotaciones por `book_id` + capítulo/página + offsets |
| `reader_list_annotations` | Anotaciones del libro ordenadas por posición |
| `add_marker` / `update_marker` | Marcadores de color superpuestos con UUIDv7 |
| `add_note` / `update_note` | Notas ricas con Delta JSON de Quill en `notes.content_json` |
| `start_voice_note_recording` / `add_voice_note` / `cancel_voice_note_recording` | Graban con `cpal` y guardan Ogg/Opus en `assets/notes_audio/<id>.opus` |
| `add_extract` / `search_extracts` / `update_extract` | Extractos con cita, búsqueda y edición |
| `add_reaction` / `update_reaction` / `reaction_map` | Reacciones, edición y heatmap por capítulo/página |
| `list_annotations` / `list_library_annotations` / `delete_annotation` | Listan por tipo/libro y aplican borrado suave |
| `update_voice_note` | Edita texto, transcripción y cita sin perder el audio |
| `export_notes` | Exporta subrayados, notas, extractos y notas de voz a Markdown/JSON |
| `reader_get_settings` / `reader_save_settings` | Ajustes tipográficos y de tema por libro |
| `reader_pdf_bytes` | Bytes del PDF por IPC (plan B si `asset:` no está disponible) |
| `get_ocr_page` | Capa de texto por página: OCR guardado o texto nativo de PDFium |
| `ocr_pdf` | Inicia o reanuda el OCR local de un PDF |
| `build_auto_index` | Genera y guarda el índice (bookmarks, heurística u OCR) |
| `get_auto_index` | Devuelve el índice guardado o `null` |
| `tts_engines` | Motores registrados: Piper en escritorio, TextToSpeech en Android |
| `tts_voices` | Voces Piper/Android y perfiles clonados disponibles |
| `tts_synthesize` | Sintetiza texto y devuelve un WAV en base64 (previsualización) |
| `tts_download_voice` | Descarga una voz Piper a `assets/tts/` con progreso |
| `start_tts` | Inicia la lectura desde `{section, offset, end?}` y emite eventos por frase; con `end` reproduce solo el extracto |
| `pause_tts` / `resume_tts` / `stop_tts` | Controlan la sesión activa |
| `set_rate` / `set_pitch` | Cambian velocidad y tono de la sesión |
| `tts_status` | Estado de la sesión (`playing`, `paused`, `finished`…) |
| `tts_characters` | Personajes detectados en una sección |
| `tts_character_voices` / `assign_character_voice` | Voces por personaje en SQLite |
| `tts_preferences` / `save_tts_preferences` | Voz, velocidad, tono, pausas y multi-voz por libro |
| `practice_start` / `practice_stop` / `practice_cancel` | Graban, transcriben y comparan un pasaje |
| `practice_models` / `practice_download_model` | Gestionan Whisper `tiny`/`base` |
| `practice_history` | Intentos de ensayo guardados |
| `voice_consent_text` | Texto exacto del consentimiento de clonación |
| `voice_models` / `voice_download_models` | Estado y descarga de los modelos NeuTTS/NeuCodec |
| `create_voice_profile` | Codifica un clip de 3–30 s y crea el perfil clonado |
| `list_voice_profiles` / `delete_voice_profile` | Listan y borran perfiles |
| `export_voice_profile` / `import_voice_profile` | Archivos `.koravoice` para LAN |
| `voice_clone_preview` | Genera una muestra WAV con marca de agua |
| `reader_add_extract` / `reader_list_extracts` | Guardan y listan extractos por libro |
| `list_all_extracts` / `reader_delete_extract` | Extractos para Tejido y borrado |
| `generate_audiobook` | Genera un M4B con estilo de narración, velocidad base y reparto de voces; en Android empaqueta con MediaCodec/MediaMuxer |
| `enqueue_audiobook` / `audiobook_queue_status` | Cola persistente de generación con progreso, reparto y elementos pendientes |
| `audiobook_album_list` / `audiobook_album_create` / `audiobook_album_update` / `audiobook_album_delete` | CRUD de álbumes de audiolibros |
| `audiobook_album_add` / `audiobook_album_remove` / `audiobook_album_reorder` | Pertenencia y orden de los audiolibros de un álbum |
| `retry_audiobook` / `move_queued_audiobook` | Reintenta una generación fallida y reordena la cola |
| `audiobook_characters` | Detecta personajes del alcance para el multivot |
| `pause_audiobook_generation` / `resume_audiobook_generation` / `cancel_audiobook_generation` | Control de la generación activa |
| `remove_queued_audiobook` / `clear_audiobook_queue` | Gestión de la cola |
| `apply_audiobook_treatment` | Normalización LUFS en dos pasadas y ecualización con ffmpeg |
| `audiobook_waveform` | Forma de onda cacheada por hash de audio |
| `audiobook_set_favorite` | Marca o desmarca un audiolibro como favorito |
| `create_audiobook_share` | Crea una clave `SHARE-AUDIOBOOK` con PIN, permiso y caducidad |
| `audiobook_chapters` / `audiobook_karaoke` | Capítulos y karaoke con anclas al texto |
| `audiobook_voice_markers` / `voice_marker_status` | Marcadores de voz y estado de la escucha |
| `start_voice_marker_listening` / `stop_voice_marker_listening` / `save_voice_marker_phrase` | Escucha del comando «Kora, marca esto» durante la reproducción |
| `list_audiobooks` / `delete_audiobook` | Biblioteca de audiolibros |
| `play_audiobook` / `pause_audiobook` / `seek_audiobook` | Reproducción con búsqueda |
| `set_audiobook_speed` / `audiobook_chapters` | Velocidad y capítulos con anclas al texto |
| `set_audiobook_sleep_timer` / `set_audiobook_keep_screen_on` | Temporizador de sueño y pantalla encendida en el servicio Android |
| `stop_audiobook` / `get_audiobook_progress` | Detención y progreso persistido |
| `import_external_audiobook` | Copia, transcribe con Whisper y crea un libro |
| `import_audio_audiobook` / `reanalyze_audiobook_chapters` / `update_audiobook_chapters` / `transcribe_audiobook_later` | Importación de audio, capítulos y transcripción diferida |
| `audiobook_locator` / `audiobook_capabilities` | Posición de texto actual y capacidades del dispositivo |
| `export_audiobook` | Copia el M4B al destino elegido |
| `weave_extracts` | Crea EPUB + portada + audiolibro desde extractos |
| `start_sync` / `stop_sync` | Arrancan o detienen listener QUIC, mDNS pasivo y cola |
| `get_sync_status` / `get_peers` | Estado, cambios pendientes y peers descubiertos/emparejados |
| `sync_now` | Diff y transferencia bidireccional con un peer |
| `start_device_pairing` / `join_device_pairing` / `cancel_device_pairing` | Código temporal + QR, SPAKE2 y confianza automática |
| `revoke_peer` | Revoca la confianza de un dispositivo |
| `start_share` / `stop_share` | Anuncio mDNS `purpose=share` para shares, handoff y espejo |
| `get_sync_collections` / `set_sync_categories` / `list_sync_devices` | Filtro selectivo por categorías y por dispositivo emparejado |
| `set_sync_collections` | Alias de compatibilidad para el dispositivo local |
| `sync_status` / `sync_peers` / `sync_start` | Alias de compatibilidad de estado, peers y arranque |
| `list_audit_log` / `export_audit_log` | Registro de actividad y exportación JSON |
| `start_mirror` / `stop_mirror` | Host de lectura en espejo |
| `join_mirror` / `leave_mirror` / `get_mirror_status` | Seguidor y estado del espejo |
| `account_status` | Cuenta y dispositivo locales (se crean al arrancar) |
| `create_account` / `get_account` / `update_display_name` | Crean, consultan y editan la cuenta local |
| `update_account_avatar` / `remove_account_avatar` | Guardan o quitan la foto de perfil local |
| `get_connection_key` / `change_pin` | Clave de conexión y re-cifrado con PIN nuevo |
| `recover_account_pin` | Restablece el PIN regenerando la identidad de la cuenta |
| `delete_account` | Borra cuenta, biblioteca, notas, audiolibros y voces clonadas |
| `rotate_connection_key` | Rota la clave de conexión sin romper dispositivos ya emparejados |
| `set_auto_sync` / `sync_now_all` / `get_sync_diagnostics` | Auto-sync en primer plano y diagnóstico de red |
| `create_share` / `list_shares` / `revoke_share` | Shares con alcance, permiso, caducidad y usos |
| `accept_share` / `list_received_shares` / `remove_received_share` | Shares recibidos y limpieza de copias de solo lectura |
| `reveal_share_key` | Muestra el QR/clave de un share con PIN |
| `list_devices` / `revoke_device` | Dispositivos emparejados y revocación |
| `get_appearance` / `save_appearance` | Tema, textura, idioma, onboarding y vista de biblioteca por cuenta |
| `save_custom_theme` / `delete_custom_theme` | CRUD de temas personalizados |
| `features_status` | Activa `ocr`, `embeddings`, `journal` y `achievements` según disponibilidad |
| `semantic_status` / `prepare_semantic_model` / `reindex_library` | Estado del modelo, descarga inicial e indexado en segundo plano |
| `search_semantic` | Búsqueda semántica local con snippet, sección y score |
| `session_start` / `session_end` | Abren y cierran sesiones de lectura/escucha; generan diario automático |
| `get_diary` | Días del mes con totales y nivel de heatmap |
| `diary_create` / `diary_update` / `diary_delete` | Entradas manuales del diario |
| `get_achievements` | Catálogo de logros con progreso y desbloqueos |
| `start_pomodoro` / `stop_pomodoro` / `get_pomodoro` | Foco/descanso configurable; el foco completado va al diario |
| `export_diary` | EPUB mensual/anual y audiolibro M4B opcional en `exports/` |

Los errores llegan al frontend como `{ code, message }`. Los códigos nuevos de
esta fase son `conflict` y `unsupported`.

## Desarrollo en Linux

### Fedora

Dependencias del sistema:

```bash
sudo dnf install -y \
  gcc gcc-c++ make pkg-config cmake git clang-devel \
  webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  openssl-devel librsvg2-devel patchelf \
  alsa-lib-devel \
  tesseract tesseract-devel leptonica-devel \
  tesseract-langpack-spa tesseract-langpack-eng \
  espeak-ng espeak-ng-devel
```

### Ubuntu, Debian y Zorin

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config cmake git clang libclang-dev \
  libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev \
  libssl-dev librsvg2-dev patchelf \
  libasound2-dev \
  tesseract-ocr libtesseract-dev libleptonica-dev \
  tesseract-ocr-spa tesseract-ocr-eng \
  espeak-ng libespeak-ng-dev ffmpeg
```

En Ubuntu 24.04 y derivados (Zorin 18) el paquete `libwebkit2gtk-4.1-dev`
arrastra `libjavascriptcoregtk-4.1-dev`. Si vas a generar AppImage en ARM64
necesitarás `linuxdeploy` para esa arquitectura; en x86_64 lo descarga Tauri.

`piper-rs` compila eSpeak-ng y usa `bindgen`, por eso hacen falta `clang-devel`,
`cmake` y `git`. El árbol incluye `src-tauri/vendor/piper-rs` (MIT) para alinear
su dependencia de ONNX Runtime rc.15 con `fastembed`; ambos comparten un único
`Environment` de ONNX Runtime en el proceso. ONNX Runtime se enlaza de forma
estática en el binario.
`rodio`/`cpal` necesitan `alsa-lib-devel` para reproducir y grabar audio;
`whisper-rs` compila whisper.cpp y usa el mismo `clang-devel`/`cmake`.

El motor de clonación vive en el binario auxiliar `kora-neutts`
(`src-tauri/neutts-helper`). Se compila aparte para no enlazar llama.cpp y
whisper.cpp en el mismo proceso (sus símbolos `ggml_*` chocan):

```bash
pnpm build:helper            # debug + copia a src-tauri/binaries
pnpm build:helper:release    # release + copia a src-tauri/binaries
```

La app lo busca junto a su ejecutable, en `KORA_NEUTTS_HELPER`, en
`/usr/lib/kora/` o en el `APPDIR` de AppImage. Para empaquetarlo en el `.deb`
con Tauri:

```bash
pnpm build:linux
# equivale a compilar el helper release, copiarlo a src-tauri/binaries
# y ejecutar tauri build --bundles deb
```

`opus-devel` solo lo necesita el helper para escribir `reference.opus`.

Para el texto nativo, las coordenadas y el OCR, Kora usa PDFium en tiempo de
ejecución. Fedora y Ubuntu no lo empaquetan; descarga un build de
[pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) y apunta
`KORA_PDFIUM_PATH` al archivo o a su carpeta:

```bash
export KORA_PDFIUM_PATH="$HOME/.local/lib/pdfium/libpdfium.so"
```

Sin PDFium, el visor PDF sigue funcionando con `pdf.js` y el texto de respaldo
del propio visor; el OCR y el auto-índice muestran un error claro.

Instala Rust y Node:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
# Node 20.19+ o 22.12+ y pnpm 9+
corepack enable
```

Arranca en modo desarrollo:

```bash
pnpm install
pnpm tauri dev
```

### Modo demostración en el navegador

Para revisar la interfaz sin compilar Rust ni disponer de libros en la base
local:

```bash
pnpm dev:demo
```

Al abrir `http://localhost:1420` fuera del WebView de Tauri, el frontend
instala un backend simulado (`src/lib/demo/`) con diez libros, portadas,
categorías, progreso, anotaciones, capítulos EPUB y PDF reales, respuestas de
IA, lectura en voz alta y modo ensayo. Los datos viven en memoria y se pierden
al recargar; el modo nunca se activa dentro de la aplicación Tauri. La consola
expone `window.__KORA_DEMO__` para inspeccionar el estado.

Comprobaciones:

```bash
pnpm build            # tsc + vite build
pnpm check            # svelte-check
pnpm build:helper     # kora-neutts + copia para externalBin
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Clonación real (requiere los modelos descargados)
KORA_NEUTTS_ROOT=/ruta/a/modelos \
KORA_NEUTTS_SAMPLE=/ruta/a/modelos/mateo.wav \
  cargo test -p kora --test voice_clone
```

## Compilar para Linux

```bash
pnpm tauri build
```

Genera `deb`, `rpm` y `AppImage` en `target/release/bundle/`.
En hosts **aarch64** (por ejemplo Fedora Asahi) `deb` y `rpm` funcionan sin
extras; `AppImage` puede requerir `linuxdeploy` para ARM64. Si solo necesitas
el binario:

```bash
pnpm tauri build --no-bundle
```

## Android

### Requisitos

- JDK 17+ (probado con 21)
- Android SDK: `platform-tools`, `platforms;android-37.0`,
  `build-tools;37.0.0` (el template de Tauri 2.12 usa `compileSdk = 37`)
- NDK `28.2.13676358`
- Targets de Rust:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi \
  i686-linux-android x86_64-linux-android
```

Instalación del SDK por línea de comandos:

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
sdkmanager --sdk_root="$ANDROID_HOME" \
  "platform-tools" "platforms;android-37.0" \
  "build-tools;37.0.0" "ndk;28.2.13676358"
```

Variables de entorno:

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
```

### Inicializar el proyecto Android

```bash
pnpm tauri android init --ci
```

Esto crea `src-tauri/gen/android/` (Gradle, `MainActivity.kt`, manifiesto y
recursos). El `applicationId` sale del identificador `dev.kora.reader` y el
`minSdkVersion` es 24.

### Desarrollo y APK

```bash
pnpm android:dev                       # dispositivo/emulador conectado
pnpm android:build                     # APK (64 bits)
pnpm android:build:release             # APK release firmado (64 bits)
```

El APK queda en
`src-tauri/gen/android/app/build/outputs/apk/universal/release/`. Whisper no
compila para ABI de 32 bits con los bindings empaquetados, así que el release
se genera con `--target aarch64 x86_64` (o solo `aarch64` para el teléfono).
El build de debug (`pnpm tauri android build --apk --debug --target aarch64`)
queda en `.../apk/universal/debug/` e incluye símbolos, por lo que pesa más.

### Firma de release

Crea un keystore y un archivo `src-tauri/gen/android/keystore.properties`:

```properties
storeFile=/ruta/a/kora-release.jks
storePassword=...
keyAlias=kora
keyPassword=...
```

`src-tauri/gen/android/app/build.gradle.kts` lee ese archivo y aplica la firma
solo si existe. No subas el keystore ni el `keystore.properties` al repositorio
(ya están en `.gitignore`). Sin keystore, el APK release se genera sin firmar.

### Nota sobre hosts ARM64 (Fedora Asahi, Apple Silicon con Linux)

El SDK y el NDK oficiales de Google para Linux son **x86_64**. En un host
aarch64:

- `tauri android init` funciona (las herramientas del SDK son Java).
- La compilación del APK con el NDK oficial **no funciona de forma nativa**:
  el clang del NDK es un binario x86_64.

Opciones reales:

1. Compilar el APK en una máquina/CI x86_64 (recomendado).
2. Usar un contenedor x86_64 con emulación (`qemu-user`/binfmt), muy lento.
3. Usar un NDK no oficial para aarch64 (por ejemplo los builds de Termux),
   aceptando que no está soportado por Google.

El resto del proyecto (frontend, Rust de escritorio y `cargo check`) sí se
compila de forma nativa en aarch64.

En esta fase, Android conserva el visor PDF con `pdf.js` y añade TTS nativo con
`TextToSpeech` mediante un plugin Kotlin propio (`dev.kora.reader.tts`). La
lectura publica una `MediaSession` de Media3 dentro de un
`MediaSessionService`: la reproducción continúa en segundo plano, con la
pantalla apagada y con controles en la notificación y la pantalla de bloqueo.
Los audiolibros M4B/MP3 usan **Media3 ExoPlayer** dentro de otro
`MediaSessionService` con el mismo tipo de controles. En Android también se
pueden importar audiolibros y transcribirlos localmente con Whisper: el audio
se decodifica con `MediaExtractor`/`MediaCodec` y Whisper se compila para la
ABI con `whisper-rs` + NDK. La generación con voces del sistema también está
disponible en Android: el motor TTS sintetiza cada fragmento, un muxer nativo
(`MediaCodec` + `MediaMuxer`) empaqueta AAC/MP4 y un servicio en primer plano
mantiene el proceso vivo con la pantalla apagada. Los capítulos se guardan en
el manifiesto de Kora; el contenedor no incluye átomos de capítulos para
reproductores externos en esta versión.
El temporizador de sueño y el ajuste opcional de mantener la pantalla encendida
también viven en el servicio nativo. El código nativo del reproductor está en
`app/src/main/java/dev/kora/reader/audio/` y el del TTS en
`app/src/main/java/dev/kora/reader/tts/`; si vuelves a ejecutar
`tauri android init`, revisa que el generador no haya sobrescrito esas clases.
Las dependencias nativas de OCR (Tesseract/Leptonica), PDFium, Piper/ONNX y
las partes de audio de escritorio quedan detrás de `cfg` y devuelven un error
claro hasta empaquetarlas por ABI. Whisper ya se compila para Android; el
código de escritorio sigue siendo la referencia para el resto.

## Lector EPUB

El lector no inyecta el CSS del EPUB: extrae el `<body>` de cada capítulo, lo
sanea con `ammonia` (sin scripts, estilos ni manejadores de eventos) y
reescribe las imágenes locales como `data:` URIs con un límite de tamaño para
no saturar el WebView. Las anotaciones viven en una capa absoluta sobre el
texto, calculada con `Range.getClientRects()`, y nunca modifican el HTML
original.

- **Tipografía:** tamaño, interlineado, márgenes, zoom y familia
  (`serif`/`sans`/`mono`) por libro.
- **Apariencia:** temas claro, oscuro y sepia, con texturas lisa, papel,
  pergamino, lino, cuadrícula, oscuro suave, lunares, ondas, rayas, grano y
  vichy dibujadas por CSS.
- **Navegación:** desplazamiento con inercia o modo paginado por columnas;
  flechas, `PageUp`/`PageDown` y botones de capítulo.
- **Progreso:** porcentaje por capítulo y por libro, guardado tras cada cambio
  con un pequeño debounce.
- **Anotaciones:** resaltado, subrayado, tachado y marcador, con nota y color;
  los offsets son unidades UTF-16 sobre el texto visible del capítulo.

Los ajustes y el progreso se guardan en SQLite (`reader_settings` y
`reading_progress`); las anotaciones usan `book_id`, `chapter_index`,
`offset_start` y `offset_end`.

## Lector PDF

El PDF se renderiza en el WebView con `pdf.js` (worker empaquetado por Vite,
sin CDN). El archivo se carga con el protocolo `asset:` de Tauri; si falla, se
usa `reader_pdf_bytes` como plan B.

- **Navegación:** scroll continuo o página única, zoom, progreso por página y
  salto desde el índice.
- **Texto y anotaciones:** cada página tiene una capa de palabras con caja y
  tamaño de fuente en puntos PDF. La selección se traduce a índices de palabra;
  las anotaciones reutilizan `annotations` con `chapter_index = página` y
  `offset_start`/`offset_end` como índices de palabra.
- **Portada:** al abrir un PDF sin portada se rasteriza la primera página.
- **Sin modificar el PDF:** todo el análisis y el OCR viven en `assets/ocr/` y
  en SQLite.

PDFium aporta el texto nativo, los bookmarks y el rasterizado. `lopdf` se usa
como plan B para contar páginas y detectar si hay operadores de texto.

## OCR local y auto-índice

Al importar o abrir un PDF escaneado, Kora lanza un job en segundo plano:

1. Rasteriza cada página con PDFium (300 DPI, con límite de tamaño).
2. Ejecuta Tesseract (`spa+eng` por defecto) y parsea su TSV para obtener
   palabras con coordenadas y confianza.
3. Convierte píxeles a puntos PDF y escribe
   `assets/ocr/<book_id>/page_<n>.json` de forma atómica.
4. Emite `ocr-progress` para que el lector muestre el avance.
5. Al terminar, genera el auto-índice si el PDF no tenía bookmarks.

El auto-índice prioriza los bookmarks nativos. Si no hay, puntúa líneas por
tamaño de fuente, mayúsculas, posición, numeración (`1.2`), números romanos y
patrones como «Capítulo», «Chapter», «Parte» o «Sección», descartando
encabezados y pies repetidos. En PDFs escaneados usa las líneas reconstruidas
del OCR. El resultado se guarda en `pdf_auto_index` y se puede regenerar desde
el panel «Índice».

## Motor TTS

### Motores

- **Escritorio:** Piper (`piper-rs` + ONNX Runtime) sintetiza y `rodio`/`cpal`
  reproduce. La pausa y la reanudación son reales y el evento `phrase_end`
  llega cuando termina el audio.
- **Android:** un plugin Kotlin propio envuelve `TextToSpeech` y publica una
  `MediaSession` de Media3 con un `MediaSessionService`. La tarjeta de la
  pantalla de bloqueo muestra libro, capítulo, portada y controles de
  play/pausa/stop. Como la API nativa no permite pausar, `pause_tts` detiene la
  locución y `resume_tts` repite la frase actual.

El catálogo de voces Piper tiene dos voces por idioma:

| Idioma | Voz | Tamaño |
|---|---|---|
| Español | `es_ES-davefx-medium` | ~63 MB |
| Español (México) | `es_MX-ald-medium` | ~63 MB |
| Inglés | `en_US-lessac-medium` | ~63 MB |
| Inglés | `en_US-ryan-medium` | ~63 MB |

Las voces se descargan desde HuggingFace a `assets/tts/<voice_id>/` con
`tts_download_voice` y progreso por evento.

### Sesión y eventos

`start_tts(book_id, {section, offset, end?})` construye o reutiliza la secuencia
expresiva, resuelve las voces y lanza una sesión. Si se envía `end`, el backend
recorta el texto al rango seleccionado (UTF-16 en EPUB, índices de palabra en
PDF), conserva los offsets absolutos para el resaltado y termina al acabar el
extracto. El lector expone **Escuchar selección** en la barra de selección del
EPUB y en la tarjeta del pasaje del panel de anotaciones del PDF. El backend
emite:

- `tts::phrase_start` con `bookId`, `section`, `offsetStart`, `offsetEnd`,
  `unit`, `character`, `voiceId` y el texto.
- `tts::phrase_end` al terminar la frase.
- `tts::state` con `playing`, `paused`, `finished`, `stopped` o `error`, más
  `excerpt` para indicar si la sesión reproduce solo una selección.

En EPUB los offsets son unidades UTF-16 del texto del capítulo, los mismos que
usan las anotaciones. En PDF son índices de palabra y `offsetEnd` es exclusivo.

### Pantalla de bloqueo y segundo plano (Android)

Cada sesión de TTS publica metadatos (libro, capítulo, autor y portada) en una
`MediaSession` de Media3. `TtsPlaybackService` extiende `MediaSessionService` y
mantiene el servicio en primer plano con una notificación de tipo
`mediaPlayback`:

- La tarjeta de la pantalla de bloqueo muestra el título, el capítulo y la
  portada del libro.
- Play/pausa/stop funcionan desde la notificación, la pantalla de bloqueo y los
  auriculares. La pausa detiene la locución; al reanudar se repite la frase
  actual, porque `TextToSpeech` no tiene pausa nativa.
- Un `PARTIAL_WAKE_LOCK` mantiene vivo el proceso mientras la lectura está
  activa; se libera al pausar, terminar o cerrar la sesión.
- Si el libro no tiene portada, se genera una portada editorial de respaldo con
  el título y el autor, siguiendo el lenguaje visual de Kora. La composición
  («Ahora suena») se diseñó con Google Stitch y se reproduce en
  `DefaultTtsArtwork`; el diseño de referencia está en
  `docs/tts-ahora-suena-stitch.png`.
- El motor `TextToSpeech` y el `MediaSession` viven en un singleton de proceso
  (`TtsPlaybackRuntime`), no en la Activity, para sobrevivir a la recreación de
  la pantalla.
- El autoavance de capítulo sigue viviendo en el frontend; con la pantalla
  bloqueada el audio continúa dentro del capítulo actual.

El código nativo está en `app/src/main/java/dev/kora/reader/tts/`.

### Preprocesador expresivo

Antes del motor, `tts::expressive` divide el texto en frases y detecta:

- Diálogo frente a narración (comillas, rayas, `—dijo X`, `X dijo:`).
- Personajes y parlamentos sin hablante identificado (`Diálogo`).
- Pausas por párrafo, punto, coma, interrogación y puntos suspensivos.
- Énfasis en mayúsculas y cambios suaves de velocidad/tono.

La secuencia se guarda en `tts_sequences` con un hash del texto y de los
ajustes, así que no se recalcula al reabrir el libro. Las voces por personaje
viven en `tts_character_voices` y se aplican a la frase siguiente.

### Velocidad, tono y multi-voz

La velocidad va de 0.5× a 3×. En Piper el tono se aproxima sintetizando con un
`length_scale` compensado y remuestreando el PCM; en Android se pasa el tono
real al plugin. La multi-voz asigna automáticamente una voz distinta a cada
personaje detectado si hay más de una voz instalada, y el usuario puede
cambiarla desde el panel «Voces».

### Modo ensayo

En escritorio, «Ensayo» graba el pasaje seleccionado con `cpal`, lo convierte a
16 kHz mono y lo transcribe con `whisper-rs` (`ggml-tiny.bin` o
`ggml-base.bin` en `assets/whisper/`). Después compara palabra por palabra
(Levenshtein sobre texto normalizado sin acentos) y muestra aciertos,
sustituciones, omisiones y palabras extra. Los intentos se guardan en
`tts_practice_attempts`.

En Android el modo ensayo devuelve un error claro de «no soportado» hasta que se
empaqueten la grabación nativa y Whisper para la ABI.

## Clonación de voz

El pipeline es el de NeuTTS: el clip de referencia se codifica con NeuCodec
(50 Hz), el backbone GGUF genera tokens `<|speech_N|>` y el decodificador
NeuCodec reconstruye audio a 24 kHz. En Rust:

- **Entrada:** `symphonia` decodifica WAV/MP3/M4A/FLAC/OGG a mono 16 kHz.
- **Codificador:** `distill-neucodec` en ONNX (`ort`), verificado contra el
  modelo PyTorch.
- **Backbone:** GGUF Q4 con `llama.cpp` (`kora-neutts`).
- **Decodificador:** NeuCodec int8 en ONNX (`ort`).
- **Salida:** WAV/Opus; la reproducción usa `rodio`.

| Modelo | Idioma | Tamaño | Licencia |
|---|---|---|---|
| NeuTTS Air Q4 | Inglés | ~502 MB | Apache 2.0 |
| NeuTTS Nano Q4 | Español | ~200 MB | NeuTTS Open License 1.0 (gratis por debajo de 5 M$ anuales) |
| Codificador distill NeuCodec | Ambos | ~495 MB | Apache 2.0 |
| Decodificador NeuCodec int8 | Ambos | ~298 MB | Apache 2.0 |

Los modelos se descargan bajo demanda en `assets/voices/models/` con progreso y
verificación SHA-256. Las URLs y huellas están fijadas en `VOICE_MODELS`.

### Perfil y consentimiento

`create_voice_profile` exige `consent = true`, comprueba que el clip dure entre
3 y 30 s y escribe:

```
assets/voices/<voice_id>/embedding.bin   códigos i32 little-endian
assets/voices/<voice_id>/reference.opus  Ogg/Opus mono 16 kHz
assets/voices/<voice_id>/reference.wav   copia PCM para reprocesar
assets/voices/<voice_id>/reference.txt   transcripción (fonemas para Air)
assets/voices/<voice_id>/profile.json    versión, modelo, idioma y duración
```

La fila de `voices` guarda nombre, motor, idioma, hashes, `kind = cloned`,
duración y propietario. Cada creación o importación escribe en `audit_log` la
acción `voice.clone.consent` con el texto «Clono solo voces propias o con
permiso».

### Uso en el lector

Los perfiles aparecen como `clone:<uuid>` en `tts_voices`, en el selector del
lector EPUB y del visor PDF, y se pueden asignar a personajes. `TtsService`
enruta ese prefijo al motor NeuTTS; el resto sigue usando Piper (escritorio) o
TextToSpeech (Android).

### Marca de agua

WAV no lleva ID3 de forma nativa, así que `tts_synthesize` y
`voice_clone_preview` insertan un chunk `id3 ` con ID3v2.4 y etiquetas TXXX:
`KORA_CLONED_VOICE=1`, `KORA_VOICE_PROFILE`, `KORA_VOICE_NAME` y
`KORA_WATERMARK=cloned-voice-v1`. La UI muestra un aviso al elegir una voz
clonada. `read_watermark` permite comprobarlo en tests o herramientas externas.

### Exportar e importar

`export_voice_profile` crea un ZIP `.koravoice` con `profile.json`,
`embedding.bin`, `reference.opus`, `reference.wav`, `reference.txt` y un
`manifest.json` con SHA-256 por archivo. `import_voice_profile` valida el
manifest, exige consentimiento y genera un `voice_id` nuevo para evitar
colisiones. Es el mismo formato que reutilizará la sync de la fase 9.

### Android

El motor de clonación no se registra en Android: `state.neutts` es `None` y los
comandos devuelven `unsupported`. Empaquetar `kora-neutts` y ONNX Runtime por
ABI queda para la fase de empaquetado móvil.

## Cuentas, emparejamiento y shares

La cuenta local se crea una vez por instalación. `create_account` guarda el
usuario, el nombre visible y un **PIN Argon2id** (`m=64 MB`, `t=3`), y genera
un par **Ed25519** de cuenta. La clave privada y la parte secreta de la clave
de conexión se guardan cifradas en SQLite con una clave derivada del PIN.

### Nombre y foto

El nombre visible se edita desde **Cuenta** y se propaga a la barra superior y
a la tarjeta lateral. La foto se sube desde el propio avatar: el frontend la
recorta en cuadrado, la reduce a 512×512 y la envía como data URL. El backend
valida el formato (PNG, JPG o WebP), la guarda deduplicada por SHA-256 en
`assets/avatars/` y actualiza `accounts.avatar_hash`/`avatar_ext`. La imagen se
muestra siempre en un círculo y se incluye en las copias de seguridad.

### Vincular dispositivos

La vinculación no usa la clave permanente de la cuenta ni pide el PIN. El
dispositivo que muestra el código crea un ticket en memoria con:

- Un código numérico de 6 dígitos.
- Un `ticket_id` aleatorio.
- Caducidad de 2 minutos y cinco intentos.

El QR de un solo uso usa:

```text
kora://pair?v=2&t=<ticket>&c=<code>&h=<ip:puerto>&f=<huella>&exp=<unix>
```

El flujo reutiliza SPAKE2 sobre QUIC:

1. La contraseña es `HKDF-SHA256(código, salt=nonce del anfitrión)`.
2. Ambos calculan un SAS interno; no se muestra ni se teclea.
3. El invitado confirma el SAS automáticamente y el anfitrión solo guarda el
   dispositivo cuando la confirmación coincide.
4. Se fija la huella TLS y se marca el dispositivo como `trusted`.
5. Se lanza el primer `sync_now` y se activa el auto-sync.

La clave `usuario-XXXX-XXXX` sigue existiendo para compatibilidad, pero ya no
forma parte del flujo de vinculación.

### Shares

Formato `SHARE-<SCOPE>-<XXXX>-<XXXX>`, con `scope_type` `library`, `book` o
`voice`, `permission` `read`/`copy`, `expires_at` y `max_uses`. Los
audiolibros usan una clave `SHARE-AUDIOBOOK-...` y `resource_type =
audiobook`; internamente conservan `scope_type = book` para respetar el CHECK
de SQLite de instalaciones anteriores. La clave se muestra al crear el share;
después se puede recuperar con PIN.

- `copy`: los libros y audiolibros se copian a la biblioteca del receptor con
  su `owner_account_id`.
- `read`: se copian como libros de solo lectura vinculados al share
  (`books.read_only = 1` y `received_share_id`).
- Un share de audiolibro incluye el M4B, su manifiesto y el libro asociado.
  Los assets grandes se reciben en archivos temporales por bloques (hasta
  4 GB) y se mueven al almacén al validar el SHA-256, sin cargarlos en memoria.
- El dueño incrementa `uses` antes de transferir y rechaza shares revocados,
  expirados o sin usos.
- `revoke_share` y `revoke_device` escriben en `audit_log`. Un dispositivo
  revocado es rechazado en la siguiente conexión de sync; un share revocado
  se rechaza en el siguiente `accept_share`.

Los audiolibros no viajan en la sincronización normal de la biblioteca: se
transfieren solo cuando el usuario crea un share explícito, para no mover M4B
grandes en cada `sync_now`.

Estado pendiente de esta fase:

- La vinculación de dispositivos ya incluye escáner QR en móvil; para shares
  en escritorio se pega la clave o el enlace.
- Compartir perfiles de voz clonada (`scope_type = voice`) está deshabilitado
  en la UI y devuelve `unsupported` al aceptar.
- La revocación de un share ya aceptado en modo `read` requiere quitarlo desde
  «Compartidos»; la propagación automática al receptor queda para la fase de
  empaquetado/sync cruzado.

### Búsqueda semántica (fase 11)

La búsqueda usa `fastembed` con `multilingual-e5-small` (384 dimensiones) y
prefijos `query:`/`passage:`. El modelo se descarga la primera vez a
`assets/models/embeddings` (unos 470 MB); después todo el índice y las
consultas son offline. Los vectores se guardan como BLOB `f32` little-endian y
el ranking se calcula con producto escalar sobre vectores normalizados, sin
depender de `sqlite-vec` ni de una extensión nativa de SQLite.

- EPUB: se indexan todos los capítulos.
- PDF: se usa la capa OCR guardada o el texto nativo de PDFium.
- La indexación corre en segundo plano y emite `semantic::progress`.
- En Android la búsqueda semántica queda deshabilitada hasta empaquetar
  `libonnxruntime` por ABI; `features_status.embeddings` devuelve `false`.

### Diario, logros y Pomodoro (fase 11)

- `session_start`/`session_end` registran lectura, escucha y Pomodoro con
  duración, páginas, extractos y notas creados durante la ventana.
- `get_diary` devuelve todos los días del mes; el frontend pinta el heatmap.
- Los 22 logros se recalculan al cerrar sesiones, importar libros, completar
  Pomodoros o abrir el diario. Los nuevos se emiten como
  `achievement::unlocked`.
- El Pomodoro usa 25/5/15 y 4 rondas por defecto; es configurable desde la
  vista Diario. Al completar un foco crea una entrada `pomodoro` y evalúa
  logros.
- La exportación mensual/anual genera EPUB en `exports/` y, opcionalmente,
  reutiliza `audiobooks::engine::synthesize_sections` para un M4B con la voz
  elegida.

## IA local, clasificación y conversión

### Asistente local

Kora no envía libros ni notas a servicios en la nube. En Android, **Ajustes →
IA local → Descargar y activar** descarga el modelo **Qwen2.5 0.5B Instruct
Q4_0** (~409 MB) y lo ejecuta en el teléfono con Candle, sin internet. El
lector ofrece resumen de escena, términos clave, personajes, consultas de
términos y chat sobre el capítulo actual. La descarga se verifica con SHA-256 y
puede borrarse con «Liberar espacio».

El **modo avanzado** (código de acceso `kora`) permite configurar un servidor
**Ollama** (`http://127.0.0.1:11434`) o compatible con OpenAI (`llama.cpp`, por
ejemplo `http://127.0.0.1:8080`). La configuración se guarda en `settings`
(`ai.config`) y se edita en **Ajustes → IA local**. En escritorio se sigue
usando Ollama/llama.cpp. Guía completa en [`docs/local-ai.md`](docs/local-ai.md).

### Clasificación asistida

`classify/` combina reglas locales, metadatos, embeddings y, si está
disponible, la IA local para proponer categorías. El catálogo base se crea con
`category_seed_apply` y las sugerencias viven en `category_suggestions`; el
usuario puede aceptarlas, descartarlas o aplicarlas todas. Las asignaciones
aceptadas usan `book_categories` con `source`, `confidence` y `locked`, así que
las hechas a mano no se pisan y siguen el flujo normal de sincronización. La
clasificación se puede activar al importar y se controla desde **Biblioteca**,
**Ajustes → Organización automática** y el modal de sugerencias.

### Conversión PDF→EPUB

La conversión analiza el PDF con `pdf_conversion_preview` y genera un EPUB con
texto, capítulos y portada. Usa PDFium/OCR cuando están disponibles y, si no,
acepta las páginas extraídas por `pdf.js` desde el visor. Los capítulos se
pueden dividir por el índice nativo, por heurística de encabezados o por bloques
de páginas. Al reemplazar el PDF original se conserva el mismo `book_id`, y las
anotaciones, extractos, notas, notas de voz y reacciones se reubican por texto;
el progreso se ajusta al nuevo número de capítulos. La operación revoca los
shares activos del PDF si su contenido cambia, reindexa el libro y avisa de que
las imágenes, tablas y fórmulas no se conservan. También se puede crear un EPUB
nuevo sin tocar el PDF.

### Papelera, copias y mantenimiento

`library_delete` mueve el libro a una papelera lógica (`books.deleted_at`), sin
borrar sus notas ni assets; `library_restore` lo recupera y reimportar el mismo
archivo también lo restaura. `storage_usage` desglosa el espacio y
`backup_library` genera un ZIP con la base de datos, libros, portadas, fotos de
perfil, audiolibros, voces, notas de voz y capas OCR (los modelos descargables quedan
fuera). `restore_library` valida el ZIP y prepara la restauración para el
siguiente arranque; `purge_storage` vacía papelera o temporales.

## Sincronización LAN

El módulo `sync` implementa descubrimiento, emparejamiento, transporte y
transferencia entre dispositivos de la misma LAN.

### Identidad y emparejamiento

Cada dispositivo genera una clave **Ed25519** y un certificado autofirmado la
primera vez que arranca sync. La clave privada queda en
`AppLocalData/sync/device.key` con permisos `0600` y el certificado en
`sync/device.crt`. La huella SHA-256 del certificado es la identidad estable.

La vinculación se anuncia por mDNS con `purpose=pair` y el `ticket_id`, sin
enviar el código por la red. El invitado puede descubrir el dispositivo, pegar
el enlace del QR o introducir la IP manualmente. El peer se guarda en `devices`
con `certificate_fingerprint`, `trusted=1` y su último `host:puerto`.

### Descubrimiento y transporte

- `mdns-sd` navega `_kora._udp.local.` mientras sync está activo. Se anuncia
  presencia con `purpose=sync`, y se cambia a `purpose=pair` o `purpose=share`
  mientras la pantalla correspondiente está abierta.
- El TXT incluye `v=2`, `purpose`, `ticket`, `account_hint`, `sas_prefix`,
  `device_id`, `fingerprint`, `name` y `platform`.
- `quinn` abre QUIC sobre UDP (puerto `47821`, con fallback efímero) y `rustls`
  exige certificado de cliente y servidor. La confianza real se comprueba
  después del handshake contra la huella guardada; un certificado distinto
  obliga a volver a emparejar.
- El protocolo usa frames JSON con longitud y binarios adjuntos para assets.
  ALPN: `kora/1`.

### Diff por hash y cola

Cada peer intercambia un manifiesto con `kind`, `id`, `hash`, `updated_at` y un
payload mínimo. Solo se piden los recursos ausentes o con hash distinto. Los
libros se deduplican por `content_hash` y se guarda un alias
`remote_id → local_id` cuando el mismo archivo ya existe.

Los triggers SQLite de `annotations`, `extracts`, `notes`, `voice_notes`,
`reactions`, `reading_progress`, `books`, `categories` y `book_categories`
alimentan `sync_pending`. `sync_state` guarda el último hash sincronizado por
peer y recurso.

### CRDT de anotaciones

Cada marcador, nota, extracto, reacción o nota de voz es un documento `yrs`
independiente con id UUIDv7. El `Y.Map` `data` guarda el payload de la fila y
la marca `deleted`. Los snapshots completos viven en `crdt_documents`; al
recibir un update se fusiona, se materializa en SQL y se conserva la tumba.
El progreso y los libros usan last-write-wins por `updated_at`.

### Colecciones, handoff y espejo

`sync_device_collections` permite elegir, para cada dispositivo emparejado,
todas las categorías (por defecto) o solo una lista. El modo se guarda en
`settings` como `sync.collections.mode.<device_id>` para distinguir «todas» de
«ninguna». Los libros sin categoría se incluyen en modo «todas».

Al abrir un libro se envía un `Handoff` a los peers conectados; el receptor
emite `sync::handoff` y el lector muestra el aviso «Retomar». La lectura en
espejo usa `start_mirror`/`join_mirror`: el host emite posiciones y estado TTS,
y el seguidor mueve su lector y usa su propia voz local. Si el host reproduce
un extracto, el evento TTS incluye `end` y el seguidor limita su propia sesión
al mismo rango. Los eventos de frontend son `mirror::position` y
`mirror::tts_state`.

## Anotaciones superpuestas

El panel **Anotaciones** del lector EPUB y del visor PDF tiene cuatro pestañas:
Marcadores, Notas, Extractos y Reacciones.

### Marcadores de color

Los marcadores reutilizan `annotations` con `highlight`, `underline`,
`bookmark` o `strike`. Se guardan con `chapter_index`, `offset_start`,
`offset_end` y `color`; el lector los pinta como capas absolutas sobre el texto
(EPUB) o sobre las palabras de cada página (PDF). `add_marker` valida colores
`#rrggbb`, `update_marker` cambia el color y `delete_annotation` aplica borrado
suave. El panel permite saltar al pasaje, cambiar el color y borrar.

### Notas ricas

`add_note` guarda el Delta JSON de Quill en `notes.content_json`. `notes.body`
conserva el texto plano para búsquedas y vistas previas, y `notes.text` guarda
la cita del pasaje. `update_note` reemplaza el contenido; el editor Quill ofrece
negrita, cursiva, subrayado, tachado, listas, citas, bloques de código y
enlaces. El visor es el mismo componente en modo de solo lectura.

### Notas de voz

La grabación usa `cpal` en un hilo independiente y un máximo de 5 minutos. Al
detener, `add_voice_note` codifica Ogg/Opus con `opus` + `ogg` y escribe
`assets/notes_audio/<uuidv7>.opus`. La fila `voice_notes` guarda el pasaje, la
duración y el propietario; el panel reproduce el archivo en línea con
`convertFileSrc` y el protocolo `asset:`. En Android la grabación devuelve
`unsupported`; el archivo existente sí puede reproducirse.

### Extractos buscables

`add_extract` guarda `chapter_index`, offsets, texto, nota y una cita legible
(`Libro · capítulo/página · rango`). `search_extracts` usa `instr(lower(...))`
sobre texto, nota y cita, sin interpretar `%` ni `_` como comodines.

### Reacciones y mapa emocional

`add_reaction` acepta `like`, `surprise`, `tension`, `boredom` y `emotion`.
`reaction_map` agrupa las reacciones activas por capítulo/página. El panel pinta
un heatmap proporcional al total, con leyenda por emoción y acceso directo a la
sección. La pestaña de reacciones incluye los cinco botones rápidos.

### UUIDv7 y CRDT

Los registros nuevos usan `Uuid::now_v7()` y conservan `created_at`,
`updated_at`, `owner_account_id` y `origin_share_id`. `annotations`, `extracts`,
`notes`, `voice_notes` y `reactions` tienen `deleted`; los listados filtran
`deleted = 0` y las tumbas se conservan para la sincronización de la fase 9.

## Audiolibros y Tejido

La generación usa el motor TTS de la fase 5 y acepta cualquier voz instalada,
incluidos los perfiles `clone:<uuid>` de la fase 6. El alcance puede ser
`chapter(n)`, `selection(section, start, end)` —offsets UTF-16 sobre el texto
visible— o `full_book`.

Cada libro se sintetiza por frases, se normaliza a mono 24 kHz y se guarda como
un WAV temporal por capítulo. Después `ffmpeg` concatena los capítulos, escribe
la metadata FFMETADATA1 y codifica AAC en un M4B con `faststart`:

```bash
ffmpeg -f concat -i concat.txt -i chapters.txt \
  -map 0:a:0 -map_metadata 1 -map_chapters 1 \
  -c:a aac -b:a 96k -ac 1 -ar 24000 -movflags +faststart libro.m4b
```

El audio queda en `assets/audiobooks/<sha256>.m4b` y el manifiesto JSON en el
mismo directorio. El manifiesto guarda, por capítulo, `startMs`, `endMs`,
`section`, `offsetStart` y `offsetEnd`, lo que permite resaltar el texto
mientras suena el audio. Si la voz es clonada, el M4B lleva metadatos
`KORA_CLONED_VOICE` y `KORA_WATERMARK`, y la UI muestra el aviso.

`play_audiobook` usa `rodio` con Symphonia (MP4/AAC y MP3) y permite pausar y
buscar. En Android la reproducción la hace Media3 ExoPlayer desde un
`MediaSessionService`, con notificación, controles de pantalla de bloqueo,
audio focus y reproducción con la pantalla apagada. `audiobook_progress`
guarda la posición por audiolibro; al volver a pulsar «Escuchar» se reanuda
donde se quedó.

`generate_audiobook` acepta un estilo de narración (neutro, calmado, dramático
o enérgico) y una velocidad base; ambos se guardan en el manifiesto y ajustan
la prosodia de cada fragmento durante la síntesis. En escritorio el M4B se
empaqueta con `ffmpeg`; en Android, `AudiobookMuxer` usa `MediaCodec` y
`MediaMuxer` y `AudiobookGenerationService` mantiene la generación en primer
plano. La vista de audiolibros expone **Crear audiolibro** desde el menú de un
libro y muestra un aviso claro si falta la voz, ffmpeg o el soporte nativo.

### Conversión entre texto y audio

- **Texto → audiolibro**: el menú de un libro EPUB o PDF abre el generador con
  el libro preseleccionado. La generación reutiliza la cola persistente, el
  reparto de voces y el mismo pipeline M4B.
- **Audiolibro sin texto → EPUB**: la tarjeta de un audiolibro marcado como
  **Solo audio** ofrece **Convertir a EPUB**. El modal permite elegir Whisper
  Tiny o Base, descargar el modelo y seguir el progreso; al terminar, el libro
  aparece en la biblioteca y el audiolibro se conserva.
- **Audiolibro con EPUB**: si el audiolibro nació de un EPUB o ya fue
  transcrito, el texto se puede **exportar** con `export_book` desde la
  biblioteca. Si nació de un PDF, se reutiliza la conversión PDF→EPUB existente.
- Los álbumes agrupan audiolibros en colecciones ordenadas. La migración
  `0026_audiobook_albums.sql` añade `audiobook_albums` y
  `audiobook_album_items`; la vista muestra un mosaico de portadas, permite
  reordenar y ofrece **Reproducir todo**. La cola de reproducción avanza al
  siguiente audiolibro mientras la app está en primer plano. Los álbumes son
  locales de la cuenta y no viajan en la sincronización normal porque los
  audiolibros solo se transfieren con un share explícito.

El **multivot por personaje** reutiliza el detector expresivo del lector. Antes
de generar, `audiobook_characters` devuelve los personajes del alcance con su
número de intervenciones y una muestra. El generador puede asignar una voz
manual a cada personaje; los que quedan en «Voz automática» reciben una voz
instalada en rotación, priorizando las que aún no usa otro personaje. El
narrador conserva siempre la voz principal. El reparto resuelto se guarda en
`cast` dentro del manifiesto (personaje, voz y nombre), y la cola lo conserva en
la columna `cast` de `audiobook_queue` (migración `0019_audiobook_cast.sql`).
Si alguna voz del reparto es clonada, el M4B se marca con la marca de agua
habitual.

Los **marcadores de voz** se activan desde el reproductor o desde la tarjeta
«Marcadores de voz». Kora abre el micrófono, segmenta las locuciones por
silencios y transcribe cada tramo con Whisper Tiny (si está instalado) o Base.
Cuando la transcripción contiene la frase configurada —por defecto «Kora, marca
esto»— crea una nota de voz anclada al pasaje que está sonando: guarda la cita
del karaoke, los offsets UTF-16 del texto, el comentario dicho después del
comando, la posición en milisegundos y el audio Opus. Los marcadores aparecen
como notas de voz en el panel de anotaciones y en un modal propio con salto al
punto de escucha. Los campos `audiobook_id`, `position_ms`, `quote`,
`transcript` y `source` viven en `voice_notes` (migración
`0020_voice_marks.sql`). Se recomienda usar auriculares para que la narración no
entre por el micrófono. En Android la escucha está deshabilitada.

Cada tarjeta y el reproductor permiten marcar el audiolibro como **favorito**
(`audiobook_favorites`, migración `0021_audiobook_favorites.sql`) y abrir el
modal de **compartir**. El share se crea con PIN y permiso, incluye el M4B, su
manifiesto y el libro asociado, y puede recuperarse después desde Cuenta. Al
crear la clave la app activa el anuncio `purpose=share` para que el receptor lo
descubra. Fuera de la pestaña de audiolibros, la barra superior de escritorio
muestra un **mini-reproductor** con portada, capítulo, tiempo, progreso,
retroceso de 15 s y pausa; en móvil se mantiene el reproductor flotante sobre
la barra inferior.

El tratamiento de audio (`apply_audiobook_treatment`) mide el volumen con una
primera pasada de `loudnorm`, aplica una segunda pasada con los valores medidos
y encadena los filtros `equalizer` del preset elegido. La re-codificación
conserva capítulos y metadatos, y el resultado se guarda como un asset nuevo.
`audiobook_waveform` decodifica el audio a PCM mono de 8 kHz y cachea 600
barras normalizadas en `cache/waveforms/<hash>.json`; la UI lo dibuja sobre la
barra de progreso del reproductor.

Cada capítulo del manifiesto guarda sus `karaoke` lines: tramos de audio con el
texto de la frase. La síntesis registra el inicio y el fin reales de cada
fragmento; la UI reparte la duración entre las palabras. En los audiolibros
importados, Whisper añade timestamps por token y el manifiesto guarda el tiempo
exacto de cada palabra. Los manifiestos antiguos caen a una estimación por
longitud de frase desde el texto del libro, cacheada en memoria. Los tiempos de
las líneas son relativos al capítulo y la respuesta de `audiobook_karaoke`
incluye el inicio absoluto para que el reproductor solo tenga que restar.

La importación de audio usa los capítulos embebidos del archivo; si no existen,
`detect_silences` busca pausas de al menos dos segundos con `silencedetect` y
`tone_boundaries` analiza el PCM a 8 kHz para encontrar marcas de 440 Hz o
1 kHz. La transcripción de Whisper se organiza por esos capítulos, no por
bloques fijos. El método queda en `chapterSource` dentro del manifiesto. La
importación comparte el control cooperativo de la cola: se puede pausar,
reanudar o cancelar entre fragmentos; al cancelar se elimina el audio copiado y
el workspace temporal.

La vista de audiolibros usa `enqueue_audiobook`: los trabajos se guardan en
`audiobook_queue` y sobreviven al cierre de la app. Un único worker procesa la
cola en orden, emite `audiobook::progress` y `audiobook::queue`, y permite
pausar, reanudar o cancelar entre fragmentos. Al arrancar, las generaciones que
quedaron a medias se marcan como canceladas y su trabajo vuelve a la cola para
reintentarse.

`list_audiobooks` devuelve el progreso, el capítulo actual y la última escucha
de cada elemento, de modo que la biblioteca muestra el porcentaje completado.
`export_audiobook` copia el M4B al destino que elija el usuario. La
importación extrae la portada embebida con ffmpeg cuando existe y la vista de
audiolibros informa del estado de ffmpeg y Whisper Base antes de transcribir.

### Importación inversa

`import_external_audiobook` acepta M4B, M4A, MP3, AAC, FLAC y OGG. Extrae
bloques de 10 minutos a WAV 16 kHz con ffmpeg, los transcribe con Whisper Base
(el modelo del modo ensayo), construye un EPUB consultable y lo registra como
`books.origin = transcribed`; el audio original queda como audiolibro
`origin = external`. Los capítulos del M4B se usan para titular los bloques.

### Tejido

`weave_extracts(extract_ids, title, voice_id)`:

1. valida que los extractos pertenecen al usuario;
2. crea un EPUB con un capítulo por extracto, índice, portada PNG generada
   (`image` + `ab_glyph` con Noto Sans incluida) y metadatos;
3. guarda el EPUB en `assets/generated_books/` y la portada en `assets/covers/`;
4. registra el libro con `origin = tejido`;
5. genera su audiolibro con el mismo pipeline y `origin = tejido`.

Los extractos se crean desde la selección del lector EPUB o del visor PDF con
el botón «Extracto» y se eligen en la pestaña **Audiolibros → Tejido**.

### Dependencias del sistema

La generación y la importación necesitan `ffmpeg` y `ffprobe` en el PATH, o las
variables `KORA_FFMPEG` y `KORA_FFPROBE`. En Fedora:

```bash
sudo dnf install ffmpeg
```

En Ubuntu/Debian/Zorin:

```bash
sudo apt install ffmpeg
```

La transcripción inversa necesita el modelo Whisper Base descargado desde
Ajustes → Modo ensayo.

## Documentación

- [`docs/INSTALACION.md`](docs/INSTALACION.md): instalación Linux y Android,
  datos, dependencias y primer arranque.
- [`docs/GUIA_DE_USO.md`](docs/GUIA_DE_USO.md): biblioteca, lector, voz,
  audiolibros, sync, cuenta, diario, ajustes y accesibilidad.
- [`docs/COMPILACION.md`](docs/COMPILACION.md): builds Linux/Android, i18n,
  hooks de librerías nativas y solución de problemas.
- [`docs/local-ai.md`](docs/local-ai.md): conexión con Ollama y llama.cpp sin
  nube.
- [`docs/lumina-adaptation.md`](docs/lumina-adaptation.md): migración visual y
  funcional, y pendientes de diseño.

## Próximas fases

1. Empaquetar `libonnxruntime` y el modelo de embeddings en el instalador, y
   habilitar la búsqueda semántica en Android.
2. Empaquetado Android de PDFium/Tesseract/Piper/ffmpeg y
   `kora-neutts` por ABI, además del modo ensayo y las notas de voz nativas,
   y validación de mDNS/QUIC en Android. La importación y la transcripción de
   audiolibros ya funcionan en el móvil con Whisper local.
3. Lector de QR de shares en móvil, compartir perfiles de voz y propagación
   automática de revocaciones.
4. Sincronización de audiolibros M4B y perfiles `.koravoice`.
5. Selector de archivos nativo en Android: las URIs `content://` ya se
   convierten a rutas temporales antes de importar; faltan metadatos y portadas
   embebidas en el propio móvil.
6. Empaquetar `ffmpeg`/`ffprobe` en `deb`/`rpm`/AppImage; el binario auxiliar
   `kora-neutts` ya se incluye mediante `externalBin` en
   `tauri.linux.conf.json`.

## Licencia

MIT.

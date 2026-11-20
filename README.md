# Kora

Lector de EPUB/PDF local-first con síntesis de voz, clonación de voz y
sincronización peer-to-peer en red local. Construido con **Rust + Tauri 2** y
**Svelte 5**, con objetivos Linux y Android.

> **Estado: fase 3 — lector EPUB.** La biblioteca importa EPUB/PDF por SHA-256
> y el lector abre EPUB con capítulos saneados, temas, texturas, tipografía
> adaptable, modo paginado opcional, progreso por libro y capítulo, y
> anotaciones superpuestas no destructivas. PDF, TTS y sync siguen pendientes.

### Verificación de esta fase

| Comprobación | Resultado |
|---|---|
| `pnpm build` (tsc + Vite) | ✅ |
| `pnpm check` (svelte-check) | ✅ 0 errores, 0 warnings |
| `cargo fmt --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ 25 tests |
| `tauri android init --ci` | ✅ `src-tauri/gen/android` generado |
| `tauri android build --apk` | ⚠️ no ejecutado: el NDK oficial es x86_64 y este host es aarch64 |

El lado Rust se compiló en un contenedor rootless `fedora:42` con
`webkit2gtk4.1-devel` y compañía, porque el host no tiene los headers de
desarrollo. Los tests cubren migraciones y checksums, CRUD por entidad,
cascadas, identidad local, assets SHA-256, papelera, el flujo de importación
con sustitución de portada, el parser EPUB con un fixture real (metadatos,
índice, portada, imágenes y `<script>` malicioso), el saneado del HTML y los
ajustes del lector.

## Stack

| Capa | Tecnología |
|---|---|
| Shell nativo | Tauri 2.12 |
| Backend | Rust 1.90+ (edition 2024) |
| Frontend | Svelte 5 + Vite 8 + TypeScript 5.9 |
| Base de datos | SQLite embebido (`sqlx` 0.8, feature `sqlite` con `libsqlite3-sys` bundled) |
| Hash de contenido | `sha2` + `hex` (SHA-256 en streaming) |
| Sync | mDNS (`mdns-sd`), QUIC (`quinn` + `rustls`), CRDT (`yrs`) |
| EPUB / PDF | `rbook` (EPUB) + `ammonia`/`scraper` (saneado) / `lopdf` (PDF) |
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
│  │  ├─ features/              # OCR, embeddings, diario, logros
│  │  ├─ commands/              # Comandos #[tauri::command]
│  │  ├─ state.rs               # AppState compartido
│  │  └─ error.rs               # KoraError { code, message }
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
assets/voices/           perfiles y muestras de voz
assets/notes_audio/      audio de notas
assets/audiobooks/       manifiestos y audio generado
assets/generated_books/  exportaciones generadas
exports/                 archivos generados por el usuario
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
| `library_import` | Copia un EPUB/PDF por SHA-256 y crea el libro |
| `library_delete` | Borra el libro, cascadas y mueve assets huérfanos a `trash/` |
| `library_replace_cover` | Guarda una portada nueva y limpia la anterior |
| `library_assign_category` | Asocia un libro a una categoría |
| `category_list` / `category_create` | Lista y crea categorías de la cuenta local |
| `reader_open` | Inspecciona el EPUB, completa metadatos/portada y devuelve sesión, progreso y ajustes |
| `reader_get_chapter` | Devuelve un capítulo saneado con imágenes incrustadas |
| `reader_update_progress` | Guarda capítulo, offset y porcentaje del libro |
| `reader_add_annotation` / `reader_update_annotation` / `reader_delete_annotation` | Anotaciones por `book_id` + capítulo + offsets |
| `reader_list_annotations` | Anotaciones del libro ordenadas por posición |
| `reader_get_settings` / `reader_save_settings` | Ajustes tipográficos y de tema por libro |
| `tts_engines` | Lista de motores registrados (vacía) |
| `tts_voices` | Lista vacía (fase 4) |
| `tts_synthesize` | `not_implemented` (fase 4) |
| `sync_status` / `sync_peers` | Estado inicial, sin pares |
| `sync_start` | `not_implemented` (fase 5) |
| `account_status` | Cuenta y dispositivo locales (se crean al arrancar) |
| `features_status` | Todos los flags apagados |

Los errores llegan al frontend como `{ code, message }`. Los códigos nuevos de
esta fase son `conflict` y `unsupported`.

## Desarrollo en Linux (Fedora)

Dependencias del sistema:

```bash
sudo dnf install -y \
  gcc gcc-c++ make pkg-config cmake \
  webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  openssl-devel librsvg2-devel patchelf
```

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

Comprobaciones:

```bash
pnpm build            # tsc + vite build
pnpm check            # svelte-check
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Compilar para Linux

```bash
pnpm tauri build
```

Genera `deb`, `rpm` y `AppImage` en `src-tauri/target/release/bundle/`.
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
pnpm tauri android dev                 # dispositivo/emulador conectado
pnpm tauri android build --apk         # APK debug
pnpm tauri android build --apk --release
```

El APK queda en
`src-tauri/gen/android/app/build/outputs/apk/universal/release/`.

### Firma de release

Crea un keystore y un archivo `src-tauri/gen/android/keystore.properties`:

```properties
storeFile=/ruta/a/kora-release.jks
storePassword=...
keyAlias=kora
keyPassword=...
```

Tauri lo detecta al compilar con `--release`. No subas el keystore ni el
`keystore.properties` al repositorio (ya están en `.gitignore`).

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

## Lector EPUB

El lector no inyecta el CSS del EPUB: extrae el `<body>` de cada capítulo, lo
sanea con `ammonia` (sin scripts, estilos ni manejadores de eventos) y
reescribe las imágenes locales como `data:` URIs con un límite de tamaño para
no saturar el WebView. Las anotaciones viven en una capa absoluta sobre el
texto, calculada con `Range.getClientRects()`, y nunca modifican el HTML
original.

- **Tipografía:** tamaño, interlineado, márgenes, zoom y familia
  (`serif`/`sans`/`mono`) por libro.
- **Apariencia:** temas claro, oscuro y sepia, con texturas lisa, papel, lino
  y cuadrícula dibujadas por CSS.
- **Navegación:** desplazamiento con inercia o modo paginado por columnas;
  flechas, `PageUp`/`PageDown` y botones de capítulo.
- **Progreso:** porcentaje por capítulo y por libro, guardado tras cada cambio
  con un pequeño debounce.
- **Anotaciones:** resaltado, subrayado, tachado y marcador, con nota y color;
  los offsets son unidades UTF-16 sobre el texto visible del capítulo.

Los ajustes y el progreso se guardan en SQLite (`reader_settings` y
`reading_progress`); las anotaciones usan `book_id`, `chapter_index`,
`offset_start` y `offset_end`.

## Próximas fases

1. PDF: texto + render detrás del mismo lector, con progreso real.
2. TTS: motores locales/remotos y reproducción.
3. Perfiles y clonación de voz.
4. Sync: identidad, pairing, mDNS, QUIC y CRDT.
5. Features: OCR, embeddings, diario y logros.
6. Extracción de metadatos y portadas embebidas al importar; selector de
   archivos nativo en Android.

## Licencia

MIT.

# Instalación de Kora

## Descarga oficial

La app se descarga solo desde el clúster:

- Landing: <https://kora.iguzman.com.mx/#descargas>
- Android: <https://kora.iguzman.com.mx/downloads/Kora-0.1.5-android.apk>
- Linux Debian/Ubuntu: <https://kora.iguzman.com.mx/downloads/Kora_0.1.5_amd64.deb>
- Linux Fedora/RPM: <https://kora.iguzman.com.mx/downloads/Kora-0.1.5-1.x86_64.rpm>

GitHub es únicamente el repositorio de código: no hay releases, binarios ni
GitHub Pages con descargas. Las secciones siguientes describen cómo generar los
paquetes desde el código para desarrollo.

## Linux

### Paquete `.deb`

El artefacto se genera en:

```text
target/release/bundle/deb/Kora_0.1.5_<arch>.deb
```

El sufijo depende de la arquitectura del host (`amd64` en x86_64, `arm64` en
aarch64). Instálalo con:

```bash
sudo apt install ./Kora_0.1.5_amd64.deb
```

> Usa `apt install ./archivo.deb`, no `dpkg -i`: `apt` descarga las
> dependencias que falten. Con `dpkg -i` verás errores como
> `kora depends on espeak-ng` o `liblept5`. Si ya lo intentaste con `dpkg`,
> completa la instalación con:
>
> ```bash
> sudo apt-get -f install
> sudo apt install ./Kora_0.1.5_amd64.deb
> ```

El paquete declara estas dependencias de ejecución: `espeak-ng`, `libasound2`
(o `libasound2t64`), `libgomp1`, `liblept5`, `libsecret-1-0`, `libsqlite3-0`,
`libtesseract5`, `libwebkit2gtk-4.1-0` y `libgtk-3-0`. `apt` las instala
automáticamente.

Si tu distribución prefiere RPM, Tauri también puede generar `rpm`; el
paquete `.rpm` debe compilarse en Fedora 42 para que coincidan las versiones de
Tesseract/Leptonica. El `.deb` es el objetivo verificado en Ubuntu/Debian.

### Actualizaciones

Desde la versión 0.1.4, la app consulta:

```text
https://kora.iguzman.com.mx/downloads/latest.json
```

El manifiesto lo genera `deploy.sh` con la versión, las URLs de los paquetes y
sus hashes SHA-256. Las builds nuevas usan siempre ese manifiesto y ya no permiten cambiarlo desde
Ajustes. Para pruebas de desarrollo se puede definir
`KORA_UPDATE_MANIFEST_URL` en el entorno del proceso. El APK de Android
descarga la actualización y la instala desde la propia app; en escritorio se
abre la sección de descargas del clúster.

### Dependencias opcionales

Kora funciona sin ellas, pero habilitan funciones adicionales:

| Función | Dependencia |
|---|---|
| PDF con texto/OCR | `libpdfium` (`KORA_PDFIUM_PATH`) y Tesseract 5 |
| TTS de escritorio | voces Piper descargadas desde **Voz** |
| Audiolibros M4B | `ffmpeg` en el `PATH` |
| Clonación de voz | modelos NeuTTS/NeuCodec descargados desde **Voz** |
| Modo ensayo | modelos Whisper descargados desde **Ajustes** |

### Importación en Android

En Android, el selector del sistema entrega URIs `content://`. Kora copia el
archivo elegido a su caché interna antes de importarlo, así que puedes abrir
EPUB y PDF desde Descargas, Drive o cualquier proveedor de documentos.

La importación de audiolibros en Android usa `MediaMetadataRetriever` para leer
título, artista, duración y portada embebida. Kora guarda el audio y crea
capítulos en bloques de diez minutos. La transcripción automática con Whisper
también funciona en el teléfono: descarga **Whisper Base** o **Tiny** desde el
modal de importación (147 MB / 78 MB), elige **Audio + texto** y mantén la app o
la pantalla bloqueada; un servicio en primer plano evita que Android corte la
tarea. La generación de audiolibros con voces sigue siendo exclusiva del
escritorio.

### Primer arranque

1. Elige idioma y tema en el onboarding.
2. Crea una cuenta local con PIN o pulsa **Empezar sin cuenta**.
3. Importa tu primer EPUB/PDF.
4. Si quieres búsqueda semántica, abre **Ajustes → Almacenamiento** y pulsa
   **Preparar modelo**. La primera descarga ronda los 470 MB y después todo es
   offline.

### Datos y copias

Los datos viven en `~/.local/share/dev.kora.reader/`:

```text
db/app.sqlite
assets/books, covers, avatars, voices, audiobooks, generated_books, ocr, tts, whisper
assets/models/embeddings
exports/
logs/
```

Para hacer una copia de seguridad, cierra Kora y copia la carpeta completa.
No copies solo `app.sqlite` mientras la aplicación está abierta: usa la copia
completa o exporta tus libros y el diario.

## Android (APK núcleo)

El APK de esta fase se compila con `.github/workflows/android.yml`, que solo
**valida** que la compilación Android siga funcionando; no publica artefactos ni
releases. El APK que se descarga es el que `deploy.sh` publica en el clúster.
Para obtener un APK, compílalo en una máquina x86_64 con el SDK/NDK
instalados:

```bash
pnpm android:build           # APK release (sin firmar si no hay keystore)
pnpm android:build:release   # APK release firmado si existe keystore.properties
```

El resultado queda en
`src-tauri/gen/android/app/build/outputs/apk/`. La firma de release se
configura en `src-tauri/gen/android/keystore.properties` (ver
`docs/COMPILACION.md`); sin ese archivo el APK se genera sin firmar.

El APK de release se firma con `src-tauri/gen/android/keystore.properties`
(firma V2). Android siempre pide permitir «Instalar apps desconocidas» la
primera vez porque el APK no viene de Play Store; eso no significa que use una
firma de depuración. Mientras la firma no cambie, las actualizaciones se
instalan encima de la versión anterior sin desinstalar.

Incluye:

- Biblioteca local, onboarding, temas e i18n.
- Lector EPUB y visor PDF con `pdf.js`.
- TTS nativo de Android.
- Diario, logros, Pomodoro y sync LAN.
- Exportación EPUB del diario.

No incluye todavía OCR, clonación NeuTTS, Piper, Whisper, audiolibros M4B
nativos ni PDFium/Tesseract embebidos. El directorio
`src-tauri/gen/android/app/src/main/jniLibs/` queda preparado para añadir esas
librerías por ABI. Consulta `docs/COMPILACION.md`.

Instala el APK con `adb install -r kora.apk` o desde el explorador de archivos
del dispositivo, permitiendo instalaciones de fuentes desconocidas.

## Primeros pasos rápidos

- **Biblioteca**: importa, filtra por categoría, cambia entre cuadrícula/lista
  y usa la búsqueda semántica.
- **Lector**: toca el texto para seleccionar, abre anotaciones o ajustes
  rápidos y usa los controles TTS.
- **Audiolibros**: reproduce, cambia la velocidad, salta por capítulos o abre
  el texto en la posición actual.
- **Voz**: descarga voces del sistema, crea perfiles clonados y expórtalos.
- **Sync**: inicia el anuncio, empareja desde **Cuenta** y elige categorías por
  dispositivo.
- **Diario**: revisa el heatmap, logros, Pomodoro y exporta EPUB/audiolibro.
- **Ajustes**: temas, texturas, idioma, bienestar y almacenamiento.

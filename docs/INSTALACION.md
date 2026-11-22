# Instalación de Kora

## Linux

### Paquete `.deb`

El artefacto se genera en:

```text
target/release/bundle/deb/Kora_0.1.0_<arch>.deb
```

El sufijo depende de la arquitectura del host (`amd64` en x86_64, `arm64` en
aarch64). Instálalo con:

```bash
sudo apt install ./Kora_0.1.0_amd64.deb
# o en Fedora/derivados con dpkg instalado:
sudo dpkg -i Kora_0.1.0_amd64.deb
```

Si tu distribución prefiere RPM, Tauri también puede generar `rpm`; el
paquete `.deb` es el objetivo verificado de esta fase.

### Dependencias opcionales

Kora funciona sin ellas, pero habilitan funciones adicionales:

| Función | Dependencia |
|---|---|
| PDF con texto/OCR | `libpdfium` (`KORA_PDFIUM_PATH`) y Tesseract 5 |
| TTS de escritorio | voces Piper descargadas desde **Voz** |
| Audiolibros M4B | `ffmpeg` en el `PATH` |
| Clonación de voz | modelos NeuTTS/NeuCodec descargados desde **Voz** |
| Modo ensayo | modelos Whisper descargados desde **Ajustes** |

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
**valida** que la compilación Android siga funcionando; no publica artefactos en
GitHub. Para obtener un APK, compílalo en una máquina x86_64 con el SDK/NDK
instalados:

```bash
pnpm android:build           # APK release (sin firmar si no hay keystore)
pnpm android:build:release   # APK release firmado si existe keystore.properties
```

El resultado queda en
`src-tauri/gen/android/app/build/outputs/apk/`. La firma de release se
configura en `src-tauri/gen/android/keystore.properties` (ver
`docs/COMPILACION.md`); sin ese archivo el APK se genera sin firmar.

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

# Compilación de Kora

## Requisitos generales

- Rust 1.90+ con `cargo`
- Node 22+ y `pnpm` 9
- Linux: `webkit2gtk4.1-devel`, `gtk3-devel`, `libsoup3-devel`, `openssl-devel`,
  `alsa-lib-devel`, `clang-devel`, `cmake`, `ffmpeg` y las dependencias de
  OCR/PDFium si quieres las funciones nativas de escritorio.
- Ubuntu/Debian/Zorin:

  ```bash
  sudo apt update
  sudo apt install -y \
    build-essential pkg-config cmake git clang libclang-dev \
    libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev \
    libssl-dev librsvg2-dev patchelf libasound2-dev \
    tesseract-ocr libtesseract-dev libleptonica-dev \
    tesseract-ocr-spa tesseract-ocr-eng \
    espeak-ng libespeak-ng-dev ffmpeg
  ```

- El proyecto incluye `scripts/tauri.mjs`, un wrapper que localiza el binding
  nativo de Tauri cuando pnpm no lo enlaza. Usa siempre `pnpm tauri ...`.

```bash
pnpm install
pnpm check
pnpm build
pnpm build:helper
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
KORA_PDFIUM_PATH=/ruta/libpdfium.so \
KORA_TTS_ROOT=/ruta/voces \
  cargo test --workspace
```

## Linux: `.deb`

```bash
pnpm tauri build --bundles deb
```

Salida:

```text
target/release/bundle/deb/Kora_0.1.2_<arch>.deb
```

Copia el paquete a `artifacts/` si quieres conservarlo antes de ejecutar
`cargo clean`:

```bash
mkdir -p artifacts
cp target/release/bundle/deb/Kora_0.1.2_amd64.deb artifacts/
```

Si compilas en un contenedor sin Node, puedes construir el frontend en el host
y saltarte `beforeBuildCommand` en el contenedor:

```bash
# host
pnpm build

# contenedor con Rust y dpkg
pnpm tauri build --bundles deb \
  --config '{"build":{"beforeBuildCommand":""}}'
```

`AppImage` en ARM64 requiere `linuxdeploy` para esa arquitectura; por eso el
paquete verificado de esta fase es `.deb`. También puedes generar `rpm` con
`--bundles rpm` si el contenedor tiene `rpm-build`.

## Android: APK núcleo

El NDK oficial de Google para Linux incluye un toolchain x86_64, así que un
host x86_64 compila el APK de forma nativa. En un host **aarch64** (Fedora
Asahi, Apple Silicon con Linux) la compilación con ese NDK no funciona: usa una
máquina x86_64 o el workflow `.github/workflows/android.yml`, que valida la
compilación en un runner x86_64.

El APK núcleo incluye EPUB/PDF con `pdf.js`, TTS nativo, sync, diario, logros,
Pomodoro, temas, i18n y exportación EPUB. No incluye OCR, clonación, Piper,
Whisper, audiolibros nativos ni PDFium embebido.

### Hooks para funciones nativas

1. Copia las `.so` compiladas para cada ABI en
   `src-tauri/gen/android/app/src/main/jniLibs/<abi>/`.
2. Añade el `cfg(target_os = "android")` o la feature correspondiente en
   `src-tauri/Cargo.toml` y registra el motor en `src-tauri/src/state.rs`.
3. Ajusta el workflow para compilar/descargar las librerías antes de
   `pnpm tauri android build --apk`.
4. Verifica el APK en un dispositivo arm64 real; el emulador x86_64 usa otra
   ABI.

Librerías previstas: `libpdfium.so`, `libtesseract.so`, `liblept.so`,
`libonnxruntime.so`, `libwhisper.so`, `libkora_neutts.so` y las dependencias de
ffmpeg que necesite el pipeline M4B.

### Comandos Android

```bash
pnpm tauri android init --ci
pnpm tauri android dev
pnpm tauri android build --apk          # release (firmado si hay keystore)
pnpm tauri android build --apk --debug  # debug universal
```

El APK de debug queda en
`src-tauri/gen/android/app/build/outputs/apk/universal/debug/` y puede ser
grande porque incluye las cuatro ABI y símbolos. El release firmado queda en
`src-tauri/gen/android/app/build/outputs/apk/universal/release/`.

## i18n

Los diccionarios están en `src/lib/i18n/index.ts`. `MessageKey` se deriva del
diccionario español y el diccionario inglés está tipado como
`Record<MessageKey, string>`, así que `pnpm check` falla si falta una clave.
Para añadir un idioma:

1. Crea un diccionario con todas las claves.
2. Añádelo a `dictionaries`.
3. Amplía `Locale` y la validación del backend (`commands/appearance.rs`).
4. Añade el selector en Ajustes/Onboarding.

## Solución de problemas

- **`resource path binaries/kora-neutts-... doesn't exist`**: ejecuta
  `pnpm build:helper` (debug) o `pnpm build:helper:release`; el script copia el
  binario al nombre que exige `externalBin`.
- **`pnpm tauri` termina con error de binding**: usa `pnpm tauri`, no
  `node_modules/.bin/tauri`; el wrapper busca el `.node` en `.pnpm`.
- **El modelo semántico no descarga**: revisa la conexión y vuelve a pulsar
  **Preparar modelo**. Después de la descarga todo es offline.
- **PDF sin texto**: instala PDFium/Tesseract en escritorio; en Android el
  visor funciona con `pdf.js`, pero el OCR nativo queda pendiente.
- **Sin audio**: revisa las voces Piper o el TTS nativo y el dispositivo de
  salida.
- **APK no instalado**: usa `adb install -r` y permite fuentes desconocidas.

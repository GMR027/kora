# Librerías nativas Android

Este directorio es el punto de entrada de las dependencias nativas del APK.
Cada carpeta ABI debe contener las `.so` que el backend Android cargue en
tiempo de ejecución:

```text
arm64-v8a/   libpdfium.so, libtesseract.so, liblept.so,
             libonnxruntime.so, libwhisper.so, libkora_neutts.so
x86_64/      las mismas librerías para emulador
armeabi-v7a/ opcional
x86/         opcional
```

El APK núcleo de esta fase no incluye esas librerías: el lector EPUB/PDF usa
`pdf.js`, el TTS usa `TextToSpeech` de Android, y OCR, clonación, audiolibros
nativos y transcripción quedan deshabilitados. Para habilitarlos hay que
copiar las `.so` compiladas para cada ABI y añadir los `cfg`/features
correspondientes en `src-tauri/Cargo.toml` y `src-tauri/src/state.rs`.

El workflow `.github/workflows/android.yml` construye el APK núcleo en un
runner x86_64. Los pasos para generar las librerías nativas están documentados
en `docs/COMPILACION.md`.

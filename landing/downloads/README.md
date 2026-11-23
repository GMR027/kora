# Descargas de la landing

La sección de descargas de `index.html` busca estos nombres exactos:

```text
Kora-0.1.2-android.apk
Kora_0.1.2_amd64.deb
Kora-0.1.2-1.x86_64.rpm
```

## APK

En este workspace hay un enlace simbólico a `artifacts/kora-release.apk`. Para
una copia independiente:

```bash
# desde landing/downloads/
cp ../../artifacts/kora-release.apk ./Kora-0.1.2-android.apk
```

## Linux

Genera los paquetes desde la raíz del repositorio:

```bash
pnpm build:linux                                   # .deb (compila el helper de voz)
pnpm build:helper:release && pnpm tauri build --bundles rpm   # .rpm
# o todos los bundles:
pnpm tauri build
```

Copia los artefactos (desde `landing/downloads/`):

```bash
cp ../../target/release/bundle/deb/Kora_0.1.2_amd64.deb .
cp ../../target/release/bundle/rpm/Kora-0.1.2-1.x86_64.rpm .
```

El sufijo de arquitectura cambia en hosts ARM (`arm64` / `aarch64`). Ajusta el
nombre y el `href` si publicas una arquitectura distinta.

## Comprobación automática

`main.js` hace un `HEAD` a cada archivo al cargar. Si responde 404/410, el botón
pasa a «En preparación» y muestra el comando de compilación al pulsarlo. Cuando
copias el archivo, la landing lo detecta en la siguiente recarga.

Para producción se recomienda subir los binarios a GitHub Releases y apuntar los
botones a las URLs de la release; así no se versionan archivos de ~100 MB.

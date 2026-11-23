# Descargas de la landing

La sección de descargas de `index.html` busca estos nombres exactos:

```text
Kora-0.1.3-android.apk
Kora_0.1.3_amd64.deb
Kora-0.1.3-1.x86_64.rpm
```

## APK

En este workspace hay un enlace simbólico a `artifacts/kora-release.apk`. Para
una copia independiente:

```bash
# desde landing/downloads/
cp ../../artifacts/kora-release.apk ./Kora-0.1.3-android.apk
```

## Linux

Genera los paquetes desde la raíz del repositorio:

```bash
pnpm build:linux                                   # .deb (compila el helper de voz)
pnpm build:helper:release && pnpm tauri build --bundles rpm   # .rpm (en Fedora 42)
# o todos los bundles:
pnpm tauri build
```

> Compila el `.rpm` dentro de Fedora 42 o un contenedor Fedora. Ubuntu genera
> un binario que enlaza `libtesseract.so.5`/`liblept.so.5`; Fedora 42 usa
> `libtesseract.so.5.5`/`libleptonica.so.6` y la app no arrancaría.

Copia los artefactos (desde `landing/downloads/`):

```bash
cp ../../target/release/bundle/deb/Kora_0.1.3_amd64.deb .
cp ../../target/release/bundle/rpm/Kora-0.1.3-1.x86_64.rpm .
```

En Debian/Ubuntu instala con `sudo apt install ./Kora_0.1.3_amd64.deb`, no con
`dpkg -i`: `apt` resuelve e instala `espeak-ng`, `liblept5`, `libtesseract5` y
el resto de dependencias. En Fedora, `sudo dnf install ./Kora-*.rpm`.

El sufijo de arquitectura cambia en hosts ARM (`arm64` / `aarch64`). Ajusta el
nombre y el `href` si publicas una arquitectura distinta.

## Comprobación automática

`main.js` hace un `HEAD` a cada archivo al cargar. Si responde 404/410, el botón
pasa a «En preparación» y muestra el comando de compilación al pulsarlo. Cuando
copias el archivo, la landing lo detecta en la siguiente recarga.

Para producción, `deploy.sh` publica `latest.json` junto a los binarios y la app
consulta ese manifiesto desde el clúster. No se usan releases de GitHub para las
descargas.

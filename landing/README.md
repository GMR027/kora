# Landing de Kora

Página de presentación estática de Kora: lector EPUB/PDF local-first, estudio de
audiolibros y sincronización LAN. Está separada de la app Tauri para poder
publicarla en cualquier hosting estático.

## Vista local

```bash
python3 -m http.server 4173 --directory landing
# Abre http://localhost:4173
```

También puedes abrir `landing/index.html` directamente, pero conviene usar un
servidor local para que funcionen las comprobaciones de descarga y las rutas
relativas.

## Estructura

```text
landing/
├─ index.html        # Página completa (hero, funciones, herramientas, descargas, FAQ)
├─ styles.css        # Sistema visual: papel cálido, Fraunces + Inter (Newsreader en el mockup)
├─ main.js           # Tema, menú, animaciones, copiar comandos y estado de descargas
├─ assets/           # Logo y fuentes locales (con sus licencias OFL)
└─ downloads/        # APK y paquetes Linux (ver downloads/README.md)
```

La página no usa CDN, ni cookies, ni analítica. Las fuentes y el logo se sirven
desde `assets/`.

## Descargas

La sección de descargas espera estos archivos:

| Plataforma | Archivo |
|---|---|
| Android | `downloads/Kora-0.1.4-android.apk` |
| Linux Debian/Ubuntu | `downloads/Kora_0.1.4_amd64.deb` |
| Linux Fedora/RPM | `downloads/Kora-0.1.4-1.x86_64.rpm` |

En este workspace el APK es un enlace simbólico a `artifacts/kora-release.apk`.
Los paquetes `.deb` y `.rpm` se generan con:

```bash
pnpm build:linux
# o, para todos los bundles de escritorio:
pnpm tauri build
```

Tauri deja los archivos en `target/release/bundle/`. Cópialos a
`landing/downloads/` con los nombres de la tabla. La página comprueba cada
archivo con `HEAD`; si falta, el botón muestra «En preparación» y ofrece el
comando de compilación, sin romper el resto de la sección.

Para una web pública, lo más cómodo es publicar el APK y los paquetes como
assets de una release de GitHub y cambiar los `href` de `index.html` por las
URLs de descarga. La lógica de disponibilidad seguirá funcionando.

## Stitch

El diseño se generó en Google Stitch y se implementó después con HTML/CSS/JS
propio para que sea autocontenido y fácil de desplegar.

- Proyecto: **Kora — Landing Page**
- ID: `7994893607947842224`
- URL: <https://stitch.withgoogle.com/projects/7994893607947842224>
- Sistema visual: **Warm Editorial Literary** (`assets/f6634c258c44446fa03ba4ba2e6457e4`)
- Pantalla de escritorio: `projects/7994893607947842224/screens/4cdf1a8b391f46fd9b7da0769cfaef66`
- Pantalla móvil (390 px): `projects/7994893607947842224/screens/cf6fa97ad6ab448a9cf1c7c6370794c8`

La API key vive en `.env` (ignorado por git):

```bash
STITCH_API_KEY=...
```

El puente `scripts/stitch.mjs` lee ese archivo y habla con el MCP de Stitch sin
depender de la configuración global de OpenCode:

```bash
node scripts/stitch.mjs list
node scripts/stitch.mjs list-tools
node scripts/stitch.mjs call list_projects '{}'
```

## Publicar

Copia el contenido de `landing/` a la raíz del hosting estático (GitHub Pages,
Netlify, Cloudflare Pages, un servidor propio…). Recuerda incluir:

- `index.html`, `styles.css`, `main.js`
- `assets/` completo
- `downloads/` con los binarios o enlaces a releases

Licencia del proyecto: MIT. Las fuentes incluidas conservan sus licencias OFL en
`assets/fonts/OFL-*.txt`.

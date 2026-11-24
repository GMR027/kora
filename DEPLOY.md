# Despliegue de Kora en Kubernetes

## URLs y recursos

- URL: **https://kora.iguzman.com.mx**
- Namespace: `kora`
- Release Helm: `kora`
- Imagen: `gmr27/kora:<tag>` y `gmr27/kora:latest`
- Contenido: landing estática, fuentes, capturas y descargas
- Sin base de datos ni almacenamiento persistente: todo el sitio vive en la imagen

> Kora es una aplicación Tauri local-first. Lo que se publica en Kubernetes es
> su **landing pública y las descargas** (APK y paquetes de Linux), no el
> backend Rust, que se ejecuta en el dispositivo del usuario.

## Archivos

| Archivo | Función |
|---|---|
| `docker/Dockerfile` | nginx Alpine con la landing y las descargas |
| `docker/nginx.conf` | Sitio estático, cachés, compresión y `/healthz` |
| `helm/kora/` | Chart con Deployment, Service e Ingress/TLS |
| `deploy.sh` | Prepara el contexto, compila, publica y despliega |
| `landing/` | HTML, CSS, JS, fuentes y logos del sitio |
| `artifacts/kora-release.apk` | APK que se publica como descarga |

## Primer despliegue y actualizaciones

```bash
cd /home/edgar/Documentos/programacion/kora

# Tag autoincremental
./deploy.sh

# O un tag explícito
./deploy.sh 4
```

El script:

1. Verifica Docker, Helm, kubectl y la sesión de Docker Hub.
2. Crea el namespace `kora` si no existe.
3. Copia `landing/` a un contexto temporal, reemplaza el enlace simbólico del
   APK por `artifacts/kora-release.apk` y añade `docker/nginx.conf`.
4. Construye y publica `gmr27/kora:<tag>` y `:latest`.
5. Ejecuta `helm upgrade --install` en el namespace `kora`.
6. Espera el rollout, el certificado TLS y comprueba `https://kora.iguzman.com.mx`.

## Descargas

La landing espera estos nombres dentro de `landing/downloads/`:

| Plataforma | Archivo |
|---|---|
| Android | `Kora-0.1.5-android.apk` |
| Linux Debian/Ubuntu | `Kora_0.1.5_amd64.deb` |
| Linux Fedora/RPM | `Kora-0.1.5-1.x86_64.rpm` |

`deploy.sh` sustituye el APK por `artifacts/kora-release.apk`. Para publicar los
paquetes de escritorio, cópialos a `landing/downloads/` antes de desplegar:

```bash
pnpm build:linux
pnpm build:helper:release && pnpm tauri build --bundles rpm

cp target/release/bundle/deb/Kora_0.1.5_amd64.deb landing/downloads/
cp target/release/bundle/rpm/Kora-0.1.5-1.x86_64.rpm landing/downloads/
```

> El `.rpm` debe compilarse en Fedora 42 (o un contenedor Fedora): Ubuntu enlaza
> `libtesseract.so.5`/`liblept.so.5` y Fedora usa
> `libtesseract.so.5.5`/`libleptonica.so.6`. Un RPM generado en Ubuntu instala
> pero la app no arranca por esas diferencias de soname.

`landing/main.js` hace un `HEAD` a cada archivo: si falta, el botón muestra
«En preparación» sin romper la sección.

Se publican el APK de Android y los instaladores `.deb` y `.rpm` de Linux.
`deploy.sh` copia todo `landing/downloads/`, así que los paquetes deben existir
ahí antes de desplegar y falla si falta alguno. GitHub no es un canal de
descarga: los binarios viven solo en el clúster.

## Actualizaciones

`deploy.sh` genera `downloads/latest.json` dentro de la imagen con la versión de
`package.json`, las URLs de los tres artefactos y sus hashes SHA-256. La app
consulta:

```text
https://kora.iguzman.com.mx/downloads/latest.json
```

El manifiesto se sirve con `Cache-Control: no-store`; los binarios versionados
conservan la caché de una hora. Para publicar una actualización:

1. Sube la versión en `package.json`, `src-tauri/tauri.conf.json` y
   `versionCode` de Android.
2. Compila APK, `.deb` y `.rpm` con los nombres nuevos.
3. Copia los paquetes a `landing/downloads/`, actualiza `landing/index.html` y
   ejecuta `./deploy.sh`.

Las builds nuevas consultan siempre este manifiesto y ya no tienen campo
editable en Ajustes. La URL solo se puede sobrescribir con
`KORA_UPDATE_MANIFEST_URL` en builds de desarrollo. GitHub no es un canal de
descarga ni de actualizaciones.

## Verificación rápida

```bash
kubectl get pods,svc,ingress -n kora
kubectl get certificate -n kora
helm list -n kora
curl -I https://kora.iguzman.com.mx
curl -I https://kora.iguzman.com.mx/downloads/Kora-0.1.5-android.apk
curl -fsS https://kora.iguzman.com.mx/downloads/latest.json | jq
curl -sS https://kora.iguzman.com.mx/healthz
```

## Notas

- TLS automático con `cert-manager` y `ClusterIssuer letsencrypt-prod`.
- Ingress clase `public`, igual que `cause.iguzman.com.mx`.
- El dominio `kora.iguzman.com.mx` ya resuelve al mismo clúster que Cauce.
- Los cambios de contenido requieren un despliegue nuevo; no hay PVC que
  editar. Para actualizar solo el APK, reemplaza `artifacts/kora-release.apk` y
  ejecuta `./deploy.sh`.
- A diferencia de Cauce, no hay Secret, MySQL ni CronJob: el sitio es estático.

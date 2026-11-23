#!/usr/bin/env bash
# ===========================================================================
# deploy.sh — construye, publica y despliega una actualización de Kora.
#
# Uso:
#   ./deploy.sh            # tag auto-incremental (último + 1)
#   ./deploy.sh 7          # usar un tag concreto
#
# Flujo:
#   1. Verifica docker, helm, kubectl y sesión de Docker Hub
#   2. Crea el namespace kora si no existe
#   3. Prepara un contexto con landing/ + el APK de artifacts/
#   4. docker build → gmr27/kora:<tag> (+ :latest)
#   5. docker push
#   6. helm upgrade --install en el namespace kora
#   7. Espera el rollout y el certificado TLS
#
# El sitio es estático: no hay base de datos ni PVC que conservar entre
# actualizaciones.
# ===========================================================================
set -euo pipefail

REPO="gmr27/kora"
NAMESPACE="kora"
RELEASE="kora"
URL="https://kora.iguzman.com.mx"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHART_DIR="$ROOT_DIR/helm/kora"
TAG_FILE="$ROOT_DIR/.deploy-tag"
APK_SOURCE="$ROOT_DIR/artifacts/kora-release.apk"
APP_VERSION="$(node -p "require('${ROOT_DIR}/package.json').version")"
APK_NAME="Kora-${APP_VERSION}-android.apk"
DEB_NAME="Kora_${APP_VERSION}_amd64.deb"
RPM_NAME="Kora-${APP_VERSION}-1.x86_64.rpm"
DEB_SOURCE="$ROOT_DIR/landing/downloads/$DEB_NAME"
RPM_SOURCE="$ROOT_DIR/landing/downloads/$RPM_NAME"
NOTES_FILE="$ROOT_DIR/RELEASE_NOTES.md"

[[ "$APP_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || { echo "Versión inválida en package.json: $APP_VERSION" >&2; exit 1; }

verde()    { printf '\033[32m%s\033[0m\n' "$*"; }
amarillo() { printf '\033[33m%s\033[0m\n' "$*"; }
rojo()     { printf '\033[31m%s\033[0m\n' "$*" >&2; }

# --- Tag -------------------------------------------------------------------
if [[ $# -ge 1 ]]; then
    TAG="$1"
    [[ "$TAG" =~ ^[A-Za-z0-9._-]+$ ]] || { rojo "Tag inválido: $TAG"; exit 1; }
else
    ULTIMO=0
    [[ -f "$TAG_FILE" ]] && ULTIMO="$(cat "$TAG_FILE")"
    [[ "$ULTIMO" =~ ^[0-9]+$ ]] || ULTIMO=0
    TAG=$((ULTIMO + 1))
fi
verde "==> Tag de esta versión: $TAG"

# --- Requisitos -------------------------------------------------------------
for cmd in docker helm kubectl node; do
    command -v "$cmd" >/dev/null 2>&1 || { rojo "Falta '$cmd' en el PATH."; exit 1; }
done

if ! grep -q '"auths"' "$HOME/.docker/config.json" 2>/dev/null \
   || grep -q '"auths"[[:space:]]*:[[:space:]]*{}' "$HOME/.docker/config.json" 2>/dev/null; then
    rojo "No hay credenciales de Docker Hub. Ejecuta primero: docker login"
    exit 1
fi

kubectl get namespace "$NAMESPACE" >/dev/null 2>&1 \
    || kubectl create namespace "$NAMESPACE"

# --- Contexto de build ------------------------------------------------------
# Se copia landing/ a un directorio temporal para cambiar el enlace simbólico
# del APK por el archivo real y no enviar al daemon todo el repositorio.
CONTEXTO="$(mktemp -d "${TMPDIR:-/tmp}/kora-deploy.XXXXXX")"
trap 'rm -rf "$CONTEXTO"' EXIT

mkdir -p "$CONTEXTO/html/downloads"
cp -a "$ROOT_DIR/landing/." "$CONTEXTO/html/"
rm -f "$CONTEXTO/html/downloads/"*.apk
cp "$ROOT_DIR/docker/nginx.conf" "$CONTEXTO/nginx.conf"

for source in "$APK_SOURCE" "$DEB_SOURCE" "$RPM_SOURCE"; do
    if [[ ! -f "$source" ]]; then
        rojo "Falta el artefacto requerido: $source"
        rojo "Compila los tres paquetes antes de desplegar."
        exit 1
    fi
done

cp "$APK_SOURCE" "$CONTEXTO/html/downloads/$APK_NAME"
find "$CONTEXTO/html/downloads" -maxdepth 1 -type f \
    \( -name 'Kora_*.deb' -o -name 'Kora-*.rpm' \) \
    ! -name "$DEB_NAME" ! -name "$RPM_NAME" -delete
verde "==> APK incluido: $APK_NAME ($(du -h "$APK_SOURCE" | cut -f1))"
verde "==> DEB incluido: $DEB_NAME ($(du -h "$DEB_SOURCE" | cut -f1))"
verde "==> RPM incluido: $RPM_NAME ($(du -h "$RPM_SOURCE" | cut -f1))"

APK_SHA="$(sha256sum "$CONTEXTO/html/downloads/$APK_NAME" | awk '{print $1}')"
DEB_SHA="$(sha256sum "$CONTEXTO/html/downloads/$DEB_NAME" | awk '{print $1}')"
RPM_SHA="$(sha256sum "$CONTEXTO/html/downloads/$RPM_NAME" | awk '{print $1}')"

if [[ -f "$NOTES_FILE" ]]; then
    RELEASE_NOTES="$(cat "$NOTES_FILE")"
else
    RELEASE_NOTES="Actualización de Kora $APP_VERSION."
fi

CONTEXTO="$CONTEXTO" PUBLIC_URL="$URL" APP_VERSION="$APP_VERSION" \
APK_NAME="$APK_NAME" APK_SHA="$APK_SHA" \
DEB_NAME="$DEB_NAME" DEB_SHA="$DEB_SHA" \
RPM_NAME="$RPM_NAME" RPM_SHA="$RPM_SHA" \
RELEASE_NOTES="$RELEASE_NOTES" \
node <<'NODE'
const fs = require("fs");
const path = require("path");

const env = process.env;
const base = env.PUBLIC_URL.replace(/\/$/, "");
const asset = (name, sha256) => ({
  url: `${base}/downloads/${name}`,
  sha256,
});

const apk = asset(env.APK_NAME, env.APK_SHA);
const manifest = {
  version: env.APP_VERSION,
  notes: env.RELEASE_NOTES,
  publishedAt: new Date().toISOString(),
  // Compatibilidad con clientes antiguos: URL directa del APK.
  url: apk.url,
  sha256: apk.sha256,
  releaseUrl: `${base}/#descargas`,
  android: apk,
  linux: {
    deb: asset(env.DEB_NAME, env.DEB_SHA),
    rpm: asset(env.RPM_NAME, env.RPM_SHA),
  },
};

const target = path.join(env.CONTEXTO, "html", "downloads", "latest.json");
fs.writeFileSync(target, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`Manifiesto generado: ${target}`);
NODE

verde "==> Contexto preparado en $CONTEXTO"

# --- Build ------------------------------------------------------------------
COMMIT="$(git -C "$ROOT_DIR" rev-parse --short HEAD 2>/dev/null || echo sin-git)"
FECHA_BUILD="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

verde "==> Construyendo imagen $REPO:$TAG..."
docker build \
    -f "$ROOT_DIR/docker/Dockerfile" \
    --label "org.opencontainers.image.revision=$COMMIT" \
    --label "org.opencontainers.image.created=$FECHA_BUILD" \
    --label "org.opencontainers.image.title=kora" \
    -t "$REPO:$TAG" \
    -t "$REPO:latest" \
    "$CONTEXTO"

# --- Push -------------------------------------------------------------------
verde "==> Subiendo a Docker Hub..."
docker push "$REPO:$TAG"
docker push "$REPO:latest"

# --- Deploy -----------------------------------------------------------------
verde "==> Desplegando con Helm en el namespace '$NAMESPACE'..."
helm upgrade --install "$RELEASE" "$CHART_DIR" \
    --namespace "$NAMESPACE" \
    --create-namespace \
    -f "$CHART_DIR/values.yaml" \
    --set-string image.tag="$TAG" \
    --timeout 10m

echo "$TAG" > "$TAG_FILE"

# --- Verificación -----------------------------------------------------------
verde "==> Esperando el rollout..."
kubectl rollout status deployment/kora -n "$NAMESPACE" --timeout=180s

verde "==> Esperando el certificado TLS..."
kubectl wait --for=condition=Ready certificate/kora-cert -n "$NAMESPACE" --timeout=300s 2>/dev/null \
    || amarillo "    El certificado aún puede estar emitiéndose; revisa: kubectl get certificate -n $NAMESPACE"

echo
verde "==> Despliegue completado. Estado actual:"
kubectl get pods,svc,ingress -n "$NAMESPACE"
echo

if curl -fsS --max-time 10 "$URL/healthz" >/dev/null 2>&1; then
    verde "==> Sitio respondiendo en $URL"
else
    amarillo "==> El sitio aún no responde por HTTPS; revisa DNS, Ingress y certificado."
fi

if MANIFEST="$(curl -fsS --max-time 10 "$URL/downloads/latest.json" 2>/dev/null)"; then
    if printf '%s' "$MANIFEST" | grep -q "\"version\": \"$APP_VERSION\""; then
        verde "==> Manifiesto de actualizaciones sirviendo v$APP_VERSION"
    else
        rojo "==> latest.json responde, pero no contiene la versión $APP_VERSION."
    fi
else
    amarillo "==> latest.json aún no responde."
fi

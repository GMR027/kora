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
APK_NAME="Kora-0.1.2-android.apk"

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
for cmd in docker helm kubectl; do
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

if [[ -f "$APK_SOURCE" ]]; then
    cp "$APK_SOURCE" "$CONTEXTO/html/downloads/$APK_NAME"
    verde "==> APK incluido: $APK_NAME ($(du -h "$APK_SOURCE" | cut -f1))"
else
    amarillo "==> No existe $APK_SOURCE: la landing mostrará el APK como «En preparación»."
fi

# Los instaladores de Linux se copian con el resto de landing/downloads/.
for pkg in "$CONTEXTO"/html/downloads/*.deb "$CONTEXTO"/html/downloads/*.rpm; do
    if [[ -f "$pkg" ]]; then
        verde "==> Instalador incluido: $(basename "$pkg") ($(du -h "$pkg" | cut -f1))"
    else
        amarillo "==> Falta un instalador de Linux: la landing lo mostrará como «En preparación»."
    fi
done

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

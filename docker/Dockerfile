# ===========================================================================
# Dockerfile — Kora (landing estática + descargas)
#
# El contexto de build lo prepara deploy.sh: contiene `html/` con el contenido
# de landing/ y `nginx.conf`. Así el APK real sustituye al enlace simbólico
# local y el paquete queda disponible en /downloads/.
# ===========================================================================

FROM nginx:stable-alpine

LABEL org.opencontainers.image.title="kora" \
      org.opencontainers.image.description="Kora · landing y descargas"

RUN rm -rf /usr/share/nginx/html/*

COPY html/ /usr/share/nginx/html/
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

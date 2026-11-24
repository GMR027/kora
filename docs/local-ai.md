# Asistente local en Kora

Kora no usa ninguna API de pago ni envía libros, notas o conversaciones a
Internet. El asistente de lectura se ejecuta en el propio dispositivo o habla
con un servidor de modelos en la red local.

## Android: asistente integrado (recomendado)

1. Abre **Ajustes → IA local**.
2. Pulsa **Descargar y activar**.
3. Espera la descarga única de unos 475 MB.
4. Pulsa **Probar** para comprobar que responde.

Kora descarga **Qwen2.5 0.5B Instruct Q4_K_M**, verifica su SHA-256 y lo ejecuta
en el teléfono con Candle. Después funciona sin internet y sin servidores
externos. El modelo se puede borrar con **Liberar espacio** y volver a descargar
cuando se necesite.

El lector ofrece resumen de escena, términos clave, personajes y chat sobre el
capítulo actual. La organización automática usa el mismo asistente cuando está
activo.

## Modo avanzado: Ollama o llama.cpp

El modo avanzado está oculto para no pedir URLs ni modelos a usuarios normales.
Para abrirlo, en **Ajustes → IA local → Avanzado** introduce el código:

```text
kora
```

Dentro puedes elegir proveedor, URL, modelo, temperatura y límite de tokens.

### PC: Ollama

1. Instala [Ollama](https://ollama.com/) y arranca el servicio:

   ```bash
   ollama serve
   ```

2. Descarga un modelo de chat:

   ```bash
   ollama pull qwen2.5:3b-instruct
   ```

3. En Kora, elige **Ollama**, usa `http://127.0.0.1:11434` y pulsa **Comprobar**.

Kora detecta los modelos con `/v1/models` y, si el servidor es antiguo, con
`/api/tags`. Para responder usa `/v1/chat/completions` y, como respaldo,
`/api/chat`.

### llama.cpp

Kora usa el protocolo OpenAI-compatible de `llama.cpp`. El servidor puede
ejecutarse en otro equipo de la red local:

```bash
llama-server \
  -m Qwen2.5-1.5B-Instruct-Q4_K_M.gguf \
  --host 0.0.0.0 \
  --port 8080 \
  -c 8192
```

Después, en el modo avanzado de Kora, elige **llama.cpp** y usa
`http://<ip-del-equipo>:8080`.

### Usar Ollama del PC desde Android

Si el móvil y el PC están en la misma red, cambia la URL de Ollama por la IP
local del PC, por ejemplo `http://192.168.1.40:11434`. Asegúrate de que Ollama
escuche en la interfaz de red y de que el cortafuegos permita el puerto. Kora no
requiere cuenta ni clave para este modo.

## Qué envía Kora al modelo

- El título, autor y capítulo actual.
- Un fragmento del capítulo actual (hasta ~7.000 caracteres).
- La pregunta o la tarea de análisis.

No envía la biblioteca completa, el historial de lectura ni las notas. El panel
permite guardar el resumen como nota local.

## Estado actual

- Android incluye un asistente local con Qwen2.5 0.5B y Candle; no necesita
  servidor externo después de la descarga.
- Ollama y llama.cpp siguen disponibles en el modo avanzado.
- La app permite HTTP local en Android para el modo avanzado; el asistente
  integrado no usa red.
- El modelo se descarga desde Hugging Face una sola vez y se verifica con
  SHA-256.

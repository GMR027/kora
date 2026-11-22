# IA local en Kora

Kora no usa ninguna API de pago ni envía libros, notas o conversaciones a Internet. El asistente de lectura habla únicamente con un servidor de modelos que se ejecuta en el equipo del usuario o en su red local.

Hay dos caminos, ambos gratuitos:

## PC: Ollama

1. Instala [Ollama](https://ollama.com/) y arranca el servicio:

   ```bash
   ollama serve
   ```

2. Descarga un modelo de chat. Recomendados para lectura:

   ```bash
   ollama pull qwen2.5:3b-instruct
   # o, si el equipo tiene más memoria:
   ollama pull qwen2.5:7b-instruct
   ```

3. Abre **Ajustes → IA local**, elige **Ollama**, usa `http://127.0.0.1:11434` y pulsa **Comprobar**.

Kora detecta los modelos con `/v1/models` y, si el servidor es antiguo, con `/api/tags`. Para responder usa `/v1/chat/completions` y, como respaldo, `/api/chat`.

## Android: llama.cpp

Kora usa el protocolo OpenAI-compatible de `llama.cpp`. El servidor puede ejecutarse en el propio dispositivo (por ejemplo, desde Termux o una app que lance `llama-server`) o en otro equipo de la red local.

```bash
llama-server \
  -m Qwen2.5-1.5B-Instruct-Q4_K_M.gguf \
  --host 127.0.0.1 \
  --port 8080 \
  -c 8192
```

Después, en **Ajustes → IA local**, elige **llama.cpp** y usa `http://127.0.0.1:8080`.

Para modelos pequeños en Android se recomienda un GGUF cuantizado Q4 con 1–3 mil millones de parámetros. El contexto de Kora está limitado a unos 7.000 caracteres por capítulo para que quepa en contextos de 4.096–8.192 tokens.

### Usar Ollama del PC desde Android

Si el móvil y el PC están en la misma red, cambia la URL de Ollama por la IP local del PC, por ejemplo `http://192.168.1.40:11434`. Asegúrate de que Ollama escuche en la interfaz de red y de que el cortafuegos permita el puerto. Kora no requiere cuenta ni clave para este modo.

## Qué envía Kora al modelo

- El título, autor y capítulo actual.
- Un fragmento del capítulo actual (hasta ~7.000 caracteres).
- La pregunta o la tarea de análisis.

No envía la biblioteca completa, el historial de lectura ni las notas. El panel permite guardar el resumen como nota local.

## Estado actual

- La integración con Ollama y con el servidor OpenAI-compatible de llama.cpp está implementada.
- El APK de Android **no incluye todavía un binario de llama.cpp embebido**; se conecta al servidor local configurado. Embeberlo requiere compilar llama.cpp por ABI y empaquetar el modelo, un trabajo aparte que no se simula en la interfaz.
- No hay descarga automática de modelos: se usan los que el usuario ya tenga en Ollama o llama.cpp.
- La app permite HTTP local en Android para que el asistente funcione en producción; solo debe apuntar a servidores propios.

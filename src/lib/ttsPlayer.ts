import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { get, writable } from "svelte/store";

import { ttsPause, ttsResume, ttsStart, ttsStatus, ttsStop } from "./api";
import type { TtsPhraseEvent, TtsSessionEvent, TtsStatus } from "./types";

/**
 * Estado global de la lectura en voz alta.
 *
 * La sesión TTS vive en Rust y no depende de que el lector siga montado. Este
 * store permite que el mini-reproductor de AppShell la controle desde
 * cualquier pantalla.
 */
export const ttsPlayback = writable<TtsStatus | null>(null);

const AUTO_ADVANCE_KEY = "kora.tts.autoAdvance";

function loadAutoAdvance(): boolean {
  try {
    return globalThis.localStorage?.getItem(AUTO_ADVANCE_KEY) !== "false";
  } catch {
    return true;
  }
}

/** Avance automático de capítulo compartido por lector y reproducción global. */
export const ttsAutoAdvance = writable(loadAutoAdvance());
ttsAutoAdvance.subscribe((value) => {
  try {
    globalThis.localStorage?.setItem(AUTO_ADVANCE_KEY, value ? "true" : "false");
  } catch {
    // Sin almacenamiento local se mantiene el valor de la sesión.
  }
});

/** El lector montado ya gestiona el autoavance; el global solo actúa fuera. */
export const ttsReaderVisible = writable(false);

let initialized = false;
let unlisten: UnlistenFn | undefined;
let unlistenPhrase: UnlistenFn | undefined;

export async function initTtsPlayer() {
  if (initialized) return;
  initialized = true;

  try {
    unlisten = await listen<TtsSessionEvent>("tts::state", (event) => {
      void handleTtsState(event.payload);
    });
    unlistenPhrase = await listen<TtsPhraseEvent>(
      "tts::phrase_start",
      (event) => {
        const phrase = event.payload;
        ttsPlayback.update((status) =>
          status && status.bookId === phrase.bookId
            ? {
                ...status,
                section: phrase.section,
                chunkIndex: phrase.chunkIndex,
                phraseStart: phrase.offsetStart,
                phraseEnd: phrase.offsetEnd,
              }
            : status,
        );
      },
    );
  } catch {
    initialized = false;
    return;
  }
  await refreshTtsPlayback();
}

async function handleTtsState(event: TtsSessionEvent) {
  await refreshTtsPlayback();

  if (event.state !== "finished" || event.excerpt) return;
  if (!get(ttsAutoAdvance) || get(ttsReaderVisible)) return;
  if (!event.bookId) return;

  try {
    const next = await ttsStart(event.bookId, {
      section: event.section + 1,
      offset: 0,
    });
    ttsPlayback.set(next);
  } catch {
    // Fin del libro o sección sin texto: se cierra la sesión terminada.
    try {
      await ttsStop();
    } catch {
      // La sesión ya no existe.
    }
    ttsPlayback.set(null);
  }
}

export async function refreshTtsPlayback() {
  try {
    const status = await ttsStatus();
    ttsPlayback.set(
      status.state === "idle" || status.state === "stopped" ? null : status,
    );
  } catch {
    ttsPlayback.set(null);
  }
}

export async function toggleTtsPlayback() {
  const current = get(ttsPlayback);
  if (!current) return;
  try {
    if (current.state === "preparing") {
      await ttsStop();
      ttsPlayback.set(null);
    } else if (current.state === "playing") {
      ttsPlayback.set(await ttsPause());
    } else if (current.state === "paused") {
      ttsPlayback.set(await ttsResume());
    }
  } catch {
    await refreshTtsPlayback();
  }
}

export async function stopTtsPlayback() {
  try {
    await ttsStop();
  } finally {
    ttsPlayback.set(null);
  }
}

export function disposeTtsPlayer() {
  unlisten?.();
  unlistenPhrase?.();
  unlisten = undefined;
  unlistenPhrase = undefined;
  initialized = false;
}

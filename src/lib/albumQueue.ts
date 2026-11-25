import { get, writable } from "svelte/store";

import { playAudiobook } from "./api";
import type { PlaybackSnapshot } from "./types";

/**
 * Cola de reproducción de un álbum. Vive fuera de la vista para que el
 * reproductor global pueda avanzar al siguiente audiolibro aunque el usuario
 * cambie de pestaña. En Android, con la app suspendida, el avance nativo queda
 * pendiente para una fase posterior.
 */
export interface AlbumPlaybackQueue {
  albumId: string;
  audiobookIds: string[];
  index: number;
}

export const albumPlayback = writable<AlbumPlaybackQueue | null>(null);

let advancing = false;

export function startAlbumQueue(
  albumId: string,
  audiobookIds: string[],
  startIndex = 0,
): void {
  if (audiobookIds.length === 0) {
    albumPlayback.set(null);
    return;
  }
  albumPlayback.set({
    albumId,
    audiobookIds: [...audiobookIds],
    index: Math.max(0, Math.min(startIndex, audiobookIds.length - 1)),
  });
}

export function clearAlbumQueue(): void {
  albumPlayback.set(null);
}

export function albumQueueFor(audiobookId: string): AlbumPlaybackQueue | null {
  const queue = get(albumPlayback);
  if (!queue) return null;
  return queue.audiobookIds.includes(audiobookId) ? queue : null;
}

/**
 * Si el audio actual es el último de la cola y terminó, arranca el siguiente.
 * Se llama después de cada sondeo del reproductor.
 */
export async function advanceAlbumPlayback(
  snapshot: PlaybackSnapshot | null,
): Promise<PlaybackSnapshot | null> {
  const queue = get(albumPlayback);
  if (!queue || advancing || !snapshot) return snapshot;
  if (snapshot.playing || snapshot.durationMs <= 0) return snapshot;
  if (snapshot.positionMs < snapshot.durationMs) return snapshot;

  const currentId = queue.audiobookIds[queue.index];
  if (snapshot.audiobookId !== currentId) {
    // El usuario cambió de audiolibro manualmente: la cola ya no aplica.
    albumPlayback.set(null);
    return snapshot;
  }

  if (queue.index + 1 >= queue.audiobookIds.length) {
    albumPlayback.set(null);
    return snapshot;
  }

  advancing = true;
  try {
    const nextIndex = queue.index + 1;
    const nextId = queue.audiobookIds[nextIndex];
    albumPlayback.set({ ...queue, index: nextIndex });
    return await playAudiobook(nextId);
  } catch {
    albumPlayback.set(null);
    return snapshot;
  } finally {
    advancing = false;
  }
}

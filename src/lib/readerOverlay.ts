import { writable } from "svelte/store";

/**
 * Indica que el panel del asistente del lector está abierto. La barra
 * persistente del audiolibro se oculta mientras tanto para no tapar el
 * cuadro de texto del chat en Android.
 */
export const readerOverlayOpen = writable(false);

import { writable } from "svelte/store";

/**
 * Visibilidad del reproductor de audiolibros minimizado.
 *
 * Se comparte entre AppShell (reproductor global) y la vista de Audiolibros
 * para que ocultar el reproductor en una pestaña no haga reaparecer la barra
 * al cambiar de vista. La reproducción nunca se detiene al ocultarlo.
 */
export const audiobookPlayerHidden = writable(false);

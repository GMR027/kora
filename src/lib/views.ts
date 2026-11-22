// Carga diferida de las vistas principales. Sin esto, Vite empaqueta el
// lector, PDF, audiolibros, Quill y el resto de pantallas en el bundle de
// arranque, aunque el usuario solo vaya a abrir la biblioteca.
const loaders = {
  library: () => import("../views/Library.svelte"),
  reader: () => import("../views/Reader.svelte"),
  audiobooks: () => import("../views/Audiobooks.svelte"),
  notes: () => import("../views/Notes.svelte"),
  diary: () => import("../views/Diary.svelte"),
  voice: () => import("../views/Voice.svelte"),
  account: () => import("../views/Account.svelte"),
  sync: () => import("../views/Sync.svelte"),
  settings: () => import("../views/Settings.svelte"),
};

export type ViewKey = keyof typeof loaders;
type ModuleFor<K extends ViewKey> = Awaited<ReturnType<(typeof loaders)[K]>>;

const cache = new Map<ViewKey, Promise<unknown>>();

export function loadView<K extends ViewKey>(key: K): Promise<ModuleFor<K>> {
  let promise = cache.get(key);
  if (!promise) {
    promise = loaders[key]();
    cache.set(key, promise);
  }
  return promise as Promise<ModuleFor<K>>;
}

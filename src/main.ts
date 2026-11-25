import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

// pdf.js usa Promise.withResolvers; algunos WebView antiguos no lo traen.
if (typeof (Promise as unknown as { withResolvers?: unknown }).withResolvers !== "function") {
  (Promise as unknown as {
    withResolvers: <T>() => {
      promise: Promise<T>;
      resolve: (value: T | PromiseLike<T>) => void;
      reject: (reason?: unknown) => void;
    };
  }).withResolvers = <T>() => {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve, reject };
  };
}

const target = document.getElementById("app");
if (!target) {
  throw new Error("No se encontró el contenedor #app");
}

// En Android nativo MainActivity aplica los insets del sistema como padding
// sobre la vista. Marcamos el documento para que el CSS no vuelva a reservar
// las mismas zonas con env(safe-area-inset-*).
if (
  "__TAURI_INTERNALS__" in window &&
  /Android/i.test(navigator.userAgent)
) {
  document.documentElement.dataset.platform = "android";
}

async function boot() {
  // `pnpm dev` en un navegador normal no tiene el IPC de Tauri. En ese caso
  // se instala un backend de demostración con datos realistas para poder
  // recorrer la biblioteca y el lector sin la aplicación nativa.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installDemoBackend } = await import("./lib/demo");
    installDemoBackend();
  }
  mount(App, { target: target! });
}

void boot();

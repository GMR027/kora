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

mount(App, { target });

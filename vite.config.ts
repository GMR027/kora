import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],

  // Tauri espera un puerto fijo y no debe limpiar la consola.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || "0.0.0.0",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**", "**/dist/**", "**/artifacts/**"],
    },
  },

  envPrefix: ["VITE_", "TAURI_ENV_*"],

  build: {
    // Linux usa WebKitGTK moderno y Android un WebView Chromium actualizado;
    // es2022 evita que esbuild tenga que transpilar estructuras de Svelte 5.
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "es2022",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    chunkSizeWarningLimit: 900,
  },
});

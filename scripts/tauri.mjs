#!/usr/bin/env node
// Wrapper para pnpm: en algunas instalaciones pnpm no enlaza el paquete
// nativo opcional de Tauri y el CLI termina en un error de carga. Buscamos el
// binding de la plataforma y lo cargamos explícitamente antes de arrancar.
import { createRequire } from "node:module";
import { existsSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function libc() {
  if (process.platform !== "linux") return "";
  try {
    const report = process.report?.getReport?.();
    if (report?.header?.glibcVersionRuntime) return "-gnu";
    if (JSON.stringify(report?.sharedObjects ?? "").includes("musl")) return "-musl";
  } catch {
    // seguimos con la heurística de ldd
  }
  return "-gnu";
}

function candidateNames() {
  const arch = process.arch;
  const platform = process.platform;
  if (platform === "linux") {
    return [`@tauri-apps/cli-linux-${arch}${libc()}`];
  }
  if (platform === "darwin") {
    return [`@tauri-apps/cli-darwin-${arch}`];
  }
  if (platform === "win32") {
    return [
      `@tauri-apps/cli-win32-${arch}-msvc`,
      `@tauri-apps/cli-win32-${arch}-gnu`,
    ];
  }
  return [];
}

function findBinding() {
  for (const name of candidateNames()) {
    try {
      const packageJson = require.resolve(`${name}/package.json`);
      const dir = dirname(packageJson);
      const file = readdirSync(dir).find((entry) => entry.endsWith(".node"));
      if (file) return join(dir, file);
    } catch {
      // pnpm puede no haber enlazado el opcional; buscamos en .pnpm.
    }
    const pnpmStore = join(root, "node_modules", ".pnpm");
    if (existsSync(pnpmStore)) {
      const prefix = name.replace("@tauri-apps/", "@tauri-apps+");
      for (const entry of readdirSync(pnpmStore)) {
        if (!entry.startsWith(prefix)) continue;
        const dir = join(pnpmStore, entry, "node_modules", "@tauri-apps", name.split("/")[1]);
        if (!existsSync(dir)) continue;
        const file = readdirSync(dir).find((candidate) => candidate.endsWith(".node"));
        if (file) return join(dir, file);
      }
    }
  }
  return null;
}

const binding = findBinding();
if (binding) {
  process.env.NAPI_RS_NATIVE_LIBRARY_PATH = binding;
}

let cli;
try {
  cli = require("@tauri-apps/cli");
} finally {
  // La variable solo sirve para que el CLI de Tauri encuentre su binding.
  // Si se hereda a `beforeBuildCommand`, rolldown también intenta cargar ese
  // binario y falla con «TsconfigCache is not a constructor».
  delete process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
}

cli.run(process.argv.slice(2), "tauri").catch((error) => {
  console.error(error?.message ?? error);
  process.exit(1);
});

#!/usr/bin/env node
// Copia el helper de clonación a src-tauri/binaries con el sufijo de target
// que Tauri exige para los externalBin de Linux.
//
// Por defecto copia el binario de debug (para `pnpm tauri dev`); con
// `--release` copia el de release (para empaquetar).
import { chmodSync, copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const release = process.argv.includes("--release");
const profile = release ? "release" : "debug";
const arch = process.env.CARGO_BUILD_TARGET?.split("-")[0] ?? process.arch;

const triples = {
  arm64: "aarch64-unknown-linux-gnu",
  aarch64: "aarch64-unknown-linux-gnu",
  x64: "x86_64-unknown-linux-gnu",
  x86_64: "x86_64-unknown-linux-gnu",
};

const triple = triples[arch];
if (!triple) {
  console.error(`Arquitectura no soportada para el helper: ${arch}`);
  process.exit(1);
}

const source = join(root, "target", profile, "kora-neutts");
const destinationDir = join(root, "src-tauri", "binaries");
const destination = join(destinationDir, `kora-neutts-${triple}`);

if (!existsSync(source)) {
  console.error(
    `No existe ${source}. Ejecuta antes: cargo build ${release ? "--release " : ""}-p kora-neutts`,
  );
  process.exit(1);
}

mkdirSync(destinationDir, { recursive: true });
copyFileSync(source, destination);
chmodSync(destination, 0o755);
console.log(`Helper (${profile}) copiado a ${destination}`);

#!/usr/bin/env node
/**
 * Puente mínimo entre la API key de `.env` y el servidor MCP de Google Stitch.
 *
 * Uso:
 *   node scripts/stitch.mjs list
 *   node scripts/stitch.mjs list-tools
 *   node scripts/stitch.mjs call create_project '{"title":"Kora"}'
 *   node scripts/stitch.mjs call generate_screen_from_text '{"projectId":"...","prompt":"..."}'
 *
 * No imprime la clave. Si `.env` no existe, también acepta STITCH_API_KEY
 * desde el entorno del proceso.
 */
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const ENDPOINT = "https://stitch.googleapis.com/mcp";
const PROTOCOL = "2024-11-05";

function loadEnv(file = ".env") {
  const full = path.resolve(process.cwd(), file);
  if (!fs.existsSync(full)) return {};
  const out = {};
  for (const raw of fs.readFileSync(full, "utf8").split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq === -1) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    out[key] = value;
  }
  return out;
}

const fileEnv = loadEnv();
const apiKey = process.env.STITCH_API_KEY || fileEnv.STITCH_API_KEY || "";

if (!apiKey) {
  console.error(
    "Falta STITCH_API_KEY. Añádela a .env o expórtala en el entorno.",
  );
  process.exit(1);
}

let sessionId = null;

async function rpc(body, { timeoutMs = 10 * 60 * 1000, notify = false } = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const headers = {
    "Content-Type": "application/json",
    Accept: "application/json, text/event-stream",
    "X-Goog-Api-Key": apiKey,
  };
  if (sessionId) headers["Mcp-Session-Id"] = sessionId;

  try {
    const res = await fetch(ENDPOINT, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
      signal: controller.signal,
    });

    const sid = res.headers.get("mcp-session-id");
    if (sid) sessionId = sid;

    const text = await res.text();
    if (!res.ok) {
      throw new Error(`HTTP ${res.status}: ${text.slice(0, 800)}`);
    }
    if (notify || !text.trim()) return null;

    const type = res.headers.get("content-type") || "";
    if (type.includes("text/event-stream")) {
      for (const line of text.split(/\r?\n/)) {
        if (!line.startsWith("data:")) continue;
        const payload = line.slice(5).trim();
        if (!payload || payload === "[DONE]") continue;
        try {
          const msg = JSON.parse(payload);
          if (msg.id === body.id) {
            if (msg.error) throw new Error(JSON.stringify(msg.error));
            return msg.result;
          }
        } catch (err) {
          if (err instanceof SyntaxError) continue;
          throw err;
        }
      }
      throw new Error(`Respuesta SSE sin resultado: ${text.slice(0, 500)}`);
    }

    const msg = JSON.parse(text);
    if (msg.error) throw new Error(JSON.stringify(msg.error));
    return msg.result;
  } finally {
    clearTimeout(timer);
  }
}

async function connect() {
  const init = await rpc({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      protocolVersion: PROTOCOL,
      capabilities: {},
      clientInfo: { name: "kora-stitch-bridge", version: "1.0.0" },
    },
  });
  await rpc(
    { jsonrpc: "2.0", method: "notifications/initialized", params: {} },
    { notify: true },
  );
  return init;
}

function printResult(result) {
  if (result?.content?.length) {
    for (const item of result.content) {
      if (item.type === "text") console.log(item.text);
      else console.log(JSON.stringify(item, null, 2));
    }
    if (result.isError) process.exitCode = 2;
    return;
  }
  console.log(JSON.stringify(result, null, 2));
}

const [command, ...rest] = process.argv.slice(2);

try {
  if (!command || command === "help") {
    console.log(`Uso:
  node scripts/stitch.mjs list                 Lista los proyectos de Stitch
  node scripts/stitch.mjs list-tools           Lista las herramientas del MCP
  node scripts/stitch.mjs call <tool> [json]   Ejecuta una herramienta
`);
    process.exit(0);
  }

  await connect();

  if (command === "list") {
    printResult(await rpc({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "list_projects", arguments: {} } }));
  } else if (command === "list-tools") {
    const result = await rpc({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} });
    for (const tool of result?.tools || []) {
      console.log(`- ${tool.name}: ${(tool.description || "").split("\n")[0]}`);
    }
  } else if (command === "call") {
    const name = rest[0];
    if (!name) throw new Error("Falta el nombre de la herramienta.");
    let args = {};
    if (rest[1]) args = JSON.parse(rest.slice(1).join(" "));
    const result = await rpc(
      {
        jsonrpc: "2.0",
        id: 2,
        method: "tools/call",
        params: { name, arguments: args },
      },
      { timeoutMs: 15 * 60 * 1000 },
    );
    printResult(result);
  } else {
    throw new Error(`Comando desconocido: ${command}`);
  }
} catch (err) {
  console.error(`Error de Stitch: ${err.message}`);
  process.exit(1);
}

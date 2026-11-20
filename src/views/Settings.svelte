<script lang="ts">
  import { onMount } from "svelte";
  import StatusCard from "../lib/components/StatusCard.svelte";
  import {
    accountStatus,
    errorMessage,
    featuresStatus,
    storageLayout,
    syncStatus,
    ttsEngines,
  } from "../lib/api";
  import type {
    AccountStatus,
    EngineInfo,
    FeaturesStatus,
    StorageLayout,
    SyncStatus,
  } from "../lib/types";

  let layout = $state<StorageLayout | null>(null);
  let engines = $state<EngineInfo[]>([]);
  let sync = $state<SyncStatus | null>(null);
  let account = $state<AccountStatus | null>(null);
  let features = $state<FeaturesStatus | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      [layout, engines, sync, account, features] = await Promise.all([
        storageLayout(),
        ttsEngines(),
        syncStatus(),
        accountStatus(),
        featuresStatus(),
      ]);
    } catch (cause) {
      error = errorMessage(cause);
    }
  });
</script>

<section class="settings">
  <header>
    <h1>Ajustes y diagnóstico</h1>
    <p class="subtitle">
      Este panel solo lee el estado del esqueleto; todavía no hay preferencias editables.
    </p>
  </header>

  {#if error}
    <p class="notice" role="status">{error}</p>
  {/if}

  <h2>Almacenamiento privado</h2>
  {#if layout}
    <div class="grid">
      <StatusCard title="Raíz" value={layout.root} hint="BaseDirectory::AppLocalData" />
      <StatusCard title="Base de datos" value={layout.db} hint="SQLite + WAL" />
      <StatusCard title="Assets" value={layout.assets} hint="Libros y voces" />
      <StatusCard title="Exportaciones" value={layout.exports} hint="Salidas de usuario" />
      <StatusCard title="Papelera" value={layout.trash} hint="Borrado diferido" />
      <StatusCard title="Logs" value={layout.logs} hint="Rotación diaria" />
    </div>
  {:else}
    <p class="empty">Cargando rutas…</p>
  {/if}

  <h2>Motores de voz</h2>
  {#if engines.length === 0}
    <p class="empty">
      Sin motores registrados. Las implementaciones (Android TTS, Piper/ONNX, etc.)
      llegan en la fase de TTS.
    </p>
  {:else}
    <div class="grid">
      {#each engines as engine (engine.id)}
        <StatusCard
          title={engine.id}
          value={engine.name}
          hint={`offline: ${engine.offline} · clonación: ${engine.voiceCloning}`}
        />
      {/each}
    </div>
  {/if}

  <h2>Sincronización y cuenta</h2>
  <div class="grid">
    <StatusCard
      title="Sync"
      value={sync?.running ? "Activo" : "Detenido"}
      hint={sync ? `${sync.peerCount} peers · mDNS/QUIC/CRDT` : "Sin estado"}
      tone={sync?.running ? "ok" : "neutral"}
    />
    <StatusCard
      title="Cuenta"
      value={account?.signedIn ? (account.displayName ?? "Conectada") : "Local"}
      hint={account ? `${account.deviceCount} dispositivos vinculados` : "Sin estado"}
    />
    <StatusCard
      title="OCR"
      value={features?.ocr ? "Disponible" : "Pendiente"}
      hint="Fase de features"
    />
    <StatusCard
      title="Embeddings"
      value={features?.embeddings ? "Disponible" : "Pendiente"}
      hint="Búsqueda semántica"
    />
    <StatusCard
      title="Diario"
      value={features?.journal ? "Disponible" : "Pendiente"}
      hint="Notas y reflexiones"
    />
    <StatusCard
      title="Logros"
      value={features?.achievements ? "Disponible" : "Pendiente"}
      hint="Progreso gamificado"
    />
  </div>
</section>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
  }

  h2 {
    margin: 1rem 0 0;
    font-size: 1rem;
    font-weight: 600;
  }

  .subtitle {
    margin: 0.25rem 0 0;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 0.8rem;
  }

  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .notice {
    margin: 0;
    padding: 0.7rem 0.9rem;
    border-radius: 10px;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
    font-size: 0.85rem;
  }
</style>

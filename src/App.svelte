<script lang="ts">
  import { onMount } from "svelte";
  import Library from "./views/Library.svelte";
  import Reader from "./views/Reader.svelte";
  import Settings from "./views/Settings.svelte";
  import { appInfo, errorMessage } from "./lib/api";
  import type { AppInfo } from "./lib/types";

  type Tab = "library" | "reader" | "settings";

  const tabs: { id: Tab; label: string }[] = [
    { id: "library", label: "Biblioteca" },
    { id: "reader", label: "Lector" },
    { id: "settings", label: "Ajustes" },
  ];

  let tab = $state<Tab>("library");
  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);
  let readerBookId = $state<string | null>(null);

  function openReader(id: string) {
    readerBookId = id;
    tab = "reader";
  }

  onMount(async () => {
    try {
      info = await appInfo();
    } catch (cause) {
      error = errorMessage(cause);
    }
  });
</script>

<div class="shell">
  <header class="topbar">
    <div class="brand">
      <span class="mark" aria-hidden="true">K</span>
      <div>
        <p class="name">Kora</p>
        <p class="tagline">Lectura local-first con voz y sincronización LAN</p>
      </div>
    </div>

    <nav aria-label="Secciones">
      {#each tabs as item (item.id)}
        <button
          class:active={tab === item.id}
          aria-current={tab === item.id ? "page" : undefined}
          onclick={() => (tab = item.id)}
        >
          {item.label}
        </button>
      {/each}
    </nav>
  </header>

  {#if error}
    <p class="banner" role="alert">{error}</p>
  {/if}

  <main>
    {#if tab === "library"}
      <Library onRead={openReader} />
    {:else if tab === "reader"}
      <Reader bookId={readerBookId} />
    {:else}
      <Settings />
    {/if}
  </main>

  <footer>
    <span>{info ? `${info.name} ${info.version}` : "Kora"}</span>
    <span>{info ? `${info.platform}/${info.arch} · ${info.identifier}` : "Tauri 2"}</span>
  </footer>
</div>

<style>
  .shell {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    padding: clamp(1rem, 3vw, 2.25rem);
    max-width: 1180px;
    margin: 0 auto;
  }

  .topbar {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.8rem;
  }

  .mark {
    width: 42px;
    height: 42px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: linear-gradient(150deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    font-weight: 700;
    font-size: 1.2rem;
  }

  .name {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: 0.01em;
  }

  .tagline {
    margin: 0.1rem 0 0;
    font-size: 0.78rem;
    color: var(--muted);
  }

  nav {
    display: flex;
    gap: 0.25rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 0.25rem;
  }

  nav button {
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.88rem;
    padding: 0.5rem 0.9rem;
    border-radius: 9px;
    cursor: pointer;
  }

  nav button.active {
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
  }

  nav button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  main {
    flex: 1;
  }

  .banner {
    margin: 0;
    padding: 0.75rem 0.95rem;
    border-radius: 12px;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
    font-size: 0.86rem;
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1.5rem;
    justify-content: space-between;
    padding-top: 1rem;
    border-top: 1px solid var(--border);
    color: var(--muted);
    font-size: 0.75rem;
  }
</style>

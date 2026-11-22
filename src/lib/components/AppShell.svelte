<script lang="ts">
  import type { Snippet } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import {
    getAudiobookProgress,
    libraryOverview,
    pauseAudiobook,
    playAudiobook,
    seekAudiobook,
  } from "../api";
  import { t, formatDuration, type MessageKey } from "../i18n";
  import type { AppTab } from "../navigation";
  import type { LibraryOverview, PlaybackSnapshot } from "../types";
  import Icon from "./Icon.svelte";
  import ComfortHost from "./ComfortHost.svelte";

  type IconName =
    | "library"
    | "book"
    | "headphones"
    | "diary"
    | "voice"
    | "account"
    | "sync"
    | "settings"
    | "bookmark";

  let {
    tab,
    onNavigate,
    version,
    immersive = false,
    reading = false,
    account = null,
    children,
  }: {
    tab: AppTab;
    onNavigate: (tab: AppTab) => void;
    version: string;
    immersive?: boolean;
    reading?: boolean;
    account?: { displayName: string; username: string; avatarPath?: string | null } | null;
    children: Snippet;
  } = $props();

  let moreOpen = $state(false);
  let overview = $state<LibraryOverview | null>(null);
  let playback = $state<PlaybackSnapshot | null>(null);
  let playerBusy = $state(false);
  let shelfTab = $state<AppTab>("library");
  let stackReturnTab = $state<AppTab>("library");
  let profileName = $derived(
    account?.displayName?.trim() || account?.username?.trim() || "",
  );
  let profileInitial = $derived(profileName.slice(0, 1).toUpperCase());
  let avatarUrl = $derived(
    account?.avatarPath ? convertFileSrc(account.avatarPath) : null,
  );

  const desktopItems: { id: AppTab; icon: IconName }[] = [
    { id: "library", icon: "library" },
    { id: "reader", icon: "book" },
    { id: "audiobooks", icon: "headphones" },
    { id: "notes", icon: "bookmark" },
    { id: "diary", icon: "diary" },
    { id: "voice", icon: "voice" },
    { id: "sync", icon: "sync" },
    { id: "settings", icon: "settings" },
  ];

  const mobileItems: { id: AppTab; icon: IconName }[] = [
    { id: "library", icon: "library" },
    { id: "reader", icon: "book" },
    { id: "audiobooks", icon: "headphones" },
    { id: "diary", icon: "diary" },
  ];
  const mobileMore = desktopItems.filter(
    (item) => !mobileItems.some((entry) => entry.id === item.id),
  );

  let nowBook = $derived(overview?.current ?? null);
  let inProgress = $derived(overview?.inProgress ?? 0);
  let currentCover = $derived(playback?.coverPath ?? null);
  let playerVisible = $derived(playback !== null && tab !== "audiobooks");
  let playerPercent = $derived.by(() => {
    if (!playback || playback.durationMs <= 0) return 0;
    return Math.min(
      100,
      Math.max(0, (playback.positionMs / playback.durationMs) * 100),
    );
  });

  async function loadShelf() {
    try {
      overview = await libraryOverview();
    } catch {
      overview = null;
    }
  }

  async function loadPlayback() {
    try {
      playback = await getAudiobookProgress();
    } catch {
      playback = null;
    }
  }

  async function toggleMobilePlayback() {
    if (!playback || playerBusy) return;
    playerBusy = true;
    try {
      playback = playback.playing
        ? await pauseAudiobook()
        : await playAudiobook(playback.audiobookId);
    } catch {
      // Si el audio ya no está disponible, se oculta al siguiente sondeo.
    } finally {
      playerBusy = false;
    }
  }

  async function rewindMobilePlayback() {
    if (!playback || playerBusy) return;
    playerBusy = true;
    try {
      playback =
        (await seekAudiobook(Math.max(0, playback.positionMs - 15_000))) ??
        playback;
    } catch {
      // Sin cambios si el reproductor no responde.
    } finally {
      playerBusy = false;
    }
  }

  onMount(() => {
    void loadShelf();
    void loadPlayback();

    const refreshShelf = () => void loadShelf();
    window.addEventListener("kora:library-changed", refreshShelf);
    return () => window.removeEventListener("kora:library-changed", refreshShelf);
  });

  // Solo se sondea mientras hay audio reproduciéndose. En pausa o en reposo
  // la UI se actualiza con la respuesta de play/pause/seek.
  $effect(() => {
    if (!playback?.playing) return;
    const timer = setTimeout(() => void loadPlayback(), 1500);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    if (tab === shelfTab) return;
    const previous = shelfTab;
    shelfTab = tab;
    // El progreso cambia al leer o escuchar; en el resto de pestañas no hace
    // falta volver a cargar toda la estantería.
    if (previous === "reader" || previous === "audiobooks") {
      void loadShelf();
      void loadPlayback();
    }
  });

  function isStack(id: AppTab) {
    return id === "settings" || id === "voice" || id === "sync" || id === "account";
  }

  function go(id: AppTab) {
    moreOpen = false;
    if (isStack(id) && !isStack(tab)) {
      stackReturnTab = tab;
    }
    onNavigate(id);
  }

  function closeStack() {
    go(stackReturnTab);
  }

  function label(id: AppTab) {
    return $t(`nav.${id}` as MessageKey);
  }

  function convertCover(path: string) {
    return convertFileSrc(path);
  }

  function mobileLabel(id: AppTab) {
    return id === "diary"
      ? $t("nav.diaryShort")
      : $t(`nav.${id}` as MessageKey);
  }

  function openNow() {
    go(nowBook ? "reader" : "library");
  }
</script>

<ComfortHost />

<a class="skip-link" href="#main-content">{$t("nav.skipToContent")}</a>
<div
  class="app-shell"
  class:immersive
  class:reading
  class:stack={isStack(tab)}
  class:has-mobile-player={playerVisible}
>
  <aside class="sidebar" aria-label={$t("nav.primary")}>
    <div class="sidebar-brand">
      <div class="brand-lockup">
        <img class="brand-logo" src="/logo.png" alt="" />
        <span class="brand-name">Kora</span>
      </div>
      <span class="brand-pill">{$t("app.localBadge")}</span>
    </div>
    <nav class="side-nav">
      {#each desktopItems as item (item.id)}
        <button
          class:active={tab === item.id}
          aria-current={tab === item.id ? "page" : undefined}
          onclick={() => go(item.id)}
        >
          <span class="nav-icon"><Icon name={item.icon} size={20} /></span>
          <span>{label(item.id)}</span>
        </button>
      {/each}
    </nav>

    <div class="sidebar-foot">
      <button class="profile-card" onclick={() => go("account")}>
        <span class="profile-avatar" aria-hidden="true">
          {#if avatarUrl}
            <img src={avatarUrl} alt="" />
          {:else if profileInitial}
            {profileInitial}
          {:else}
            <Icon name="account" size={18} />
          {/if}
        </span>
        <span class="profile-copy">
          <strong>{account?.displayName ?? $t("nav.account")}</strong>
          <span>
            <i aria-hidden="true"></i>
            {account ? `@${account.username}` : $t("app.localFirst")}
          </span>
        </span>
        <span class="profile-chevron" aria-hidden="true">›</span>
      </button>
      <span class="sidebar-version">Kora {version}</span>
    </div>
  </aside>

  {#if isStack(tab)}
    <div class="mobile-stack-bar">
      <button onclick={closeStack} aria-label={$t("common.back")} title={$t("common.back")}>
        <span aria-hidden="true">←</span>
      </button>
      <strong>{label(tab)}</strong>
      <img class="mobile-stack-mark" src="/logo.png" alt="" aria-hidden="true" />
    </div>
  {/if}

  <div class="content">
    <header class="topbar">
      <div class="topbar-brand">
        <img src="/logo.png" alt="" />
        <div>
          <span>KORA READER</span>
          <strong>{label(tab).split(" ")[0]}</strong>
        </div>
      </div>

      {#if playerVisible && playback}
        <div class="topbar-player" role="group" aria-label={$t("audiobooks.nowPlaying")}>
          <button
            class="mini-cover"
            type="button"
            onclick={() => go("audiobooks")}
            aria-label={$t("nav.audiobooks")}
          >
            {#if currentCover}
              <img src={convertCover(currentCover)} alt="" />
            {:else}
              <Icon name="headphones" size={17} />
            {/if}
          </button>
          <button
            class="mini-copy"
            type="button"
            onclick={() => go("audiobooks")}
            title={$t("audiobooks.nowPlaying")}
          >
            <span class="mini-line">
              <strong>{playback.title}</strong>
              <small>{playback.chapterIndex + 1} · {playback.chapterTitle}</small>
            </span>
            <span class="mini-progress" aria-hidden="true">
              <i style={`width:${playerPercent}%`}></i>
            </span>
            <span class="mini-time">
              {formatDuration(playback.positionMs)} / {formatDuration(playback.durationMs)}
            </span>
          </button>
          <button
            class="mini-button"
            type="button"
            onclick={rewindMobilePlayback}
            disabled={playerBusy}
            aria-label={$t("audiobooks.back15")}
            title={$t("audiobooks.back15")}
          >
            <span aria-hidden="true">−15</span>
          </button>
          <button
            class="mini-button primary"
            type="button"
            onclick={toggleMobilePlayback}
            disabled={playerBusy}
            aria-label={playback.playing ? $t("audiobooks.pause") : $t("audiobooks.play")}
            title={playback.playing ? $t("audiobooks.pause") : $t("audiobooks.play")}
          >
            <Icon name={playback.playing ? "pause" : "play"} size={15} />
          </button>
          <button
            class="mini-button open"
            type="button"
            onclick={() => go("audiobooks")}
            aria-label={$t("audiobooks.openPlayer")}
            title={$t("audiobooks.openPlayer")}
          >
            <Icon name="chevron" size={14} />
          </button>
        </div>
      {:else}
        <button class="now-pill" onclick={openNow} title={$t("nav.reader")}>
          <span class="wave-icon"><Icon name={nowBook ? "headphones" : "book"} size={19} /></span>
          <span class="now-copy">
            <strong>{nowBook?.title ?? $t("app.tagline")}</strong>
            <small>
              {#if nowBook}
                {nowBook.format.toUpperCase()} · {$t("library.progressRead", {
                  percent: Math.round(nowBook.progress * 100),
                })}
              {:else}
                {$t("library.noBooks")}
              {/if}
            </small>
          </span>
          <span class="now-open" aria-hidden="true">
            <Icon name="chevron" size={15} />
          </span>
        </button>
      {/if}

      <div class="topbar-actions">
        <span class="streak-chip" title={$t("library.statReading")}>
          <Icon name="flame" size={17} />
          <strong>{inProgress}</strong>
          <span>{$t("library.statReading").toLowerCase()}</span>
        </span>
        <button
          class="topbar-icon"
          aria-label={$t("library.search")}
          onclick={() => {
            go("library");
            window.setTimeout(
              () =>
                document
                  .querySelector<HTMLInputElement>("main input[type='search']")
                  ?.focus(),
              120,
            );
          }}
        >
          <Icon name="search" size={20} />
        </button>
        <button
          class="topbar-icon sync-shortcut"
          aria-label={$t("nav.sync")}
          onclick={() => go("sync")}
        >
          <Icon name="sync" size={19} />
        </button>
        <button
          class="topbar-account"
          onclick={() => go("account")}
          aria-label={account ? `${$t("nav.account")}: ${account.displayName}` : $t("nav.account")}
          title={account ? `${account.displayName} · @${account.username}` : $t("nav.account")}
        >
          <span class="topbar-avatar" aria-hidden="true">
            {#if avatarUrl}
              <img src={avatarUrl} alt="" />
            {:else if profileInitial}
              {profileInitial}
            {:else}
              <Icon name="account" size={17} />
            {/if}
          </span>
          <span class="topbar-account-name">
            {profileName || $t("nav.account")}
          </span>
        </button>
      </div>
    </header>

    <main id="main-content">
      {@render children()}
    </main>
  </div>

  <nav class="bottom-nav" aria-label={$t("nav.primary")}>
    {#each mobileItems as item (item.id)}
      <button
        class:active={tab === item.id}
        aria-current={tab === item.id ? "page" : undefined}
        onclick={() => go(item.id)}
      >
        <Icon name={item.icon} />
        <span>{mobileLabel(item.id)}</span>
      </button>
    {/each}
    <button
      class:active={tab === "settings" || tab === "account" || tab === "voice" || tab === "sync" || tab === "notes"}
      aria-expanded={moreOpen}
      onclick={() => (moreOpen = !moreOpen)}
    >
      <Icon name="more" />
      <span>{$t("nav.more")}</span>
    </button>
  </nav>

  {#if playerVisible && playback}
    <div class="mobile-player" aria-label={$t("audiobooks.nowPlaying")}>
      <button
        class="mobile-player-cover"
        onclick={() => go("audiobooks")}
        aria-label={$t("nav.audiobooks")}
      >
        {#if currentCover}
          <img src={convertCover(currentCover)} alt="" />
        {:else}
          <Icon name="headphones" size={20} />
        {/if}
      </button>
      <button class="mobile-player-copy" onclick={() => go("audiobooks")}>
        <strong>{playback.title}</strong>
        <span>{playback.chapterIndex + 1} · {playback.chapterTitle}</span>
        <small>
          {formatDuration(playback.positionMs)} / {formatDuration(playback.durationMs)}
        </small>
      </button>
      <span class="mobile-wave" class:playing={playback.playing} aria-hidden="true">
        <i></i><i></i><i></i><i></i>
      </span>
      <button
        class="mobile-player-skip"
        onclick={rewindMobilePlayback}
        disabled={playerBusy}
        aria-label={$t("audiobooks.back15")}
      >
        <span aria-hidden="true">−15</span>
      </button>
      <button
        class="mobile-player-toggle"
        onclick={toggleMobilePlayback}
        disabled={playerBusy}
        aria-label={playback.playing ? $t("audiobooks.pause") : $t("audiobooks.play")}
      >
        <Icon name={playback.playing ? "pause" : "play"} size={17} />
      </button>
    </div>
  {/if}

  {#if moreOpen}
    <button
      class="sheet-backdrop"
      aria-label={$t("common.close")}
      onclick={() => (moreOpen = false)}
    ></button>
    <div class="more-sheet" in:fly={{ y: 20, duration: 180 }}>
      <strong>{$t("nav.more")}</strong>
      <div class="more-grid">
        {#each mobileMore as item (item.id)}
          <button
            class:active={tab === item.id}
            aria-current={tab === item.id ? "page" : undefined}
            onclick={() => go(item.id)}
          >
            <Icon name={item.icon} />
            <span>{label(item.id)}</span>
          </button>
        {/each}
        <button
          class:active={tab === "account"}
          aria-current={tab === "account" ? "page" : undefined}
          onclick={() => go("account")}
        >
          <Icon name="account" />
          <span>{$t("nav.account")}</span>
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .skip-link {
    position: fixed;
    top: 0.5rem;
    left: 0.5rem;
    z-index: 100;
    transform: translateY(-160%);
    padding: 0.7rem 1rem;
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-contrast);
    font: inherit;
    font-weight: 650;
    text-decoration: none;
  }

  .skip-link:focus {
    transform: translateY(0);
  }

  .app-shell {
    min-height: 100vh;
  }

  .app-shell.immersive {
    height: 100vh;
    overflow: hidden;
  }

  .app-shell.immersive .sidebar,
  .app-shell.immersive .topbar,
  .app-shell.immersive .topbar-brand,
  .app-shell.immersive .bottom-nav,
  .app-shell.immersive .more-sheet,
  .app-shell.immersive .sheet-backdrop {
    display: none !important;
  }

  .app-shell.immersive .content {
    margin-left: 0;
    height: 100vh;
    overflow: hidden;
  }

  .app-shell.immersive main {
    max-width: none;
    height: 100vh;
    padding: 0;
    margin: 0;
  }

  .app-shell.immersive :global(.screen) {
    height: 100%;
    min-height: 0;
  }

  .sidebar {
    position: fixed;
    inset: 0 auto 0 0;
    z-index: 50;
    width: var(--sidebar-width);
    display: flex;
    flex-direction: column;
    padding: 16px 16px 14px;
    border-right: 1px solid color-mix(in srgb, var(--border-warm) 34%, var(--border));
    background: var(--surface-2);
    box-shadow: 0 1px 8px rgba(0, 0, 0, 0.03);
  }

  .sidebar-brand {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.25rem 0.35rem;
  }

  .brand-lockup {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    min-width: 0;
  }

  .brand-logo {
    width: 36px;
    height: 36px;
    flex: 0 0 auto;
    border-radius: 10px;
    object-fit: cover;
    box-shadow: var(--shadow-sm);
  }

  .brand-name {
    overflow: hidden;
    font-family: var(--font-sans);
    font-size: 1.25rem;
    font-weight: 600;
    letter-spacing: -0.035em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .brand-pill {
    flex: 0 0 auto;
    padding: 0.18rem 0.48rem;
    border-radius: 999px;
    background: #ffdcc3;
    color: #6e3900;
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .side-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 1.55rem;
  }

  .side-nav button {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    min-height: 40px;
    padding: 0.5rem 0.65rem;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    text-align: left;
    transition:
      background-color 150ms ease,
      color 150ms ease,
      transform 150ms ease;
  }

  .side-nav button:hover {
    background: var(--surface-4);
    color: var(--text);
  }

  .nav-icon {
    display: inline-flex;
    color: color-mix(in srgb, var(--muted) 88%, var(--text));
  }

  .side-nav button.active {
    background: linear-gradient(
      100deg,
      var(--accent),
      var(--accent-2) 58%,
      var(--accent-3)
    );
    color: var(--accent-contrast);
    box-shadow:
      0 6px 16px -10px color-mix(in srgb, var(--accent) 55%, transparent),
      inset 0 1px 0 color-mix(in srgb, #fff 18%, transparent);
  }

  .side-nav button.active .nav-icon {
    color: currentColor;
  }

  .sidebar-foot {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    margin-top: auto;
    padding-top: 0.9rem;
  }

  .profile-card {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    width: 100%;
    min-height: 58px;
    padding: 0.5rem;
    border: 0;
    border-radius: 12px;
    background: var(--surface-3);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .profile-card:hover {
    background: var(--surface-4);
  }

  .profile-avatar {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    overflow: hidden;
    border-radius: 50%;
    background: linear-gradient(145deg, var(--accent-2), var(--accent-3));
    color: #fff;
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 600;
    box-shadow: 0 4px 10px -5px color-mix(in srgb, var(--accent) 55%, transparent);
  }

  .profile-avatar img,
  .topbar-avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .profile-copy {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    flex: 1;
    min-width: 0;
  }

  .profile-copy strong {
    overflow: hidden;
    font-size: 0.75rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-copy span {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.625rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-copy i {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--accent-2);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 14%, transparent);
  }

  .profile-chevron {
    color: var(--muted);
    font-size: 1.2rem;
    line-height: 1;
  }

  .sidebar-version {
    padding-left: 0.55rem;
    color: color-mix(in srgb, var(--muted) 65%, transparent);
    font-size: 0.5625rem;
    font-weight: 500;
    letter-spacing: 0.04em;
  }

  .content {
    min-width: 0;
    min-height: 100vh;
    margin-left: var(--sidebar-width);
    padding-top: var(--topbar-height);
  }

  .topbar {
    position: fixed;
    top: 0;
    left: var(--sidebar-width);
    right: 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    height: var(--topbar-height);
    padding: 0 24px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    /* Sin backdrop-filter: en WebKitGTK obliga a desenfocar el contenido en
       cada frame de scroll. Un fondo casi opaco da el mismo efecto visual. */
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    box-shadow: 0 1px 8px rgba(49, 24, 12, 0.035);
  }

  .topbar-brand {
    display: none;
  }

  .now-pill {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    max-width: 420px;
    min-height: 42px;
    padding: 0.25rem 0.4rem 0.25rem 0.55rem;
    border: 0;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .now-pill:hover {
    background: var(--surface-4);
  }

  .topbar-player {
    display: flex;
    align-items: center;
    gap: 8px;
    width: min(470px, 46vw);
    min-height: 48px;
    padding: 4px 5px 4px 6px;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    box-shadow:
      0 10px 28px -24px var(--shadow-color),
      inset 0 1px 0 color-mix(in srgb, #fff 18%, transparent);
  }

  .mini-cover {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 36px;
    height: 40px;
    padding: 0;
    overflow: hidden;
    border: 0;
    border-radius: 8px;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
  }

  .mini-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .mini-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .mini-line {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .mini-line strong {
    overflow: hidden;
    flex: 0 1 auto;
    font-size: 0.73rem;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mini-line small {
    overflow: hidden;
    flex: 1 1 auto;
    color: var(--muted);
    font-size: 0.6rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mini-progress {
    width: 100%;
    height: 3px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .mini-progress i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--gold), var(--accent));
    transition: width 180ms linear;
  }

  .mini-time {
    color: var(--muted);
    font-size: 0.57rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .mini-button {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 31px;
    min-width: 31px;
    height: 31px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.58rem;
    font-weight: 750;
  }

  .mini-button:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text);
  }

  .mini-button.primary {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .mini-button.primary:hover:not(:disabled) {
    background: var(--accent-2);
    color: var(--accent-contrast);
  }

  .mini-button.open {
    color: var(--muted);
  }

  .mini-button:disabled {
    opacity: 0.45;
  }

  .wave-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
    color: var(--accent);
  }

  .now-copy {
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
    flex: 1;
    min-width: 0;
  }

  .now-copy strong {
    overflow: hidden;
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .now-copy small {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.6875rem;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .now-open {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--surface);
    color: var(--muted);
    box-shadow: var(--shadow-xs);
  }

  .topbar-actions {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex: 0 0 auto;
  }

  .streak-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-height: 34px;
    padding: 0.3rem 0.62rem;
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.6875rem;
    white-space: nowrap;
  }

  .streak-chip :global(svg) {
    color: var(--accent-2);
  }

  .streak-chip strong {
    color: var(--accent);
    font-size: 0.75rem;
    font-weight: 700;
  }

  .topbar-icon {
    display: grid;
    place-items: center;
    width: 38px;
    min-width: 38px;
    height: 38px;
    min-height: 38px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
  }

  .topbar-icon:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .topbar-avatar {
    display: grid;
    place-items: center;
    width: 34px;
    min-width: 34px;
    height: 34px;
    overflow: hidden;
    margin-left: 0.2rem;
    padding: 0;
    border: 2px solid var(--surface);
    border-radius: 50%;
    background: linear-gradient(145deg, var(--accent-2), var(--accent-3));
    color: #fff;
    font-family: var(--font-display);
    font-size: 0.95rem;
    font-weight: 600;
    box-shadow: var(--shadow-sm);
  }

  .topbar-account {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    min-height: 38px;
    padding: 0 0.45rem 0 0.15rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--text);
    font: inherit;
  }

  .topbar-account:hover {
    background: var(--surface-3);
  }

  .topbar-account-name {
    max-width: 140px;
    overflow: hidden;
    font-size: 0.75rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  main {
    width: 100%;
    max-width: 1360px;
    margin: 0 auto;
    padding: 24px 40px 64px;
  }

  .app-shell.reading main {
    max-width: none;
    padding: 0;
  }

  .bottom-nav,
  .more-sheet,
  .sheet-backdrop {
    display: none;
  }

  @media (max-width: 1100px) {
    main {
      padding-right: 28px;
      padding-left: 28px;
    }

    .topbar {
      padding: 0 18px;
    }

    .topbar-player {
      width: min(310px, 35vw);
    }

    .mini-line small {
      display: none;
    }

    .streak-chip span {
      display: none;
    }

    .topbar-account-name {
      display: none;
    }
  }

  @media (max-width: 900px) {
    .sidebar {
      display: none;
    }

    .content {
      margin-left: 0;
      padding-top: calc(68px + env(safe-area-inset-top));
    }

    .topbar {
      left: 0;
      right: 0;
      top: 0;
      height: calc(68px + env(safe-area-inset-top));
      padding: env(safe-area-inset-top) 16px 0;
      background: color-mix(in srgb, var(--bg) 92%, transparent);
      border-bottom: 0;
      box-shadow: none;
    }

    .topbar-brand {
      display: flex;
      align-items: center;
      gap: 0.55rem;
      min-width: 0;
    }

    .topbar-brand img {
      width: 34px;
      height: 34px;
      border-radius: 9px;
      object-fit: cover;
    }

    .topbar-brand div {
      display: flex;
      flex-direction: column;
      min-width: 0;
    }

    .topbar-brand span {
      display: none;
    }

    .topbar-brand div::before {
      content: "Kora";
      font-family: var(--font-sans);
      font-size: 1.22rem;
      font-weight: 600;
      letter-spacing: -0.035em;
      line-height: 1.05;
    }

    .topbar-brand strong {
      overflow: hidden;
      color: var(--muted);
      font-family: var(--font-sans);
      font-size: 0.58rem;
      font-weight: 600;
      letter-spacing: 0.08em;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .now-pill,
    .topbar-player,
    .sync-shortcut {
      display: none;
    }

    .topbar-actions {
      gap: 0.15rem;
    }

    .streak-chip {
      background: var(--surface);
      box-shadow: var(--shadow-xs);
    }

    .app-shell {
      padding-bottom: calc(72px + env(safe-area-inset-bottom));
    }

    .app-shell.reading {
      padding-bottom: calc(72px + env(safe-area-inset-bottom));
    }

    .app-shell.has-mobile-player {
      padding-bottom: calc(142px + env(safe-area-inset-bottom));
    }

    main {
      padding: 16px 16px 24px;
    }

    .app-shell.reading main {
      padding: 0;
    }

    .bottom-nav {
      position: fixed;
      left: 0;
      right: 0;
      bottom: 0;
      z-index: 60;
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 0.1rem;
      padding: 0.35rem 0.35rem calc(0.35rem + env(safe-area-inset-bottom));
      border-top: 1px solid color-mix(in srgb, var(--border) 76%, transparent);
      background: color-mix(in srgb, var(--surface) 98%, transparent);
      box-shadow: 0 -10px 28px -26px var(--shadow-color);
    }

    .bottom-nav button {
      min-height: 54px;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 0.18rem;
      border: 0;
      border-radius: 10px;
      background: transparent;
      color: var(--muted);
      font: inherit;
      font-size: 0.6rem;
      font-weight: 600;
      cursor: pointer;
    }

    .bottom-nav button :global(svg) {
      color: var(--muted);
    }

    .bottom-nav button.active {
      color: var(--accent);
    }

    .bottom-nav button.active :global(svg) {
      color: var(--accent);
      filter: drop-shadow(0 3px 6px color-mix(in srgb, var(--accent-2) 28%, transparent));
    }

    .sheet-backdrop {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 70;
      border: 0;
      background: rgba(28, 16, 8, 0.35);
      backdrop-filter: blur(2px);
    }

    .more-sheet {
      display: block;
      position: fixed;
      left: 0;
      right: 0;
      bottom: calc(64px + env(safe-area-inset-bottom));
      z-index: 75;
      padding: 1rem 0.9rem calc(1rem + env(safe-area-inset-bottom));
      border-top: 1px solid var(--border);
      border-radius: 22px 22px 0 0;
      background: var(--surface);
      box-shadow: 0 -24px 60px -28px var(--shadow-color);
    }

    .more-sheet::before {
      content: "";
      display: block;
      width: 38px;
      height: 4px;
      margin: -0.25rem auto 0.8rem;
      border-radius: 999px;
      background: var(--border-strong);
    }

    .more-sheet > strong {
      display: block;
      margin: 0.2rem 0.3rem 0.65rem;
      color: var(--muted);
      font-size: 0.68rem;
      letter-spacing: 0.12em;
      text-transform: uppercase;
    }

    .more-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 0.4rem;
    }

    .more-grid button {
      min-height: 56px;
      display: flex;
      align-items: center;
      gap: 0.55rem;
      padding: 0.6rem 0.7rem;
      border: 1px solid var(--border);
      border-radius: 11px;
      background: var(--surface-2);
      color: var(--text);
      font: inherit;
      font-size: 0.82rem;
      font-weight: 600;
      cursor: pointer;
    }

    .more-grid button.active {
      border-color: var(--accent);
      background: var(--accent-soft);
      color: var(--accent);
    }
  }


  .mobile-player {
    display: none;
  }

  @media (max-width: 900px) {
    .mobile-player {
      position: fixed;
      right: 10px;
      bottom: calc(66px + env(safe-area-inset-bottom));
      left: 10px;
      z-index: 65;
      display: grid;
      grid-template-columns: 40px minmax(0, 1fr) auto auto auto;
      align-items: center;
      gap: 8px;
      padding: 7px 8px;
      border: 1px solid var(--accent-line);
      border-radius: 15px;
      background: color-mix(in srgb, var(--surface) 98%, transparent);
      box-shadow: 0 16px 38px -26px var(--shadow-color);
    }

    .mobile-player-cover {
      display: grid;
      place-items: center;
      width: 40px;
      height: 44px;
      padding: 0;
      overflow: hidden;
      border: 0;
      border-radius: 8px;
      background: var(--surface-2);
      color: var(--accent);
    }

    .mobile-player-cover img {
      width: 100%;
      height: 100%;
      object-fit: cover;
    }

    .mobile-player-copy {
      display: flex;
      flex-direction: column;
      min-width: 0;
      padding: 0;
      border: 0;
      background: transparent;
      color: var(--text);
      text-align: left;
    }

    .mobile-player-copy strong,
    .mobile-player-copy span,
    .mobile-player-copy small {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .mobile-player-copy strong {
      font-size: 0.76rem;
      font-weight: 650;
    }

    .mobile-player-copy span {
      color: var(--muted);
      font-size: 0.62rem;
    }

    .mobile-player-copy small {
      color: var(--accent);
      font-size: 0.56rem;
      font-variant-numeric: tabular-nums;
    }

    .mobile-wave {
      display: flex;
      align-items: center;
      gap: 2px;
      height: 18px;
    }

    .mobile-wave i {
      display: block;
      width: 2px;
      height: 5px;
      border-radius: 999px;
      background: var(--accent);
      opacity: 0.45;
    }

    .mobile-wave.playing i {
      animation: mobile-wave 900ms ease-in-out infinite alternate;
    }

    .mobile-wave.playing i:nth-child(2) {
      animation-delay: 160ms;
      background: var(--accent-2);
    }

    .mobile-wave.playing i:nth-child(3) {
      animation-delay: 320ms;
    }

    .mobile-wave.playing i:nth-child(4) {
      animation-delay: 480ms;
      background: var(--accent-2);
    }

    .mobile-player-skip,
    .mobile-player-toggle {
      display: grid;
      place-items: center;
      min-width: 32px;
      height: 32px;
      padding: 0;
      border: 0;
      border-radius: 50%;
      font: inherit;
    }

    .mobile-player-skip {
      background: transparent;
      color: var(--muted);
      font-size: 0.58rem;
      font-weight: 700;
    }

    .mobile-player-toggle {
      background: var(--accent);
      color: var(--accent-contrast);
    }

    .mobile-player button:disabled {
      opacity: 0.5;
    }
  }

  @keyframes mobile-wave {
    from {
      height: 4px;
    }
    to {
      height: 16px;
    }
  }


  .mobile-stack-bar {
    display: none;
  }

  @media (max-width: 900px) {
    .app-shell.stack .topbar {
      display: none;
    }

    .app-shell.stack .content {
      padding-top: calc(58px + env(safe-area-inset-top));
    }

    .app-shell.stack {
      padding-bottom: 0;
    }

    .app-shell.stack .bottom-nav {
      display: none;
    }

    .mobile-stack-bar {
      position: fixed;
      top: 0;
      right: 0;
      left: 0;
      z-index: 45;
      display: flex;
      align-items: center;
      gap: 10px;
      height: calc(58px + env(safe-area-inset-top));
      padding: env(safe-area-inset-top) 14px 0;
      border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
      background: color-mix(in srgb, var(--surface) 97%, transparent);
    }

    .mobile-stack-bar button {
      display: grid;
      place-items: center;
      width: 36px;
      height: 36px;
      padding: 0;
      border: 0;
      border-radius: 50%;
      background: var(--surface-2);
      color: var(--text);
      font-size: 1.25rem;
    }

    .mobile-stack-bar strong {
      flex: 1;
      overflow: hidden;
      font-family: var(--font-display);
      font-size: 1.18rem;
      font-weight: 500;
      letter-spacing: -0.02em;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .mobile-stack-mark {
      display: block;
      flex: 0 0 auto;
      width: 30px;
      height: 30px;
      border-radius: 9px;
      background: var(--accent);
      object-fit: cover;
      box-shadow: var(--shadow-xs);
    }

    .app-shell.stack .mobile-player {
      bottom: calc(10px + env(safe-area-inset-bottom));
    }

    .app-shell.stack.has-mobile-player {
      padding-bottom: calc(78px + env(safe-area-inset-bottom));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .more-sheet {
      animation: none;
    }

    .mini-progress i {
      transition: none;
    }
  }
</style>

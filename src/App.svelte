<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import AppShell from "./lib/components/AppShell.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import Spinner from "./lib/components/Spinner.svelte";
  import ToastHost from "./lib/components/ToastHost.svelte";
  import {
    initTtsPlayer,
    stopTtsPlayback,
    toggleTtsPlayback,
    ttsPlayback,
    ttsReaderVisible,
  } from "./lib/ttsPlayer";
  import Onboarding from "./views/Onboarding.svelte";
  import { appInfo, getAccount, errorMessage } from "./lib/api";
  import { loadView } from "./lib/views";
  import {
    appearance,
    loadAppearance,
    updateAppearance,
  } from "./lib/appearance";
  import { t, translate } from "./lib/i18n";
  import type { AppTab } from "./lib/navigation";
  import { notifyError, notifyImportant } from "./lib/toast";
  import { checkUpdatesInBackground } from "./lib/updates";
  import type {
    AccountView,
    Achievement,
    AppearanceState,
    AppInfo,
    Locator,
    PdfConversionResult,
    PdfConversionState,
  } from "./lib/types";

  let tab = $state<AppTab>("library");
  let info = $state<AppInfo | null>(null);
  let appearanceState = $state<AppearanceState | null>(null);
  let account = $state<AccountView | null>(null);
  let ready = $state(false);
  let error = $state<string | null>(null);
  let readerBookId = $state<string | null>(null);
  let readerLocator = $state<Locator | null>(null);
  let readerAutoListen = $state(false);
  let audiobookIntent = $state<{ bookId: string } | null>(null);

  $effect(() => {
    ttsReaderVisible.set(tab === "reader");
  });

  function openReader(id: string, locator: Locator | null = null) {
    readerAutoListen = false;
    readerBookId = id;
    readerLocator = locator;
    tab = "reader";
  }

  function openReaderListen(id: string, section = 0) {
    readerAutoListen = true;
    readerBookId = id;
    readerLocator = section > 0 ? { section, offset: 0 } : null;
    tab = "reader";
  }

  function openAudiobookGenerator(bookId: string) {
    audiobookIntent = { bookId };
    navigate("audiobooks");
  }

  async function boot(attempt = 0) {
    ready = false;
    error = null;
    try {
      const [currentAppearance, currentAccount] = await Promise.all([
        loadAppearance(),
        getAccount(),
      ]);
      appearanceState = currentAppearance;
      account = currentAccount;
      info = await appInfo();
      ready = true;
      if (currentAppearance.preferences.locale) {
        document.documentElement.lang = currentAppearance.preferences.locale;
      }
    } catch (cause) {
      // En Android el WebView puede cargar antes de que el backend termine de
      // construir AppState. Reintentamos unos segundos en vez de mostrar el
      // error interno de Tauri.
      if (isStateNotManaged(cause) && attempt < 60) {
        await new Promise((resolve) => setTimeout(resolve, 250));
        return boot(attempt + 1);
      }
      error = isStateNotManaged(cause)
        ? translate("common.startupStateError")
        : errorMessage(cause);
    }
  }

  function isStateNotManaged(cause: unknown): boolean {
    const message = cause instanceof Error ? cause.message : String(cause);
    return (
      message.includes("state not managed") ||
      message.includes("You must call `.manage()`")
    );
  }

  onMount(() => {
    void boot();
    void checkUpdatesInBackground();
    void initTtsPlayer();
    let unlisten: UnlistenFn | undefined;
    let unlistenConversion: UnlistenFn | undefined;
    let unlistenConversionError: UnlistenFn | undefined;
    void listen<Achievement>("achievement::unlocked", (event) => {
      notifyImportant(
        "success",
        $t("toast.achievement", { title: event.payload.title }),
        event.payload.description,
      );
    }).then((stop) => {
      unlisten = stop;
    });
    void listen<PdfConversionResult>("pdf-convert-done", (event) => {
      notifyImportant(
        "success",
        $t("pdf.convertDoneToast"),
        event.payload.book.title,
        "library",
      );
      window.dispatchEvent(new Event("kora:library-changed"));
    }).then((stop) => {
      unlistenConversion = stop;
    });
    void listen<PdfConversionState>("pdf-convert-failed", (event) => {
      if (event.payload.error) {
        notifyError(event.payload.error);
      }
    }).then((stop) => {
      unlistenConversionError = stop;
    });

    const onRejection = (event: PromiseRejectionEvent) => {
      notifyError(event.reason);
    };
    window.addEventListener("unhandledrejection", onRejection);

    const onAccountChanged = (event: Event) => {
      account = (event as CustomEvent<AccountView>).detail;
    };
    window.addEventListener("kora:account-changed", onAccountChanged);

    return () => {
      unlisten?.();
      unlistenConversion?.();
      unlistenConversionError?.();
      window.removeEventListener("unhandledrejection", onRejection);
      window.removeEventListener("kora:account-changed", onAccountChanged);
    };
  });

  async function refreshAppearance() {
    try {
      appearanceState = await loadAppearance();
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function refreshAccount() {
    try {
      account = await getAccount();
    } catch (cause) {
      notifyError(cause);
    }
  }

  function changedAppearance(state: AppearanceState) {
    appearanceState = state;
    updateAppearance(state);
    appearance.set(state);
  }

  function navigate(next: AppTab) {
    readerAutoListen = false;
    tab = next;
    window.scrollTo(0, 0);
  }
</script>

{#if !ready && !error}
  <div class="boot">
    <img class="boot-mark" src="/logo.png" alt="Kora" />
    <Spinner label={$t("common.loading")} />
  </div>
{:else if error && !appearanceState}
  <div class="boot">
    <p class="boot-error" role="alert">{error}</p>
    <button class="primary" onclick={() => void boot()}>{$t("common.retry")}</button>
  </div>
{:else if appearanceState && !appearanceState.preferences.onboardingDone}
  <Onboarding
    initial={appearanceState}
    {account}
    platform={info?.platform ?? ""}
    onChanged={changedAppearance}
    onDone={() => {
      void refreshAppearance();
      void refreshAccount();
      tab = "library";
    }}
  />
{:else if appearanceState}
  <AppShell
    tab={tab}
    onNavigate={navigate}
    version={info?.version ?? "0.1.0"}
    reading={tab === "reader"}
    {account}
  >
    {#key tab}
      <div class="screen" in:fly={{ y: 8, duration: 180 }} out:fade={{ duration: 90 }}>
        {#if tab === "library"}
          {@const view = loadView("library")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View
              onRead={openReader}
              onListen={openReaderListen}
              onNavigate={navigate}
              onCreateAudiobook={openAudiobookGenerator}
            />
          {/await}
        {:else if tab === "reader"}
          {@const view = loadView("reader")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View
              bookId={readerBookId}
              initialLocator={readerLocator}
              autoListen={readerAutoListen}
              onExit={() => navigate("library")}
              onSettings={() => navigate("settings")}
            />
          {/await}
        {:else if tab === "audiobooks"}
          {@const view = loadView("audiobooks")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View
              onRead={openReader}
              onNavigate={navigate}
              initialBookId={audiobookIntent?.bookId ?? ""}
              autoOpenGenerator={audiobookIntent !== null}
              onIntentConsumed={() => (audiobookIntent = null)}
            />
          {/await}
        {:else if tab === "notes"}
          {@const view = loadView("notes")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View onRead={openReader} onNavigate={navigate} />
          {/await}
        {:else if tab === "diary"}
          {@const view = loadView("diary")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View onRead={openReader} />
          {/await}
        {:else if tab === "voice"}
          {@const view = loadView("voice")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View />
          {/await}
        {:else if tab === "account"}
          {@const view = loadView("account")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View />
          {/await}
        {:else if tab === "sync"}
          {@const view = loadView("sync")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View />
          {/await}
        {:else}
          {@const view = loadView("settings")}
          {#await view}
            <div class="view-loading"><Spinner label={$t("common.loading")} /></div>
          {:then module}
            {@const View = module.default}
            <View onNavigate={navigate} profile={account} appVersion={info?.version ?? ""} platform={info?.platform ?? ""} />
          {/await}
        {/if}
      </div>
    {/key}
  </AppShell>
{/if}

{#if $ttsPlayback && tab !== "reader" && ($ttsPlayback.state === "preparing" || $ttsPlayback.state === "playing" || $ttsPlayback.state === "paused")}
  <div class="tts-global" role="region" aria-label={$t("reader.listen")}>
    <button
      class="tts-global-copy"
      onclick={() => {
        if (!$ttsPlayback?.bookId) return;
        openReader($ttsPlayback.bookId, {
          section: $ttsPlayback.section ?? 0,
          offset: $ttsPlayback.phraseStart ?? 0,
        });
      }}
      title={$t("reader.showPlayer")}
    >
      <span class="tts-global-cover">
        {#if $ttsPlayback.media?.coverPath}
          <img
            src={convertFileSrc($ttsPlayback.media.coverPath)}
            alt=""
            loading="lazy"
          />
        {:else}
          <Icon name="headphones" size={16} />
        {/if}
      </span>
      <span class="tts-global-text">
        <strong>{$ttsPlayback.media?.title ?? $t("reader.listen")}</strong>
        <small>
          {$ttsPlayback.state === "preparing"
            ? $t("reader.preparingVoice")
            : $ttsPlayback.media?.artist ?? $t("reader.listen")}
        </small>
      </span>
    </button>
    {#if $ttsPlayback.state !== "preparing"}
      <button
        class="tts-global-control"
        onclick={() => void toggleTtsPlayback()}
        aria-label={$ttsPlayback.state === "playing"
          ? $t("reader.pause")
          : $t("reader.resume")}
        title={$ttsPlayback.state === "playing"
          ? $t("reader.pause")
          : $t("reader.resume")}
      >
        <Icon
          name={$ttsPlayback.state === "playing" ? "pause" : "play"}
          size={16}
        />
      </button>
    {/if}
    <button
      class="tts-global-control stop"
      onclick={() => void stopTtsPlayback()}
      aria-label={$t("reader.stop")}
      title={$t("reader.stop")}
    >
      <Icon name="stop" size={14} />
    </button>
  </div>
{/if}

<ToastHost />

<style>
  .view-loading {
    display: grid;
    place-items: center;
    min-height: 40vh;
  }

  .boot {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 1rem;
    background:
      radial-gradient(
        900px 500px at 20% 0%,
        color-mix(in srgb, var(--accent) 16%, transparent),
        transparent
      ),
      var(--bg);
  }

  .boot-mark {
    width: 76px;
    height: 76px;
    display: block;
    border-radius: 20px;
    object-fit: cover;
    box-shadow:
      0 20px 44px -22px color-mix(in srgb, var(--gold-deep) 60%, transparent),
      inset 0 1px 0 color-mix(in srgb, #fff 22%, transparent);
  }

  .boot-error {
    max-width: 48ch;
    padding: 0.8rem 1rem;
    border: 1px solid var(--warn);
    border-radius: 12px;
    background: color-mix(in srgb, var(--warn) 10%, transparent);
    text-align: center;
  }

  .screen {
    min-height: calc(100vh - 8rem);
  }

  .tts-global {
    position: fixed;
    right: 1.25rem;
    bottom: 1.25rem;
    z-index: 70;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    max-width: min(26rem, calc(100vw - 2rem));
    padding: 0.4rem;
    border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
    border-radius: 14px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow: 0 18px 44px -22px color-mix(in srgb, #000 55%, transparent);
    backdrop-filter: blur(14px);
  }

  .tts-global-copy {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-width: 0;
    padding: 0.2rem 0.35rem;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .tts-global-copy:hover {
    background: color-mix(in srgb, var(--accent) 9%, transparent);
  }

  .tts-global-cover {
    display: grid;
    place-items: center;
    width: 2.35rem;
    height: 2.35rem;
    flex: none;
    overflow: hidden;
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent) 14%, var(--surface));
    color: var(--accent);
  }

  .tts-global-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .tts-global-text {
    display: grid;
    min-width: 0;
  }

  .tts-global-text strong,
  .tts-global-text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tts-global-text strong {
    font-size: 0.82rem;
  }

  .tts-global-text small {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .tts-global-control {
    display: grid;
    place-items: center;
    width: 2.15rem;
    height: 2.15rem;
    flex: none;
    border: 0;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    cursor: pointer;
  }

  .tts-global-control:hover {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .tts-global-control.stop {
    background: transparent;
    color: var(--muted);
  }

  @media (max-width: 700px) {
    .tts-global {
      left: 0.75rem;
      right: 0.75rem;
      bottom: calc(5.2rem + env(safe-area-inset-bottom));
      max-width: none;
    }
  }

  .primary {
    min-height: 48px;
    padding: 0.6rem 1rem;
    border: 1px solid var(--accent);
    border-radius: 10px;
    background: var(--accent);
    color: var(--accent-contrast);
    font: inherit;
    cursor: pointer;
  }

  @media (prefers-reduced-motion: reduce) {
    .screen {
      animation: none;
    }
  }
</style>

<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import AppShell from "./lib/components/AppShell.svelte";
  import Spinner from "./lib/components/Spinner.svelte";
  import ToastHost from "./lib/components/ToastHost.svelte";
  import Onboarding from "./views/Onboarding.svelte";
  import { appInfo, getAccount, errorMessage } from "./lib/api";
  import { loadView } from "./lib/views";
  import {
    appearance,
    loadAppearance,
    updateAppearance,
  } from "./lib/appearance";
  import { t } from "./lib/i18n";
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

  async function boot() {
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
      error = errorMessage(cause);
    }
  }

  onMount(() => {
    void boot();
    void checkUpdatesInBackground();
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
    <button class="primary" onclick={boot}>{$t("common.retry")}</button>
  </div>
{:else if appearanceState && !appearanceState.preferences.onboardingDone}
  <Onboarding
    initial={appearanceState}
    {account}
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
            <View onRead={openReader} onListen={openReaderListen} onNavigate={navigate} />
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
            <View onRead={openReader} onNavigate={navigate} />
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
            <View onNavigate={navigate} />
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

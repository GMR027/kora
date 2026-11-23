<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import {
    accountStatus,
    aiGetConfig,
    backupLibrary,
    aiSaveConfig,
    aiStatus,
    categorySeedApply,
    checkAppUpdate,
    classificationGetConfig,
    classificationSaveConfig,
    classificationStatus,
    deleteCustomTheme,
    errorMessage,
    featuresStatus,
    getSyncCollections,
    libraryClassifyPending,
    listSyncDevices,
    openExternalUrl,
    prepareSemanticModel,
    readerApplyToAll,
    readerGetDefaults,
    reindexLibrary,
    restoreLibrary,
    saveAppearance,
    purgeStorage,
    saveCustomTheme,
    semanticStatus,
    setSyncCategories,
    storageLayout,
    storageUsage,
    syncStatus,
  } from "../lib/api";
  import {
    BUILTIN_THEMES,
    TEXTURES,
    appearance,
    contrastRatio,
    customThemeDefinition,
    updateAppearance,
  } from "../lib/appearance";
  import {
    setLocale,
    t,
    translate,
    type MessageKey,
  } from "../lib/i18n";
  import {
    comfort,
    resetComfort,
    updateComfort,
  } from "../lib/comfort";
  import { notifyError, notifyImportant, notifySuccess } from "../lib/toast";
  import {
    DEFAULT_UPDATE_MANIFEST_URL,
    getUpdateManifestUrl,
    openUpdatesRequest,
    saveUpdateManifestUrl,
  } from "../lib/updates";
  import type { AppTab } from "../lib/navigation";
  import Button from "../lib/components/Button.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import type {
    AccountStatus,
    AiConfig,
    AiStatus,
    AppTexture,
    ClassificationConfig,
    ClassificationProgress,
    ClassificationStatus,
    FeaturesStatus,
    ReaderSettings,
    SemanticModelProgress,
    SemanticProgress,
    SemanticStatus,
    StorageLayout,
    StorageUsage,
    SyncCollections,
    SyncDeviceInfo,
    SyncStatus,
    UpdateCheck,
  } from "../lib/types";

  type SettingsTab =
  | "appearance"
  | "reading"
  | "wellbeing"
  | "ai"
  | "sync"
  | "storage"
  | "about";

  let {
    onNavigate,
    profile = null,
    appVersion = "",
    platform = "",
  }: {
    onNavigate?: (tab: AppTab) => void;
    profile?: { displayName: string; username: string; avatarPath?: string | null } | null;
    appVersion?: string;
    platform?: string;
  } = $props();

  let tab = $state<SettingsTab>("appearance");
  let basePromise: Promise<void> | null = null;
  let readerPromise: Promise<void> | null = null;
  let usagePromise: Promise<void> | null = null;
  let aiPromise: Promise<void> | null = null;
  let classificationPromise: Promise<void> | null = null;
  let syncPromise: Promise<void> | null = null;
  let layout = $state<StorageLayout | null>(null);
  let usage = $state<StorageUsage | null>(null);
  let readerDefaults = $state<ReaderSettings | null>(null);
  let features = $state<FeaturesStatus | null>(null);
  let semantic = $state<SemanticStatus | null>(null);
  let semanticProgress = $state<SemanticProgress | null>(null);
  let semanticMessage = $state<SemanticModelProgress | null>(null);
  let classificationProgress = $state<ClassificationProgress | null>(null);
  let sync = $state<SyncStatus | null>(null);
  let account = $state<AccountStatus | null>(null);
  let busy = $state<string | null>(null);
  let backupBusy = $state(false);
  let restoreBusy = $state(false);
  let updateCheck = $state<UpdateCheck | null>(null);
  let updateChecking = $state(false);
  let updateDownloading = $state(false);
  let updateError = $state<string | null>(null);
  let updateManifestUrl = $state("");
  let showUpdateAdvanced = $state(false);

  let themeName = $state("");
  let themeBackground = $state("#fbf9f5");
  let themeForeground = $state("#221f1a");
  let themeAccent = $state("#c2410c");
  let editingThemeId = $state<string | null>(null);

  let devices = $state<SyncDeviceInfo[]>([]);
  let selectedDeviceId = $state<string | null>(null);
  let collections = $state<SyncCollections | null>(null);
  let syncAll = $state(true);
  let selectedCategories = $state<string[]>([]);
  let aiConfig = $state<AiConfig | null>(null);
  let aiState = $state<AiStatus | null>(null);
  let aiBusy = $state(false);
  let classificationConfig = $state<ClassificationConfig | null>(null);
  let classificationState = $state<ClassificationStatus | null>(null);
  let classificationBusy = $state(false);

  let themes = $derived([
    ...BUILTIN_THEMES,
    ...($appearance?.themes ?? []).map(customThemeDefinition),
  ]);
  let currentThemeId = $derived($appearance?.preferences.themeId ?? "light");
  let currentTexture = $derived($appearance?.preferences.texture ?? "paper");
  let currentLocale = $derived($appearance?.preferences.locale ?? "es");
  let customThemes = $derived($appearance?.themes ?? []);
  let contrast = $derived(contrastRatio(themeForeground, themeBackground));

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unlistenSemantic: (() => void) | undefined;
    let unlistenSemanticModel: (() => void) | undefined;
    let unlistenClassification: (() => void) | undefined;
    listen("classification::suggestions", () => {
      scheduleClassificationStatus();
    })
      .then((stop) => {
        unlisten = stop;
      })
      .catch(() => undefined);

    listen<ClassificationProgress>("classification::progress", (event) => {
      classificationProgress = event.payload;
      if (event.payload.status === "done" || event.payload.status === "failed") {
        scheduleClassificationStatus();
        setTimeout(() => {
          classificationProgress = null;
        }, 2200);
      }
    })
      .then((stop) => {
        unlistenClassification = stop;
      })
      .catch(() => undefined);

    listen<SemanticProgress>("semantic::progress", (event) => {
      semanticProgress = event.payload;
      if (event.payload.status === "done" || event.payload.percent >= 100) {
        void load();
        setTimeout(() => {
          semanticProgress = null;
        }, 1800);
      }
    })
      .then((stop) => {
        unlistenSemantic = stop;
      })
      .catch(() => undefined);

    listen<SemanticModelProgress>("semantic::model", (event) => {
      semanticMessage = event.payload;
      if (event.payload.status === "ready") {
        void load();
        setTimeout(() => {
          semanticMessage = null;
        }, 2000);
      }
    })
      .then((stop) => {
        unlistenSemanticModel = stop;
      })
      .catch(() => undefined);

    return () => {
      unlisten?.();
      unlistenClassification?.();
      unlistenSemantic?.();
      unlistenSemanticModel?.();
      if (classificationRefresh) clearTimeout(classificationRefresh);
    };
  });

  // Cada pestaña carga solo lo que muestra. La pestaña de apariencia abre al
  // instante y el resto se pide bajo demanda.
  function ensureBase() {
    return (basePromise ??= load());
  }

  function ensureReader() {
    return (readerPromise ??= loadReaderDefaults());
  }

  function ensureUsage() {
    return (usagePromise ??= loadUsage());
  }

  function ensureAi() {
    return (aiPromise ??= loadAi());
  }

  function ensureClassification() {
    return (classificationPromise ??= loadClassification());
  }

  function ensureSync() {
    return (syncPromise ??= loadSync());
  }

  function selectTab(next: SettingsTab) {
    tab = next;
    if (next === "reading") void ensureReader();
    if (next === "ai") {
      void ensureBase();
      void ensureAi();
      void ensureClassification();
    }
    if (next === "sync") void ensureSync();
    if (next === "storage") {
      void ensureBase();
      void ensureUsage();
    }
    if (next === "about") void ensureBase();
  }

  async function load() {
    try {
      [layout, features, semantic, sync, account] = await Promise.all([
        storageLayout(),
        featuresStatus(),
        semanticStatus(),
        syncStatus(),
        accountStatus(),
      ]);
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function loadAi() {
    try {
      const [config, status] = await Promise.all([aiGetConfig(), aiStatus()]);
      aiConfig = {
        ...config,
        temperature: Number.isFinite(config.temperature)
          ? config.temperature
          : 0.55,
        maxTokens: Number.isFinite(config.maxTokens) ? config.maxTokens : 900,
      };
      aiState = status;
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function loadClassification() {
    try {
      [classificationConfig, classificationState] = await Promise.all([
        classificationGetConfig(),
        classificationStatus(),
      ]);
    } catch (cause) {
      notifyError(cause);
    }
  }

  let classificationRefresh: ReturnType<typeof setTimeout> | undefined;

  function scheduleClassificationStatus() {
    if (classificationRefresh) clearTimeout(classificationRefresh);
    classificationRefresh = setTimeout(() => {
      classificationRefresh = undefined;
      void classificationStatus()
        .then((status) => {
          classificationState = status;
        })
        .catch(() => undefined);
    }, 500);
  }

  async function saveClassification() {
    if (!classificationConfig) return;
    classificationBusy = true;
    try {
      classificationConfig = await classificationSaveConfig(
        classificationConfig,
      );
      classificationState = await classificationStatus();
      notifySuccess(translate("settings.classificationSaved"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      classificationBusy = false;
    }
  }

  async function classifyPending(force: boolean) {
    classificationBusy = true;
    try {
      await libraryClassifyPending(force);
      notifySuccess(translate("settings.classificationQueued"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      classificationBusy = false;
    }
  }

  async function applyBaseCategories() {
    classificationBusy = true;
    try {
      const created = await categorySeedApply();
      classificationState = await classificationStatus();
      notifySuccess(
        translate("settings.classificationSeedApplied", {
          count: created.length,
        }),
      );
    } catch (cause) {
      notifyError(cause);
    } finally {
      classificationBusy = false;
    }
  }

  async function refreshAi() {
    aiBusy = true;
    try {
      aiState = await aiStatus();
    } catch (cause) {
      notifyError(cause);
    } finally {
      aiBusy = false;
    }
  }

  async function saveAi() {
    if (!aiConfig) return;
    aiBusy = true;
    try {
      aiConfig = await aiSaveConfig(aiConfig);
      aiState = await aiStatus();
      notifySuccess(translate("settings.aiSaved"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      aiBusy = false;
    }
  }

  async function loadReaderDefaults() {
    try {
      readerDefaults = await readerGetDefaults();
    } catch {
      readerDefaults = null;
    }
  }

  async function loadUsage() {
    try {
      usage = await storageUsage();
    } catch {
      usage = null;
    }
  }

  async function loadSync() {
    try {
      devices = await listSyncDevices();
      const initial = devices.find((device) => !device.isLocal && device.trusted);
      if (initial) {
        selectedDeviceId = initial.id;
        applyCollections(await getSyncCollections(initial.id));
      } else {
        selectedDeviceId = null;
        collections = null;
      }
    } catch (cause) {
      notifyError(cause);
    }
  }

  function applyCollections(value: SyncCollections) {
    collections = value;
    selectedDeviceId = value.deviceId;
    syncAll = value.syncAll;
    selectedCategories = value.categories
      .filter((item) => item.enabled)
      .map((item) => item.category.id);
  }

  async function selectSyncDevice(id: string) {
    if (!id) return;
    try {
      applyCollections(await getSyncCollections(id));
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function saveSyncCategories() {
    if (!selectedDeviceId) return;
    busy = "sync";
    try {
      applyCollections(
        await setSyncCategories(selectedDeviceId, selectedCategories, syncAll),
      );
      notifySuccess(translate("settings.syncSaved"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function persistAppearance(
    themeId = currentThemeId,
    texture: AppTexture = currentTexture,
    locale = currentLocale,
  ) {
    const state = $appearance;
    if (!state) return;
    busy = "appearance";
    try {
      const saved = await saveAppearance(
        themeId,
        texture,
        locale,
        state.preferences.onboardingDone,
        state.preferences.libraryLayout,
      );
      updateAppearance(saved);
      setLocale(locale as "es" | "en");
      notifySuccess(translate("settings.themeApplied"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function selectTheme(themeId: string) {
    const state = $appearance;
    if (!state) return;
    if (editingThemeId) cancelThemeEdit();
    // Aplica el tema de inmediato para que el acento se vea al instante,
    // sin esperar la ida y vuelta con el backend.
    updateAppearance({
      ...state,
      preferences: { ...state.preferences, themeId },
    });
    await persistAppearance(themeId);
  }

  async function createTheme() {
    if (!themeName.trim()) return;
    const editing = editingThemeId;
    busy = "theme";
    try {
      const theme = await saveCustomTheme(
        editing,
        themeName.trim(),
        themeBackground,
        themeForeground,
        themeAccent,
      );
      const state = $appearance;
      if (state) {
        const exists = state.themes.some((item) => item.id === theme.id);
        updateAppearance({
          ...state,
          themes: exists
            ? state.themes.map((item) => (item.id === theme.id ? theme : item))
            : [...state.themes, theme],
          preferences: {
            ...state.preferences,
            themeId: editing ? state.preferences.themeId : theme.id,
          },
        });
      }
      // Un tema nuevo se activa de inmediato para que el acento elegido se
      // refleje en los botones y en el resto de la interfaz.
      if (!editing) await persistAppearance(theme.id);
      themeName = "";
      editingThemeId = null;
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function editTheme(theme: { id: string; name: string; background: string; foreground: string; accent: string }) {
    editingThemeId = theme.id;
    themeName = theme.name;
    themeBackground = theme.background;
    themeForeground = theme.foreground;
    themeAccent = theme.accent;
  }

  function cancelThemeEdit() {
    editingThemeId = null;
    themeName = "";
  }

  async function removeTheme(id: string) {
    if (!window.confirm(`${translate("settings.deleteTheme")}?`)) return;
    if (editingThemeId === id) cancelThemeEdit();
    busy = `delete-${id}`;
    try {
      const state = await deleteCustomTheme(id);
      updateAppearance(state);
      notifySuccess(translate("settings.themeDeleted"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function prepareSemantic() {
    busy = "semantic";
    try {
      semantic = await prepareSemanticModel();
      notifySuccess(translate("settings.prepareModel"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function reindexSemantic() {
    busy = "semantic";
    try {
      semantic = await reindexLibrary();
      notifySuccess(translate("settings.reindex"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function formatBytes(value: number): string {
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
    if (value < 1024 * 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
    return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  async function cleanStorage(scope: "trash" | "temporary") {
    if (scope === "trash" && !window.confirm(translate("settings.emptyTrashConfirm"))) {
      return;
    }
    busy = scope;
    try {
      usage = await purgeStorage(scope);
      notifySuccess(translate("settings.storageCleaned"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function createBackup() {
    backupBusy = true;
    try {
      const destination = await save({
        defaultPath: "kora-copia-de-seguridad.zip",
        filters: [{ name: "ZIP", extensions: ["zip"] }],
      });
      if (!destination) return;
      const info = await backupLibrary(destination);
      notifySuccess(
        translate("settings.backupCreated"),
        translate("settings.backupCreatedDetail", {
          path: info.path,
          entries: info.entries,
        }),
      );
      usage = await storageUsage();
    } catch (cause) {
      notifyError(cause);
    } finally {
      backupBusy = false;
    }
  }

  async function restoreBackup() {
    if (!window.confirm(translate("settings.restoreConfirm"))) return;
    restoreBusy = true;
    try {
      const source = await open({
        multiple: false,
        filters: [{ name: "ZIP", extensions: ["zip"] }],
      });
      if (typeof source !== "string") return;
      const info = await restoreLibrary(source);
      notifySuccess(
        translate("settings.restorePrepared"),
        info.requiresRestart
          ? translate("settings.restoreRestart")
          : undefined,
      );
    } catch (cause) {
      notifyError(cause);
    } finally {
      restoreBusy = false;
    }
  }

  let previewFont = $derived(
    ({
      newsreader: '"Newsreader", Georgia, serif',
      literata: '"Literata", Georgia, serif',
      garamond: '"EB Garamond", Garamond, serif',
      atkinson: '"Atkinson Hyperlegible", sans-serif',
      jakarta: '"Plus Jakarta Sans", sans-serif',
      mono: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
      opendyslexic: '"OpenDyslexic", sans-serif',
    })[readerDefaults?.fontFace ?? "newsreader"] ?? '"Newsreader", Georgia, serif',
  );

  async function applyReadingDefaults() {
    if (!readerDefaults) return;
    busy = "reading";
    try {
      await readerApplyToAll({
        fontSize: readerDefaults.fontSize,
        lineHeight: readerDefaults.lineHeight,
        margin: readerDefaults.margin,
        fontFamily: readerDefaults.fontFamily,
        fontFace: readerDefaults.fontFace,
        columnWidth: readerDefaults.columnWidth,
        textAlign: readerDefaults.textAlign,
        paragraphIndent: readerDefaults.paragraphIndent,
        hyphenate: readerDefaults.hyphenate,
        theme: readerDefaults.theme,
        texture: readerDefaults.texture,
        mode: readerDefaults.mode,
        zoom: readerDefaults.zoom,
      });
      notifySuccess(translate("settings.readingApplied"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function themeLabel(id: string) {
    const theme = themes.find((item) => item.id === id);
    if (!theme) return id;
    return theme.builtin ? $t(theme.nameKey as MessageKey) : theme.nameKey;
  }

  function textureLabel(id: AppTexture) {
    return $t(
      (TEXTURES.find((item) => item.id === id)?.nameKey ??
        "texture.none") as MessageKey,
    );
  }

  function toggleCategory(id: string) {
    selectedCategories = selectedCategories.includes(id)
      ? selectedCategories.filter((value) => value !== id)
      : [...selectedCategories, id];
  }

  async function checkUpdates() {
    updateChecking = true;
    updateError = null;
    updateCheck = null;
    saveUpdateManifestUrl(updateManifestUrl);
    try {
      const result = await checkAppUpdate(updateManifestUrl.trim() || undefined);
      updateCheck = result;
      try {
        localStorage.setItem("kora.updateCheckedAt", String(Date.now()));
      } catch {
        // Sin persistencia; el chequeo automático podría repetirse.
      }
      if (result.available) {
        notifyImportant(
          "update",
          translate("notifications.updateTitle"),
          translate("notifications.updateBody", {
            version: result.latestVersion,
          }),
          "updates",
        );
      } else {
        notifySuccess(translate("settings.upToDate"));
      }
    } catch (cause) {
      const message = errorMessage(cause);
      const missing = /404|no hay actualizaciones|not found/i.test(message);
      updateError = missing
        ? translate("settings.updatesNotPublished")
        : message;
      if (!missing) notifyError(cause);
    } finally {
      updateChecking = false;
    }
  }

  function installUpdate() {
    if (platform === "android") {
      const url = updateCheck?.androidUrl ?? updateCheck?.url;
      if (!url) return;
      const bridge = (
        window as unknown as {
          KoraChrome?: {
            downloadUpdate?: (
              url: string,
              sha256: string,
              fileName: string,
            ) => void;
          };
        }
      ).KoraChrome;
      if (!bridge?.downloadUpdate) {
        updateError = translate("settings.updateError");
        return;
      }
      updateDownloading = true;
      updateError = null;
      notifyImportant(
        "info",
        translate("settings.updateDownloading"),
        translate("settings.updateAvailable", {
          version: updateCheck?.latestVersion ?? "",
        }),
      );
      bridge.downloadUpdate(
        url,
        updateCheck?.androidSha256 ?? updateCheck?.sha256 ?? "",
        `kora-${updateCheck?.latestVersion ?? "update"}-${Date.now()}.apk`,
      );
      return;
    }
    const url = updateCheck?.releaseUrl ?? updateCheck?.url;
    if (!url) return;
    void openExternalUrl(url)
      .then(() => notifySuccess(translate("settings.updateDesktopHint")))
      .catch((cause) => {
        updateError = errorMessage(cause);
        notifyError(cause);
      });
  }

  function persistUpdateManifest() {
    saveUpdateManifestUrl(updateManifestUrl);
  }

  onMount(() => {
    updateManifestUrl = getUpdateManifestUrl();
    const unsubscribe = openUpdatesRequest.subscribe((value) => {
      if (value <= 0) return;
      tab = "about";
      openUpdatesRequest.set(0);
      setTimeout(() => {
        document
          .querySelector(".update-panel")
          ?.scrollIntoView({ behavior: "smooth", block: "start" });
      }, 140);
    });

    const onReady = () => {
      updateDownloading = false;
      notifyImportant("success", translate("settings.updateReady"));
    };
    const onError = (event: Event) => {
      updateDownloading = false;
      const detail = (event as CustomEvent<{ message?: string }>).detail;
      const message = detail?.message || translate("settings.updateError");
      updateError = message;
      notifyError(new Error(message));
    };
    const onPermission = () => {
      updateDownloading = false;
      notifyImportant("warning", translate("settings.updatePermission"));
    };
    window.addEventListener("kora:update-ready", onReady);
    window.addEventListener("kora:update-error", onError);
    window.addEventListener("kora:update-permission", onPermission);
    return () => {
      unsubscribe();
      window.removeEventListener("kora:update-ready", onReady);
      window.removeEventListener("kora:update-error", onError);
      window.removeEventListener("kora:update-permission", onPermission);
    };
  });
</script>

<section class="settings">
  <header class="hero">
    <div>
      <p class="eyebrow">{$t("nav.settings")}</p>
      <h1 class="page-title">{$t("settings.title")}</h1>
      <p class="page-subtitle">{$t("settings.subtitle")}</p>
    </div>
  </header>

  {#if profile}
    <button class="mobile-account-card" onclick={() => onNavigate?.("account")}>
      <span class="mobile-account-avatar" aria-hidden="true">
        {#if profile.avatarPath}
          <img src={convertFileSrc(profile.avatarPath)} alt="" />
        {:else}
          {(profile.displayName || profile.username).trim().slice(0, 1).toUpperCase()}
        {/if}
      </span>
      <span class="mobile-account-copy">
        <strong>{profile.displayName}</strong>
        <small>@{profile.username} · {$t("settings.localData")}</small>
      </span>
      <Icon name="chevron" size={17} />
    </button>
  {/if}

  <nav class="tabs" aria-label={$t("settings.title")}>
    {#each [
      ["appearance", "settings.appearance"],
      ["reading", "settings.reading"],
      ["wellbeing", "settings.wellbeing"],
      ["ai", "settings.ai"],
      ["sync", "settings.sync"],
      ["storage", "settings.storage"],
      ["about", "settings.about"],
    ] as [id, key] (id)}
      <button
        class:active={tab === id}
        aria-pressed={tab === id}
        onclick={() => selectTab(id as SettingsTab)}
      >
        {$t(key as MessageKey)}
      </button>
    {/each}
  </nav>

  {#if tab === "appearance"}
    <div class="appearance-grid">
      <aside class="panel theme-preview-panel">
        <div class="preview-head">
          <span>{$t("settings.preview")}</span>
          <small>{themeLabel(currentThemeId)}</small>
        </div>
        <div class="preview-sheet">
          <p class="preview-kicker">{$t("settings.previewChapter")}</p>
          <h3>{$t("settings.previewTitle")}</h3>
          <span class="preview-rule" aria-hidden="true"></span>
          <p class="preview-text">{$t("settings.previewText")}</p>
          <p class="preview-quote">{$t("settings.previewQuote")}</p>
        </div>
        <div class="preview-metrics">
          <span><strong>19 px</strong> {$t("settings.reading")}</span>
          <span><strong>1.7×</strong> {$t("reader.lineHeight")}</span>
        </div>
        <Button variant="primary" onclick={() => onNavigate?.("reader")}>
          <Icon name="book" size={17} /> {$t("settings.previewAction")}
        </Button>
      </aside>

      <div class="appearance-controls">
    <section class="panel">
      <h2>{$t("settings.themes")}</h2>
      <div class="theme-grid">
        {#each themes as theme (theme.id)}
          <button
            class="theme-card"
            class:selected={currentThemeId === theme.id}
            aria-pressed={currentThemeId === theme.id}
            style:background={theme.colors.background}
            style:color={theme.colors.text}
            onclick={() => selectTheme(theme.id)}
            disabled={busy !== null}
          >
            <span class="theme-dot" style:background={theme.colors.accent}></span>
            <strong>{themeLabel(theme.id)}</strong>
            <span class="theme-preview">
              <i style:background={theme.colors.surface}></i>
              <i style:background={theme.colors.accent}></i>
            </span>
            {#if currentThemeId === theme.id}
              <span class="theme-check"><Icon name="check" size={14} /></span>
            {/if}
            {#if !theme.builtin}
              <span class="custom-tag">{$t("settings.customThemes")}</span>
            {/if}
          </button>
        {/each}
      </div>
    </section>

    <section class="panel">
      <h2>
        {editingThemeId ? $t("settings.editTheme") : $t("settings.customThemes")}
      </h2>
      <div class="form-grid">
        <label>
          <span>{$t("common.name")}</span>
          <input bind:value={themeName} maxlength="60" placeholder={$t("settings.themePlaceholder")} />
        </label>
        <label>
          <span>{$t("settings.background")}</span>
          <input type="color" bind:value={themeBackground} />
        </label>
        <label>
          <span>{$t("settings.foreground")}</span>
          <input type="color" bind:value={themeForeground} />
        </label>
        <label>
          <span>{$t("settings.accent")}</span>
          <input type="color" bind:value={themeAccent} />
        </label>
      </div>
      <div class="contrast-line">
        <span>{$t("common.preview")}</span>
        <span class:low={contrast < 4.5}>
          {contrast.toFixed(2)}:1 {contrast < 4.5 ? "⚠" : "✓"}
        </span>
      </div>
      <div class="theme-actions">
        <Button
          variant="primary"
          loading={busy === "theme"}
          disabled={!themeName.trim()}
          onclick={createTheme}
        >
          <Icon name={editingThemeId ? "check" : "plus"} />
          {editingThemeId ? $t("common.save") : $t("settings.createTheme")}
        </Button>
        {#if editingThemeId}
          <Button variant="ghost" onclick={cancelThemeEdit}>
            {$t("common.cancel")}
          </Button>
        {/if}
      </div>
      {#if customThemes.length > 0}
        <div class="custom-list">
          {#each customThemes as theme (theme.id)}
            <div class="custom-row">
              <span class="swatch" style:background={theme.background}>
                <i style:background={theme.accent}></i>
              </span>
              <strong>{theme.name}</strong>
              <Button
                variant="ghost"
                size="small"
                onclick={() => editTheme(theme)}
              >
                {$t("common.edit")}
              </Button>
              <Button
                variant="danger"
                size="small"
                loading={busy === `delete-${theme.id}`}
                onclick={() => removeTheme(theme.id)}
              >
                {$t("settings.deleteTheme")}
              </Button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="panel">
      <h2>{$t("settings.textures")}</h2>
      <div class="texture-grid">
        {#each TEXTURES as texture (texture.id)}
          <button
            class="texture-card"
            class:selected={currentTexture === texture.id}
            aria-pressed={currentTexture === texture.id}
            onclick={() => persistAppearance(currentThemeId, texture.id)}
            disabled={busy !== null}
          >
            <span class="texture-swatch" data-texture={texture.id}></span>
            <span>{textureLabel(texture.id)}</span>
          </button>
        {/each}
      </div>
    </section>

      </div>
    </div>

    <section class="panel language-panel">
      <h2>{$t("settings.language")}</h2>
      <div class="language-row">
        <Button
          variant={currentLocale === "es" ? "primary" : "ghost"}
          onclick={() => persistAppearance(currentThemeId, currentTexture, "es")}
        >
          Español
        </Button>
        <Button
          variant={currentLocale === "en" ? "primary" : "ghost"}
          onclick={() => persistAppearance(currentThemeId, currentTexture, "en")}
        >
          English
        </Button>
      </div>
    </section>
  {:else if tab === "reading"}
    {#if readerDefaults}
      <div class="reading-layout">
        <section class="panel reading-controls">
          <div class="panel-heading">
            <div>
              <p class="eyebrow">{$t("settings.readingEyebrow")}</p>
              <h2>{$t("settings.typography")}</h2>
            </div>
            <span class="badge">{$t("settings.globalDefaults")}</span>
          </div>
          <p class="hint">{$t("settings.readingHelp")}</p>
          <div class="form-grid">
            <label class="wide">
              <span>{$t("reader.fontFace")}</span>
              <select bind:value={readerDefaults.fontFace}>
                <option value="newsreader">Newsreader</option>
                <option value="literata">Literata</option>
                <option value="garamond">EB Garamond</option>
                <option value="atkinson">Atkinson Hyperlegible</option>
                <option value="jakarta">Plus Jakarta Sans</option>
                <option value="mono">Monoespaciada</option>
                <option value="opendyslexic">OpenDyslexic</option>
              </select>
            </label>
            <label>
              <span>{$t("reader.fontSize")} · {readerDefaults.fontSize}px</span>
              <input type="range" min="12" max="36" step="1" bind:value={readerDefaults.fontSize} />
            </label>
            <label>
              <span>{$t("reader.lineHeight")} · {readerDefaults.lineHeight.toFixed(1)}</span>
              <input type="range" min="1" max="2.5" step="0.1" bind:value={readerDefaults.lineHeight} />
            </label>
            <label>
              <span>{$t("reader.columnWidth")} · {readerDefaults.columnWidth}px</span>
              <input type="range" min="520" max="1000" step="20" bind:value={readerDefaults.columnWidth} />
            </label>
            <label>
              <span>{$t("reader.margins")} · {readerDefaults.margin}px</span>
              <input type="range" min="0" max="160" step="4" bind:value={readerDefaults.margin} />
            </label>
            <label class="wide">
              <span>{$t("reader.alignment")}</span>
              <div class="segmented wide">
                <button
                  class:active={readerDefaults.textAlign === "left"}
                  onclick={() => {
                    if (readerDefaults) readerDefaults.textAlign = "left";
                  }}
                >
                  {$t("reader.alignLeft")}
                </button>
                <button
                  class:active={readerDefaults.textAlign === "justify"}
                  onclick={() => {
                    if (readerDefaults) readerDefaults.textAlign = "justify";
                  }}
                >
                  {$t("reader.alignJustify")}
                </button>
              </div>
            </label>
            <label class="check">
              <input
                type="checkbox"
                checked={readerDefaults.paragraphIndent > 0}
                onchange={(event) => {
                  if (readerDefaults) {
                    readerDefaults.paragraphIndent = event.currentTarget.checked ? 1 : 0;
                  }
                }}
              />
              <span>{$t("reader.paragraphIndent")}</span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={readerDefaults.hyphenate} />
              <span>{$t("reader.hyphenate")}</span>
            </label>
          </div>
          <div class="actions">
            <Button variant="primary" loading={busy === "reading"} onclick={applyReadingDefaults}>
              <Icon name="check" /> {$t("settings.applyReading")}
            </Button>
            <Button variant="ghost" onclick={() => onNavigate?.("reader")}>
              <Icon name="book" /> {$t("settings.openReader")}
            </Button>
          </div>
        </section>

        <aside class="panel reading-preview">
          <div class="panel-heading">
            <span class="eyebrow">{$t("settings.preview")}</span>
            <span class="badge">{$t("settings.globalDefaults")}</span>
          </div>
          <div
            class="reading-sheet"
            style:font-family={previewFont}
            style:font-size={`${readerDefaults.fontSize}px`}
            style:line-height={readerDefaults.lineHeight}
            style:text-align={readerDefaults.textAlign}
          >
            <p class="reading-kicker">{$t("settings.previewChapter")}</p>
            <h3>{$t("settings.previewTitle")}</h3>
            <p>{$t("settings.previewText")}</p>
            <p class="reading-quote">{$t("settings.previewQuote")}</p>
          </div>
          <p class="hint">
            {$t("settings.readingPreviewHelp")}
          </p>
        </aside>
      </div>
    {/if}

    <section class="panel">
      <h2>{$t("settings.textures")}</h2>
      <p class="hint">{$t("settings.textureHelp")}</p>
      <div class="texture-grid">
        {#each TEXTURES as texture (texture.id)}
          <button
            class="texture-card"
            class:selected={currentTexture === texture.id}
            aria-pressed={currentTexture === texture.id}
            onclick={() => persistAppearance(currentThemeId, texture.id)}
          >
            <span class="texture-swatch" data-texture={texture.id}></span>
            <span>{textureLabel(texture.id)}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else if tab === "wellbeing"}
    <div class="wellbeing-layout">
      <section class="panel comfort-panel">
        <div class="panel-heading">
          <div>
            <p class="eyebrow">{$t("settings.wellbeingEyebrow")}</p>
            <h2>{$t("settings.comfort")}</h2>
          </div>
          <span class="comfort-preview" aria-hidden="true">
            <i style:background={`rgba(255, 154, 60, ${$comfort.amber / 100})`}></i>
            <i style:background={`rgba(20, 12, 5, ${$comfort.dim / 100})`}></i>
          </span>
        </div>
        <p class="hint">{$t("settings.comfortHelp")}</p>
        <label>
          <span>{$t("settings.amberFilter")} · {$comfort.amber}%</span>
          <input
            type="range"
            min="0"
            max="100"
            step="5"
            value={$comfort.amber}
            oninput={(event) => updateComfort({ amber: Number(event.currentTarget.value) })}
          />
        </label>
        <label>
          <span>{$t("settings.dimScreen")} · {$comfort.dim}%</span>
          <input
            type="range"
            min="0"
            max="100"
            step="5"
            value={$comfort.dim}
            oninput={(event) => updateComfort({ dim: Number(event.currentTarget.value) })}
          />
        </label>
        <div class="scale-row">
          <span>{$t("settings.interfaceScale")}</span>
          {#each [80, 90, 100, 110, 125] as value (value)}
            <button
              class:active={$comfort.scale === value}
              aria-pressed={$comfort.scale === value}
              onclick={() => updateComfort({ scale: value })}
            >
              {value}%
            </button>
          {/each}
        </div>
      </section>

      <section class="panel comfort-panel">
        <h2>{$t("settings.readingHabits")}</h2>
        <label class="check">
          <input
            type="checkbox"
            checked={$comfort.twenty}
            onchange={(event) => updateComfort({ twenty: event.currentTarget.checked })}
          />
          <span>
            <strong>{$t("settings.twentyRule")}</strong>
            <small>{$t("settings.twentyRuleHelp")}</small>
          </span>
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={$comfort.nightLimit}
            onchange={(event) => updateComfort({ nightLimit: event.currentTarget.checked })}
          />
          <span>
            <strong>{$t("settings.nightLimit")}</strong>
            <small>{$t("settings.nightLimitHelp")}</small>
          </span>
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={$comfort.bedtime}
            onchange={(event) => updateComfort({ bedtime: event.currentTarget.checked })}
          />
          <span>
            <strong>{$t("settings.bedtimeMode")}</strong>
            <small>{$t("settings.bedtimeModeHelp")}</small>
          </span>
        </label>
        <Button variant="ghost" size="small" onclick={resetComfort}>
          {$t("settings.resetComfort")}
        </Button>
      </section>
    </div>

    <div class="card-grid">
      <article class="panel">
        <h2>{$t("settings.diary")}</h2>
        <p class="hint">{$t("settings.diaryHelp")}</p>
        <Button variant="primary" onclick={() => onNavigate?.("diary")}>
          <Icon name="diary" /> {$t("settings.openDiary")}
        </Button>
      </article>
      <article class="panel">
        <h2>{$t("settings.achievements")}</h2>
        <p class="hint">{$t("settings.achievementsHelp")}</p>
        <Button variant="ghost" onclick={() => onNavigate?.("diary")}>
          <Icon name="check" /> {$t("settings.openAchievements")}
        </Button>
      </article>
      <article class="panel">
        <h2>{$t("settings.pomodoro")}</h2>
        <p class="hint">{$t("settings.pomodoroHelp")}</p>
        <Button variant="ghost" onclick={() => onNavigate?.("diary")}>
          <Icon name="diary" /> {$t("settings.openPomodoro")}
        </Button>
      </article>
    </div>
  {:else if tab === "ai"}
    {#if aiConfig}
      <div class="ai-layout">
        <section class="panel ai-config">
          <div class="ai-status-head">
            <div>
              <p class="eyebrow">{$t("settings.aiEyebrow")}</p>
              <h2>{$t("settings.aiTitle")}</h2>
            </div>
            <span class="ai-status" class:online={aiState?.available}>
              <i aria-hidden="true"></i>
              {aiState?.available ? $t("settings.aiOnline") : $t("settings.aiOffline")}
            </span>
          </div>
          <p class="hint">{$t("settings.aiHelp")}</p>

          <div class="provider-grid" role="group" aria-label={$t("settings.aiProvider")}>
            {#each [
              ["auto", "settings.aiAuto", "sparkles"],
              ["ollama", "settings.aiOllama", "book"],
              ["llama_cpp", "settings.aiLlama", "voice"],
            ] as [id, key, icon] (id)}
              <button
                class:active={aiConfig?.provider === id}
                aria-pressed={aiConfig?.provider === id}
                onclick={() => {
                  if (aiConfig) aiConfig.provider = id as AiConfig["provider"];
                }}
              >
                <Icon name={icon as "sparkles" | "book" | "voice"} size={18} />
                <strong>{$t(key as MessageKey)}</strong>
              </button>
            {/each}
          </div>

          <div class="form-grid">
            <label>
              <span>{$t("settings.aiOllamaUrl")}</span>
              <input bind:value={aiConfig.ollamaUrl} placeholder="http://127.0.0.1:11434" />
            </label>
            <label>
              <span>{$t("settings.aiLlamaUrl")}</span>
              <input bind:value={aiConfig.llamaUrl} placeholder="http://127.0.0.1:8080" />
            </label>
            <label class="wide">
              <span>{$t("settings.aiModel")}</span>
              <select bind:value={aiConfig.model}>
                <option value="">{$t("settings.aiModelAuto")}</option>
                {#each aiState?.models ?? [] as model (model)}
                  <option value={model}>{model}</option>
                {/each}
                {#if aiConfig.model && !(aiState?.models ?? []).includes(aiConfig.model)}
                  <option value={aiConfig.model}>{aiConfig.model}</option>
                {/if}
              </select>
            </label>
            <label>
              <span>{$t("settings.aiTemperature")} · {aiConfig.temperature.toFixed(2)}</span>
              <input type="range" min="0" max="2" step="0.05" bind:value={aiConfig.temperature} />
            </label>
            <label>
              <span>{$t("settings.aiMaxTokens")}</span>
              <input type="number" min="64" max="8192" step="64" bind:value={aiConfig.maxTokens} />
            </label>
          </div>

          {#if aiState?.error}
            <p class="notice" role="status">{aiState.error}</p>
          {/if}
          {#if aiState?.available}
            <p class="ai-detected">
              <strong>{aiState.provider === "ollama" ? "Ollama" : "llama.cpp"}</strong>
              <span>{aiState.baseUrl}</span>
              <span>
                {aiState.models.length}
                {aiState.models.length === 1
                  ? $t("settings.aiModelSingular")
                  : $t("settings.aiModels")}
              </span>
            </p>
          {/if}

          <div class="actions">
            <Button variant="ghost" loading={aiBusy} onclick={refreshAi}>
              <Icon name="sync" size={17} /> {$t("settings.aiRefresh")}
            </Button>
            <Button variant="primary" loading={aiBusy} onclick={saveAi}>
              <Icon name="check" size={17} /> {$t("common.save")}
            </Button>
          </div>
        </section>

        <aside class="panel ai-guide">
          <h2>{$t("settings.aiGuide")}</h2>
          <p class="hint">{$t("settings.aiPrivacy")}</p>
          <div class="ai-guide-block">
            <strong>Ollama · PC</strong>
            <code>ollama serve</code>
            <code>ollama pull qwen2.5:7b-instruct</code>
            <code>http://127.0.0.1:11434</code>
          </div>
          <div class="ai-guide-block">
            <strong>llama.cpp · Android</strong>
            <code>llama-server -m modelo.gguf --port 8080 -c 8192</code>
            <code>http://127.0.0.1:8080</code>
          </div>
          <p class="hint">{$t("settings.aiFree")}</p>
        </aside>
      </div>
      {#if classificationConfig}
        <section class="panel classification-panel">
          <div class="ai-status-head">
            <div>
              <p class="eyebrow">{$t("settings.classification")}</p>
              <h2>{$t("settings.classification")}</h2>
            </div>
            {#if classificationState}
              <span class="ai-status" class:online={classificationState.aiAvailable}>
                <i aria-hidden="true"></i>
                {classificationState.aiAvailable
                  ? $t("settings.aiOnline")
                  : $t("settings.aiOffline")}
              </span>
            {/if}
          </div>
          <p class="hint">{$t("settings.classificationHelp")}</p>

          <div class="form-grid">
            <label class="check">
              <input type="checkbox" bind:checked={classificationConfig.enabled} />
              <span>{$t("settings.classificationEnabled")}</span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={classificationConfig.classifyOnImport} />
              <span>{$t("settings.classificationOnImport")}</span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={classificationConfig.useAi} />
              <span>{$t("settings.classificationAi")}</span>
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={classificationConfig.allowNewCategories} />
              <span>{$t("settings.classificationNew")}</span>
            </label>
            <label>
              <span>
                {$t("settings.classificationEmbeddingThreshold")} ·
                {Math.round(classificationConfig.embeddingThreshold * 100)}%
              </span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                bind:value={classificationConfig.embeddingThreshold}
              />
            </label>
            <label>
              <span>
                {$t("settings.classificationAiThreshold")} ·
                {Math.round(classificationConfig.aiThreshold * 100)}%
              </span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                bind:value={classificationConfig.aiThreshold}
              />
            </label>
            <label>
              <span>{$t("settings.classificationMax")}</span>
              <input
                type="number"
                min="1"
                max="8"
                step="1"
                bind:value={classificationConfig.maxSuggestions}
              />
            </label>
          </div>

          {#if classificationState}
            <p class="hint">
              {$t("settings.classificationPending", {
                count: classificationState.pendingSuggestions,
              })}
              · {classificationState.classifiedBooks}/{classificationState.totalBooks}
            </p>
            {#if !classificationState.embeddingsReady}
              <p class="notice" role="status">
                {$t("settings.classificationEmbeddingsUnavailable")}
              </p>
            {/if}
            {#if classificationConfig.useAi && !classificationState.aiAvailable}
              <p class="notice" role="status">
                {$t("settings.classificationAiUnavailable")}
              </p>
            {/if}
          {/if}

          {#if classificationProgress}
            <div class="progress-row">
              <span class="progress-track">
                <span style:width={`${Math.round(classificationProgress.percent)}%`}></span>
              </span>
              <span class="hint">
                {classificationProgress.title} ·
                {classificationProgress.message || classificationProgress.stage} ·
                {Math.round(classificationProgress.percent)}%
              </span>
            </div>
          {/if}

          <div class="actions">
            <Button
              variant="primary"
              loading={classificationBusy}
              disabled={classificationBusy}
              onclick={saveClassification}
            >
              <Icon name="check" size={17} /> {$t("common.save")}
            </Button>
            <Button
              variant="ghost"
              loading={classificationBusy}
              disabled={classificationBusy || !classificationConfig.enabled}
              onclick={() => classifyPending(false)}
            >
              <Icon name="sparkles" size={17} />
              {$t("settings.classificationClassifyPending")}
            </Button>
            <Button
              variant="ghost"
              loading={classificationBusy}
              disabled={classificationBusy || !classificationConfig.enabled}
              onclick={() => classifyPending(true)}
            >
              <Icon name="sync" size={17} />
              {$t("settings.classificationReclassify")}
            </Button>
            <Button
              variant="ghost"
              loading={classificationBusy}
              disabled={classificationBusy}
              onclick={applyBaseCategories}
            >
              <Icon name="plus" size={17} />
              {$t("settings.classificationSeedApply")}
            </Button>
          </div>
        </section>
      {/if}
    {:else}
      <Spinner label={$t("common.loading")} />
    {/if}
  {:else if tab === "sync"}
    <section class="panel">
      <h2>{$t("sync.categories")}</h2>
      <p class="hint">{$t("settings.syncHelp")}</p>
      {#if devices.some((device) => !device.isLocal)}
        <label>
          <span>{$t("sync.shareWith")}</span>
          <select
            value={selectedDeviceId ?? ""}
            onchange={(event) => selectSyncDevice(event.currentTarget.value)}
          >
            {#each devices.filter((device) => !device.isLocal) as device (device.id)}
              <option value={device.id}>{device.name}</option>
            {/each}
          </select>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={syncAll} />
          <span>{$t("sync.syncAll")}</span>
        </label>
        {#if collections?.categories.length}
          <div class="category-list">
            {#each collections.categories as item (item.category.id)}
              <label class="check">
                <input
                  type="checkbox"
                  checked={syncAll || selectedCategories.includes(item.category.id)}
                  disabled={syncAll}
                  onchange={() => toggleCategory(item.category.id)}
                />
                <span>{item.category.name}</span>
              </label>
            {/each}
          </div>
        {/if}
        <Button
          variant="primary"
          loading={busy === "sync"}
          disabled={!selectedDeviceId}
          onclick={saveSyncCategories}
        >
          {$t("sync.save")}
        </Button>
      {:else}
        <p class="hint">{$t("sync.noPeers")}</p>
      {/if}
      <Button variant="ghost" onclick={() => onNavigate?.("sync")}>
        <Icon name="sync" /> {$t("settings.openSync")}
      </Button>
    </section>
  {:else if tab === "storage"}
    {#if usage}
      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="eyebrow">{$t("settings.storageEyebrow")}</p>
            <h2>{$t("settings.storageUsage")}</h2>
          </div>
          <strong class="storage-total">{formatBytes(usage.total)}</strong>
        </div>
        <div class="storage-bars">
          {#each ([
            ["settings.storageBooks", usage.library, "books"],
            ["settings.storageAudio", usage.audiobooks, "audio"],
            ["settings.storageVoices", usage.voices, "voices"],
            ["settings.storageModels", usage.models, "models"],
            ["settings.storageTrash", usage.trash, "trash"],
            ["settings.storageTemporary", usage.temporary, "temp"],
          ] as [string, number, string][]) as [key, value, tone] (key)}
            <div class="storage-row">
              <span>{$t(key as MessageKey)}</span>
              <div class="storage-track">
                <span
                  class="tone-{tone}"
                  style:width={`${usage.total > 0 ? Math.max(2, (value / usage.total) * 100) : 0}%`}
                ></span>
              </div>
              <strong>{formatBytes(value)}</strong>
            </div>
          {/each}
        </div>
        <div class="actions">
          <Button
            variant="primary"
            size="small"
            loading={backupBusy}
            onclick={createBackup}
          >
            <Icon name="download" size={15} /> {$t("settings.createBackup")}
          </Button>
          <Button
            variant="ghost"
            size="small"
            loading={restoreBusy}
            onclick={restoreBackup}
          >
            <Icon name="upload" size={15} /> {$t("settings.restoreBackup")}
          </Button>
          <Button variant="ghost" size="small" loading={busy === "temporary"} onclick={() => cleanStorage("temporary")}>
            <Icon name="download" size={15} /> {$t("settings.clearTemporary")}
          </Button>
          <Button variant="ghost" size="small" loading={busy === "trash"} onclick={() => cleanStorage("trash")}>
            <Icon name="trash" size={15} /> {$t("settings.emptyTrash")}
          </Button>
        </div>
        <p class="hint">{$t("settings.backupHelp")}</p>
      </section>
    {/if}

    <section class="panel">
      <h2>{$t("settings.storage")}</h2>
      <p class="hint">{$t("settings.storageHelp")}</p>
      {#if layout}
        <div class="path-grid">
          {#each ([
            ["settings.storageRoot", layout.root],
            ["settings.storageDatabase", layout.db],
            ["settings.storageAssets", layout.assets],
            ["settings.storageExports", layout.exports],
            ["settings.storageTrashPath", layout.trash],
            ["settings.storageLogs", layout.logs],
          ] as [MessageKey, string][]) as [label, value] (label)}
            <div class="path-card">
              <strong>{$t(label)}</strong>
              <code>{value}</code>
            </div>
          {/each}
        </div>
      {:else}
        <Spinner label={$t("common.loading")} />
      {/if}
    </section>

    <section class="panel">
      <h2>{$t("settings.semantic")}</h2>
      <p class="hint">
        {$t("settings.model")}: <code>{semantic?.model ?? "multilingual-e5-small"}</code> ·
        {semantic?.indexedBooks ?? 0}/{semantic?.totalBooks ?? 0}
      </p>
      {#if semantic?.lastError}
        <p class="error" role="alert">{semantic.lastError}</p>
      {/if}
      {#if semanticMessage}
        <p class="hint" role="status">{semanticMessage.message}</p>
      {/if}
      {#if semanticProgress}
        <div class="progress-row">
          <span class="progress-track">
            <span style:width={`${Math.round(semanticProgress.percent)}%`}></span>
          </span>
          <span class="hint">
            {semanticProgress.message || semanticProgress.title} ·
            {Math.round(semanticProgress.percent)}%
          </span>
        </div>
      {/if}
      <div class="actions">
        {#if semantic && !semantic.supported}
          <p class="hint">{$t("error.unsupported")}</p>
        {:else if !semantic?.available}
          <Button variant="primary" loading={busy === "semantic"} onclick={prepareSemantic}>
            {$t("settings.prepareModel")}
          </Button>
        {:else}
          <Button variant="ghost" loading={busy === "semantic"} onclick={reindexSemantic}>
            {$t("settings.reindex")}
          </Button>
        {/if}
      </div>
    </section>

    <section class="panel">
      <h2>{$t("settings.sync")}</h2>
      <p class="hint">
        {sync?.running ? $t("sync.active") : $t("sync.stopped")} · {account?.deviceCount ?? 0}
        {$t("account.devices").toLowerCase()}
      </p>
      <p class="hint">
        OCR: {features?.ocr ? $t("common.enabled") : $t("common.disabled")} · Embeddings:
        {features?.embeddings ? $t("common.enabled") : $t("common.disabled")}
      </p>
    </section>
  {:else}
    <section class="panel update-panel">
      <div class="panel-heading">
        <div>
          <p class="eyebrow">{$t("settings.updates")}</p>
          <h2>{$t("settings.updates")}</h2>
        </div>
        <span class="badge">
          v{appVersion || updateCheck?.currentVersion || "—"}
        </span>
      </div>
      <p class="hint">{$t("settings.updatesHelp")}</p>

      {#if updateCheck}
        {#if updateCheck.available}
          <div class="update-available">
            <strong>
              {$t("settings.updateAvailable", {
                version: updateCheck.latestVersion,
              })}
            </strong>
            {#if updateCheck.notes}
              <p class="hint">{updateCheck.notes}</p>
            {/if}
            <Button
              variant="primary"
              loading={updateDownloading}
              disabled={updateDownloading}
              onclick={installUpdate}
            >
              <Icon name="download" size={17} />
              {$t("settings.downloadInstall")}
            </Button>
          </div>
        {:else}
          <p class="hint">{$t("settings.upToDate")}</p>
        {/if}
      {/if}

      {#if updateError}
        <p class="error">{updateError}</p>
      {/if}

      <div class="actions">
        <Button
          variant="ghost"
          loading={updateChecking}
          disabled={updateChecking || updateDownloading}
          onclick={checkUpdates}
        >
          <Icon name="sync" size={17} />
          {updateChecking
            ? $t("settings.checkingUpdates")
            : $t("settings.checkUpdates")}
        </Button>
      </div>

      <p class="hint">
        {platform === "android"
          ? $t("settings.updateAndroidHint")
          : $t("settings.updateDesktopHint")}
      </p>

      <details class="update-advanced" bind:open={showUpdateAdvanced}>
        <summary>{$t("settings.updateManifest")}</summary>
        <label class="wide">
          <span>{$t("settings.updateManifest")}</span>
          <input
            bind:value={updateManifestUrl}
            onblur={persistUpdateManifest}
            placeholder={DEFAULT_UPDATE_MANIFEST_URL}
          />
        </label>
        <p class="hint">{$t("settings.updateManifestHelp")}</p>
      </details>
    </section>

    <section class="panel about">
      <img class="about-mark" src="/logo.png" alt="Kora" />
      <h2>Kora</h2>
      <p>{$t("settings.aboutHelp")}</p>
      <p class="hint">Rust + Tauri 2 + Svelte 5</p>
      <p class="hint">{$t("common.license")}: MIT</p>
      <Button variant="ghost" onclick={() => onNavigate?.("voice")}>
        <Icon name="voice" /> {$t("nav.voice")}
      </Button>
    </section>
  {/if}
</section>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 1.35rem;
  }

  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
  }

  .tabs {
    display: inline-flex;
    flex-wrap: wrap;
    align-self: flex-start;
    gap: 0.2rem;
    max-width: 100%;
    padding: 0.25rem;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
  }

  .tabs button {
    min-height: 36px;
    padding: 0.35rem 0.82rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.78rem;
    font-weight: 500;
    white-space: nowrap;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    background: var(--surface);
    color: var(--text);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 1.05rem;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .panel h2 {
    margin: 0;
    font-size: 1rem;
  }

  .hint,
  .error {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .progress-row {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    flex-wrap: wrap;
  }

  .progress-track {
    flex: 1;
    min-width: 120px;
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-2);
  }

  .progress-track > span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 180ms ease;
  }

  .error {
    color: var(--danger);
  }

  .theme-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(148px, 1fr));
    gap: 0.6rem;
  }

  .theme-card {
    position: relative;
    min-height: 116px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.4rem;
    padding: 0.8rem;
    border: 2px solid transparent;
    border-radius: 13px;
    font: inherit;
    text-align: left;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, #000 8%, transparent);
    transition:
      border-color 150ms ease,
      transform 150ms ease,
      box-shadow 150ms ease;
  }

  .theme-card:hover {
    transform: translateY(-1px);
  }

  .theme-card.selected {
    border-color: var(--accent);
    box-shadow:
      0 0 0 3px var(--accent-soft),
      inset 0 0 0 1px color-mix(in srgb, #000 8%, transparent);
  }

  .theme-dot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 14%, transparent);
  }

  .theme-card strong {
    font-family: var(--font-display);
    font-size: 0.93rem;
    font-weight: 600;
  }

  .theme-preview {
    display: flex;
    gap: 0.25rem;
    margin-top: auto;
  }

  .theme-preview i {
    width: 32px;
    height: 9px;
    border-radius: 999px;
  }

  .theme-check {
    position: absolute;
    top: 0.6rem;
    right: 0.6rem;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .custom-tag {
    font-size: 0.6rem;
    letter-spacing: 0.04em;
    opacity: 0.7;
    text-transform: uppercase;
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 0.6rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    color: var(--muted);
    font-size: 0.74rem;
  }

  label
    input:not([type="color"]):not([type="checkbox"]):not([type="radio"]):not(
      [type="range"]
    ),
  label select {
    min-height: 40px;
    padding: 0.48rem 0.58rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
  }

  input[type="color"] {
    width: 100%;
    min-height: 42px;
    padding: 0.2rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
  }

  .check {
    flex-direction: row;
    align-items: center;
    gap: 0.55rem;
    min-height: 40px;
    padding: 0.15rem 0;
    color: var(--text);
    cursor: pointer;
  }

  .check input {
    flex: 0 0 auto;
    width: 18px;
    height: 18px;
    min-width: 18px;
    min-height: 18px;
    margin: 0;
  }

  .contrast-line {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    color: var(--muted);
    font-size: 0.76rem;
  }

  .contrast-line .low {
    color: var(--danger);
  }

  .theme-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .custom-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .custom-row {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .custom-row strong {
    flex: 1;
    font-size: 0.82rem;
  }

  .swatch {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
  }

  .swatch i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .texture-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 0.5rem;
  }

  .texture-card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.72rem;
    text-align: left;
  }

  .texture-card:hover {
    border-color: var(--accent-line);
  }

  .texture-card.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .texture-swatch {
    height: 56px;
    border-radius: 7px;
    background-color: var(--surface);
    background-image: var(--app-texture, none);
    background-position: var(--app-texture-position, 0 0);
    background-size: var(--app-texture-size, auto);
    background-repeat: repeat;
    border: 1px solid var(--border);
  }

  .language-row,
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
  }

  .card-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 0.8rem;
  }

  .category-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .path-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.45rem;
  }

  .path-card {
    min-width: 0;
    padding: 0.6rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .path-card strong {
    display: block;
    margin-bottom: 0.2rem;
    font-size: 0.74rem;
  }

  .path-card code {
    display: block;
    overflow-wrap: anywhere;
    color: var(--muted);
    font-size: 0.69rem;
  }

  .update-panel .panel-heading {
    align-items: center;
  }

  .update-available {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.55rem;
    padding: 0.85rem;
    border: 1px solid var(--accent-line);
    border-radius: 12px;
    background: var(--accent-soft);
  }

  .update-available strong {
    color: var(--accent);
    font-size: 0.9rem;
  }

  .update-advanced {
    margin-top: 0.15rem;
    padding-top: 0.65rem;
    border-top: 1px dashed var(--border);
  }

  .update-advanced summary {
    cursor: pointer;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .update-advanced label {
    margin-top: 0.55rem;
  }

  .about {
    align-items: flex-start;
  }

  .about-mark {
    width: 58px;
    height: 58px;
    display: block;
    border-radius: 16px;
    object-fit: cover;
    box-shadow: 0 10px 24px -14px color-mix(in srgb, var(--gold-deep) 60%, transparent);
  }

  @media (max-width: 720px) {
    .tabs {
      width: calc(100% + 32px);
      max-width: none;
      align-self: stretch;
      flex-wrap: wrap;
      justify-content: flex-start;
      gap: 0.35rem;
      margin-inline: -16px;
      padding: 0.3rem 16px;
      overflow: visible;
      border-inline: 0;
      border-radius: 0;
      background: transparent;
      scrollbar-width: none;
    }

    .tabs::-webkit-scrollbar {
      display: none;
    }

    .tabs button {
      flex: 0 0 auto;
      justify-content: center;
      padding-inline: 0.72rem;
      background: var(--surface-2);
    }

    .tabs button.active {
      background: var(--accent-soft);
      color: var(--accent);
    }

    .theme-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .theme-preview-panel {
      position: static;
      display: flex;
    }

    .preview-sheet {
      max-height: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .theme-card:hover {
      transform: none;
    }
  }

  .appearance-grid {
    display: grid;
    grid-template-columns: minmax(270px, 0.78fr) minmax(0, 1.22fr);
    align-items: start;
    gap: 16px;
  }

  .appearance-controls {
    grid-column: 2;
    grid-row: 1;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  .theme-preview-panel {
    grid-column: 1;
    grid-row: 1;
    position: sticky;
    top: calc(var(--topbar-height) + 18px);
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--surface-2);
  }

  .preview-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 2px;
  }

  .preview-head span {
    color: var(--muted);
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .preview-head small {
    color: var(--accent);
    font-size: 0.625rem;
    font-weight: 600;
  }

  .preview-sheet {
    padding: 24px 22px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
    color: var(--text);
    box-shadow: var(--shadow-sm);
  }

  .preview-kicker {
    margin: 0;
    color: var(--accent);
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  .preview-sheet h3 {
    margin: 5px 0 7px;
    font-family: var(--font-display);
    font-size: 1.65rem;
    font-style: italic;
    font-weight: 500;
    letter-spacing: -0.02em;
    line-height: 1.08;
  }

  .preview-rule {
    display: block;
    width: 34px;
    height: 2px;
    margin-bottom: 14px;
    background: var(--accent-2);
  }

  .preview-text {
    margin: 0;
    font-family: var(--font-reading);
    font-size: 0.92rem;
    line-height: 1.65;
  }

  .preview-quote {
    margin: 14px 0 0;
    padding: 9px 11px;
    border-left: 2px solid var(--accent-2);
    border-radius: 0 6px 6px 0;
    background: color-mix(in srgb, var(--accent) 6%, var(--surface-2));
    color: var(--muted);
    font-family: var(--font-reading);
    font-size: 0.82rem;
    font-style: italic;
    line-height: 1.45;
  }

  .preview-metrics {
    display: flex;
    gap: 6px;
  }

  .preview-metrics span {
    flex: 1;
    padding: 7px 8px;
    border-radius: 8px;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.55rem;
    text-align: center;
  }

  .preview-metrics strong {
    display: block;
    color: var(--text);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .theme-preview-panel :global(.ui-button) {
    justify-content: center;
    width: 100%;
  }

  @media (max-width: 1040px) {
    .appearance-grid {
      grid-template-columns: minmax(0, 1fr);
    }

    .appearance-controls,
    .theme-preview-panel {
      grid-column: 1;
      grid-row: auto;
    }

    .theme-preview-panel {
      display: none;
      position: static;
      top: auto;
    }
  }

  .ai-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(260px, 0.65fr);
    align-items: start;
    gap: 16px;
  }

  .ai-config {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .ai-status-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .ai-status-head h2 {
    margin: 3px 0 0;
    font-family: var(--font-display);
    font-size: 1.45rem;
    font-weight: 500;
  }

  .ai-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0.3rem 0.55rem;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--muted);
    font-size: 0.65rem;
    font-weight: 700;
    white-space: nowrap;
  }

  .ai-status i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
  }

  .ai-status.online {
    background: color-mix(in srgb, var(--ok) 12%, var(--surface));
    color: var(--ok);
  }

  .ai-status.online i {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 14%, transparent);
  }

  .provider-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .provider-grid button {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 44px;
    padding: 0.55rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
    text-align: left;
  }

  .provider-grid button:hover {
    border-color: var(--accent-line);
    color: var(--text);
  }

  .provider-grid button.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .provider-grid strong {
    font-weight: 600;
  }

  .form-grid label.wide {
    grid-column: 1 / -1;
  }

  .form-grid input[type="range"] {
    min-height: 32px;
    padding: 0;
    border: 0;
    background: transparent;
  }

  .ai-detected {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    margin: 0;
    padding: 8px 10px;
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.65rem;
  }

  .ai-detected strong {
    color: var(--text);
    font-size: 0.7rem;
  }

  .ai-guide {
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--surface-2);
  }

  .ai-guide h2 {
    font-family: var(--font-display);
    font-size: 1.2rem;
    font-weight: 500;
  }

  .ai-guide-block {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
  }

  .ai-guide-block strong {
    color: var(--accent);
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .ai-guide-block code {
    overflow-x: auto;
    color: var(--muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.62rem;
    white-space: nowrap;
  }

  @media (max-width: 900px) {
    .ai-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .ai-guide {
      order: 2;
    }
  }

  @media (max-width: 560px) {
    .provider-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .wellbeing-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
    align-items: start;
    gap: 16px;
  }

  .comfort-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .comfort-panel h2 {
    font-family: var(--font-display);
    font-size: 1.25rem;
    font-weight: 500;
  }

  .panel-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .comfort-preview {
    display: inline-flex;
    flex: 0 0 auto;
    gap: 3px;
    padding: 4px;
    border-radius: 8px;
    background: var(--surface-2);
  }

  .comfort-preview i {
    width: 26px;
    height: 26px;
    border-radius: 5px;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--border);
  }

  .comfort-panel input[type="range"] {
    width: 100%;
    min-height: 32px;
    margin: 0;
    accent-color: var(--accent);
  }

  .comfort-panel .check span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .comfort-panel .check strong {
    color: var(--text);
    font-size: 0.8rem;
    font-weight: 600;
  }

  .comfort-panel .check small {
    color: var(--muted);
    font-size: 0.65rem;
    line-height: 1.35;
  }

  .scale-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    padding-top: 3px;
  }

  .scale-row span {
    flex: 1;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .scale-row button {
    min-width: 44px;
    min-height: 30px;
    padding: 0.2rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.65rem;
    font-weight: 600;
  }

  .scale-row button.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  @media (max-width: 900px) {
    .wellbeing-layout {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .storage-total {
    color: var(--accent);
    font-family: var(--font-display);
    font-size: 1.45rem;
    font-weight: 500;
  }

  .storage-bars {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .storage-row {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr) 64px;
    align-items: center;
    gap: 8px;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .storage-track {
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .storage-track span {
    display: block;
    height: 100%;
    min-width: 2px;
    border-radius: inherit;
    background: var(--accent);
  }

  .storage-track .tone-audio {
    background: var(--accent-2);
  }

  .storage-track .tone-voices {
    background: var(--accent-3);
  }

  .storage-track .tone-models {
    background: #545f73;
  }

  .storage-track .tone-trash {
    background: var(--warn);
  }

  .storage-track .tone-temp {
    background: var(--muted);
  }

  .storage-row strong {
    color: var(--text);
    font-size: 0.62rem;
    font-weight: 600;
    text-align: right;
  }

  .reading-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(280px, 0.85fr);
    align-items: start;
    gap: 16px;
  }

  .reading-controls {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .reading-controls h2 {
    font-family: var(--font-display);
    font-size: 1.35rem;
    font-weight: 500;
  }

  .reading-controls .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 3px;
    padding: 3px;
    border-radius: 8px;
    background: var(--surface-2);
  }

  .reading-controls .segmented button {
    min-height: 32px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
  }

  .reading-controls .segmented button.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .reading-controls .check span {
    flex-direction: row;
  }

  .reading-preview {
    position: sticky;
    top: calc(var(--topbar-height) + 18px);
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--surface-2);
  }

  .reading-sheet {
    padding: 22px 20px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
    color: var(--text);
    box-shadow: var(--shadow-sm);
  }

  .reading-sheet p {
    margin: 0 0 1em;
  }

  .reading-sheet p:first-of-type {
    margin-bottom: 6px;
  }

  .reading-kicker {
    color: var(--accent);
    font-family: var(--font-sans);
    font-size: 0.55rem !important;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .reading-sheet h3 {
    margin: 0 0 10px;
    font-family: inherit;
    font-size: 1.6em;
    font-style: italic;
    font-weight: 500;
    letter-spacing: -0.02em;
    line-height: 1.08;
  }

  .reading-quote {
    padding: 8px 10px;
    border-left: 2px solid var(--accent-2);
    border-radius: 0 6px 6px 0;
    background: var(--accent-soft);
    color: var(--muted);
    font-style: italic;
  }

  .reading-preview .hint {
    margin: 0;
  }

  @media (max-width: 1040px) {
    .reading-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .reading-preview {
      position: static;
    }
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 0.18rem 0.48rem;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--muted);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  /* El aviso de 1040px oculta la previsualización; en móvil se recupera
     para que funcione como la pantalla «Temas y tipografía» de Lumina. */
  @media (max-width: 720px) {
    .theme-preview-panel {
      display: flex;
    }
  }

  .mobile-account-card {
    display: none;
  }

  @media (max-width: 720px) {
    .mobile-account-card {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 10px 12px;
      border: 1px solid var(--border);
      border-radius: 14px;
      background: var(--surface);
      color: var(--text);
      text-align: left;
      box-shadow: var(--shadow-xs);
    }

    .mobile-account-avatar {
      display: grid;
      place-items: center;
      flex: 0 0 auto;
      width: 40px;
      height: 40px;
      overflow: hidden;
      border-radius: 50%;
      background: linear-gradient(145deg, var(--accent-2), var(--accent-3));
      color: #fff;
      font-family: var(--font-display);
      font-size: 1rem;
      font-weight: 600;
    }

    .mobile-account-avatar img {
      width: 100%;
      height: 100%;
      object-fit: cover;
    }

    .mobile-account-copy {
      display: flex;
      flex: 1;
      flex-direction: column;
      min-width: 0;
    }

    .mobile-account-copy strong {
      overflow: hidden;
      font-size: 0.84rem;
      font-weight: 650;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .mobile-account-copy small {
      overflow: hidden;
      color: var(--muted);
      font-size: 0.62rem;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }
</style>

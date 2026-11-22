<script lang="ts">
  import { documentDir, downloadDir } from "@tauri-apps/api/path";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import {
    createAccount,
    libraryImport,
    saveAppearance,
  } from "../lib/api";
  import {
    BUILTIN_THEMES,
    TEXTURES,
    customThemeDefinition,
  } from "../lib/appearance";
  import {
    setLocale,
    t,
    translate,
    type Locale,
    type MessageKey,
  } from "../lib/i18n";
  import { notifyError, notifySuccess } from "../lib/toast";
  import Button from "../lib/components/Button.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import type {
    AccountView,
    AppearanceState,
    AppTexture,
    BookSummary,
  } from "../lib/types";

  let {
    initial,
    account,
    onChanged,
    onDone,
  }: {
    initial: AppearanceState;
    account: AccountView | null;
    onChanged: (state: AppearanceState) => void;
    onDone: () => void;
  } = $props();

  let step = $state(0);
  // La onboarding captura los valores iniciales; los cambios posteriores se
  // persisten y se reflejan al terminar.
  // svelte-ignore state_referenced_locally
  let themeId = $state(initial.preferences.themeId);
  // svelte-ignore state_referenced_locally
  let texture = $state<AppTexture>(initial.preferences.texture);
  // svelte-ignore state_referenced_locally
  let locale = $state<Locale>(initial.preferences.locale);
  let username = $state("");
  let displayName = $state("");
  let pin = $state("");
  let importing = $state(false);
  let imported = $state<BookSummary | null>(null);
  let pathValue = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let dropActive = $state(false);

  const themes = $derived([
    ...BUILTIN_THEMES,
    ...initial.themes.map(customThemeDefinition),
  ]);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (step !== 2) return;
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          dropActive = true;
        } else if (payload.type === "leave") {
          dropActive = false;
        } else if (payload.type === "drop") {
          dropActive = false;
          const path = payload.paths.find((candidate) =>
            /\.(epub|pdf)$/i.test(candidate),
          );
          if (path) void importPath(path);
          else notifyError(new Error(translate("onboarding.dropInvalid")));
        }
      })
      .then((stop) => {
        unlisten = stop;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  });

  async function persist(onboardingDone = false) {
    const state = await saveAppearance(
      themeId,
      texture,
      locale,
      onboardingDone,
      initial.preferences.libraryLayout,
    );
    onChanged(state);
    return state;
  }

  async function selectTheme(id: string) {
    themeId = id;
    setLocale(locale);
    await persist(false).catch((cause) => {
      error = String(cause);
    });
  }

  async function selectLocale(next: Locale) {
    locale = next;
    setLocale(next);
    await persist(false).catch((cause) => {
      error = String(cause);
    });
  }

  async function selectTexture(next: AppTexture) {
    texture = next;
    await persist(false).catch((cause) => {
      error = String(cause);
    });
  }

  async function createLocalAccount() {
    if (!username.trim() || !displayName.trim() || !pin.trim()) {
      error = $t("account.createHelp");
      return;
    }
    busy = true;
    error = null;
    try {
      const created = await createAccount(username.trim(), displayName.trim(), pin);
      window.dispatchEvent(
        new CustomEvent("kora:account-changed", { detail: created.account }),
      );
      notifySuccess($t("account.create"), $t("account.createHelp"));
      step = 2;
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = false;
    }
  }

  async function chooseFile(defaultPath?: string) {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "EPUB/PDF", extensions: ["epub", "pdf"] }],
        defaultPath,
      });
      if (typeof selected === "string") {
        pathValue = selected;
        await importPath(selected);
      }
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function chooseFrom(folder: "documents" | "downloads") {
    try {
      const path =
        folder === "documents" ? await documentDir() : await downloadDir();
      await chooseFile(path);
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function importPath(path: string) {
    if (!path.trim()) return;
    importing = true;
    error = null;
    try {
      imported = await libraryImport(path.trim());
      notifySuccess($t("library.import"), imported.title);
      pathValue = "";
    } catch (cause) {
      notifyError(cause);
    } finally {
      importing = false;
    }
  }

  async function finish() {
    busy = true;
    try {
      await persist(true);
      onDone();
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = false;
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
</script>

<section class="onboarding">
  <div class="onboarding-shell">
    <aside class="showcase">
      <div class="brand">
        <img class="brand-logo" src="/logo.png" alt="" />
        <span>
          <strong>Kora</strong>
          <small>{$t("app.tagline")}</small>
        </span>
      </div>

      <div class="showcase-copy">
        <p class="eyebrow">{$t("app.localFirst")}</p>
        <h2>{$t("onboarding.subtitle")}</h2>
      </div>

      <ul class="showcase-list">
        <li>
          <span class="showcase-icon"><Icon name="library" size={18} /></span>
          <div>
            <strong>{$t("library.title")}</strong>
            <small>{$t("library.subtitle")}</small>
          </div>
        </li>
        <li>
          <span class="showcase-icon"><Icon name="voice" size={18} /></span>
          <div>
            <strong>{$t("voice.title")}</strong>
            <small>{$t("voice.subtitle")}</small>
          </div>
        </li>
        <li>
          <span class="showcase-icon"><Icon name="sync" size={18} /></span>
          <div>
            <strong>{$t("sync.title")}</strong>
            <small>{$t("sync.subtitle")}</small>
          </div>
        </li>
      </ul>

      <p class="showcase-foot">Kora · {$t("app.localFirst")}</p>
    </aside>

    <div class="onboarding-card">
      <div class="progress" aria-hidden="true">
        {#each [0, 1, 2, 3] as index}
          <span class:active={step >= index}></span>
        {/each}
      </div>

      <div class="step-head">
        <p class="eyebrow">{$t("onboarding.step", { current: step + 1, total: 4 })}</p>
        {#if step === 0}
          <Button variant="primary" size="small" onclick={() => (step = 1)}>
            {$t("common.next")}
          </Button>
        {/if}
      </div>

    {#if step === 0}
      <h1>{$t("onboarding.welcome")}</h1>
      <p class="lead">{$t("onboarding.subtitle")}</p>

      <div class="section">
        <h2>{$t("onboarding.languageTitle")}</h2>
        <p class="hint">{$t("onboarding.languageHelp")}</p>
        <div class="choice-row">
          <Button
            variant={locale === "es" ? "primary" : "ghost"}
            onclick={() => selectLocale("es")}
          >
            <Icon name="globe" /> Español
          </Button>
          <Button
            variant={locale === "en" ? "primary" : "ghost"}
            onclick={() => selectLocale("en")}
          >
            <Icon name="globe" /> English
          </Button>
        </div>
      </div>

      <div class="section">
        <h2>{$t("onboarding.themeTitle")}</h2>
        <p class="hint">{$t("onboarding.themeHelp")}</p>
        <div class="theme-grid">
          {#each themes as theme (theme.id)}
            <button
              class="theme-card"
              class:selected={themeId === theme.id}
              aria-pressed={themeId === theme.id}
              style:background={theme.colors.background}
              style:color={theme.colors.text}
              onclick={() => selectTheme(theme.id)}
            >
              <span class="theme-dot" style:background={theme.colors.accent}></span>
              <strong>{themeLabel(theme.id)}</strong>
              <span class="theme-preview">
                <i style:background={theme.colors.surface}></i>
                <i style:background={theme.colors.accent}></i>
              </span>
              {#if themeId === theme.id}
                <span class="theme-check"><Icon name="check" size={14} /></span>
              {/if}
            </button>
          {/each}
        </div>
        <div class="texture-grid">
          {#each TEXTURES as item (item.id)}
            <button
              class="texture-card"
              class:selected={texture === item.id}
              aria-pressed={texture === item.id}
              onclick={() => selectTexture(item.id)}
            >
              <span class="texture-swatch" data-texture={item.id}></span>
              <span>{textureLabel(item.id)}</span>
            </button>
          {/each}
        </div>
      </div>

    {:else if step === 1}
      <h1>{$t("onboarding.accountTitle")}</h1>
      <p class="lead">{$t("onboarding.accountHelp")}</p>
      {#if account}
        <div class="account-existing">
          <Icon name="account" />
          <div>
            <strong>@{account.username}</strong>
            <span>{account.displayName}</span>
          </div>
        </div>
        <Button variant="primary" onclick={() => (step = 2)}>
          {$t("common.next")}
        </Button>
      {:else}
        <div class="form-grid">
          <label>
            <span>{$t("onboarding.username")}</span>
            <input bind:value={username} maxlength="32" autocomplete="username" />
          </label>
          <label>
            <span>{$t("onboarding.displayName")}</span>
            <input bind:value={displayName} maxlength="80" />
          </label>
          <label>
            <span>{$t("onboarding.pin")}</span>
            <input
              type="password"
              bind:value={pin}
              maxlength="12"
              inputmode="numeric"
              autocomplete="new-password"
            />
          </label>
        </div>
        <p class="hint">{$t("onboarding.usernameHelp")}</p>
        <div class="choice-row">
          <Button variant="primary" loading={busy} onclick={createLocalAccount}>
            {$t("onboarding.createAccount")}
          </Button>
          <Button variant="ghost" onclick={() => (step = 2)}>
            {$t("onboarding.withoutAccount")}
          </Button>
        </div>
      {/if}
    {:else if step === 2}
      <h1>{$t("onboarding.importTitle")}</h1>
      <p class="lead">{$t("onboarding.importHelp")}</p>
      <div class="import-zone" class:drop-active={dropActive}>
        <div class="import-row">
          <input
            bind:value={pathValue}
            placeholder={$t("onboarding.pathPlaceholder")}
            aria-label={$t("onboarding.importTitle")}
            onkeydown={(event) => {
              if (event.key === "Enter") void importPath(pathValue);
            }}
          />
          <Button variant="primary" onclick={() => importPath(pathValue)} loading={importing}>
            {$t("library.import")}
          </Button>
        </div>
        <div class="folder-row">
          <Button variant="ghost" onclick={() => chooseFrom("documents")}>
            <Icon name="folder" /> {$t("onboarding.documents")}
          </Button>
          <Button variant="ghost" onclick={() => chooseFrom("downloads")}>
            <Icon name="folder" /> {$t("onboarding.downloads")}
          </Button>
          <Button variant="ghost" onclick={() => chooseFile()}>
            <Icon name="folder" /> {$t("onboarding.chooseFile")}
          </Button>
        </div>
        <p class="hint drop-hint">
          {dropActive
            ? $t("onboarding.dropActive")
            : $t("onboarding.dropHint")}
        </p>
      </div>
      {#if imported}
        <p class="ok" role="status">{$t("common.ready")}: {imported.title}</p>
      {/if}
      <div class="choice-row">
        <Button variant="ghost" onclick={() => (step = 3)}>
          {$t("onboarding.skipImport")}
        </Button>
        {#if imported}
          <Button variant="primary" onclick={() => (step = 3)}>
            {$t("common.next")}
          </Button>
        {/if}
      </div>
    {:else}
      <h1>{$t("onboarding.doneTitle")}</h1>
      <p class="lead">{$t("onboarding.doneHelp")}</p>
      <ul class="summary">
        <li><Icon name="check" /> {$t("common.theme")}: {themeLabel(themeId)}</li>
        <li><Icon name="check" /> {$t("common.texture")}: {textureLabel(texture)}</li>
        <li><Icon name="check" /> {$t("common.language")}: {locale === "es" ? "Español" : "English"}</li>
      </ul>
      <Button variant="primary" loading={busy} onclick={finish}>
        {$t("onboarding.enter")}
      </Button>
    {/if}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    </div>
  </div>
</section>

<style>
  .onboarding {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: clamp(1rem, 3vw, 2rem);
    background:
      radial-gradient(
        1100px 600px at 12% -8%,
        color-mix(in srgb, var(--accent) 16%, transparent),
        transparent 68%
      ),
      radial-gradient(
        900px 500px at 100% 100%,
        color-mix(in srgb, var(--accent-2) 12%, transparent),
        transparent 66%
      ),
      var(--bg);
  }

  .onboarding-shell {
    width: min(1120px, 100%);
    display: grid;
    grid-template-columns: minmax(300px, 0.82fr) minmax(0, 1.18fr);
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 28px;
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .showcase {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 1.35rem;
    overflow: hidden;
    padding: 2rem;
    color: #f6f8f3;
    background:
      radial-gradient(
        520px 320px at 82% 10%,
        rgba(245, 158, 11, 0.3),
        transparent 70%
      ),
      radial-gradient(
        380px 380px at 15% 90%,
        rgba(234, 88, 12, 0.2),
        transparent 72%
      ),
      linear-gradient(155deg, #2a0e07, #7c2d12 58%, #9a3412);
  }

  .showcase::after {
    content: "";
    position: absolute;
    left: -90px;
    bottom: -130px;
    width: 340px;
    height: 340px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow:
      0 0 0 38px rgba(255, 255, 255, 0.04),
      0 0 0 82px rgba(255, 255, 255, 0.025);
    pointer-events: none;
  }

  .showcase > * {
    position: relative;
    z-index: 1;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }

  .brand-logo {
    width: 44px;
    height: 44px;
    flex: 0 0 auto;
    display: block;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 13px;
    object-fit: cover;
    box-shadow: 0 10px 22px -12px rgba(0, 0, 0, 0.6);
  }

  .brand span:last-child {
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
    min-width: 0;
  }

  .brand strong {
    font-family: var(--font-display);
    font-size: 1.2rem;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .brand small {
    color: rgba(255, 255, 255, 0.62);
    font-size: 0.63rem;
    line-height: 1.3;
  }

  .showcase-copy {
    margin-top: clamp(0.5rem, 5vh, 2.5rem);
  }

  .showcase-copy .eyebrow {
    color: rgba(255, 255, 255, 0.6);
  }

  .showcase-copy h2 {
    margin: 0.5rem 0 0;
    color: #f6f8f3;
    font-family: var(--font-display);
    font-size: clamp(1.35rem, 2.4vw, 1.9rem);
    font-weight: 600;
    line-height: 1.25;
    letter-spacing: -0.02em;
  }

  .showcase-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin: 0.5rem 0 0;
    padding: 0;
  }

  .showcase-list li {
    display: flex;
    align-items: flex-start;
    gap: 0.65rem;
  }

  .showcase-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.1);
  }

  .showcase-list div {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .showcase-list strong {
    font-size: 0.8rem;
    font-weight: 600;
  }

  .showcase-list small {
    color: rgba(255, 255, 255, 0.62);
    font-size: 0.7rem;
    line-height: 1.45;
  }

  .showcase-foot {
    margin: auto 0 0;
    padding-top: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.14);
    color: rgba(255, 255, 255, 0.55);
    font-size: 0.64rem;
    letter-spacing: 0.04em;
  }

  .onboarding-card {
    max-height: min(92vh, 940px);
    overflow-y: auto;
    padding: clamp(1.3rem, 4vw, 2.5rem);
  }

  .progress {
    display: flex;
    gap: 0.35rem;
    margin-bottom: 1.15rem;
  }

  .progress span {
    flex: 1;
    height: 4px;
    border-radius: 999px;
    background: var(--surface-2);
  }

  .progress span.active {
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .eyebrow {
    color: var(--muted);
  }

  h1 {
    margin: 0.35rem 0 0.45rem;
    font-family: var(--font-display);
    font-size: clamp(1.55rem, 4vw, 2.1rem);
    font-weight: 650;
    line-height: 1.12;
    letter-spacing: -0.025em;
  }

  h2 {
    margin: 0;
    font-size: 0.96rem;
  }

  .lead {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.6;
  }

  .section {
    margin-top: 1.45rem;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .choice-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .step-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .step-head .eyebrow {
    margin: 0;
  }

  .theme-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.55rem;
  }

  .theme-card {
    position: relative;
    min-height: 96px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.35rem;
    padding: 0.7rem;
    border: 2px solid transparent;
    border-radius: 12px;
    font: inherit;
    text-align: left;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
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
      inset 0 0 0 1px rgba(0, 0, 0, 0.08);
  }

  .theme-dot {
    width: 18px;
    height: 18px;
    border-radius: 50%;
  }

  .theme-card strong {
    font-family: var(--font-display);
    font-size: 0.82rem;
    font-weight: 600;
  }

  .theme-preview {
    display: flex;
    gap: 0.25rem;
    margin-top: auto;
  }

  .theme-preview i {
    width: 28px;
    height: 8px;
    border-radius: 999px;
  }

  .theme-check {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .texture-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.5rem;
    margin-top: 0.2rem;
  }

  .texture-card {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.45rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
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
    height: 44px;
    border-radius: 7px;
    background-color: var(--surface);
    background-image: var(--app-texture, none);
    background-position: var(--app-texture-position, 0 0);
    background-size: var(--app-texture-size, auto);
    background-repeat: repeat;
    border: 1px solid var(--border);
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.7rem;
    margin-top: 1.1rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.76rem;
  }

  label input {
    min-height: 42px;
    padding: 0.55rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
  }

  .account-existing {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    margin: 1rem 0;
    padding: 0.85rem;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface-2);
  }

  .account-existing div {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
  }

  .account-existing strong {
    font-size: 0.9rem;
  }

  .account-existing span {
    color: var(--muted);
    font-size: 0.78rem;
  }

  .import-zone {
    margin: 1rem 0 0.6rem;
    padding: 0.9rem;
    border: 1px dashed var(--border);
    border-radius: 16px;
    background:
      radial-gradient(
        380px 180px at 50% 0%,
        color-mix(in srgb, var(--accent) 7%, transparent),
        transparent 70%
      ),
      color-mix(in srgb, var(--surface-2) 54%, transparent);
    transition:
      border-color 160ms ease,
      background 160ms ease;
  }

  .import-zone.drop-active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .import-row {
    display: flex;
    gap: 0.45rem;
    margin: 0;
  }

  .folder-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 0.65rem;
  }

  .drop-hint {
    margin-top: 0.6rem;
  }

  .import-row input {
    flex: 1;
    min-width: 0;
    min-height: 42px;
    padding: 0.55rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
  }

  .summary {
    list-style: none;
    margin: 1.1rem 0 1.3rem;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .summary li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.84rem;
  }

  .summary :global(svg) {
    color: var(--accent);
  }

  .error {
    margin: 0.8rem 0 0;
    padding: 0.65rem 0.8rem;
    border: 1px solid color-mix(in srgb, var(--danger) 55%, var(--border));
    border-radius: 10px;
    background: color-mix(in srgb, var(--danger) 9%, transparent);
    font-size: 0.8rem;
  }

  .ok {
    margin: 0.6rem 0 0;
    color: var(--ok);
    font-size: 0.8rem;
  }

  @media (max-width: 900px) {
    .onboarding {
      place-items: start center;
      padding: 1rem;
    }

    .onboarding-shell {
      grid-template-columns: 1fr;
      max-width: 720px;
    }

    .showcase {
      gap: 0.8rem;
      padding: 1.2rem 1.35rem;
    }

    .showcase-copy {
      margin-top: 0.4rem;
    }

    .showcase-copy h2 {
      font-size: 1.18rem;
    }

    .showcase-list,
    .showcase-foot {
      display: none;
    }

    .onboarding-card {
      max-height: none;
    }
  }

  @media (max-width: 560px) {
    .onboarding {
      padding: 0;
    }

    .onboarding-shell {
      min-height: 100vh;
      border: 0;
      border-radius: 0;
    }

    .showcase {
      padding: 1rem 1.05rem;
    }

    .onboarding-card {
      padding: 1.1rem 1rem 1.4rem;
    }

    .import-row {
      flex-direction: column;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .theme-card:hover {
      transform: none;
    }
  }
</style>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import { formatDuration, t } from "../i18n";
  import type { AudiobookInfo, SharePermission, ShareView } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    audiobook = null,
    busy = false,
    error = null,
    result = null,
    onClose,
    onCreate,
  }: {
    open: boolean;
    audiobook?: AudiobookInfo | null;
    busy?: boolean;
    error?: string | null;
    result?: ShareView | null;
    onClose: () => void;
    onCreate: (
      pin: string,
      permission: SharePermission,
      expiresAt: string | null,
      maxUses: number | null,
    ) => Promise<void>;
  } = $props();

  let modalEl = $state<HTMLElement | null>(null);
  let wasOpen = false;
  let pin = $state("");
  let permission = $state<SharePermission>("copy");
  let days = $state("7");
  let maxUses = $state("");
  let localError = $state<string | null>(null);
  let copied = $state<"key" | "uri" | null>(null);

  let cover = $derived(
    audiobook?.coverPath ? convertFileSrc(audiobook.coverPath) : null,
  );
  let canCreate = $derived(
    Boolean(audiobook) && pin.trim().length > 0 && !busy && !result,
  );

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    pin = "";
    permission = "copy";
    days = "7";
    maxUses = "";
    localError = null;
    copied = null;
    void tick().then(() => modalEl?.focus());
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      onClose();
    }
  }

  async function create() {
    if (!canCreate) return;
    localError = null;
    const daysText = String(days).trim();
    const validDays = Number(daysText);
    const expiresAt =
      daysText && Number.isFinite(validDays) && validDays > 0
        ? new Date(Date.now() + validDays * 86_400_000).toISOString()
        : null;
    const usesText = String(maxUses).trim();
    const parsedUses = Number(usesText);
    try {
      await onCreate(
        pin,
        permission,
        expiresAt,
        usesText && Number.isFinite(parsedUses) && parsedUses > 0
          ? Math.floor(parsedUses)
          : null,
      );
      pin = "";
    } catch (cause) {
      localError = cause instanceof Error ? cause.message : String(cause);
    }
  }

  async function copy(value: string | null, field: "key" | "uri") {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      copied = field;
      setTimeout(() => {
        if (copied === field) copied = null;
      }, 1800);
    } catch {
      localError = $t("error.generic");
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div class="modal-backdrop">
    <button
      class="backdrop-button"
      type="button"
      onclick={onClose}
      aria-label={$t("common.close")}
    ></button>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="audiobook-share-title"
      tabindex="-1"
      bind:this={modalEl}
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="share" size={20} /></span>
          <div>
            <p class="eyebrow">{$t("audiobooks.share")}</p>
            <h2 id="audiobook-share-title">{$t("audiobooks.shareTitle")}</h2>
            <p class="head-help">{$t("audiobooks.shareHelp")}</p>
          </div>
        </div>
        <button
          class="icon-close"
          type="button"
          onclick={onClose}
          aria-label={$t("common.close")}
        >
          <Icon name="close" size={18} />
        </button>
      </header>

      <div class="modal-body">
        {#if audiobook}
          <section class="book-card">
            <span class="book-cover">
              {#if cover}
                <img src={cover} alt="" />
              {:else}
                <Icon name="headphones" size={20} />
              {/if}
            </span>
            <div class="book-copy">
              <strong>{audiobook.bookTitle}</strong>
              <span>{audiobook.author ?? "—"}</span>
              <small>
                {formatDuration(audiobook.durationMs)} · {audiobook.chapterCount}
                {$t("audiobooks.chapters").toLowerCase()}
              </small>
            </div>
            <span class="book-badge">{$t("audiobooks.shareIncludes")}</span>
          </section>
        {/if}

        {#if result?.key}
          <section class="result-card">
            <div class="result-head">
              <span class="result-icon"><Icon name="check" size={17} /></span>
              <div>
                <strong>{$t("audiobooks.shareKey")}</strong>
                <p>{$t("audiobooks.shareOnlyNow")}</p>
              </div>
            </div>

            <div class="share-grid">
              {#if result.qrSvg}
                <div class="qr" aria-hidden="true">{@html result.qrSvg}</div>
              {/if}
              <div class="share-values">
                <div class="value-row">
                  <span>{$t("audiobooks.shareKey")}</span>
                  <code>{result.key}</code>
                  <button
                    class="copy-button"
                    type="button"
                    onclick={() => void copy(result.key, "key")}
                  >
                    <Icon name={copied === "key" ? "check" : "edit"} size={13} />
                    {copied === "key"
                      ? $t("audiobooks.shareCopied")
                      : $t("audiobooks.shareCopyKey")}
                  </button>
                </div>
                {#if result.uri}
                  <div class="value-row">
                    <span>{$t("audiobooks.shareUri")}</span>
                    <code>{result.uri}</code>
                    <button
                      class="copy-button"
                      type="button"
                      onclick={() => void copy(result.uri, "uri")}
                    >
                      <Icon name={copied === "uri" ? "check" : "edit"} size={13} />
                      {copied === "uri"
                        ? $t("audiobooks.shareCopied")
                        : $t("audiobooks.shareCopyUri")}
                    </button>
                  </div>
                {/if}
              </div>
            </div>
            <p class="notice">
              <Icon name="sync" size={14} />
              {$t("audiobooks.shareHint")}
            </p>
          </section>
        {:else}
          <section class="form-card">
            <div class="form-row">
              <label>
                <span>{$t("audiobooks.sharePin")}</span>
                <input
                  type="password"
                  bind:value={pin}
                  maxlength="12"
                  inputmode="numeric"
                  autocomplete="current-password"
                  placeholder="••••"
                  onkeydown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void create();
                    }
                  }}
                />
                <small>{$t("audiobooks.sharePinHelp")}</small>
              </label>
              <label>
                <span>{$t("audiobooks.sharePermission")}</span>
                <select bind:value={permission}>
                  <option value="copy">{$t("audiobooks.shareCopy")}</option>
                  <option value="read">{$t("audiobooks.shareRead")}</option>
                </select>
              </label>
            </div>
            <div class="form-row">
              <label>
                <span>{$t("audiobooks.shareDays")}</span>
                <input
                  type="number"
                  min="1"
                  bind:value={days}
                  placeholder="7"
                />
              </label>
              <label>
                <span>{$t("audiobooks.shareMaxUses")}</span>
                <input
                  type="number"
                  min="1"
                  bind:value={maxUses}
                  placeholder={$t("account.unlimited")}
                />
              </label>
            </div>
            <div class="form-notes">
              <p>{$t("audiobooks.shareIncludes")}</p>
              <p>{$t("audiobooks.shareAccountNeeded")}</p>
              <p>{$t("audiobooks.shareLimit")}</p>
            </div>
          </section>
        {/if}

        {#if localError || error}
          <p class="error" role="alert">{localError ?? error}</p>
        {/if}
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose}>
          {result ? $t("common.close") : $t("common.cancel")}
        </Button>
        {#if !result}
          <Button onclick={() => void create()} loading={busy} disabled={!canCreate}>
            <Icon name="share" size={15} />
            {$t("audiobooks.shareCreate")}
          </Button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 1.25rem;
    background: color-mix(in srgb, #0f172a 42%, transparent);
    backdrop-filter: blur(6px);
  }

  .backdrop-button {
    position: absolute;
    inset: 0;
    border: 0;
    background: transparent;
    cursor: default;
  }

  .modal {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    width: min(680px, 100%);
    max-height: min(90vh, 860px);
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
    animation: modal-in 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding: 18px 20px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }

  .head-copy {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    min-width: 0;
  }

  .head-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .eyebrow {
    margin: 0 0 2px;
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 750;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.35rem;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .head-help {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.45;
  }

  .icon-close {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .icon-close:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .book-card {
    display: grid;
    grid-template-columns: 54px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 11px 12px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface-2);
  }

  .book-cover {
    display: grid;
    place-items: center;
    width: 54px;
    height: 54px;
    overflow: hidden;
    border-radius: 9px;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
  }

  .book-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .book-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .book-copy strong,
  .book-copy span,
  .book-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .book-copy strong {
    font-family: var(--font-display);
    font-size: 0.95rem;
    font-weight: 620;
  }

  .book-copy span {
    color: var(--muted);
    font-size: 0.72rem;
  }

  .book-copy small {
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 600;
  }

  .book-badge {
    max-width: 180px;
    color: var(--muted);
    font-size: 0.63rem;
    line-height: 1.4;
    text-align: right;
  }

  .form-card,
  .result-card {
    display: flex;
    flex-direction: column;
    gap: 13px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface-2);
  }

  .form-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .form-row label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .form-row label > span,
  .value-row > span {
    color: var(--muted);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  input,
  select {
    min-height: 38px;
    padding: 0.4rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
  }

  input:focus,
  select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }

  label small {
    color: var(--muted);
    font-size: 0.64rem;
    line-height: 1.35;
  }

  .form-notes {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-top: 9px;
    border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  }

  .form-notes p {
    margin: 0;
    color: var(--muted);
    font-size: 0.67rem;
    line-height: 1.45;
  }

  .result-head {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .result-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--success) 12%, var(--surface));
    color: var(--success);
  }

  .result-head strong {
    display: block;
    font-size: 0.82rem;
    font-weight: 650;
  }

  .result-head p {
    margin: 1px 0 0;
    color: var(--muted);
    font-size: 0.67rem;
  }

  .share-grid {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
    gap: 14px;
  }

  .qr {
    width: 142px;
    padding: 5px;
    border-radius: 10px;
    background: #fff;
  }

  .qr :global(svg) {
    display: block;
    width: 100%;
    height: auto;
  }

  .share-values {
    display: flex;
    flex-direction: column;
    gap: 9px;
    min-width: 0;
  }

  .value-row {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .value-row code {
    overflow-wrap: anywhere;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.72rem;
  }

  .copy-button {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 28px;
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    color: var(--accent);
    font: inherit;
    font-size: 0.65rem;
    font-weight: 650;
  }

  .copy-button:hover {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .notice {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    padding-top: 9px;
    border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.45;
  }

  .notice :global(svg) {
    flex: 0 0 auto;
    margin-top: 1px;
    color: var(--accent);
  }

  .error {
    margin: 0;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--danger) 30%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 7%, var(--surface));
    color: var(--danger);
    font-size: 0.74rem;
    line-height: 1.45;
  }

  .modal-foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 13px 20px calc(13px + var(--safe-bottom));
    border-top: 1px solid var(--border);
    background: var(--surface-2);
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (max-width: 640px) {
    .modal-backdrop {
      align-items: flex-end;
      padding: 0;
    }

    .modal {
      width: 100%;
      max-height: 94vh;
      border-radius: var(--radius-xl) var(--radius-xl) 0 0;
    }

    .modal-head,
    .modal-body {
      padding-left: 16px;
      padding-right: 16px;
    }

    .book-card {
      grid-template-columns: 48px minmax(0, 1fr);
    }

    .book-cover {
      width: 48px;
      height: 48px;
    }

    .book-badge {
      display: none;
    }

    .form-row,
    .share-grid {
      grid-template-columns: 1fr;
    }

    .qr {
      width: 122px;
      margin: 0 auto;
    }

    .modal-foot {
      padding: 12px 16px calc(12px + var(--safe-bottom));
    }

    .modal-foot :global(.ui-button) {
      flex: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal {
      animation: none;
    }
  }
</style>

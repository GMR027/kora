<script lang="ts">
  import { formatBytes, t } from "../i18n";
  import type { AudiobookInfo, WhisperModelInfo } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    audiobook = null,
    whisperModel = null,
    whisperTiny = null,
    whisperProgress = null,
    whisperTinyProgress = null,
    busy = false,
    error = null,
    onClose,
    onConfirm,
    onDownloadModel,
  }: {
    open: boolean;
    audiobook?: AudiobookInfo | null;
    whisperModel?: WhisperModelInfo | null;
    whisperTiny?: WhisperModelInfo | null;
    whisperProgress?: number | null;
    whisperTinyProgress?: number | null;
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onConfirm: (modelId: "tiny" | "base") => void;
    onDownloadModel: (modelId: "tiny" | "base") => void;
  } = $props();

  let modelId = $state<"tiny" | "base">("base");
  let wasOpen = false;

  let selectedModel = $derived(modelId === "tiny" ? whisperTiny : whisperModel);
  let selectedReady = $derived(selectedModel?.installed ?? false);
  let selectedProgress = $derived(
    modelId === "tiny" ? whisperTinyProgress : whisperProgress,
  );
  let downloading = $derived(selectedProgress !== null && selectedProgress !== undefined);
  let modelName = $derived(modelId === "tiny" ? "Whisper Tiny" : "Whisper Base");
  let modelSize = $derived(
    selectedModel?.sizeBytes
      ? formatBytes(selectedModel.sizeBytes)
      : modelId === "tiny"
        ? "74 MB"
        : "141 MB",
  );

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    modelId = whisperTiny?.installed && !whisperModel?.installed ? "tiny" : "base";
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && audiobook}
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
      aria-labelledby="audiobook-transcribe-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="book" size={20} /></span>
          <div>
            <h2 id="audiobook-transcribe-title">{$t("audiobooks.convertTitle")}</h2>
            <p>{audiobook.bookTitle}</p>
          </div>
        </div>
        <button
          class="close-button"
          type="button"
          onclick={onClose}
          aria-label={$t("common.close")}
          title={$t("common.close")}
        >
          <Icon name="close" size={18} />
        </button>
      </header>

      <div class="modal-body">
        {#if error}
          <p class="notice" role="alert">{error}</p>
        {/if}

        <p class="help">{$t("audiobooks.convertHelp")}</p>

        <div class="mode-section">
          <span class="field-label">{$t("audiobooks.convertModel")}</span>
          <div class="model-grid" role="radiogroup" aria-label={$t("audiobooks.convertModel")}>
            <button
              class="model-option"
              class:active={modelId === "tiny"}
              type="button"
              role="radio"
              aria-checked={modelId === "tiny"}
              onclick={() => (modelId = "tiny")}
              disabled={busy}
            >
              <strong>{$t("audiobooks.transcribeFast")}</strong>
              <small>{$t("audiobooks.transcribeFastHelp")}</small>
            </button>
            <button
              class="model-option"
              class:active={modelId === "base"}
              type="button"
              role="radio"
              aria-checked={modelId === "base"}
              onclick={() => (modelId = "base")}
              disabled={busy}
            >
              <strong>{$t("audiobooks.transcribePrecise")}</strong>
              <small>{$t("audiobooks.transcribePreciseHelp")}</small>
            </button>
          </div>
        </div>

        <div class="status-row" class:ok={selectedReady} class:missing={!selectedReady}>
          <span class="status-icon">
            <Icon name={selectedReady ? "check" : "close"} size={15} />
          </span>
          <div>
            <strong>{modelName}</strong>
            <small>
              {#if selectedReady}
                {$t("audiobooks.whisperInstalled")}
              {:else if downloading}
                {$t("audiobooks.whisperDownloading", {
                  percent: Math.round(selectedProgress ?? 0),
                })}
              {:else}
                {$t("audiobooks.whisperModelDownload", {
                  model: modelName,
                  size: modelSize,
                })}
              {/if}
            </small>
          </div>
          {#if !selectedReady}
            <button
              class="download-button"
              type="button"
              onclick={() => onDownloadModel(modelId)}
              disabled={downloading || busy}
            >
              {$t("voice.download")}
            </button>
          {/if}
        </div>

        {#if downloading && !selectedReady}
          <div class="download-track" aria-hidden="true">
            <span style={`width:${Math.round(selectedProgress ?? 0)}%`}></span>
          </div>
        {/if}

        {#if !selectedReady}
          <p class="notice soft" role="status">{$t("audiobooks.convertNeedsModel")}</p>
        {/if}
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={() => onConfirm(modelId)}
          loading={busy}
          disabled={busy || !selectedReady}
        >
          <Icon name="sparkles" size={16} />
          {busy ? $t("audiobooks.importing") : $t("audiobooks.convertStart")}
        </Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(18, 14, 10, 0.46);
    backdrop-filter: blur(6px);
    animation: fade-in 150ms ease;
  }

  .backdrop-button {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
  }

  .modal {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    width: min(560px, 100%);
    max-height: min(88vh, 760px);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: 0 36px 80px -40px rgba(15, 23, 42, 0.4);
    animation: rise-in 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 19px 20px 16px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background:
      radial-gradient(
        360px 150px at 0% 0%,
        color-mix(in srgb, var(--accent) 9%, transparent),
        transparent 70%
      ),
      var(--surface);
  }

  .head-copy {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    min-width: 0;
  }

  .head-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
    border: 1px solid var(--accent-line);
    border-radius: 11px;
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.12rem;
    font-weight: 620;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.74rem;
  }

  .close-button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .close-button:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .help {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .field-label {
    display: block;
    margin-bottom: 7px;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .model-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .model-option {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .model-option.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
  }

  .model-option strong {
    font-size: 0.78rem;
    font-weight: 650;
  }

  .model-option small {
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.35;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .status-row.ok {
    border-color: color-mix(in srgb, var(--success) 40%, var(--border));
  }

  .status-row.missing {
    border-color: color-mix(in srgb, var(--warning) 40%, var(--border));
  }

  .status-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 25px;
    height: 25px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
  }

  .status-row.ok .status-icon {
    background: color-mix(in srgb, var(--success) 14%, transparent);
    color: var(--success);
  }

  .status-row.missing .status-icon {
    background: color-mix(in srgb, var(--warning) 14%, transparent);
    color: var(--warning);
  }

  .status-row > div {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .status-row strong {
    font-size: 0.76rem;
    font-weight: 650;
  }

  .status-row small {
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.35;
  }

  .download-button {
    flex: 0 0 auto;
    min-height: 28px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.68rem;
    font-weight: 650;
  }

  .download-button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .download-button:disabled {
    opacity: 0.5;
  }

  .download-track {
    height: 4px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .download-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 180ms ease;
  }

  .notice {
    margin: 0;
    padding: 9px 11px;
    border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 7%, transparent);
    color: var(--danger);
    font-size: 0.75rem;
  }

  .notice.soft {
    border-color: var(--border);
    background: var(--surface-2);
    color: var(--muted);
  }

  .modal-foot {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding: 13px 20px;
    border-top: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background: var(--surface-2);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }

  @keyframes rise-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
  }

  @media (max-width: 640px) {
    .modal-backdrop {
      align-items: flex-end;
      padding: 0;
    }

    .modal {
      width: 100%;
      max-height: 92vh;
      border-radius: var(--radius-xl) var(--radius-xl) 0 0;
    }

    .model-grid {
      grid-template-columns: 1fr;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }
  }
</style>

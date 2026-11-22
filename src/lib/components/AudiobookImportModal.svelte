<script lang="ts">
  import { formatBytes, t } from "../i18n";
  import type {
    AiStatus,
    AudioImportMode,
    AudiobookCapabilities,
    WhisperModelInfo,
  } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  type ImportKind = "audio" | "transcribe";

  let {
    open,
    busy = false,
    capabilities = null,
    whisperModel = null,
    whisperTiny = null,
    whisperProgress = null,
    whisperTinyProgress = null,
    aiStatus = null,
    initialKind = "audio",
    error = null,
    onClose,
    onChoose,
    onDownloadWhisper,
    onDownloadWhisperModel,
  }: {
    open: boolean;
    busy?: boolean;
    capabilities?: AudiobookCapabilities | null;
    whisperModel?: WhisperModelInfo | null;
    whisperTiny?: WhisperModelInfo | null;
    whisperProgress?: number | null;
    whisperTinyProgress?: number | null;
    aiStatus?: AiStatus | null;
    initialKind?: ImportKind;
    error?: string | null;
    onClose: () => void;
    onChoose: (kind: ImportKind, detection: AudioImportMode) => void;
    onDownloadWhisper?: () => void;
    onDownloadWhisperModel?: (modelId: string) => void;
  } = $props();

  let kind = $state<ImportKind>("audio");
  let detection = $state<AudioImportMode>("aiFast");

  let supported = $derived(capabilities?.platformSupported ?? true);
  let ffmpegReady = $derived(capabilities?.ffmpeg ?? false);
  let whisperBaseReady = $derived(
    whisperModel?.installed ?? capabilities?.whisperBase ?? false,
  );
  let whisperTinyReady = $derived(whisperTiny?.installed ?? false);
  let analysisModelReady = $derived(whisperTinyReady || whisperBaseReady);
  let analysisModelLabel = $derived(
    whisperTinyReady ? "Tiny" : whisperBaseReady ? "Base" : "",
  );
  let aiReady = $derived(aiStatus?.available ?? false);
  let baseDownloading = $derived(
    whisperProgress !== null && whisperProgress !== undefined,
  );
  let tinyDownloading = $derived(
    whisperTinyProgress !== null && whisperTinyProgress !== undefined,
  );
  let ready = $derived(
    supported &&
      ffmpegReady &&
      (kind === "audio" || whisperBaseReady),
  );
  let whisperSize = $derived(
    whisperModel?.sizeBytes ? formatBytes(whisperModel.sizeBytes) : "141 MB",
  );
  function download(modelId: "tiny" | "base") {
    if (onDownloadWhisperModel) {
      onDownloadWhisperModel(modelId);
    } else if (modelId === "base") {
      onDownloadWhisper?.();
    }
  }

  $effect(() => {
    if (!open) return;
    kind = initialKind;
    detection = "aiFast";
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
      aria-labelledby="audiobook-import-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="upload" size={20} /></span>
          <div>
            <h2 id="audiobook-import-title">{$t("audiobooks.modalImportTitle")}</h2>
            <p>{$t("audiobooks.modalImportHelp")}</p>
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

        {#if !supported}
          <p class="notice" role="status">{$t("audiobooks.importUnavailable")}</p>
        {/if}

        <div class="format-strip">
          <span class="format-icon"><Icon name="headphones" size={23} /></span>
          <div>
            <strong>{$t("audiobooks.importFormats")}</strong>
            <p>{$t("audiobooks.importHelp")}</p>
          </div>
        </div>

        <div class="mode-section">
          <span class="field-label">{$t("audiobooks.importModeTitle")}</span>
          <div class="mode-grid" role="radiogroup" aria-label={$t("audiobooks.importModeTitle")}>
            <button
              class="mode-option"
              class:active={kind === "audio"}
              type="button"
              role="radio"
              aria-checked={kind === "audio"}
              onclick={() => (kind = "audio")}
              disabled={busy}
            >
              <span class="mode-icon"><Icon name="headphones" size={18} /></span>
              <span>
                <strong>{$t("audiobooks.importModeAudio")}</strong>
                <small>{$t("audiobooks.importModeAudioHelp")}</small>
              </span>
            </button>
            <button
              class="mode-option"
              class:active={kind === "transcribe"}
              type="button"
              role="radio"
              aria-checked={kind === "transcribe"}
              onclick={() => (kind = "transcribe")}
              disabled={busy}
            >
              <span class="mode-icon"><Icon name="book" size={18} /></span>
              <span>
                <strong>{$t("audiobooks.importModeTranscribe")}</strong>
                <small>{$t("audiobooks.importModeTranscribeHelp")}</small>
              </span>
            </button>
          </div>
        </div>

        {#if kind === "audio"}
          <div class="mode-section">
            <span class="field-label">{$t("audiobooks.detectionMode")}</span>
            <div
              class="detection-grid"
              role="radiogroup"
              aria-label={$t("audiobooks.detectionMode")}
            >
              <button
                class="detection-option"
                class:active={detection === "acoustic"}
                type="button"
                role="radio"
                aria-checked={detection === "acoustic"}
                onclick={() => (detection = "acoustic")}
                disabled={busy}
              >
                <strong>{$t("audiobooks.detectionAcoustic")}</strong>
                <small>{$t("audiobooks.detectionAcousticHelp")}</small>
              </button>
              <button
                class="detection-option"
                class:active={detection === "aiFast"}
                type="button"
                role="radio"
                aria-checked={detection === "aiFast"}
                onclick={() => (detection = "aiFast")}
                disabled={busy}
              >
                <strong>{$t("audiobooks.detectionFast")}</strong>
                <small>{$t("audiobooks.detectionFastHelp")}</small>
              </button>
              <button
                class="detection-option"
                class:active={detection === "aiPrecise"}
                type="button"
                role="radio"
                aria-checked={detection === "aiPrecise"}
                onclick={() => (detection = "aiPrecise")}
                disabled={busy}
              >
                <strong>{$t("audiobooks.detectionPrecise")}</strong>
                <small>{$t("audiobooks.detectionPreciseHelp")}</small>
              </button>
            </div>
          </div>
        {/if}

        <div class="status-list">
          <div class="status-row" class:ok={ffmpegReady} class:missing={!ffmpegReady}>
            <span class="status-icon">
              <Icon name={ffmpegReady ? "check" : "close"} size={15} />
            </span>
            <div>
              <strong>
                {ffmpegReady
                  ? $t("audiobooks.ffmpegReady")
                  : $t("audiobooks.ffmpegMissing")}
              </strong>
              {#if !ffmpegReady}
                <small>{$t("audiobooks.ffmpegMissingHelp")}</small>
              {/if}
            </div>
          </div>

          {#if kind === "transcribe"}
            <div class="status-row" class:ok={whisperBaseReady} class:missing={!whisperBaseReady}>
              <span class="status-icon">
                <Icon name={whisperBaseReady ? "check" : "close"} size={15} />
              </span>
              <div>
                <strong>{$t("audiobooks.whisperBase")}</strong>
                <small>
                  {#if whisperBaseReady}
                    {$t("audiobooks.whisperInstalled")}
                  {:else if baseDownloading}
                    {$t("audiobooks.whisperDownloading", {
                      percent: Math.round(whisperProgress ?? 0),
                    })}
                  {:else}
                    {$t("audiobooks.whisperDownload", { size: whisperSize })}
                  {/if}
                </small>
              </div>
              {#if !whisperBaseReady && supported}
                <button
                  class="download-button"
                  type="button"
                  onclick={() => download("base")}
                  disabled={baseDownloading}
                >
                  {$t("voice.download")}
                </button>
              {/if}
            </div>
          {:else}
            <div class="status-row" class:ok={aiReady}>
              <span class="status-icon">
                <Icon name={aiReady ? "check" : "waveform"} size={15} />
              </span>
              <div>
                <strong>
                  {aiReady
                    ? $t("audiobooks.aiAvailable", {
                        provider: aiStatus?.provider ?? "IA",
                        model: aiStatus?.model ?? "",
                      })
                    : $t("audiobooks.aiUnavailable")}
                </strong>
                <small>{$t("audiobooks.analysisFallbackHint")}</small>
              </div>
            </div>

            <div class="status-row" class:ok={analysisModelReady}>
              <span class="status-icon">
                <Icon name={analysisModelReady ? "check" : "close"} size={15} />
              </span>
              <div>
                <strong>
                  {analysisModelReady
                    ? $t("audiobooks.whisperAnalysisReady", {
                        model: analysisModelLabel,
                      })
                    : $t("audiobooks.whisperAnalysisMissing")}
                </strong>
                <small>
                  {#if analysisModelReady}
                    {$t("audiobooks.detectionFastHelp")}
                  {:else}
                    {$t("audiobooks.analysisFallbackHint")}
                  {/if}
                </small>
              </div>
              {#if !analysisModelReady && supported}
                <button
                  class="download-button"
                  type="button"
                  onclick={() => download("tiny")}
                  disabled={tinyDownloading}
                >
                  {$t("voice.download")}
                </button>
              {/if}
            </div>
          {/if}
        </div>

        {#if kind === "transcribe" && baseDownloading}
          <div class="download-track" aria-hidden="true">
            <span style={`width:${Math.round(whisperProgress ?? 0)}%`}></span>
          </div>
        {:else if kind === "audio" && tinyDownloading}
          <div class="download-track" aria-hidden="true">
            <span style={`width:${Math.round(whisperTinyProgress ?? 0)}%`}></span>
          </div>
        {/if}

        <p class="hint">
          {kind === "transcribe"
            ? $t("audiobooks.importRequires")
            : $t("audiobooks.chapterDetectionHelp")}
        </p>
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={() => onChoose(kind, detection)}
          loading={busy}
          disabled={busy || !ready}
        >
          <Icon name="folder" size={17} />
          {busy
            ? kind === "audio"
              ? $t("audiobooks.importAnalyzing")
              : $t("audiobooks.importing")
            : kind === "audio"
              ? $t("audiobooks.importAnalyze")
              : $t("audiobooks.importStart")}
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
    min-height: 0;
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
    max-height: min(90vh, 760px);
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
    background: var(--accent-soft);
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.25rem;
    font-weight: 620;
    letter-spacing: -0.018em;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.45;
  }

  .close-button {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    min-width: 34px;
    height: 34px;
    min-height: 34px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
  }

  .close-button:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .notice {
    margin: 0;
    padding: 0.62rem 0.78rem;
    border: 1px solid color-mix(in srgb, var(--danger) 50%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    font-size: 0.78rem;
  }

  .format-strip {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 1px dashed var(--border);
    border-radius: 12px;
    background:
      radial-gradient(
        240px 100px at 50% 0%,
        color-mix(in srgb, var(--accent) 8%, transparent),
        transparent 70%
      ),
      var(--surface-2);
  }

  .format-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 42px;
    height: 42px;
    border-radius: 11px;
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .format-strip strong {
    display: block;
    font-size: 0.8rem;
    font-weight: 700;
  }

  .format-strip p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.4;
  }

  .mode-section {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .field-label {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .mode-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .mode-option {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    text-align: left;
    font: inherit;
  }

  .mode-option.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .mode-option > span:last-child {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .mode-option strong {
    font-size: 0.74rem;
    font-weight: 700;
  }

  .mode-option small {
    color: var(--muted);
    font-size: 0.66rem;
    line-height: 1.35;
  }

  .mode-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: var(--surface);
    color: var(--accent);
  }

  .detection-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }

  .detection-option {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 9px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    text-align: left;
    font: inherit;
  }

  .detection-option.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .detection-option strong {
    font-size: 0.72rem;
  }

  .detection-option small {
    color: var(--muted);
    font-size: 0.64rem;
    line-height: 1.35;
  }

  @media (max-width: 640px) {
    .mode-grid,
    .detection-grid {
      grid-template-columns: 1fr;
    }
  }

  .status-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .status-row > div {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .status-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 25px;
    height: 25px;
    border-radius: 50%;
    background: var(--surface);
    color: var(--muted);
  }

  .status-row.ok .status-icon {
    background: color-mix(in srgb, var(--ok) 13%, var(--surface));
    color: var(--ok);
  }

  .status-row.missing .status-icon {
    background: color-mix(in srgb, var(--warn) 13%, var(--surface));
    color: var(--warn);
  }

  .status-row strong {
    font-size: 0.76rem;
    font-weight: 650;
  }

  .status-row small {
    color: var(--muted);
    font-size: 0.68rem;
  }

  .download-button {
    flex: 0 0 auto;
    min-height: 30px;
    padding: 0.24rem 0.65rem;
    border: 1px solid var(--accent-line);
    border-radius: 8px;
    background: var(--surface);
    color: var(--accent);
    font: inherit;
    font-size: 0.68rem;
    font-weight: 650;
  }

  .download-button:hover:not(:disabled) {
    background: var(--accent-soft);
  }

  .download-track {
    width: 100%;
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .download-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--gold), var(--accent-2));
    transition: width 220ms ease;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.45;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 9px;
    padding: 14px 20px;
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
      max-height: 94vh;
      border-radius: var(--radius-xl) var(--radius-xl) 0 0;
    }

    .modal-head {
      padding: 15px 16px 13px;
    }

    .modal-body {
      padding: 15px 16px;
    }

    .modal-foot {
      padding: 12px 16px calc(12px + env(safe-area-inset-bottom));
    }

    .modal-foot :global(.ui-button) {
      flex: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }
  }
</style>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import { formatDate, formatDuration, t } from "../i18n";
  import type {
    AudiobookChapterInfo,
    AudiobookInfo,
    VoiceMarkerStatus,
    VoiceNote,
  } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    audiobook = null,
    chapters = [],
    markers = [],
    status = null,
    phrase,
    whisperDownloading = null,
    busy = false,
    canListen = false,
    externalError = null,
    onClose,
    onSavePhrase,
    onToggleListening,
    onDownloadWhisper,
    onJump,
    onDelete,
  }: {
    open: boolean;
    audiobook?: AudiobookInfo | null;
    chapters?: AudiobookChapterInfo[];
    markers?: VoiceNote[];
    status?: VoiceMarkerStatus | null;
    phrase: string;
    whisperDownloading?: number | null;
    busy?: boolean;
    canListen?: boolean;
    externalError?: string | null;
    onClose: () => void;
    onSavePhrase: (phrase: string) => Promise<void>;
    onToggleListening: () => Promise<void>;
    onDownloadWhisper: () => Promise<void>;
    onJump: (positionMs: number) => void;
    onDelete: (id: string) => Promise<void>;
  } = $props();

  let modalEl = $state<HTMLElement | null>(null);
  let wasOpen = false;
  let phraseDraft = $state("");
  let savedPhrase = $state("");
  let savingPhrase = $state(false);
  let deletingId = $state<string | null>(null);
  let localError = $state<string | null>(null);
  let savedNotice = $state(false);

  let listening = $derived(status?.listening ?? false);
  let supported = $derived(status?.supported ?? true);
  let modelReady = $derived(status?.modelReady ?? false);
  let modelLabel = $derived(
    status?.modelId
      ? `Whisper ${status.modelId.charAt(0).toUpperCase()}${status.modelId.slice(1)}`
      : "Whisper",
  );
  let dirty = $derived(phraseDraft.trim() !== savedPhrase.trim());
  let ordered = $derived(
    [...markers].sort((left, right) => left.positionMs - right.positionMs),
  );
  let canToggle = $derived(
    supported && modelReady && canListen && Boolean(audiobook) && !busy,
  );

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    phraseDraft = phrase;
    savedPhrase = phrase;
    localError = null;
    savedNotice = false;
    void tick().then(() => modalEl?.focus());
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      onClose();
    }
  }

  async function savePhrase() {
    if (savingPhrase || !dirty) return;
    const candidate = phraseDraft.trim();
    if (candidate.split(/\s+/).filter(Boolean).length < 2) {
      localError = $t("audiobooks.voiceCommandInvalid");
      return;
    }
    localError = null;
    savingPhrase = true;
    try {
      await onSavePhrase(candidate);
      savedPhrase = candidate;
      savedNotice = true;
      setTimeout(() => (savedNotice = false), 1800);
    } catch (cause) {
      localError = cause instanceof Error ? cause.message : String(cause);
    } finally {
      savingPhrase = false;
    }
  }

  async function resetPhrase() {
    const fallback = $t("audiobooks.voiceCommandDefault");
    phraseDraft = fallback;
    await savePhrase();
  }

  async function toggleListening() {
    if (!canToggle && !listening) return;
    localError = null;
    try {
      await onToggleListening();
    } catch (cause) {
      localError = cause instanceof Error ? cause.message : String(cause);
    }
  }

  async function downloadWhisper() {
    localError = null;
    try {
      await onDownloadWhisper();
    } catch (cause) {
      localError = cause instanceof Error ? cause.message : String(cause);
    }
  }

  async function removeMarker(marker: VoiceNote) {
    if (!window.confirm($t("audiobooks.voiceMarkerDeleteConfirm"))) return;
    deletingId = marker.id;
    localError = null;
    try {
      await onDelete(marker.id);
    } catch (cause) {
      localError = cause instanceof Error ? cause.message : String(cause);
    } finally {
      deletingId = null;
    }
  }

  function chapterTitle(marker: VoiceNote) {
    const chapter = chapters.find((entry) => entry.section === marker.chapterIndex);
    return (
      chapter?.title ??
      $t("audiobooks.chapterNumber", { number: marker.chapterIndex + 1 })
    );
  }

  function audioUrl(path: string | null) {
    return path ? convertFileSrc(path) : null;
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
      aria-labelledby="voice-marks-title"
      tabindex="-1"
      bind:this={modalEl}
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="mic" size={21} /></span>
          <div>
            <p class="eyebrow">{$t("audiobooks.toolVoiceMarks")}</p>
            <h2 id="voice-marks-title">{$t("audiobooks.voiceMarksTitle")}</h2>
            <p class="head-help">{$t("audiobooks.voiceMarksHelp")}</p>
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
        {#if !supported}
          <p class="notice warn">{$t("audiobooks.voiceDesktopOnly")}</p>
        {/if}

        <section class="command-card">
          <div class="section-head">
            <div>
              <span class="section-kicker">{$t("audiobooks.voiceCommand")}</span>
              <p>{$t("audiobooks.voiceCommandHelp")}</p>
            </div>
            {#if savedNotice}
              <span class="saved" role="status">
                <Icon name="check" size={13} />
                {$t("audiobooks.voiceCommandSaved")}
              </span>
            {/if}
          </div>
          <div class="phrase-row">
            <input
              type="text"
              bind:value={phraseDraft}
              maxlength="80"
              placeholder={$t("audiobooks.voiceCommandPlaceholder")}
              aria-label={$t("audiobooks.voiceCommand")}
              onkeydown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void savePhrase();
                }
              }}
              disabled={!supported || savingPhrase}
            />
            <Button
              variant="subtle"
              size="small"
              onclick={() => void savePhrase()}
              loading={savingPhrase}
              disabled={!dirty || !supported || savingPhrase}
            >
              {$t("audiobooks.voiceCommandSave")}
            </Button>
            <button
              class="reset"
              type="button"
              onclick={() => void resetPhrase()}
              disabled={!supported || savingPhrase}
            >
              {$t("audiobooks.voiceCommandReset")}
            </button>
          </div>
          {#if listening}
            <p class="listening-line" role="status">
              <span class="live-dot" aria-hidden="true"></span>
              {$t("audiobooks.voiceListening")} · {$t("audiobooks.voiceListeningHint", {
                phrase: savedPhrase,
              })}
            </p>
          {/if}
        </section>

        <section class="status-card">
          <div class="status-row" class:ready={modelReady}>
            <span class="status-icon">
              <Icon name={modelReady ? "check" : "waveform"} size={16} />
            </span>
            <div class="status-copy">
              <strong>
                {#if modelReady}
                  {$t("audiobooks.voiceWhisperReady", { model: modelLabel })}
                {:else}
                  {$t("audiobooks.voiceWhisperMissing")}
                {/if}
              </strong>
              <small>{$t("audiobooks.voiceCommandHelp")}</small>
            </div>
            {#if !modelReady && supported}
              <button
                class="download"
                type="button"
                onclick={() => void downloadWhisper()}
                disabled={whisperDownloading !== null}
              >
                {#if whisperDownloading !== null}
                  {$t("audiobooks.voiceWhisperDownloading", {
                    percent: Math.round(whisperDownloading),
                  })}
                {:else}
                  {$t("audiobooks.voiceWhisperDownload")}
                {/if}
              </button>
            {/if}
          </div>

          <div class="listen-row">
            <div class="listen-copy">
              <strong>{$t("audiobooks.voiceListen")}</strong>
              <small>
                {#if !audiobook}
                  {$t("audiobooks.voiceNeedsPlayback")}
                {:else}
                  {$t("audiobooks.voiceListeningHint", { phrase: savedPhrase })}
                {/if}
              </small>
            </div>
            <button
              class="switch"
              class:active={listening}
              type="button"
              role="switch"
              aria-checked={listening}
              aria-label={listening
                ? $t("audiobooks.voiceStop")
                : $t("audiobooks.voiceListen")}
              onclick={() => void toggleListening()}
              disabled={busy || (!listening && !canToggle)}
            >
              <span aria-hidden="true"></span>
            </button>
          </div>
        </section>

        {#if localError || externalError}
          <p class="error" role="alert">{externalError ?? localError}</p>
        {/if}

        <section class="markers-section">
          <div class="markers-head">
            <div>
              <h3>{$t("audiobooks.voiceMarksTitle")}</h3>
              <p>
                {$t("audiobooks.voiceMarkersCount", { count: ordered.length })}
                {#if audiobook}
                  · {audiobook.bookTitle}
                {/if}
              </p>
            </div>
            {#if listening}
              <span class="listening-badge">
                <span class="live-dot" aria-hidden="true"></span>
                {$t("audiobooks.voiceListening")}
              </span>
            {/if}
          </div>

          {#if ordered.length === 0}
            <p class="empty">
              {$t("audiobooks.voiceMarkerEmpty", { phrase: savedPhrase })}
            </p>
          {:else}
            <ul class="markers-list">
              {#each ordered as marker (marker.id)}
                <li>
                  <div class="marker-top">
                    <button
                      class="time"
                      type="button"
                      onclick={() => onJump(marker.positionMs)}
                      title={$t("audiobooks.voiceMarkerJump")}
                    >
                      <Icon name="play" size={11} />
                      {formatDuration(marker.positionMs)}
                    </button>
                    <span class="chapter">{chapterTitle(marker)}</span>
                    <span class="created">{formatDate(marker.createdAt)}</span>
                    <button
                      class="delete"
                      type="button"
                      onclick={() => void removeMarker(marker)}
                      disabled={deletingId === marker.id}
                      aria-label={$t("audiobooks.voiceMarkerDelete")}
                    >
                      <Icon name="trash" size={14} />
                    </button>
                  </div>
                  {#if marker.quote || marker.text}
                    <p class="quote">{marker.quote || marker.text}</p>
                  {/if}
                  {#if marker.transcript}
                    <p class="transcript">
                      <span>{$t("audiobooks.voiceMarkerComment")}</span>
                      {marker.transcript}
                    </p>
                  {/if}
                  {#if audioUrl(marker.audioPath)}
                    <audio
                      controls
                      preload="none"
                      src={audioUrl(marker.audioPath) ?? undefined}
                    ></audio>
                  {:else}
                    <p class="missing">{$t("audiobooks.voiceMarkerNoAudio")}</p>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose}>{$t("common.close")}</Button>
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
    width: min(720px, 100%);
    max-height: min(88vh, 900px);
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

  .notice,
  .error {
    margin: 0;
    padding: 10px 12px;
    border-radius: 9px;
    font-size: 0.74rem;
    line-height: 1.45;
  }

  .notice.warn {
    border: 1px solid color-mix(in srgb, var(--accent-3) 32%, var(--border));
    background: color-mix(in srgb, var(--accent-3) 8%, var(--surface));
    color: var(--text);
  }

  .error {
    border: 1px solid color-mix(in srgb, var(--danger) 30%, var(--border));
    background: color-mix(in srgb, var(--danger) 7%, var(--surface));
    color: var(--danger);
  }

  .command-card,
  .status-card {
    display: flex;
    flex-direction: column;
    gap: 11px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface-2);
  }

  .section-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .section-kicker {
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 750;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .section-head p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .saved {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: 0 0 auto;
    color: var(--success);
    font-size: 0.66rem;
    font-weight: 650;
  }

  .phrase-row {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }

  .phrase-row input {
    flex: 1 1 240px;
    min-height: 38px;
    padding: 0.4rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
  }

  .phrase-row input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }

  .reset {
    min-height: 32px;
    padding: 0 0.4rem;
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
  }

  .reset:hover:not(:disabled) {
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .reset:disabled {
    opacity: 0.45;
  }

  .listening-line {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--accent);
    font-size: 0.7rem;
    font-weight: 600;
  }

  .live-dot {
    display: inline-block;
    flex: 0 0 auto;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 45%, transparent);
    animation: pulse 1.6s ease-out infinite;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .status-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: var(--surface-3);
    color: var(--muted);
  }

  .status-row.ready .status-icon {
    background: color-mix(in srgb, var(--success) 12%, var(--surface));
    color: var(--success);
  }

  .status-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .status-copy strong {
    font-size: 0.78rem;
    font-weight: 650;
  }

  .status-copy small,
  .listen-copy small {
    color: var(--muted);
    font-size: 0.67rem;
    line-height: 1.4;
  }

  .download {
    flex: 0 0 auto;
    min-height: 32px;
    padding: 0.2rem 0.6rem;
    border: 1px solid var(--accent-line);
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: 0.68rem;
    font-weight: 650;
  }

  .download:disabled {
    opacity: 0.6;
  }

  .listen-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
  }

  .listen-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .listen-copy strong {
    font-size: 0.78rem;
    font-weight: 650;
  }

  .switch {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    width: 42px;
    min-width: 42px;
    max-width: 42px;
    height: 24px;
    min-height: 24px;
    max-height: 24px;
    padding: 3px;
    border: 0;
    border-radius: 999px;
    background: var(--surface-4);
    line-height: 0;
    transition: background-color 150ms ease;
  }

  .switch span {
    display: block;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
    transition: transform 150ms ease;
  }

  .switch.active {
    background: var(--accent);
  }

  .switch.active span {
    transform: translateX(18px);
  }

  .switch:disabled {
    opacity: 0.45;
  }

  .markers-section {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .markers-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .markers-head h3 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 650;
  }

  .markers-head p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.69rem;
  }

  .listening-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    padding: 3px 8px;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 700;
  }

  .empty {
    margin: 0;
    padding: 18px 14px;
    border: 1px dashed var(--border);
    border-radius: 11px;
    color: var(--muted);
    font-size: 0.74rem;
    line-height: 1.5;
    text-align: center;
  }

  .markers-list {
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .markers-list li {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 11px 12px;
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: 11px;
    background: color-mix(in srgb, var(--surface-2) 72%, transparent);
  }

  .marker-top {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .time {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 24px;
    padding: 0.05rem 0.45rem;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: 0.66rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .time:hover {
    background: color-mix(in srgb, var(--accent) 16%, var(--surface));
  }

  .chapter {
    overflow: hidden;
    color: var(--text);
    font-size: 0.7rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .created {
    margin-left: auto;
    color: var(--muted);
    font-size: 0.62rem;
    white-space: nowrap;
  }

  .delete {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
  }

  .delete:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 10%, var(--surface));
    color: var(--danger);
  }

  .delete:disabled {
    opacity: 0.4;
  }

  .quote {
    margin: 0;
    padding-left: 9px;
    border-left: 2px solid color-mix(in srgb, var(--accent) 42%, transparent);
    color: var(--text);
    font-family: var(--font-display);
    font-size: 0.85rem;
    font-style: italic;
    line-height: 1.5;
  }

  .transcript {
    margin: 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .transcript span {
    margin-right: 5px;
    color: var(--accent);
    font-size: 0.6rem;
    font-weight: 750;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .markers-list audio {
    width: 100%;
    height: 30px;
  }

  .missing {
    margin: 0;
    color: var(--muted);
    font-size: 0.66rem;
    font-style: italic;
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

  @keyframes pulse {
    0% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 45%, transparent);
    }
    70% {
      box-shadow: 0 0 0 7px transparent;
    }
    100% {
      box-shadow: 0 0 0 0 transparent;
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

    .phrase-row {
      align-items: stretch;
    }

    .phrase-row input {
      flex-basis: 100%;
    }

    .status-row {
      align-items: flex-start;
      flex-wrap: wrap;
    }

    .download {
      width: 100%;
    }

    .created {
      display: none;
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

    .live-dot {
      animation: none;
    }

    .switch,
    .switch span {
      transition: none;
    }
  }
</style>

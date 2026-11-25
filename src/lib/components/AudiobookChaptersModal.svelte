<script lang="ts">
  import { formatDuration, t, type MessageKey } from "../i18n";
  import type {
    AudioImportMode,
    AudiobookChapterInfo,
    AudiobookInfo,
    ChapterSource,
  } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  let {
    open,
    audiobooks,
    selectedId = null,
    chapters = [],
    loading = false,
    busy = false,
    onClose,
    onSelect,
    onPlayChapter,
    onEdit,
    onReanalyze,
    onTranscribe,
  }: {
    open: boolean;
    audiobooks: AudiobookInfo[];
    selectedId?: string | null;
    chapters?: AudiobookChapterInfo[];
    loading?: boolean;
    busy?: boolean;
    onClose: () => void;
    onSelect?: (audiobookId: string) => void;
    onPlayChapter?: (chapter: AudiobookChapterInfo) => void;
    onEdit?: (audiobook: AudiobookInfo) => void;
    onReanalyze?: (audiobook: AudiobookInfo, mode: AudioImportMode) => void;
    onTranscribe?: (audiobook: AudiobookInfo) => void;
  } = $props();

  const SOURCE_KEYS: Record<ChapterSource, MessageKey> = {
    embedded: "audiobooks.chapterSourceEmbedded",
    silence: "audiobooks.chapterSourceSilence",
    tone: "audiobooks.chapterSourceTone",
    fixed: "audiobooks.chapterSourceFixed",
    sections: "audiobooks.chapterSourceSections",
    ai: "audiobooks.chapterSourceAi",
    manual: "audiobooks.chapterSourceManual",
  };

  let reanalyzeMode = $state<AudioImportMode>("aiFast");

  let selected = $derived(
    audiobooks.find((item) => item.id === selectedId) ?? null,
  );
  let sourceLabel = $derived(
    selected?.chapterSource
      ? $t(SOURCE_KEYS[selected.chapterSource])
      : null,
  );

  function castName(name: string) {
    return name === "Diálogo" ? $t("audiobooks.castDialogue") : name;
  }

  $effect(() => {
    if (!open) return;
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
      aria-labelledby="audiobook-chapters-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="chapters" size={20} /></span>
          <div>
            <h2 id="audiobook-chapters-title">{$t("audiobooks.chaptersPanel")}</h2>
            <p>{$t("audiobooks.chaptersModalHelp")}</p>
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
        {#if audiobooks.length === 0}
          <p class="empty">{$t("audiobooks.empty")}</p>
        {:else}
          <label class="field">
            <span>{$t("audiobooks.selectAudiobook")}</span>
            <select
              value={selectedId ?? ""}
              onchange={(event) => onSelect?.(event.currentTarget.value)}
            >
              {#each audiobooks as item (item.id)}
                <option value={item.id}>
                  {item.bookTitle} · {formatDuration(item.durationMs)}
                </option>
              {/each}
            </select>
          </label>

          {#if sourceLabel}
            <span class="chapter-source">
              <Icon name="chapters" size={13} />
              {$t("audiobooks.chapterSourceBadge", { method: sourceLabel })}
            </span>
          {/if}

          {#if selected?.audioOnly}
            <div class="audio-only-panel">
              <p>
                <Icon name="waveform" size={14} />
                <span>{$t("audiobooks.audioOnlyHelp")}</span>
              </p>
              <div class="audio-only-actions">
                <Button
                  variant="subtle"
                  size="small"
                  onclick={() => onEdit?.(selected)}
                  disabled={busy}
                >
                  <Icon name="edit" size={14} />
                  {$t("audiobooks.editChapters")}
                </Button>
                <label class="reanalyze-select">
                  <span>{$t("audiobooks.reanalyze")}</span>
                  <select bind:value={reanalyzeMode} disabled={busy}>
                    <option value="acoustic">{$t("audiobooks.reanalyzeAcoustic")}</option>
                    <option value="aiFast">{$t("audiobooks.reanalyzeFast")}</option>
                    <option value="aiPrecise">{$t("audiobooks.reanalyzePrecise")}</option>
                  </select>
                </label>
                <Button
                  variant="subtle"
                  size="small"
                  onclick={() => onReanalyze?.(selected, reanalyzeMode)}
                  disabled={busy}
                >
                  <Icon name="sparkles" size={14} />
                  {$t("audiobooks.reanalyze")}
                </Button>
                <Button
                  variant="primary"
                  size="small"
                  onclick={() => onTranscribe?.(selected)}
                  disabled={busy}
                >
                  <Icon name="book" size={14} />
                  {$t("audiobooks.convertToEpub")}
                </Button>
              </div>
              <small>{$t("audiobooks.transcribeLaterHelp")}</small>
            </div>
          {/if}

          {#if selected && selected.cast.length > 0}
            <div class="cast-panel">
              <span class="cast-panel-title">
                <Icon name="voice" size={13} />
                {$t("audiobooks.castPanel")}
              </span>
              <ul class="cast-members">
                <li>
                  <span>{$t("audiobooks.castNarrator")}</span>
                  <strong>{selected.voiceName ?? "—"}</strong>
                </li>
                {#each selected.cast as member (member.character)}
                  <li>
                    <span>{castName(member.character)}</span>
                    <strong>{member.voiceName}</strong>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}

          {#if loading}
            <div class="loading"><Spinner label={$t("common.loading")} /></div>
          {:else if !selectedId}
            <p class="empty">{$t("audiobooks.noChapterSelection")}</p>
          {:else if chapters.length === 0}
            <p class="empty">{$t("audiobooks.noChapters")}</p>
          {:else}
            <ol class="chapter-list">
              {#each chapters as chapter (chapter.index)}
                <li>
                  <button type="button" onclick={() => onPlayChapter?.(chapter)}>
                    <span class="chapter-index">{chapter.index + 1}</span>
                    <span class="chapter-copy">
                      <strong>{chapter.title}</strong>
                      <small>
                        {formatDuration(chapter.startMs)} · {formatDuration(chapter.endMs)}
                        {#if chapter.karaoke}
                          · <span class="chapter-karaoke">
                            {$t("audiobooks.chapterKaraoke")}
                          </span>
                        {/if}
                      </small>
                    </span>
                    <span class="chapter-play" aria-hidden="true">
                      <Icon name="play" size={14} />
                    </span>
                  </button>
                </li>
              {/each}
            </ol>
          {/if}
        {/if}
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
    width: min(620px, 100%);
    max-height: min(88vh, 820px);
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
    gap: 12px;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field span {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .field select {
    min-height: 42px;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.84rem;
  }

  .chapter-source {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    min-height: 24px;
    padding: 0.1rem 0.5rem;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 650;
  }

  .audio-only-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border: 1px dashed var(--accent-line);
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 5%, transparent);
  }

  .audio-only-panel p {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.4;
  }

  .audio-only-panel p :global(svg) {
    flex: 0 0 auto;
    margin-top: 2px;
    color: var(--accent);
  }

  .audio-only-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .audio-only-panel > small {
    color: var(--muted);
    font-size: 0.66rem;
  }

  .reanalyze-select {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--muted);
    font-size: 0.66rem;
  }

  .reanalyze-select select {
    min-height: 30px;
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.68rem;
  }

  .cast-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-2) 70%, transparent);
  }

  .cast-panel-title {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .cast-members {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .cast-members li {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
    min-width: 0;
    padding: 2px 7px;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: 999px;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.68rem;
  }

  .cast-members strong {
    color: var(--text);
    font-weight: 650;
  }

  .loading {
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .empty {
    margin: 0;
    padding: 18px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    color: var(--muted);
    font-size: 0.8rem;
    text-align: center;
  }

  .chapter-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .chapter-list button {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr) 28px;
    align-items: center;
    gap: 9px;
    width: 100%;
    min-height: 48px;
    padding: 7px 9px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .chapter-list button:hover {
    border-color: var(--border);
    background: var(--surface-2);
  }

  .chapter-index {
    display: grid;
    place-items: center;
    width: 27px;
    height: 27px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
    font-size: 0.65rem;
    font-weight: 650;
  }

  .chapter-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .chapter-copy strong {
    overflow: hidden;
    font-size: 0.79rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chapter-copy small {
    color: var(--muted);
    font-size: 0.66rem;
    font-variant-numeric: tabular-nums;
  }

  .chapter-karaoke {
    color: var(--accent);
    font-weight: 650;
  }

  .chapter-play {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .modal-foot {
    display: flex;
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

    .modal-head {
      padding: 15px 16px 13px;
    }

    .modal-body {
      padding: 15px 16px;
    }

    .modal-foot {
      padding: 12px 16px calc(12px + var(--safe-bottom));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }
  }
</style>

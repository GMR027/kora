<script lang="ts">
  import { t } from "../i18n";
  import type { ChapterEditInput } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    title,
    durationMs,
    chapters = [],
    playbackPositionMs = null,
    busy = false,
    error = null,
    onClose,
    onSave,
  }: {
    open: boolean;
    title: string;
    durationMs: number;
    chapters?: ChapterEditInput[];
    playbackPositionMs?: number | null;
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onSave: (chapters: ChapterEditInput[]) => void;
  } = $props();

  let draft = $state<ChapterEditInput[]>([]);
  let localError = $state<string | null>(null);
  let wasOpen = false;

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    draft = chapters.map((chapter) => ({ ...chapter }));
    localError = null;
  });

  function formatClock(milliseconds: number) {
    const total = Math.max(0, Math.round(milliseconds / 1000));
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const seconds = total % 60;
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
      : `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  function split(index: number) {
    const chapter = draft[index];
    if (!chapter) return;
    const position =
      playbackPositionMs !== null &&
      playbackPositionMs > chapter.startMs + 1_000 &&
      playbackPositionMs < chapter.endMs - 1_000
        ? playbackPositionMs
        : chapter.startMs + Math.round((chapter.endMs - chapter.startMs) / 2);
    if (position <= chapter.startMs + 1_000 || position >= chapter.endMs - 1_000) {
      localError = $t("audiobooks.chapterEditorTooShort");
      return;
    }
    const end = chapter.endMs;
    draft[index] = { ...chapter, endMs: position };
    draft = [
      ...draft.slice(0, index + 1),
      {
        startMs: position,
        endMs: end,
        title: `${chapter.title} (2)`,
      },
      ...draft.slice(index + 1),
    ];
    localError = null;
  }

  function mergeNext(index: number) {
    const current = draft[index];
    const next = draft[index + 1];
    if (!current || !next) return;
    draft[index] = { ...current, endMs: next.endMs };
    draft = draft.filter((_, position) => position !== index + 1);
  }

  function remove(index: number) {
    if (draft.length <= 1) return;
    const current = draft[index];
    const previous = draft[index - 1];
    if (current && previous) {
      draft[index - 1] = { ...previous, endMs: current.endMs };
    }
    draft = draft.filter((_, position) => position !== index);
  }

  function addChapter() {
    const last = draft[draft.length - 1];
    if (!last) return;
    const position = last.startMs + Math.round((last.endMs - last.startMs) / 2);
    if (position <= last.startMs + 1_000 || position >= last.endMs - 1_000) {
      localError = $t("audiobooks.chapterEditorTooShort");
      return;
    }
    const end = last.endMs;
    draft[draft.length - 1] = { ...last, endMs: position };
    draft = [
      ...draft,
      {
        startMs: position,
        endMs: end,
        title: $t("audiobooks.chapterNumber", { number: draft.length + 1 }),
      },
    ];
  }

  function updateEnd(index: number, seconds: number) {
    const chapter = draft[index];
    if (!chapter || !Number.isFinite(seconds)) return;
    const end = Math.max(chapter.startMs + 1_000, Math.round(seconds * 1000));
    draft[index] = { ...chapter, endMs: Math.min(durationMs, end) };
  }

  function save() {
    localError = null;
    if (draft.length === 0) {
      localError = $t("audiobooks.chapterEditorInvalid");
      return;
    }
    let cursor = 0;
    const normalized: ChapterEditInput[] = [];
    for (const chapter of draft) {
      const title = chapter.title.trim();
      if (!title) {
        localError = $t("audiobooks.chapterEditorInvalid");
        return;
      }
      const end = Math.min(durationMs, Math.max(cursor + 1_000, chapter.endMs));
      if (end <= cursor) {
        localError = $t("audiobooks.chapterEditorTooShort");
        return;
      }
      normalized.push({ startMs: cursor, endMs: end, title });
      cursor = end;
    }
    if (normalized.length > 0) {
      normalized[normalized.length - 1].endMs = durationMs;
    }
    onSave(normalized);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
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
      aria-labelledby="chapter-editor-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="chapters" size={20} /></span>
          <div>
            <h2 id="chapter-editor-title">{$t("audiobooks.chapterEditorTitle")}</h2>
            <p>{title}</p>
          </div>
        </div>
        <button
          class="close-button"
          type="button"
          onclick={onClose}
          aria-label={$t("common.close")}
        >
          <Icon name="close" size={18} />
        </button>
      </header>

      <div class="modal-body">
        {#if error || localError}
          <p class="notice" role="alert">{localError ?? error}</p>
        {/if}

        <p class="hint">{$t("audiobooks.chapterEditorHelp")}</p>

        <ol class="chapter-list">
          {#each draft as chapter, index (index)}
            <li>
              <div class="chapter-head">
                <span class="chapter-index">{index + 1}</span>
                <input
                  class="title-input"
                  type="text"
                  maxlength="200"
                  value={chapter.title}
                  oninput={(event) =>
                    (draft[index] = {
                      ...chapter,
                      title: event.currentTarget.value,
                    })}
                  aria-label={$t("audiobooks.chapterTitle")}
                  disabled={busy}
                />
              </div>
              <div class="chapter-times">
                <span>
                  {$t("audiobooks.chapterStart")}:
                  <strong>{formatClock(chapter.startMs)}</strong>
                </span>
                <label>
                  {$t("audiobooks.chapterEnd")}
                  <input
                    type="number"
                    min={(chapter.startMs + 1_000) / 1000}
                    max={durationMs / 1000}
                    step="0.1"
                    value={(chapter.endMs / 1000).toFixed(1)}
                    oninput={(event) =>
                      updateEnd(index, Number(event.currentTarget.value))}
                    disabled={busy}
                  />
                </label>
                <span class="duration">
                  {formatClock(chapter.endMs - chapter.startMs)}
                </span>
              </div>
              <div class="chapter-actions">
                <button
                  type="button"
                  onclick={() => split(index)}
                  disabled={busy}
                >
                  <Icon name="expand" size={13} />
                  {$t("audiobooks.splitHere")}
                </button>
                <button
                  type="button"
                  onclick={() => mergeNext(index)}
                  disabled={busy || index + 1 >= draft.length}
                >
                  <Icon name="chapters" size={13} />
                  {$t("audiobooks.mergeNext")}
                </button>
                <button
                  class="danger"
                  type="button"
                  onclick={() => remove(index)}
                  disabled={busy || draft.length <= 1}
                >
                  <Icon name="trash" size={13} />
                  {$t("common.delete")}
                </button>
              </div>
            </li>
          {/each}
        </ol>

        <Button variant="subtle" size="small" onclick={addChapter} disabled={busy}>
          <Icon name="plus" size={14} />
          {$t("audiobooks.addChapter")}
        </Button>
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("common.cancel")}
        </Button>
        <Button variant="primary" onclick={save} loading={busy}>
          <Icon name="check" size={15} />
          {busy ? $t("common.loading") : $t("audiobooks.saveChapters")}
        </Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 82;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(18, 14, 10, 0.46);
    backdrop-filter: blur(6px);
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
    width: min(760px, 100%);
    max-height: min(90vh, 900px);
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 19px 20px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
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
    border-radius: 11px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.25rem;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .close-button {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    overflow-y: auto;
    padding: 18px 20px;
  }

  .notice {
    margin: 0;
    padding: 0.6rem 0.75rem;
    border: 1px solid color-mix(in srgb, var(--danger) 50%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    font-size: 0.76rem;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .chapter-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .chapter-list li {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .chapter-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .chapter-index {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 700;
  }

  .title-input {
    width: 100%;
    min-height: 32px;
    padding: 0.3rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  .chapter-times {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .chapter-times strong {
    color: var(--text);
  }

  .chapter-times label {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .chapter-times input {
    width: 86px;
    min-height: 28px;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
  }

  .duration {
    margin-left: auto;
    color: var(--accent);
    font-weight: 650;
  }

  .chapter-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chapter-actions button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 27px;
    padding: 0.18rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--muted);
    font: inherit;
    font-size: 0.64rem;
  }

  .chapter-actions button:hover:not(:disabled) {
    border-color: var(--accent-line);
    color: var(--accent);
  }

  .chapter-actions button.danger:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--danger) 50%, var(--border));
    color: var(--danger);
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 9px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
  }
</style>

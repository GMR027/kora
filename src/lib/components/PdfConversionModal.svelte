<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    convertPdfToEpub,
    errorMessage,
    pdfConversionPreview,
    pdfConversionStatus,
  } from "../api";
  import { t, translate, type MessageKey } from "../i18n";
  import type {
    AutoIndexEntry,
    BookSummary,
    PageTextLayer,
    PdfConversionPreview,
    PdfConversionResult,
    PdfConversionState,
  } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  let {
    open,
    book,
    onClose,
    onConverted,
  }: {
    open: boolean;
    book: BookSummary | null;
    onClose: () => void;
    onConverted?: (result: PdfConversionResult) => void;
  } = $props();

  type ChapterMode = "auto" | "outline" | "heuristic" | "pages";

  const PHASE_KEYS: Record<string, MessageKey> = {
    preparing: "pdf.convertPhasePreparing",
    ocr: "pdf.convertPhaseOcr",
    extracting: "pdf.convertPhaseExtracting",
    chapters: "pdf.convertPhaseChapters",
    building: "pdf.convertPhaseBuilding",
    verifying: "pdf.convertPhaseVerifying",
    migrating: "pdf.convertPhaseMigrating",
    cleanup: "pdf.convertPhaseCleanup",
    done: "pdf.convertDone",
    failed: "pdf.convertFailed",
  };

  let preview = $state<PdfConversionPreview | null>(null);
  let job = $state<PdfConversionState | null>(null);
  let result = $state<PdfConversionResult | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let extracting = $state(false);
  let extractDone = $state(0);
  let extractTotal = $state(0);
  let error = $state<string | null>(null);
  let loadedFor = $state<string | null>(null);
  let activeBookId = $state<string | null>(null);
  let loadSequence = 0;

  let title = $state("");
  let author = $state("");
  let language = $state("es");
  let chapterMode = $state<ChapterMode>("auto");
  let replaceOriginal = $state(true);

  let running = $derived(
    extracting || busy || (job !== null && !job.done),
  );
  let canStart = $derived(
    preview?.canConvert === true && !running && result === null && !loading,
  );
  let percent = $derived(
    extracting
      ? extractTotal > 0
        ? Math.round((extractDone / extractTotal) * 100)
        : 0
      : Math.round(job?.percent ?? 0),
  );
  let phaseLabel = $derived(
    extracting
      ? translate("pdf.convertPhaseExtracting")
      : translate(PHASE_KEYS[job?.phase ?? ""] ?? "pdf.convertRunning"),
  );
  let progressMessage = $derived(
    extracting
      ? translate("pdf.convertExtractingPages", {
          done: extractDone,
          total: extractTotal,
        })
      : (job?.message ?? ""),
  );
  let report = $derived(result?.report ?? job?.report ?? null);
  let finished = $derived(
    result !== null || (job !== null && job.done && job.report !== null),
  );

  $effect(() => {
    activeBookId = open && book ? book.id : null;
  });

  $effect(() => {
    if (!open) {
      loadedFor = null;
      loadSequence += 1;
      loading = false;
      return;
    }
    const target = book;
    if (!target || loadedFor === target.id) return;
    loadedFor = target.id;
    void load(target);
  });

  $effect(() => {
    if (!open) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  onMount(() => {
    let unlistenProgress: UnlistenFn | undefined;
    let unlistenDone: UnlistenFn | undefined;
    let unlistenFailed: UnlistenFn | undefined;

    void listen<PdfConversionState>("pdf-convert-progress", (event) => {
      if (event.payload.bookId !== activeBookId || result !== null) return;
      job = event.payload;
    }).then((stop) => {
      unlistenProgress = stop;
    });

    void listen<PdfConversionResult>("pdf-convert-done", (event) => {
      // Aunque el modal esté cerrado, la biblioteca sigue montada y debe
      // actualizar la fila del libro. Si hay otro libro abierto, se ignora.
      if (activeBookId && event.payload.sourceBookId !== activeBookId) return;
      result = event.payload;
      job = null;
      busy = false;
      error = null;
      onConverted?.(event.payload);
    }).then((stop) => {
      unlistenDone = stop;
    });

    void listen<PdfConversionState>("pdf-convert-failed", (event) => {
      if (event.payload.bookId !== activeBookId || result !== null) return;
      job = event.payload;
      busy = false;
      error = event.payload.error ?? translate("pdf.convertFailed");
    }).then((stop) => {
      unlistenFailed = stop;
    });

    const onKeydown = (event: KeyboardEvent) => {
      if (open && event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeydown);

    return () => {
      unlistenProgress?.();
      unlistenDone?.();
      unlistenFailed?.();
      window.removeEventListener("keydown", onKeydown);
    };
  });

  async function load(target: BookSummary) {
    const sequence = ++loadSequence;
    loading = true;
    error = null;
    result = null;
    job = null;
    preview = null;
    busy = false;
    extracting = false;
    extractDone = 0;
    extractTotal = 0;

    try {
      const data = await pdfConversionPreview(target.id);
      if (sequence !== loadSequence) return;
      preview = data;
      title = data.title;
      author = data.author ?? "";
      language = data.language ?? "es";
      chapterMode = "auto";
      replaceOriginal = true;

      const current = await pdfConversionStatus(target.id);
      if (sequence !== loadSequence) return;
      if (current) {
        job = current;
        if (current.done && current.error) {
          error = current.error;
        }
      }
    } catch (cause) {
      if (sequence === loadSequence) {
        error = errorMessage(cause);
      }
    } finally {
      if (sequence === loadSequence) {
        loading = false;
      }
    }
  }

  function applyStarted(started: PdfConversionState) {
    const current = job;
    if (
      result === null &&
      (current === null || (!current.done && started.percent >= current.percent))
    ) {
      job = started;
    }
  }

  async function start() {
    if (!book || !canStart || busy || extracting) return;
    busy = true;
    error = null;
    result = null;
    job = null;

    let pages: PageTextLayer[] | null = null;
    let outline: AutoIndexEntry[] | null = null;

    try {
      if (preview?.extraction === "frontend") {
        extracting = true;
        extractDone = 0;
        extractTotal = preview.pages;
        const { extractPdfForConversion } = await import("../pdfExtract");
        const extracted = await extractPdfForConversion(
          book.id,
          (done, total) => {
            extractDone = done;
            extractTotal = total;
          },
        );
        pages = extracted.pages;
        outline = extracted.outline;
        extracting = false;
      }

      const started = await convertPdfToEpub(
        book.id,
        {
          title: title.trim() || undefined,
          author: author.trim() || undefined,
          language: language.trim() || undefined,
          chapterMode,
          replaceOriginal,
        },
        pages,
        outline,
      );
      applyStarted(started);
    } catch (cause) {
      error = errorMessage(cause);
      job = null;
    } finally {
      extracting = false;
      busy = false;
    }
  }
</script>

{#if open && book}
  <div class="modal-backdrop" role="presentation">
    <button
      class="backdrop-button"
      type="button"
      aria-label={$t("common.close")}
      onclick={onClose}
    ></button>

    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="pdf-convert-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="book" size={21} /></span>
          <div>
            <p class="eyebrow">{$t("pdf.convertTitle")}</p>
            <h2 id="pdf-convert-title">{book.title}</h2>
            <p class="head-help">{$t("pdf.convertHelp")}</p>
          </div>
        </div>
        <button
          class="close-button"
          type="button"
          aria-label={$t("common.close")}
          title={$t("common.close")}
          onclick={onClose}
        >
          <Icon name="close" size={18} />
        </button>
      </header>

      <div class="modal-body">
        {#if error}
          <p class="notice error" role="alert">{error}</p>
        {/if}

        {#if loading}
          <div class="loading"><Spinner label={$t("common.loading")} /></div>
        {:else if preview}
          <div class="stats">
            <span>
              <strong>{preview.pages}</strong>
              {$t("common.pages")}
            </span>
            <span>
              <strong>{preview.estimatedChapters}</strong>
              {$t("pdf.convertChapterMode")}
            </span>
            <span>
              <strong>{preview.scanned ? $t("pdf.convertScanned") : $t("pdf.convertNativeText")}</strong>
              {preview.scanned
                ? $t("pdf.convertOcrProgress", {
                    done: preview.ocrPagesDone,
                    total: preview.ocrPagesTotal,
                  })
                : $t("common.ready")}
            </span>
          </div>

          {#if preview.warnings.length > 0}
            <div class="warnings">
              <strong>{$t("pdf.convertWarnings")}</strong>
              <ul>
                {#each preview.warnings as warning}
                  <li>{warning}</li>
                {/each}
              </ul>
            </div>
          {/if}

          <div class="meta-line">
            <span>{$t("pdf.convertAnnotations", {
              count: preview.annotations + preview.extracts + preview.notes + preview.voiceNotes + preview.reactions,
            })}</span>
            {#if preview.audiobooks > 0}
              <span>{$t("pdf.convertAudiobooks", { count: preview.audiobooks })}</span>
            {/if}
          </div>

          {#if !running && !finished}
            <div class="fields">
              <label>
                <span>{$t("pdf.convertTitleField")}</span>
                <input bind:value={title} maxlength="200" />
              </label>
              <label>
                <span>{$t("pdf.convertAuthorField")}</span>
                <input bind:value={author} maxlength="160" />
              </label>
              <label>
                <span>{$t("pdf.convertLanguageField")}</span>
                <input bind:value={language} maxlength="16" />
              </label>
              <label>
                <span>{$t("pdf.convertChapterMode")}</span>
                <select bind:value={chapterMode}>
                  <option value="auto">{$t("pdf.convertModeAuto")}</option>
                  <option value="outline">{$t("pdf.convertModeOutline")}</option>
                  <option value="heuristic">{$t("pdf.convertModeHeuristic")}</option>
                  <option value="pages">{$t("pdf.convertModePages")}</option>
                </select>
              </label>
            </div>

            <label class="toggle">
              <input type="checkbox" bind:checked={replaceOriginal} />
              <span>
                <strong>{$t("pdf.convertReplace")}</strong>
                <small>{$t("pdf.convertReplaceHelp")}</small>
              </span>
            </label>
            {#if !replaceOriginal}
              <p class="hint">{$t("pdf.convertCopy")}</p>
            {/if}
          {/if}

          {#if preview.sample && !finished}
            <details class="sample" open={preview.pages <= 12}>
              <summary>{$t("pdf.convertSample")}</summary>
              <pre>{preview.sample}</pre>
            </details>
          {:else if !preview.sample && !running && !finished}
            <p class="hint">{$t("pdf.convertNoSample")}</p>
          {/if}

          {#if running}
            <div class="progress-card" aria-live="polite">
              <div class="progress-head">
                <strong>{phaseLabel}</strong>
                <span>{percent}%</span>
              </div>
              <div class="progress-track">
                <span style:width={`${percent}%`}></span>
              </div>
              {#if progressMessage}
                <p>{progressMessage}</p>
              {/if}
              {#if extracting}
                <small>{$t("pdf.convertKeepOpen")}</small>
              {:else if job && job.pagesTotal > 0}
                <small>{job.pagesDone} / {job.pagesTotal} · {$t("pdf.convertKeepOpen")}</small>
              {:else}
                <small>{$t("pdf.convertKeepOpen")}</small>
              {/if}
            </div>
          {/if}

          {#if finished && report}
            <div class="result-card" role="status">
              <div class="result-icon"><Icon name="check" size={20} /></div>
              <div>
                <strong>{$t("pdf.convertResultTitle")}</strong>
                <p>{$t("pdf.convertSummary", {
                  chapters: report.chapters,
                  chars: report.chars,
                })}</p>
                <p>
                  {$t("pdf.convertRelocated", {
                    anchored: report.anchored,
                    approximate: report.approximate,
                    unmapped: report.unmapped,
                  })}
                </p>
                {#if report.warnings.length > 0}
                  <ul>
                    {#each report.warnings as warning}
                      <li>{warning}</li>
                    {/each}
                  </ul>
                {/if}
                <small>{$t("pdf.convertResultHelp")}</small>
              </div>
            </div>
          {/if}

          {#if !preview.canConvert && !running}
            <p class="notice error" role="status">{$t("pdf.convertUnavailable")}</p>
          {/if}
        {/if}
      </div>

      <footer class="modal-foot">
        {#if finished}
          <Button variant="primary" onclick={onClose}>{$t("pdf.convertClose")}</Button>
        {:else if running}
          <Button variant="ghost" onclick={onClose}>{$t("pdf.convertClose")}</Button>
        {:else}
          <Button variant="ghost" onclick={onClose}>{$t("common.cancel")}</Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={!canStart}
            onclick={() => void start()}
          >
            {$t("pdf.convertStart")}
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
    z-index: 90;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(18, 14, 10, 0.52);
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
    width: min(720px, 100%);
    max-height: min(92vh, 880px);
    max-height: min(92dvh, 880px);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: 0 36px 80px -40px rgba(15, 23, 42, 0.45);
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
    width: 42px;
    height: 42px;
    border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--border));
    border-radius: 13px;
    background: color-mix(in srgb, var(--accent) 11%, var(--surface));
    color: var(--accent);
  }

  .eyebrow {
    margin: 0 0 2px;
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.14rem;
    letter-spacing: -0.015em;
  }

  .head-help {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.45;
  }

  .close-button {
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

  .close-button:hover {
    border-color: var(--border);
    background: var(--surface-2);
    color: var(--text);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 13px;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .loading {
    display: grid;
    place-items: center;
    min-height: 130px;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .stats span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.69rem;
    line-height: 1.35;
  }

  .stats strong {
    color: var(--text);
    font-family: var(--font-display);
    font-size: 0.9rem;
    font-weight: 650;
  }

  .warnings,
  .result-card {
    padding: 11px 12px;
    border: 1px solid color-mix(in srgb, var(--warn) 28%, var(--border));
    border-radius: 11px;
    background: color-mix(in srgb, var(--warn) 8%, var(--surface));
  }

  .warnings strong,
  .result-card strong {
    display: block;
    margin-bottom: 3px;
    font-size: 0.78rem;
  }

  .warnings ul,
  .result-card ul {
    margin: 0;
    padding-left: 1.1rem;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.5;
  }

  .meta-line {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1rem;
    color: var(--muted);
    font-size: 0.72rem;
  }

  .fields {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 9px;
  }

  .fields label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 650;
  }

  .fields label:first-child {
    grid-column: 1 / -1;
  }

  input,
  select {
    width: 100%;
    min-height: 38px;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 400;
  }

  input:focus-visible,
  select:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: 1px;
  }

  .toggle {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 10px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .toggle input {
    width: 16px;
    min-height: 16px;
    margin-top: 2px;
    accent-color: var(--accent);
  }

  .toggle span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 0.78rem;
  }

  .toggle small {
    color: var(--muted);
    font-size: 0.69rem;
    line-height: 1.45;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.5;
  }

  .sample {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .sample summary {
    padding: 9px 11px;
    cursor: pointer;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 650;
  }

  .sample pre {
    max-height: 150px;
    margin: 0;
    padding: 0 11px 11px;
    overflow: auto;
    color: var(--text);
    font-family: var(--font-reading, Georgia, serif);
    font-size: 0.75rem;
    line-height: 1.55;
    white-space: pre-wrap;
  }

  .progress-card {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--border));
    border-radius: 11px;
    background: color-mix(in srgb, var(--accent) 7%, var(--surface));
  }

  .progress-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 0.78rem;
  }

  .progress-head span {
    color: var(--accent);
    font-weight: 700;
  }

  .progress-track {
    height: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .progress-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--gold), var(--accent-2));
    transition: width 220ms ease;
  }

  .progress-card p,
  .progress-card small {
    margin: 0;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.45;
  }

  .result-card {
    display: flex;
    gap: 10px;
    border-color: color-mix(in srgb, var(--ok) 30%, var(--border));
    background: color-mix(in srgb, var(--ok) 8%, var(--surface));
  }

  .result-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--ok) 16%, var(--surface));
    color: var(--ok);
  }

  .result-card p,
  .result-card small {
    margin: 0 0 2px;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .notice {
    margin: 0;
    padding: 9px 11px;
    border-radius: 9px;
    font-size: 0.76rem;
    line-height: 1.45;
  }

  .notice.error {
    border: 1px solid color-mix(in srgb, var(--danger) 26%, var(--border));
    background: color-mix(in srgb, var(--danger) 8%, var(--surface));
    color: color-mix(in srgb, var(--danger) 84%, var(--text));
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 9px;
    padding: 14px 20px calc(14px + var(--safe-bottom));
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
      max-height: 94dvh;
      border-radius: 18px 18px 0 0;
    }

    .modal-head {
      padding: 16px 15px 13px;
    }

    .modal-body {
      padding: 14px 15px;
    }

    .stats {
      grid-template-columns: 1fr;
    }

    .fields {
      grid-template-columns: 1fr;
    }

    .fields label:first-child {
      grid-column: auto;
    }

    .modal-foot {
      padding: 12px 15px calc(12px + var(--safe-bottom));
    }
  }
</style>

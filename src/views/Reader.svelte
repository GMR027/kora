<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import {
    errorMessage,
    libraryList,
    readerAddAnnotation,
    readerDeleteAnnotation,
    readerGetChapter,
    readerListAnnotations,
    readerOpen,
    readerSaveSettings,
    readerUpdateAnnotation,
    readerUpdateProgress,
  } from "../lib/api";
  import type {
    Annotation,
    AnnotationKind,
    BookSummary,
    ChapterContent,
    ReaderMode,
    ReaderSession,
    ReaderSettings,
  } from "../lib/types";

  let { bookId = null }: { bookId?: string | null } = $props();

  const colors = ["#ffd166", "#8ecae6", "#b7e4c7", "#ffb4a2", "#cdb4db"];
  const fontStacks: Record<string, string> = {
    serif: '"Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif',
    sans: '"Inter", ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
    mono: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
  };

  interface MarkRect {
    left: number;
    top: number;
    width: number;
    height: number;
  }

  interface Mark {
    id: string;
    color: string;
    kind: AnnotationKind;
    rects: MarkRect[];
    annotation: Annotation;
  }

  interface SelectionInfo {
    start: number;
    end: number;
    text: string;
    x: number;
    y: number;
  }

  let books = $state<BookSummary[]>([]);
  let selectedBookId = $state("");
  let session = $state<ReaderSession | null>(null);
  let settings = $state<ReaderSettings | null>(null);
  let chapter = $state<ChapterContent | null>(null);
  let chapterIndex = $state(0);
  let annotations = $state<Annotation[]>([]);
  let marks = $state<Mark[]>([]);
  let chapterProgress = $state(0);
  let bookProgress = $state(0);
  let loading = $state(false);
  let opening = $state(false);
  let error = $state<string | null>(null);
  let failedBookId = $state<string | null>(null);
  let tocOpen = $state(false);
  let settingsOpen = $state(false);
  let selection = $state<SelectionInfo | null>(null);
  let noteDraft = $state("");
  let activeAnnotation = $state<Annotation | null>(null);
  let annotationNote = $state("");
  let annotationColor = $state(colors[0]);
  let pageWidth = $state(0);
  let pageHeight = $state(0);

  let scrollerEl = $state<HTMLElement | null>(null);
  let contentEl = $state<HTMLElement | null>(null);
  let progressTimer: ReturnType<typeof setTimeout> | undefined;
  let settingsTimer: ReturnType<typeof setTimeout> | undefined;
  let scrollFrame = 0;

  let sectionCount = $derived(session ? session.document.sections.length : 0);
  let themeClass = $derived(`theme-${settings?.theme ?? "light"}`);
  let textureClass = $derived(`texture-${settings?.texture ?? "none"}`);
  let surfaceStyle = $derived.by(() => {
    if (!settings) return "";
    const family = fontStacks[settings.fontFamily] ?? fontStacks.serif;
    return [
      `--reader-font-size:${settings.fontSize}px`,
      `--reader-line-height:${settings.lineHeight}`,
      `--reader-margin:${settings.margin}px`,
      `--reader-font-family:${family}`,
      `--reader-zoom:${settings.zoom}`,
    ].join(";");
  });

  onMount(() => {
    void loadLibrary();

    const handleResize = () => {
      measure();
      scheduleMarks();
    };

    window.addEventListener("resize", handleResize);
    window.addEventListener("keydown", onKeydown);

    return () => {
      window.removeEventListener("resize", handleResize);
      window.removeEventListener("keydown", onKeydown);
      clearTimeout(progressTimer);
      clearTimeout(settingsTimer);
      cancelAnimationFrame(scrollFrame);
      void persistProgress();
    };
  });

  $effect(() => {
    if (bookId && bookId !== session?.bookId && bookId !== failedBookId) {
      void openBook(bookId);
    }
  });

  async function loadLibrary() {
    try {
      books = await libraryList();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function openSelected() {
    if (selectedBookId) {
      await openBook(selectedBookId);
    }
  }

  async function openBook(id: string) {
    if (session && session.bookId !== id) {
      await persistProgress();
    }

    opening = true;
    error = null;
    selection = null;
    activeAnnotation = null;
    noteDraft = "";

    try {
      const opened = await readerOpen(id);
      session = opened;
      settings = opened.settings;
      selectedBookId = id;
      failedBookId = null;
      bookProgress = opened.progress;
      chapterIndex = Math.min(
        opened.locator.section,
        Math.max(opened.document.sections.length - 1, 0),
      );
      annotations = await readerListAnnotations(id);
      await loadChapter(chapterIndex, opened.locator.offset);
    } catch (cause) {
      error = errorMessage(cause);
      failedBookId = id;
      session = null;
      chapter = null;
    } finally {
      opening = false;
    }
  }

  async function loadChapter(index: number, offset = 0) {
    if (!session) return;

    loading = true;
    error = null;
    try {
      const content = await readerGetChapter(session.bookId, index);
      chapter = content;
      chapterIndex = index;
      await tick();
      measure();
      const fraction = content.charCount > 0 ? offset / content.charCount : 0;
      scrollToFraction(fraction);
      chapterProgress = Math.min(1, Math.max(0, fraction));
      bookProgress = sectionCount
        ? (chapterIndex + chapterProgress) / sectionCount
        : 0;
      scheduleMarks();
      scheduleProgressSave();
      document.fonts?.ready.then(() => scheduleMarks());
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      loading = false;
    }
  }

  async function goToChapter(index: number) {
    if (!session || index < 0 || index >= sectionCount) return;
    if (index === chapterIndex) {
      tocOpen = false;
      return;
    }

    await persistProgress();
    selection = null;
    activeAnnotation = null;
    tocOpen = false;
    await loadChapter(index, 0);
  }

  function measure() {
    if (!scrollerEl) return;
    pageWidth = scrollerEl.clientWidth;
    pageHeight = scrollerEl.clientHeight;
  }

  function scrollToFraction(fraction: number) {
    if (!scrollerEl) return;

    const clamped = Math.min(1, Math.max(0, fraction));
    const paged = settings?.mode === "paged";
    const max = paged
      ? scrollerEl.scrollWidth - scrollerEl.clientWidth
      : scrollerEl.scrollHeight - scrollerEl.clientHeight;

    if (paged) {
      scrollerEl.scrollLeft = clamped * Math.max(0, max);
    } else {
      scrollerEl.scrollTop = clamped * Math.max(0, max);
    }
  }

  function onScroll() {
    if (scrollFrame) return;

    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      updateProgressFromScroll();
    });
  }

  function updateProgressFromScroll() {
    if (!scrollerEl || !chapter) return;

    const paged = settings?.mode === "paged";
    const max = paged
      ? scrollerEl.scrollWidth - scrollerEl.clientWidth
      : scrollerEl.scrollHeight - scrollerEl.clientHeight;
    const position = paged ? scrollerEl.scrollLeft : scrollerEl.scrollTop;

    chapterProgress = max > 0 ? Math.min(1, Math.max(0, position / max)) : 0;
    bookProgress = sectionCount
      ? Math.min(1, (chapterIndex + chapterProgress) / sectionCount)
      : 0;
    scheduleProgressSave();
  }

  function scheduleProgressSave() {
    clearTimeout(progressTimer);
    progressTimer = setTimeout(() => {
      void persistProgress();
    }, 650);
  }

  async function persistProgress() {
    if (!session || !chapter) return;

    const offset = Math.round(chapterProgress * chapter.charCount);
    try {
      await readerUpdateProgress(
        session.bookId,
        chapterIndex,
        offset,
        bookProgress,
      );
    } catch (cause) {
      console.error("No se pudo guardar el progreso", cause);
    }
  }

  async function pageStep(direction: -1 | 1) {
    if (!scrollerEl) return;

    if (settings?.mode === "paged") {
      const max = scrollerEl.scrollWidth - scrollerEl.clientWidth;
      const atEnd = scrollerEl.scrollLeft >= max - 2;
      const atStart = scrollerEl.scrollLeft <= 2;

      if (direction > 0 && atEnd) {
        await goToChapter(chapterIndex + 1);
        return;
      }
      if (direction < 0 && atStart) {
        await goToChapter(chapterIndex - 1);
        return;
      }

      const target = Math.min(
        max,
        Math.max(0, scrollerEl.scrollLeft + direction * scrollerEl.clientWidth),
      );
      scrollerEl.scrollTo({ left: target, behavior: "smooth" });
      return;
    }

    scrollerEl.scrollBy({
      top: direction * scrollerEl.clientHeight * 0.9,
      behavior: "smooth",
    });
  }

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement ||
      target?.isContentEditable
    ) {
      return;
    }

    if (event.key === "ArrowRight" || event.key === "PageDown") {
      event.preventDefault();
      void pageStep(1);
    } else if (event.key === "ArrowLeft" || event.key === "PageUp") {
      event.preventDefault();
      void pageStep(-1);
    } else if (event.key === "Escape") {
      selection = null;
      activeAnnotation = null;
      tocOpen = false;
      settingsOpen = false;
    }
  }

  function onContentMouseUp() {
    window.setTimeout(captureSelection, 0);
  }

  function captureSelection() {
    if (!contentEl) {
      selection = null;
      return;
    }

    const current = window.getSelection();
    if (!current || current.rangeCount === 0 || current.isCollapsed) {
      selection = null;
      return;
    }

    const range = current.getRangeAt(0);
    if (!contentEl.contains(range.commonAncestorContainer)) {
      selection = null;
      return;
    }

    const start = offsetAt(contentEl, range.startContainer, range.startOffset);
    const end = offsetAt(contentEl, range.endContainer, range.endOffset);
    const text = current.toString().trim();

    if (end <= start || !text) {
      selection = null;
      return;
    }

    const rect = range.getBoundingClientRect();
    selection = {
      start,
      end,
      text: text.slice(0, 2000),
      x: rect.left + rect.width / 2,
      y: rect.top,
    };
  }

  function offsetAt(root: HTMLElement, node: Node, offset: number): number {
    const range = document.createRange();
    range.selectNodeContents(root);
    range.setEnd(node, offset);
    return range.toString().length;
  }

  function pointAt(
    root: HTMLElement,
    target: number,
  ): { node: Text; offset: number } | null {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let total = 0;
    let node: Node | null;

    while ((node = walker.nextNode())) {
      const text = node.textContent ?? "";
      if (target <= total + text.length) {
        return {
          node: node as Text,
          offset: Math.max(0, Math.min(text.length, target - total)),
        };
      }
      total += text.length;
    }

    return null;
  }

  function scheduleMarks() {
    requestAnimationFrame(() => recomputeMarks());
  }

  function recomputeMarks() {
    if (!contentEl) {
      marks = [];
      return;
    }

    const rootRect = contentEl.getBoundingClientRect();
    const next: Mark[] = [];

    for (const annotation of annotations) {
      if (
        annotation.chapterIndex !== chapterIndex ||
        annotation.offsetStart == null ||
        annotation.offsetEnd == null
      ) {
        continue;
      }

      const start = pointAt(contentEl, annotation.offsetStart);
      const end = pointAt(contentEl, annotation.offsetEnd);
      if (!start || !end) continue;

      const range = document.createRange();
      range.setStart(start.node, start.offset);
      range.setEnd(end.node, end.offset);

      const rects = Array.from(range.getClientRects())
        .filter((rect) => rect.width > 0 && rect.height > 0)
        .map((rect) => ({
          left: rect.left - rootRect.left,
          top: rect.top - rootRect.top,
          width: rect.width,
          height: rect.height,
        }));

      if (rects.length > 0) {
        next.push({
          id: annotation.id,
          color: annotation.color ?? colors[0],
          kind: annotation.kind,
          rects,
          annotation,
        });
      }
    }

    marks = next;
  }

  async function createAnnotation(kind: AnnotationKind, color: string) {
    if (!session || !selection) return;

    const current = selection;
    selection = null;
    try {
      const annotation = await readerAddAnnotation({
        bookId: session.bookId,
        chapterIndex,
        offsetStart: current.start,
        offsetEnd: current.end,
        kind,
        color,
        text: current.text,
        note: noteDraft.trim() || null,
      });

      annotations = [...annotations, annotation];
      noteDraft = "";
      window.getSelection()?.removeAllRanges();
      await tick();
      scheduleMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function openAnnotation(annotation: Annotation) {
    activeAnnotation = annotation;
    annotationNote = annotation.note ?? "";
    annotationColor = annotation.color ?? colors[0];
    selection = null;
  }

  async function saveAnnotation() {
    if (!activeAnnotation) return;

    try {
      const updated = await readerUpdateAnnotation(
        activeAnnotation.id,
        annotationColor,
        annotationNote.trim() || null,
      );
      annotations = annotations.map((item) =>
        item.id === updated.id ? updated : item,
      );
      activeAnnotation = updated;
      await tick();
      scheduleMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function deleteAnnotation() {
    if (!activeAnnotation) return;

    try {
      await readerDeleteAnnotation(activeAnnotation.id);
      annotations = annotations.filter(
        (item) => item.id !== activeAnnotation?.id,
      );
      activeAnnotation = null;
      await tick();
      scheduleMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function changeMode(mode: ReaderMode) {
    if (!settings) return;

    const fraction = chapterProgress;
    settings.mode = mode;
    await tick();
    measure();
    scrollToFraction(fraction);
    scheduleMarks();
    scheduleSettingsSave();
  }

  function settingsChanged() {
    scheduleMarks();
    scheduleSettingsSave();
  }

  function scheduleSettingsSave() {
    clearTimeout(settingsTimer);
    settingsTimer = setTimeout(() => {
      void saveSettings();
    }, 500);
  }

  async function saveSettings() {
    if (!session || !settings) return;

    try {
      const saved = await readerSaveSettings(session.bookId, {
        fontSize: settings.fontSize,
        lineHeight: settings.lineHeight,
        margin: settings.margin,
        fontFamily: settings.fontFamily,
        theme: settings.theme,
        texture: settings.texture,
        mode: settings.mode,
        zoom: settings.zoom,
      });
      settings = saved;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function chapterLabel(index: number): string {
    return session?.document.sections[index]?.title ?? `Capítulo ${index + 1}`;
  }
</script>

<section class="reader">
  <header class="toolbar">
    <div class="heading">
      <h1>{session?.document.title ?? "Lector"}</h1>
      <p class="subtitle">
        {#if session}
          {session.document.author ?? "Autor desconocido"}
          {#if chapter}
            · {chapter.title}
          {/if}
        {:else}
          Elige un libro para empezar a leer.
        {/if}
      </p>
    </div>

    {#if session}
      <div class="progress" aria-label="Progreso de lectura">
        <span>{Math.round(bookProgress * 100)}% del libro</span>
        <span class="progress-track">
          <span class="progress-fill" style:width={`${bookProgress * 100}%`}></span>
        </span>
        <span>{Math.round(chapterProgress * 100)}% del capítulo</span>
      </div>

      <div class="controls">
        <button class="ghost" onclick={() => (tocOpen = !tocOpen)}>Índice</button>
        <button class="ghost" onclick={() => (settingsOpen = !settingsOpen)}>Aa</button>
        <button
          class="ghost"
          onclick={() => goToChapter(chapterIndex - 1)}
          disabled={chapterIndex <= 0 || loading}
          title="Capítulo anterior"
        >
          ⏮
        </button>
        <button class="ghost" onclick={() => pageStep(-1)} disabled={loading} title="Página anterior">
          ‹
        </button>
        <button class="ghost" onclick={() => pageStep(1)} disabled={loading} title="Página siguiente">
          ›
        </button>
        <button
          class="ghost"
          onclick={() => goToChapter(chapterIndex + 1)}
          disabled={chapterIndex >= sectionCount - 1 || loading}
          title="Capítulo siguiente"
        >
          ⏭
        </button>
      </div>
    {/if}
  </header>

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {/if}

  {#if !session}
    <div class="picker">
      <h2>Abrir un libro</h2>
      <p class="hint">
        Los EPUB se leen aquí mismo. Los PDF llegarán en la siguiente fase.
      </p>
      <div class="picker-row">
        <select bind:value={selectedBookId} aria-label="Libro para leer">
          <option value="">Selecciona un libro…</option>
          {#each books as book (book.id)}
            <option value={book.id}>{book.title} · {book.format.toUpperCase()}</option>
          {/each}
        </select>
        <button
          class="primary"
          onclick={openSelected}
          disabled={!selectedBookId || opening}
        >
          {opening ? "Abriendo…" : "Leer"}
        </button>
      </div>
      {#if books.length === 0}
        <p class="hint">Todavía no hay libros. Impórtalos desde la biblioteca.</p>
      {/if}
    </div>
  {:else}
    <div
      class="surface {themeClass} {textureClass}"
      style={surfaceStyle}
      bind:this={scrollerEl}
      class:paged={settings?.mode === "paged"}
      style:--reader-page-width={pageWidth ? `${pageWidth}px` : undefined}
      style:--reader-page-height={pageHeight ? `${pageHeight}px` : undefined}
      onscroll={onScroll}
    >
      {#if chapter}
        {#key chapterIndex}
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <article
            class="chapter"
            bind:this={contentEl}
            in:fade={{ duration: 180 }}
            onmouseup={onContentMouseUp}
            onkeyup={onContentMouseUp}
          >
            <!-- El HTML llega saneado desde Rust; nunca contiene scripts ni estilos del EPUB. -->
            {@html chapter.html}

            <div class="annotation-layer">
              {#each marks as mark (mark.id)}
                {#each mark.rects as rect, index (index)}
                  <button
                    class="annotation kind-{mark.kind}"
                    style:left={`${rect.left}px`}
                    style:top={`${rect.top}px`}
                    style:width={`${rect.width}px`}
                    style:height={`${rect.height}px`}
                    style:--mark-color={mark.color}
                    onclick={() => openAnnotation(mark.annotation)}
                    title={mark.annotation.note ?? mark.annotation.text ?? "Anotación"}
                    aria-label="Abrir anotación"
                  ></button>
                {/each}
              {/each}
            </div>
          </article>
        {/key}
      {:else if loading}
        <p class="loading">Cargando capítulo…</p>
      {/if}
    </div>

    <footer class="reader-footer">
      <span>{chapterLabel(chapterIndex)}</span>
      <span>{chapterIndex + 1} de {sectionCount}</span>
    </footer>
  {/if}

  {#if selection}
    <div
      class="selection-toolbar"
      style:left={`${selection.x}px`}
      style:top={`${Math.max(10, selection.y - 58)}px`}
    >
      {#each colors as color (color)}
        <button
          class="swatch"
          style:background={color}
          aria-label={`Resaltar con ${color}`}
          onclick={() => createAnnotation("highlight", color)}
        ></button>
      {/each}
      <button class="ghost small" onclick={() => createAnnotation("underline", colors[0])}>
        Subrayar
      </button>
      <input
        class="note-input"
        type="text"
        bind:value={noteDraft}
        placeholder="Nota (opcional)"
        aria-label="Nota de la anotación"
      />
      <button class="primary small" onclick={() => createAnnotation("highlight", colors[0])}>
        Guardar
      </button>
    </div>
  {/if}

  {#if tocOpen && session}
    <aside class="panel toc">
      <header>
        <h2>Índice</h2>
        <button class="ghost small" onclick={() => (tocOpen = false)}>Cerrar</button>
      </header>
      <ol>
        {#each session.document.sections as section, index (section.id)}
          <li>
            <button
              class:active={index === chapterIndex}
              onclick={() => goToChapter(index)}
            >
              {section.title ?? `Capítulo ${index + 1}`}
            </button>
          </li>
        {/each}
      </ol>
    </aside>
  {/if}

  {#if settingsOpen && settings}
    <aside class="panel settings">
      <header>
        <h2>Ajustes de lectura</h2>
        <button class="ghost small" onclick={() => (settingsOpen = false)}>Cerrar</button>
      </header>

      <label>
        <span>Tamaño de fuente <strong>{settings.fontSize}px</strong></span>
        <input
          type="range"
          min="12"
          max="36"
          step="1"
          bind:value={settings.fontSize}
          oninput={settingsChanged}
        />
      </label>

      <label>
        <span>Interlineado <strong>{settings.lineHeight.toFixed(1)}</strong></span>
        <input
          type="range"
          min="1"
          max="2.5"
          step="0.1"
          bind:value={settings.lineHeight}
          oninput={settingsChanged}
        />
      </label>

      <label>
        <span>Márgenes <strong>{settings.margin}px</strong></span>
        <input
          type="range"
          min="0"
          max="160"
          step="4"
          bind:value={settings.margin}
          oninput={settingsChanged}
        />
      </label>

      <label>
        <span>Zoom <strong>{settings.zoom.toFixed(1)}×</strong></span>
        <input
          type="range"
          min="0.6"
          max="2.5"
          step="0.1"
          bind:value={settings.zoom}
          oninput={settingsChanged}
        />
      </label>

      <label>
        <span>Tipografía</span>
        <select bind:value={settings.fontFamily} onchange={settingsChanged}>
          <option value="serif">Serif</option>
          <option value="sans">Sans</option>
          <option value="mono">Mono</option>
        </select>
      </label>

      <label>
        <span>Tema</span>
        <select bind:value={settings.theme} onchange={settingsChanged}>
          <option value="light">Claro</option>
          <option value="dark">Oscuro</option>
          <option value="sepia">Sepia</option>
        </select>
      </label>

      <label>
        <span>Textura</span>
        <select bind:value={settings.texture} onchange={settingsChanged}>
          <option value="none">Lisa</option>
          <option value="paper">Papel</option>
          <option value="linen">Lino</option>
          <option value="grid">Cuadrícula</option>
        </select>
      </label>

      <label>
        <span>Modo</span>
        <select
          value={settings.mode}
          onchange={(event) =>
            changeMode((event.currentTarget as HTMLSelectElement).value as ReaderMode)}
        >
          <option value="scroll">Desplazamiento</option>
          <option value="paged">Paginado</option>
        </select>
      </label>
    </aside>
  {/if}

  {#if activeAnnotation}
    <aside class="panel annotation-editor">
      <header>
        <h2>Anotación</h2>
        <button class="ghost small" onclick={() => (activeAnnotation = null)}>Cerrar</button>
      </header>
      {#if activeAnnotation.text}
        <blockquote>{activeAnnotation.text}</blockquote>
      {/if}
      <textarea bind:value={annotationNote} rows="3" placeholder="Escribe una nota…"></textarea>
      <div class="colors">
        {#each colors as color (color)}
          <button
            class="swatch"
            class:selected={annotationColor === color}
            style:background={color}
            aria-label={`Color ${color}`}
            onclick={() => (annotationColor = color)}
          ></button>
        {/each}
      </div>
      <div class="editor-actions">
        <button class="primary small" onclick={saveAnnotation}>Guardar nota</button>
        <button class="danger small" onclick={deleteAnnotation}>Borrar</button>
      </div>
    </aside>
  {/if}
</section>

<style>
  .reader {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    position: relative;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.8rem 1rem;
    align-items: flex-end;
    justify-content: space-between;
  }

  .heading {
    min-width: 12rem;
  }

  h1 {
    margin: 0;
    font-size: 1.55rem;
  }

  .subtitle {
    margin: 0.25rem 0 0;
    color: var(--muted);
    font-size: 0.86rem;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  button {
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    padding: 0.5rem 0.8rem;
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .small {
    padding: 0.3rem 0.55rem;
    font-size: 0.76rem;
  }

  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-contrast);
    font-weight: 600;
  }

  .danger {
    border-color: color-mix(in srgb, var(--warn) 60%, var(--border));
    color: var(--warn);
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.74rem;
    color: var(--muted);
  }

  .progress-track {
    width: 7rem;
    height: 6px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .progress-fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 180ms ease;
  }

  .notice {
    margin: 0;
    padding: 0.7rem 0.9rem;
    border-radius: 10px;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
    font-size: 0.85rem;
  }

  .picker {
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 1.5rem;
    background: var(--surface);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-width: 40rem;
  }

  .picker h2 {
    margin: 0;
    font-size: 1.15rem;
  }

  .picker-row {
    display: flex;
    gap: 0.5rem;
  }

  .picker select {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.9rem;
    padding: 0.55rem 0.7rem;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .surface {
    position: relative;
    border: 1px solid var(--reader-border);
    border-radius: 18px;
    background-color: var(--reader-bg);
    background-image: var(--reader-texture);
    color: var(--reader-text);
    height: min(72vh, 900px);
    overflow-y: auto;
    overscroll-behavior: contain;
    scroll-behavior: smooth;
    -webkit-overflow-scrolling: touch;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--reader-text) 4%, transparent);
    transition: background-color 160ms ease, color 160ms ease;
  }

  .surface.paged {
    overflow-x: auto;
    overflow-y: hidden;
    scroll-snap-type: x proximity;
  }

  .chapter {
    position: relative;
    max-width: 72ch;
    margin: 0 auto;
    padding: 2.5rem var(--reader-margin);
    font-family: var(--reader-font-family);
    font-size: calc(var(--reader-font-size) * var(--reader-zoom));
    line-height: var(--reader-line-height);
    color: var(--reader-text);
    overflow-wrap: anywhere;
  }

  .surface.paged .chapter {
    max-width: none;
    margin: 0;
    height: var(--reader-page-height);
    padding: 2rem var(--reader-margin);
    column-width: calc(var(--reader-page-width) - 2 * var(--reader-margin));
    column-gap: calc(2 * var(--reader-margin));
    column-fill: auto;
  }

  .chapter :global(p) {
    margin: 0 0 1em;
  }

  .chapter :global(h1),
  .chapter :global(h2),
  .chapter :global(h3),
  .chapter :global(h4) {
    line-height: 1.25;
    margin: 1.4em 0 0.6em;
  }

  .chapter :global(h1) {
    font-size: 1.7em;
  }

  .chapter :global(h2) {
    font-size: 1.4em;
  }

  .chapter :global(blockquote) {
    margin: 1.2em 0;
    padding-left: 1em;
    border-left: 3px solid color-mix(in srgb, var(--reader-text) 25%, transparent);
    color: var(--reader-muted);
    font-style: italic;
  }

  .chapter :global(ul),
  .chapter :global(ol) {
    padding-left: 1.4em;
    margin: 0 0 1em;
  }

  .chapter :global(li) {
    margin-bottom: 0.35em;
  }

  .chapter :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 1.5em auto;
    border-radius: 8px;
  }

  .chapter :global(figure) {
    margin: 1.5em 0;
    text-align: center;
  }

  .chapter :global(figcaption) {
    font-size: 0.82em;
    color: var(--reader-muted);
  }

  .chapter :global(hr) {
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--reader-text) 18%, transparent);
    margin: 2em 0;
  }

  .chapter :global(a) {
    color: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--reader-text) 40%, transparent);
  }

  .chapter :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 1.2em 0;
    font-size: 0.92em;
  }

  .chapter :global(td),
  .chapter :global(th) {
    border: 1px solid color-mix(in srgb, var(--reader-text) 20%, transparent);
    padding: 0.4em 0.55em;
    text-align: left;
  }

  .chapter :global(pre) {
    overflow-x: auto;
    padding: 0.8em;
    border-radius: 8px;
    background: color-mix(in srgb, var(--reader-text) 7%, transparent);
    font-family: var(--reader-font-family);
  }

  .chapter :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.9em;
  }

  .loading {
    margin: 3rem auto;
    text-align: center;
    color: var(--reader-muted);
  }

  .annotation-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: visible;
  }

  .annotation {
    position: absolute;
    border: 0;
    padding: 0;
    border-radius: 2px;
    pointer-events: auto;
    cursor: pointer;
    background: color-mix(in srgb, var(--mark-color) 38%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--mark-color) 55%, transparent);
    mix-blend-mode: multiply;
  }

  .annotation.kind-underline {
    background: transparent;
    box-shadow: none;
    border-bottom: 2px solid var(--mark-color);
    border-radius: 0;
  }

  .annotation.kind-strike {
    background: transparent;
    box-shadow: none;
    background-image: linear-gradient(
      to bottom,
      transparent calc(50% - 1px),
      var(--mark-color) calc(50% - 1px),
      var(--mark-color) calc(50% + 1px),
      transparent calc(50% + 1px)
    );
  }

  .annotation.kind-bookmark {
    width: 4px !important;
    border-radius: 0 0 3px 3px;
    background: var(--mark-color);
    box-shadow: none;
  }

  .annotation:hover {
    filter: brightness(0.94);
  }

  .reader-footer {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .selection-toolbar {
    position: fixed;
    transform: translateX(-50%);
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.4rem 0.5rem;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: 0 12px 32px color-mix(in srgb, #000 18%, transparent);
  }

  .swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    padding: 0;
  }

  .swatch.selected {
    border-color: var(--text);
  }

  .note-input {
    width: 11rem;
    font: inherit;
    font-size: 0.8rem;
    padding: 0.35rem 0.5rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .panel {
    position: fixed;
    z-index: 40;
    top: 5.5rem;
    right: 1.25rem;
    width: min(22rem, calc(100vw - 2.5rem));
    max-height: min(70vh, 40rem);
    overflow-y: auto;
    padding: 1rem;
    border-radius: 16px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: 0 18px 50px color-mix(in srgb, #000 22%, transparent);
  }

  .panel header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .panel h2 {
    margin: 0;
    font-size: 1rem;
  }

  .toc ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .toc li button {
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    padding: 0.45rem 0.55rem;
    border-radius: 8px;
    font-size: 0.85rem;
  }

  .toc li button.active {
    background: var(--surface-2);
    font-weight: 600;
  }

  .settings {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .settings label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.82rem;
    color: var(--muted);
  }

  .settings label span {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .settings strong {
    color: var(--text);
    font-weight: 600;
  }

  .settings input[type="range"] {
    accent-color: var(--accent);
  }

  .settings select {
    font: inherit;
    font-size: 0.85rem;
    padding: 0.45rem 0.55rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .annotation-editor {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .annotation-editor blockquote {
    margin: 0;
    padding-left: 0.7rem;
    border-left: 3px solid var(--border);
    color: var(--muted);
    font-size: 0.85rem;
  }

  .annotation-editor textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    font-size: 0.86rem;
    padding: 0.55rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .colors {
    display: flex;
    gap: 0.4rem;
  }

  .editor-actions {
    display: flex;
    gap: 0.4rem;
    justify-content: flex-end;
  }

  /* Temas del contenido: independientes del tema global de la aplicación. */
  .theme-light {
    --reader-bg: #fbf8f1;
    --reader-text: #241f19;
    --reader-muted: #6e6457;
    --reader-border: #ddd5c6;
    --reader-texture: none;
  }

  .theme-dark {
    --reader-bg: #181a18;
    --reader-text: #e8e5dc;
    --reader-muted: #a9a396;
    --reader-border: #343832;
    --reader-texture: none;
  }

  .theme-sepia {
    --reader-bg: #f2e6ce;
    --reader-text: #3d3021;
    --reader-muted: #7c6a52;
    --reader-border: #d8c6a6;
    --reader-texture: none;
  }

  .texture-paper {
    --reader-texture:
      radial-gradient(circle at 18% 22%, color-mix(in srgb, var(--reader-text) 3%, transparent) 0 1px, transparent 1px),
      radial-gradient(circle at 74% 68%, color-mix(in srgb, var(--reader-text) 2.5%, transparent) 0 1px, transparent 1px),
      repeating-linear-gradient(115deg, transparent 0 3px, color-mix(in srgb, var(--reader-text) 1.6%, transparent) 3px 4px);
    background-size: 7px 7px, 11px 11px, 100% 100%;
  }

  .texture-linen {
    --reader-texture:
      repeating-linear-gradient(0deg, transparent 0 2px, color-mix(in srgb, var(--reader-text) 2.4%, transparent) 2px 3px),
      repeating-linear-gradient(90deg, transparent 0 2px, color-mix(in srgb, var(--reader-text) 1.8%, transparent) 2px 3px);
  }

  .texture-grid {
    --reader-texture:
      repeating-linear-gradient(0deg, transparent 0 23px, color-mix(in srgb, var(--reader-text) 5%, transparent) 23px 24px),
      repeating-linear-gradient(90deg, transparent 0 23px, color-mix(in srgb, var(--reader-text) 5%, transparent) 23px 24px);
  }

  @media (max-width: 760px) {
    .progress {
      order: 3;
      width: 100%;
    }

    .surface {
      height: calc(100vh - 18rem);
      min-height: 20rem;
    }

    .panel {
      top: auto;
      bottom: 1rem;
      right: 1rem;
      left: 1rem;
      width: auto;
      max-height: 55vh;
    }
  }
</style>

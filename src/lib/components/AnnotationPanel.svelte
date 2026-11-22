<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";
  import {
    addExtract,
    addNote,
    addReaction,
    addVoiceNote,
    cancelVoiceNoteRecording,
    deleteAnnotation,
    errorMessage,
    listAnnotations,
    reactionMap,
    searchExtracts,
    startVoiceNoteRecording,
    updateMarker,
    updateNote,
  } from "../api";
  import type {
    Annotation,
    AnnotationAnchor,
    AnnotationKind,
    Extract,
    Note,
    Reaction,
    ReactionBucket,
    ReactionKind,
    VoiceNote,
  } from "../types";
  import Icon from "./Icon.svelte";
  import RichNoteEditor from "./RichNoteEditor.svelte";

  type Tab = "marker" | "note" | "extract" | "reaction";

  interface Props {
    bookId: string;
    bookTitle: string;
    format: "epub" | "pdf";
    sectionCount: number;
    sections: { index: number; title: string }[];
    currentSection: number;
    annotations: Annotation[];
    anchor: AnnotationAnchor;
    open: boolean;
    initialTab?: Tab;
    refreshToken?: number;
    recordToken?: number;
    onClose: () => void;
    onJump: (section: number, offset: number) => void;
    onRefreshMarkers: () => void | Promise<void>;
    onAddMarker: () => void;
    onMarkSelection: (kind: AnnotationKind, color: string) => void | Promise<void>;
    onCopyQuote?: () => void;
    onLookup?: () => void;
    onListenSelection?: () => void;
    onError: (message: string) => void;
  }

  let {
    bookId,
    bookTitle,
    format,
    sectionCount,
    sections,
    currentSection,
    annotations,
    anchor,
    open,
    initialTab = "marker",
    refreshToken = 0,
    recordToken = 0,
    onClose,
    onJump,
    onRefreshMarkers,
    onAddMarker,
    onMarkSelection,
    onCopyQuote,
    onLookup,
    onListenSelection,
    onError,
  }: Props = $props();

  const markerPalette = [
    "#ffd166",
    "#8ecae6",
    "#b7e4c7",
    "#ffb4a2",
    "#cdb4db",
    "#f4a261",
    "#90be6d",
    "#e9c46a",
  ];

  const reactionKinds: {
    kind: ReactionKind;
    emoji: string;
    label: string;
  }[] = [
    { kind: "like", emoji: "👍", label: "Me gusta" },
    { kind: "surprise", emoji: "😮", label: "Sorpresa" },
    { kind: "tension", emoji: "😬", label: "Tensión" },
    { kind: "boredom", emoji: "😴", label: "Aburrimiento" },
    { kind: "emotion", emoji: "🥹", label: "Emoción" },
  ];

  let activeTab = $state<Tab>("marker");
  let notes = $state<Note[]>([]);
  let voiceNotes = $state<VoiceNote[]>([]);
  let extracts = $state<Extract[]>([]);
  let reactions = $state<Reaction[]>([]);
  let buckets = $state<ReactionBucket[]>([]);
  let loading = $state(false);
  let localError = $state<string | null>(null);
  let extractQuery = $state("");
  let filterQuery = $state("");
  let markColor = $state(markerPalette[0]);
  let editingNote = $state<Note | null>(null);
  let composerOpen = $state(false);
  let viewingNote = $state<Note | null>(null);
  let expandedMarker = $state<string | null>(null);
  let recording = $state(false);
  let recordSeconds = $state(0);
  let recordTimer: ReturnType<typeof setInterval> | undefined;
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let lastRefresh = 0;
  let lastRecord = 0;
  let lastBook = "";
  let lastInitialTab: Tab | null = null;

  let sectionTitles = $derived(
    new Map(sections.map((section) => [section.index, section.title])),
  );
  let noteCount = $derived(notes.length + voiceNotes.length);
  let filteredAnnotations = $derived(
    annotations.filter((marker) => matches(marker.text, marker.note, marker.chapterIndex)),
  );
  let filteredNotes = $derived(
    notes.filter((note) => matches(note.title, note.body, note.text, note.chapterIndex)),
  );
  let filteredVoiceNotes = $derived(
    voiceNotes.filter((note) => matches(note.text, note.chapterIndex)),
  );
  let filteredExtracts = $derived(
    extracts.filter((extract) => matches(extract.text, extract.note, extract.citation)),
  );
  let filteredReactions = $derived(
    reactions.filter((reaction) => matches(reaction.text, reaction.chapterIndex)),
  );
  function matches(...values: (string | number | null | undefined)[]): boolean {
    const needle = filterQuery.trim().toLocaleLowerCase();
    if (!needle) return true;
    return values
      .filter((value) => value != null)
      .some((value) => String(value).toLocaleLowerCase().includes(needle));
  }
  let maxReaction = $derived(
    Math.max(1, ...buckets.map((bucket) => bucket.total)),
  );
  let heatmap = $derived(
    Array.from({ length: Math.max(0, sectionCount) }, (_, index) => {
      const bucket = buckets.find((item) => item.chapterIndex === index);
      return {
        index,
        total: bucket?.total ?? 0,
        bucket,
      };
    }),
  );

  $effect(() => {
    if (!open || !bookId) {
      lastBook = "";
      lastInitialTab = null;
      return;
    }
    if (lastBook !== bookId || lastInitialTab !== initialTab) {
      lastBook = bookId;
      lastInitialTab = initialTab;
      activeTab = initialTab;
      void refreshAll();
    }
  });

  $effect(() => {
    if (refreshToken !== lastRefresh) {
      lastRefresh = refreshToken;
      if (open) void refreshAll();
    }
  });

  $effect(() => {
    // Cada pasaje nuevo vuelve al color inicial del resaltador.
    if (anchor.text) markColor = markerPalette[0];
  });

  $effect(() => {
    if (recordToken !== lastRecord) {
      lastRecord = recordToken;
      if (open && !recording) void startRecording();
    }
  });

  $effect(() => {
    if (!open) {
      closeModal();
      if (recording) {
        void cancelVoiceNoteRecording();
        stopRecordTimer();
        recording = false;
      }
    }
  });

  onDestroy(() => {
    stopRecordTimer();
    if (searchTimer) clearTimeout(searchTimer);
    if (recording) void cancelVoiceNoteRecording();
  });

  async function refreshAll() {
    if (!bookId) return;
    loading = true;
    localError = null;
    try {
      const [noteEntries, voiceEntries, reactionEntries, map] =
        await Promise.all([
          listAnnotations(bookId, "note"),
          listAnnotations(bookId, "voice_note"),
          listAnnotations(bookId, "reaction"),
          reactionMap(bookId),
        ]);
      notes = noteEntries.flatMap((entry) =>
        entry.type === "note" ? [entry.data] : [],
      );
      voiceNotes = voiceEntries.flatMap((entry) =>
        entry.type === "voice_note" ? [entry.data] : [],
      );
      reactions = reactionEntries.flatMap((entry) =>
        entry.type === "reaction" ? [entry.data] : [],
      );
      buckets = map;
      await loadExtracts();
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    } finally {
      loading = false;
    }
  }

  async function loadExtracts() {
    try {
      if (extractQuery.trim()) {
        extracts = await searchExtracts(extractQuery.trim(), bookId, 200);
      } else {
        const entries = await listAnnotations(bookId, "extract");
        extracts = entries.flatMap((entry) =>
          entry.type === "extract" ? [entry.data] : [],
        );
      }
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  function onExtractInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void loadExtracts(), 250);
  }

  function sectionTitle(index: number): string {
    return (
      sectionTitles.get(index) ??
      (format === "pdf" ? `Página ${index + 1}` : `Sección ${index + 1}`)
    );
  }

  function jump(section: number | null | undefined, offset: number | null | undefined) {
    onJump(Math.max(0, section ?? 0), Math.max(0, offset ?? 0));
  }

  function openComposer() {
    localError = null;
    composerOpen = true;
    editingNote = null;
    viewingNote = null;
  }

  function openEditor(note: Note) {
    localError = null;
    editingNote = note;
    composerOpen = false;
    viewingNote = null;
  }

  function openViewer(note: Note) {
    localError = null;
    viewingNote = note;
    composerOpen = false;
    editingNote = null;
  }

  function closeModal() {
    composerOpen = false;
    editingNote = null;
    viewingNote = null;
    localError = null;
  }

  async function saveNewNote(payload: {
    title: string | null;
    contentJson: string;
    plainText: string;
  }) {
    try {
      await addNote({
        bookId,
        chapterIndex: anchor.section,
        offsetStart: anchor.offsetStart,
        offsetEnd: anchor.offsetEnd,
        text: anchor.text.trim() || null,
        title: payload.title,
        contentJson: payload.contentJson,
        plainText: payload.plainText,
      });
      closeModal();
      await refreshAll();
      activeTab = "note";
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  async function saveEditedNote(payload: {
    title: string | null;
    contentJson: string;
    plainText: string;
  }) {
    if (!editingNote) return;
    try {
      await updateNote({
        noteId: editingNote.id,
        title: payload.title,
        contentJson: payload.contentJson,
        plainText: payload.plainText,
      });
      closeModal();
      await refreshAll();
      activeTab = "note";
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  async function createExtract() {
    if (!anchor.text.trim()) {
      localError = "Selecciona un pasaje de texto para crear un extracto.";
      onError(localError);
      return;
    }
    try {
      await addExtract({
        bookId,
        chapterIndex: anchor.section,
        offsetStart: anchor.offsetStart,
        offsetEnd: anchor.offsetEnd,
        text: anchor.text.trim(),
        note: null,
        citation: null,
      });
      await refreshAll();
      await onRefreshMarkers();
      activeTab = "extract";
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  async function react(kind: ReactionKind) {
    try {
      await addReaction({
        bookId,
        chapterIndex: anchor.section,
        offsetStart: anchor.offsetStart,
        offsetEnd: anchor.offsetEnd,
        kind,
        text: null,
      });
      await refreshAll();
      await onRefreshMarkers();
      activeTab = "reaction";
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  async function startRecording() {
    if (recording) return;
    try {
      await startVoiceNoteRecording();
      if (!open) {
        void cancelVoiceNoteRecording();
        return;
      }
      recording = true;
      recordSeconds = 0;
      stopRecordTimer();
      recordTimer = setInterval(() => {
        recordSeconds += 1;
        if (recordSeconds >= 300) {
          void stopRecording();
        }
      }, 1000);
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  async function stopRecording() {
    try {
      await addVoiceNote({
        bookId,
        chapterIndex: anchor.section,
        offsetStart: anchor.offsetStart,
        offsetEnd: anchor.offsetEnd,
        text: anchor.text.trim() || null,
      });
      await refreshAll();
      activeTab = "note";
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
      // Si falló antes de detener el micrófono, no lo dejamos abierto.
      void cancelVoiceNoteRecording();
    } finally {
      stopRecordTimer();
      recording = false;
    }
  }

  function cancelRecording() {
    void cancelVoiceNoteRecording();
    stopRecordTimer();
    recording = false;
  }

  function stopRecordTimer() {
    if (recordTimer) {
      clearInterval(recordTimer);
      recordTimer = undefined;
    }
  }

  async function changeMarkerColor(marker: Annotation, color: string) {
    try {
      await updateMarker(marker.id, color, marker.note);
      await onRefreshMarkers();
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  async function remove(kind: "marker" | "note" | "voice_note" | "extract" | "reaction", id: string) {
    if (!window.confirm("¿Borrar esta anotación?")) return;
    try {
      await deleteAnnotation(kind, id);
      if (kind === "marker" || kind === "extract" || kind === "reaction") {
        await onRefreshMarkers();
      }
      await refreshAll();
    } catch (cause) {
      localError = errorMessage(cause);
      onError(localError);
    }
  }

  function formatDuration(milliseconds: number): string {
    const total = Math.max(0, Math.round(milliseconds / 1000));
    const minutes = Math.floor(total / 60);
    const seconds = total % 60;
    return `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  function formatDate(value: string): string {
    return new Date(value).toLocaleDateString("es", {
      day: "2-digit",
      month: "short",
    });
  }

  function excerpt(value: string, length = 160): string {
    const clean = value.replace(/\s+/g, " ").trim();
    return clean.length > length ? `${clean.slice(0, length)}…` : clean;
  }

  function audioUrl(path: string | null): string | null {
    return path ? convertFileSrc(path) : null;
  }

  function fallbackDelta(note: Note): string {
    return (
      note.contentJson ??
      JSON.stringify({ ops: [{ insert: `${note.body}\n` }] })
    );
  }

  function reactionSummary(bucket: ReactionBucket | undefined): string {
    if (!bucket) return "Sin reacciones";
    return reactionKinds
      .filter((item) => bucket[item.kind] > 0)
      .map((item) => `${item.emoji} ${bucket[item.kind]}`)
      .join(" · ");
  }
</script>

{#if open}
  <aside class="annotation-panel" aria-label="Anotaciones del libro">
    <header class="panel-head">
      <div class="panel-heading">
        <span class="panel-mark"><Icon name="bookmark" size={18} /></span>
        <div>
          <h2>Marcadores y citas</h2>
          <p>{bookTitle} · {format === "pdf" ? "PDF" : "EPUB"}</p>
        </div>
      </div>
      <button class="close" aria-label="Cerrar" onclick={onClose}>
        <Icon name="close" size={17} />
      </button>
    </header>

    <label class="panel-search">
      <Icon name="search" size={15} />
      <input
        bind:value={filterQuery}
        placeholder="Buscar en notas, citas o páginas…"
        aria-label="Buscar anotaciones"
      />
      {#if filterQuery}
        <button
          class="search-clear"
          aria-label="Limpiar búsqueda"
          onclick={() => (filterQuery = "")}
        >
          <Icon name="close" size={13} />
        </button>
      {/if}
    </label>

    <div class="tabs" aria-label="Tipos de anotación" role="tablist">
      <button
        class:active={activeTab === "marker"}
        role="tab"
        aria-selected={activeTab === "marker"}
        onclick={() => (activeTab = "marker")}
      >
        Marcadores <span class="count">{annotations.length}</span>
      </button>
      <button
        class:active={activeTab === "note"}
        role="tab"
        aria-selected={activeTab === "note"}
        onclick={() => (activeTab = "note")}
      >
        Notas <span class="count">{noteCount}</span>
      </button>
      <button
        class:active={activeTab === "extract"}
        role="tab"
        aria-selected={activeTab === "extract"}
        onclick={() => (activeTab = "extract")}
      >
        Citas <span class="count">{extracts.length}</span>
      </button>
      <button
        class:active={activeTab === "reaction"}
        role="tab"
        aria-selected={activeTab === "reaction"}
        onclick={() => (activeTab = "reaction")}
      >
        Reacciones <span class="count">{reactions.length}</span>
      </button>
    </div>

    {#if localError}
      <p class="error" role="alert">{localError}</p>
    {/if}
    {#if loading}
      <p class="hint">Cargando…</p>
    {/if}

    {#if activeTab === "marker"}
      <div class="tab-body">
        {#if anchor.text.trim()}
          <div class="selection-mark">
            <span class="selection-mark-label">Pasaje seleccionado</span>
            <blockquote>{excerpt(anchor.text, 220)}</blockquote>
            <div
              class="selection-mark-colors"
              aria-label="Color del resaltado"
              role="group"
            >
              {#each markerPalette as color (color)}
                <button
                  class="palette-swatch"
                  class:selected={markColor === color}
                  style:background={color}
                  aria-label={`Usar color ${color}`}
                  aria-pressed={markColor === color}
                  onclick={() => (markColor = color)}
                ></button>
              {/each}
            </div>
            <div class="tab-actions">
              <button
                class="primary small"
                onclick={() => onMarkSelection("highlight", markColor)}
              >
                Resaltar
              </button>
              <button
                class="ghost small"
                onclick={() => onMarkSelection("underline", markColor)}
              >
                Subrayar
              </button>
              {#if onListenSelection}
                <button class="ghost small" onclick={onListenSelection}>
                  <Icon name="headphones" size={14} />
                  Escuchar
                </button>
              {/if}
              {#if onCopyQuote}
                <button class="ghost small" onclick={onCopyQuote}>Copiar cita</button>
              {/if}
              {#if onLookup}
                <button class="ghost small" onclick={onLookup}>Consultar</button>
              {/if}
            </div>
          </div>
        {/if}
        <div class="tab-actions">
          <button class="primary small" onclick={openComposer}>Nueva nota</button>
          <button class="ghost small" onclick={createExtract}>Extraer pasaje</button>
        </div>
        {#if filteredAnnotations.length === 0}
          <p class="empty">
            {filterQuery
              ? "No hay marcadores que coincidan con la búsqueda."
              : "Selecciona texto y usa un color para crear un marcador."}
          </p>
        {:else}
          {#each filteredAnnotations as marker (marker.id)}
            <article
              class="entry marker-entry"
              style:--mark-color={marker.color ?? markerPalette[0]}
            >
              <button
                class="swatch"
                style:background={marker.color ?? markerPalette[0]}
                aria-label="Editar color del marcador"
                onclick={() =>
                  (expandedMarker = expandedMarker === marker.id ? null : marker.id)}
              ></button>
              <div class="entry-main">
                {#if marker.text}
                  <p class="quote">{excerpt(marker.text, 220)}</p>
                {/if}
                <span class="meta">
                  {sectionTitle(marker.chapterIndex ?? 0)} · {formatDate(marker.createdAt)}
                </span>
                {#if marker.note}
                  <p class="note-copy">{marker.note}</p>
                {/if}
                {#if expandedMarker === marker.id}
                  <div class="palette" aria-label="Colores del marcador">
                    {#each markerPalette as color (color)}
                      <button
                        class="palette-swatch"
                        class:selected={(marker.color ?? "") === color}
                        style:background={color}
                        aria-label={`Cambiar a ${color}`}
                        onclick={() => changeMarkerColor(marker, color)}
                      ></button>
                    {/each}
                  </div>
                {/if}
              </div>
              <div class="entry-actions">
                <button
                  class="ghost small"
                  onclick={() => jump(marker.chapterIndex, marker.offsetStart)}
                >
                  Ir
                </button>
                <button class="danger small" onclick={() => remove("marker", marker.id)}>
                  Borrar
                </button>
              </div>
            </article>
          {/each}
        {/if}
      </div>
    {/if}

    {#if activeTab === "note"}
      <div class="tab-body">
        <div class="tab-actions">
          <button class="primary small" onclick={openComposer}>Nueva nota</button>
          {#if recording}
            <button class="danger small" onclick={stopRecording}>
              Detener y guardar ({formatDuration(recordSeconds * 1000)})
            </button>
            <button class="ghost small" onclick={cancelRecording}>Cancelar</button>
          {:else}
            <button class="ghost small" onclick={startRecording}>🎙 Grabar voz</button>
          {/if}
        </div>
        {#if recording}
          <p class="recording" role="status">
            Grabando para {sectionTitle(anchor.section)}…
          </p>
        {/if}

        {#if filteredNotes.length === 0 && filteredVoiceNotes.length === 0}
          <p class="empty">
            {filterQuery
              ? "No hay notas que coincidan con la búsqueda."
              : "Todavía no hay notas en este libro."}
          </p>
        {:else}
          {#each filteredNotes as note (note.id)}
            <article class="entry">
              <div class="entry-main">
                {#if note.title}
                  <p class="entry-title">{note.title}</p>
                {/if}
                <p class="quote">{excerpt(note.body, 220)}</p>
                {#if note.text}
                  <p class="note-copy">Sobre: {excerpt(note.text, 120)}</p>
                {/if}
                <span class="meta">
                  {note.chapterIndex != null ? sectionTitle(note.chapterIndex) : "Libro"}
                  · {formatDate(note.createdAt)}
                </span>
              </div>
              <div class="entry-actions">
                <button class="ghost small" onclick={() => openViewer(note)}>Ver</button>
                <button class="ghost small" onclick={() => openEditor(note)}>Editar</button>
                <button class="danger small" onclick={() => remove("note", note.id)}>Borrar</button>
              </div>
            </article>
          {/each}

          {#each filteredVoiceNotes as note (note.id)}
            <article class="entry voice">
              <div class="entry-main">
                <p class="entry-title">
                  {note.audiobookId ? "Marcador de voz" : "Nota de voz"} ·
                  {formatDuration(note.durationMs)}
                  {#if note.audiobookId}
                    <span class="voice-position">
                      {formatDuration(note.positionMs)}
                    </span>
                  {/if}
                </p>
                {#if note.text}
                  <p class="quote">{excerpt(note.text, 180)}</p>
                {/if}
                {#if note.transcript}
                  <p class="note-copy">
                    Comentario: {excerpt(note.transcript, 160)}
                  </p>
                {/if}
                <span class="meta">
                  {sectionTitle(note.chapterIndex)} · {formatDate(note.createdAt)}
                </span>
                {#if audioUrl(note.audioPath)}
                  <!-- El archivo vive en assets/notes_audio y se sirve por el protocolo asset. -->
                  <audio controls preload="none" src={audioUrl(note.audioPath) ?? undefined}></audio>
                {:else}
                  <span class="error">No se encontró el archivo de audio.</span>
                {/if}
              </div>
              <div class="entry-actions">
                <button
                  class="ghost small"
                  onclick={() => jump(note.chapterIndex, note.offsetStart)}
                >
                  Ir
                </button>
                <button class="danger small" onclick={() => remove("voice_note", note.id)}>
                  Borrar
                </button>
              </div>
            </article>
          {/each}
        {/if}
      </div>
    {/if}

    {#if activeTab === "extract"}
      <div class="tab-body">
        <label class="search">
          <span>Buscar extractos</span>
          <input
            bind:value={extractQuery}
            oninput={onExtractInput}
            placeholder="Texto, nota o cita"
          />
        </label>
        <div class="tab-actions">
          <button class="primary small" onclick={createExtract}>Extraer pasaje actual</button>
        </div>

        {#if filteredExtracts.length === 0}
          <p class="empty">No hay citas que coincidan.</p>
        {:else}
          {#each filteredExtracts as extract (extract.id)}
            <article class="entry">
              <div class="entry-main">
                <p class="quote">{excerpt(extract.text, 260)}</p>
                {#if extract.note}
                  <p class="note-copy">{extract.note}</p>
                {/if}
                <span class="meta">
                  {extract.citation ??
                    `${bookTitle} · ${sectionTitle(extract.chapterIndex ?? 0)}`}
                </span>
              </div>
              <div class="entry-actions">
                <button
                  class="ghost small"
                  onclick={() => jump(extract.chapterIndex, extract.offsetStart)}
                >
                  Ir
                </button>
                <button class="danger small" onclick={() => remove("extract", extract.id)}>
                  Borrar
                </button>
              </div>
            </article>
          {/each}
        {/if}
      </div>
    {/if}

    {#if activeTab === "reaction"}
      <div class="tab-body">
        <div class="reaction-buttons">
          {#each reactionKinds as item (item.kind)}
            <button
              class="reaction"
              title={item.label}
              aria-label={`Reaccionar con ${item.label}`}
              onclick={() => react(item.kind)}
            >
              <span>{item.emoji}</span>
              <small>{item.label}</small>
            </button>
          {/each}
        </div>

        <section class="heatmap-card" aria-label="Mapa emocional del libro">
          <div class="heatmap-head">
            <strong>Mapa emocional</strong>
            <span class="hint">{reactions.length} reacción(es)</span>
          </div>
          <div class="heatmap">
            {#each heatmap as cell (cell.index)}
              <button
                class="cell"
                class:current={cell.index === currentSection}
                style:--heat={cell.total / maxReaction}
                title={`${sectionTitle(cell.index)}: ${reactionSummary(cell.bucket)}`}
                aria-label={`${sectionTitle(cell.index)}: ${cell.total} reacciones`}
                onclick={() => jump(cell.index, 0)}
              ></button>
            {/each}
          </div>
          <div class="legend">
            {#each reactionKinds as item (item.kind)}
              <span><span aria-hidden="true">{item.emoji}</span> {item.label}</span>
            {/each}
          </div>
        </section>

        {#if filteredReactions.length === 0}
          <p class="empty">Reacciona a un pasaje para construir el mapa emocional.</p>
        {:else}
          {#each filteredReactions as reaction (reaction.id)}
            <article class="entry">
              <span class="reaction-mark" aria-hidden="true">
                {reactionKinds.find((item) => item.kind === reaction.kind)?.emoji ?? "•"}
              </span>
              <div class="entry-main">
                <p class="meta">
                  {reactionKinds.find((item) => item.kind === reaction.kind)?.label}
                  · {sectionTitle(reaction.chapterIndex)}
                  · {formatDate(reaction.createdAt)}
                </p>
                {#if reaction.text}
                  <p class="quote">{excerpt(reaction.text, 180)}</p>
                {/if}
              </div>
              <div class="entry-actions">
                <button
                  class="ghost small"
                  onclick={() => jump(reaction.chapterIndex, reaction.offsetStart)}
                >
                  Ir
                </button>
                <button class="danger small" onclick={() => remove("reaction", reaction.id)}>
                  Borrar
                </button>
              </div>
            </article>
          {/each}
        {/if}
      </div>
    {/if}

    <footer class="panel-cta">
      <button class="primary" onclick={onAddMarker}>
        <Icon name="bookmark" size={16} />
        Añadir marcador en la página actual
      </button>
    </footer>
  </aside>
{/if}

{#if composerOpen || editingNote || viewingNote}
  <div class="modal-backdrop" role="presentation" onclick={closeModal}>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      aria-label={viewingNote ? "Ver nota" : editingNote ? "Editar nota" : "Nueva nota"}
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.stopPropagation()}
    >
      <header class="note-head">
        <div class="panel-heading">
          <span class="panel-mark"><Icon name="edit" size={18} /></span>
          <div>
            <h2>{viewingNote ? "Nota marginal" : editingNote ? "Editar nota" : "Nota marginal"}</h2>
            <p>Sincronizada con el diario y las exportaciones</p>
          </div>
        </div>
        <button class="close" aria-label="Cerrar" onclick={closeModal}>
          <Icon name="close" size={17} />
        </button>
      </header>
      {#if localError}
        <p class="error" role="alert">{localError}</p>
      {/if}
      {#if anchor.text && !viewingNote && !editingNote}
        <div class="quote-card">
          <span class="quote-label">Texto seleccionado</span>
          <blockquote>{excerpt(anchor.text, 240)}</blockquote>
        </div>
      {/if}
      {#if viewingNote}
        {#key viewingNote.id}
          <RichNoteEditor initialJson={fallbackDelta(viewingNote)} readOnly />
        {/key}
      {:else if editingNote}
        {#key editingNote.id}
          <RichNoteEditor
            initialJson={fallbackDelta(editingNote)}
            initialTitle={editingNote.title}
            onSave={saveEditedNote}
            onCancel={closeModal}
          />
        {/key}
      {:else}
        {#key "new-note"}
          <RichNoteEditor onSave={saveNewNote} onCancel={closeModal} />
        {/key}
      {/if}
    </div>
  </div>
{/if}

<style>
  .annotation-panel {
    position: fixed;
    z-index: 70;
    top: 4.75rem;
    right: 1rem;
    width: min(28rem, calc(100vw - 2rem));
    max-height: min(82vh, 50rem);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 18px;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    box-shadow: var(--shadow-lg);
    backdrop-filter: blur(18px) saturate(1.08);
  }

  .annotation-panel header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.9rem 1rem 0.6rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  }

  .annotation-panel h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .hint,
  .empty {
    margin: 0.15rem 0;
    color: var(--muted);
    font-size: 0.8rem;
  }

  .tabs {
    display: flex;
    flex-wrap: nowrap;
    gap: 0.3rem;
    margin: 0;
    padding: 0.65rem 0.75rem 0.55rem;
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .tabs button {
    display: flex;
    flex: none;
    align-items: center;
    gap: 0.3rem;
    padding: 0.38rem 0.55rem;
    font-size: 0.74rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--muted);
    cursor: pointer;
  }

  .tabs button.active {
    color: var(--accent-contrast);
    border-color: var(--accent);
    background: var(--accent);
    font-weight: 600;
  }

  .tabs button.active .count {
    background: color-mix(in srgb, #fff 18%, transparent);
    color: inherit;
  }

  .count {
    padding: 0 0.3rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    font-size: 0.68rem;
  }

  .tab-body {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0 0.75rem 0.85rem;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .tab-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .selection-mark {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.55rem 0.65rem;
    border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--border));
    border-left: 3px solid var(--accent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 5%, var(--surface));
  }

  .selection-mark-label {
    color: var(--accent);
    font-size: 0.58rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .selection-mark blockquote {
    margin: 0;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.8rem;
    font-style: italic;
    line-height: 1.5;
  }

  .selection-mark-colors {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .selection-mark-colors .palette-swatch {
    width: 1.25rem;
    height: 1.25rem;
  }

  .entry {
    display: flex;
    align-items: flex-start;
    gap: 0.55rem;
    padding: 0.6rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    transition:
      border-color 150ms ease,
      background 150ms ease;
  }

  .entry:hover {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
    background: color-mix(in srgb, var(--accent) 4%, var(--surface-2));
  }

  .marker-entry {
    box-shadow: inset 3px 0 0 var(--mark-color, var(--accent));
    padding-left: 0.8rem;
  }

  .marker-entry .swatch {
    align-self: stretch;
    width: 0.35rem;
    height: auto;
    min-height: 2.2rem;
    border-radius: 999px;
    border: 0;
  }

  .entry-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .entry-title {
    margin: 0;
    font-size: 0.84rem;
    font-weight: 600;
  }

  .voice-position {
    display: inline-block;
    margin-left: 4px;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.64rem;
    font-variant-numeric: tabular-nums;
  }

  .quote,
  .note-copy {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.4;
  }

  .quote {
    color: var(--text);
  }

  .note-copy {
    color: var(--muted);
    font-style: italic;
  }

  .meta {
    color: var(--muted);
    font-size: 0.72rem;
  }

  .entry-actions {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: 0.25rem;
  }

  .entry-actions button {
    min-width: 3.2rem;
  }

  .swatch,
  .palette-swatch {
    width: 1.1rem;
    height: 1.1rem;
    min-height: 0;
    flex: none;
    padding: 0;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, #000 25%, transparent);
    cursor: pointer;
  }

  .palette {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-top: 0.25rem;
  }

  .palette-swatch {
    width: 1rem;
    height: 1rem;
  }

  .palette-swatch.selected {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .search {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.76rem;
    color: var(--muted);
  }

  .search input {
    font: inherit;
    font-size: 0.82rem;
    padding: 0.4rem 0.5rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .reaction-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .reaction {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1rem;
    min-width: 3.8rem;
    padding: 0.4rem 0.35rem;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }

  .reaction span {
    font-size: 1.1rem;
  }

  .reaction small {
    font-size: 0.62rem;
    color: var(--muted);
  }

  .heatmap-card {
    padding: 0.6rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .heatmap-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 0.5rem;
    margin-bottom: 0.4rem;
    font-size: 0.8rem;
  }

  .heatmap {
    display: flex;
    align-items: stretch;
    gap: 2px;
    min-height: 2rem;
    overflow-x: auto;
    padding-bottom: 0.2rem;
  }

  .cell {
    position: relative;
    min-width: 0.45rem;
    height: 1.8rem;
    flex: 1 0 0.45rem;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--surface);
    overflow: hidden;
    cursor: pointer;
  }

  .cell::after {
    content: "";
    position: absolute;
    inset: 0;
    background: var(--accent);
    opacity: calc(0.08 + var(--heat) * 0.7);
  }

  .cell.current {
    outline: 2px solid color-mix(in srgb, var(--accent) 70%, var(--border));
    outline-offset: 1px;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem 0.6rem;
    margin-top: 0.45rem;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .reaction-mark {
    font-size: 1.05rem;
    line-height: 1.4;
  }

  .recording {
    margin: 0;
    padding: 0.4rem 0.55rem;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--warn) 55%, var(--border));
    background: color-mix(in srgb, var(--warn) 10%, transparent);
    font-size: 0.78rem;
  }

  .error {
    margin: 0;
    color: var(--warn);
    font-size: 0.78rem;
  }

  audio {
    width: 100%;
    height: 2rem;
    margin-top: 0.25rem;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.2rem;
    background: color-mix(in srgb, #000 45%, transparent);
  }

  .modal {
    width: min(44rem, 100%);
    max-height: 88vh;
    overflow-y: auto;
    padding: 1rem;
    border-radius: 16px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: 0 24px 60px color-mix(in srgb, #000 35%, transparent);
  }

  .modal header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.6rem;
  }

  .modal h2 {
    margin: 0;
    font-size: 1rem;
  }

  .modal blockquote {
    margin: 0 0 0.6rem;
    padding: 0.5rem 0.65rem;
    border-left: 3px solid var(--accent);
    background: var(--surface-2);
    border-radius: 0 8px 8px 0;
    font-size: 0.82rem;
    color: var(--muted);
  }

  button {
    font: inherit;
    font-size: 0.78rem;
    padding: 0.4rem 0.6rem;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-contrast);
    font-weight: 600;
  }

  .ghost {
    background: var(--surface-2);
    color: var(--muted);
  }

  .danger {
    border-color: color-mix(in srgb, var(--warn) 60%, var(--border));
    color: var(--warn);
  }

  .small {
    padding: 0.26rem 0.45rem;
    font-size: 0.72rem;
  }

  @media (max-width: 720px) {
    .annotation-panel {
      top: auto;
      right: 0.75rem;
      bottom: 0.75rem;
      left: 0.75rem;
      width: auto;
      max-height: 62vh;
    }
  }

  /* Lenguaje visual de las ventanas modales de Lumina: cabecera con icono,
     búsqueda omnipresente, tarjetas cálidas y CTA fija. */
  .annotation-panel {
    width: min(30rem, calc(100vw - 2rem));
    max-height: min(86vh, 52rem);
    border-color: color-mix(in srgb, var(--border-warm) 45%, var(--border));
    border-radius: 22px;
    background: var(--surface);
  }

  .panel-head {
    flex: 0 0 auto;
    padding: 14px 16px 10px;
    align-items: center;
  }

  .panel-heading {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .panel-mark {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 38px;
    height: 38px;
    border: 1px solid var(--accent-line);
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .annotation-panel h2 {
    font-size: 1.04rem;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .panel-heading p {
    margin: 2px 0 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.64rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
  }

  .close:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .panel-search {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 6px;
    margin: 0 14px 8px;
    padding: 4px 4px 4px 10px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .panel-search:focus-within {
    border-color: var(--accent-line);
    background: var(--surface);
  }

  .panel-search input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
  }

  .panel-search input:focus {
    outline: 0;
  }

  .search-clear {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
  }

  .tabs {
    flex: 0 0 auto;
    padding: 0 14px 8px;
    gap: 5px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .tabs button {
    min-height: 31px;
    padding: 0.28rem 0.62rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 0.66rem;
    font-weight: 600;
  }

  .tabs button.active {
    border-color: var(--accent);
    background: var(--accent);
  }

  .tab-body {
    flex: 1 1 auto;
    min-height: 0;
    gap: 7px;
    padding: 10px 14px;
  }

  .entry {
    gap: 8px;
    padding: 10px 11px;
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .entry:hover {
    border-color: var(--accent-line);
  }

  .marker-entry {
    box-shadow:
      inset 3px 0 0 var(--mark-color, var(--accent)),
      var(--shadow-xs);
  }

  .quote {
    font-family: var(--font-reading);
    font-size: 0.84rem;
    font-style: italic;
    line-height: 1.55;
  }

  .meta {
    font-size: 0.63rem;
  }

  .entry-actions {
    flex-direction: row;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .entry-actions button {
    min-width: 0;
  }

  .panel-cta {
    flex: 0 0 auto;
    padding: 10px 14px calc(10px + env(safe-area-inset-bottom));
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: color-mix(in srgb, var(--surface-2) 70%, var(--surface));
  }

  .panel-cta .primary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    width: 100%;
    min-height: 40px;
    border-radius: 12px;
    font-size: 0.78rem;
    font-weight: 650;
  }

  @media (max-width: 720px) {
    .annotation-panel {
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      height: 90vh;
      max-height: 90vh;
      border-radius: 22px 22px 0 0;
    }

    .annotation-panel::before {
      content: "";
      position: absolute;
      top: 7px;
      left: 50%;
      z-index: 2;
      width: 38px;
      height: 4px;
      border-radius: 999px;
      background: var(--border-strong);
      transform: translateX(-50%);
    }

    .panel-head {
      padding-top: 20px;
    }

    .tabs {
      overflow-x: auto;
      scrollbar-width: none;
    }

    .tabs::-webkit-scrollbar {
      display: none;
    }
  }

  .modal {
    width: min(34rem, 100%);
    max-height: 90vh;
    padding: 0;
    overflow: auto;
    border-color: color-mix(in srgb, var(--border-warm) 45%, var(--border));
    border-radius: 22px;
    background: var(--surface);
  }

  .note-head {
    position: sticky;
    top: 0;
    z-index: 2;
    margin: 0;
    padding: 14px 16px 11px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    backdrop-filter: blur(12px);
  }

  .note-head h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 600;
  }

  .note-head p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.64rem;
  }

  .quote-card {
    margin: 12px 16px 4px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border));
    border-left: 3px solid var(--accent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 4%, var(--surface));
  }

  .quote-label {
    display: block;
    margin-bottom: 4px;
    color: var(--accent);
    font-size: 0.58rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .quote-card blockquote {
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.86rem;
    font-style: italic;
    line-height: 1.55;
  }

  .modal :global(.rich-note) {
    padding: 10px 16px 16px;
  }

  @media (max-width: 720px) {
    .modal-backdrop {
      align-items: flex-end;
      padding: 0;
    }

    .modal {
      width: 100%;
      max-height: 92vh;
      border-radius: 22px 22px 0 0;
      border-bottom: 0;
    }

    .modal::before {
      content: "";
      display: block;
      width: 38px;
      height: 4px;
      margin: 7px auto 0;
      border-radius: 999px;
      background: var(--border-strong);
    }

    .note-head {
      padding-top: 12px;
    }
  }
</style>

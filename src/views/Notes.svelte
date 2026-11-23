<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import Button from "../lib/components/Button.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import {
    deleteAnnotation,
    errorMessage,
    listLibraryAnnotations,
    updateExtract,
    updateReaction,
    updateVoiceNote,
  } from "../lib/api";
  import { formatDate, t, translate, type MessageKey } from "../lib/i18n";
  import { notifyError, notifySuccess } from "../lib/toast";
  import type { AppTab } from "../lib/navigation";
  import type {
    AnnotationEntry,
    LibraryAnnotationEntry,
    Locator,
    ReactionKind,
  } from "../lib/types";

  type NoteFilter =
    | "all"
    | "note"
    | "extract"
    | "marker"
    | "voice_note"
    | "reaction";

  const reactionKinds: {
    kind: ReactionKind;
    emoji: string;
    label: MessageKey;
  }[] = [
    { kind: "like", emoji: "👍", label: "notes.reactionLike" },
    { kind: "surprise", emoji: "😮", label: "notes.reactionSurprise" },
    { kind: "tension", emoji: "😬", label: "notes.reactionTension" },
    { kind: "boredom", emoji: "😴", label: "notes.reactionBoredom" },
    { kind: "emotion", emoji: "🥹", label: "notes.reactionEmotion" },
  ];

  let {
    onRead,
    onNavigate,
  }: {
    onRead?: (bookId: string, locator?: Locator) => void;
    onNavigate?: (tab: AppTab) => void;
  } = $props();

  let entries = $state<LibraryAnnotationEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let query = $state("");
  let filter = $state<NoteFilter>("all");
  let deletingId = $state<string | null>(null);
  let editingExtractId = $state<string | null>(null);
  let editNote = $state("");
  let editCitation = $state("");
  let editingVoiceId = $state<string | null>(null);
  let editVoiceText = $state("");
  let editVoiceTranscript = $state("");
  let editVoiceQuote = $state("");
  let editingReactionId = $state<string | null>(null);
  let editReactionKind = $state<ReactionKind>("like");
  let editReactionText = $state("");
  let savingEdit = $state(false);

  let filtered = $derived.by(() => {
    const needle = normalizeSearch(query.trim());
    return entries.filter((item) => {
      if (filter !== "all" && item.entry.type !== filter) return false;
      if (!needle) return true;
      return normalizeSearch(searchable(item)).includes(needle);
    });
  });

  let counts = $derived.by(() => {
    const result: Record<AnnotationEntry["type"] | "all", number> = {
      all: entries.length,
      note: 0,
      extract: 0,
      marker: 0,
      voice_note: 0,
      reaction: 0,
    };
    for (const item of entries) result[item.entry.type] += 1;
    return result;
  });

  onMount(() => {
    void load();
  });

  async function load() {
    loading = true;
    error = null;
    try {
      entries = await listLibraryAnnotations();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      loading = false;
    }
  }

  function normalizeSearch(value: string): string {
    return value
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLocaleLowerCase();
  }

  function searchable(item: LibraryAnnotationEntry): string {
    const parts = [item.bookTitle, kindLabel(item.entry)];
    switch (item.entry.type) {
      case "marker":
        parts.push(entryText(item.entry));
        if (item.entry.data.note) parts.push(item.entry.data.note);
        break;
      case "note":
        if (item.entry.data.title) parts.push(item.entry.data.title);
        parts.push(item.entry.data.body);
        if (item.entry.data.text) parts.push(item.entry.data.text);
        break;
      case "extract":
        parts.push(item.entry.data.text);
        if (item.entry.data.note) parts.push(item.entry.data.note);
        if (item.entry.data.citation) parts.push(item.entry.data.citation);
        break;
      case "voice_note":
        if (item.entry.data.text) parts.push(item.entry.data.text);
        if (item.entry.data.transcript) parts.push(item.entry.data.transcript);
        if (item.entry.data.quote) parts.push(item.entry.data.quote);
        break;
      case "reaction":
        parts.push(reactionLabel(item.entry.data.kind));
        if (item.entry.data.text) parts.push(item.entry.data.text);
        break;
    }
    return parts.filter(Boolean).join(" ");
  }

  function entryText(entry: AnnotationEntry): string {
    switch (entry.type) {
      case "marker":
      case "extract":
        return entry.data.text ?? "";
      case "note":
        return entry.data.body;
      case "voice_note":
        return entry.data.transcript ?? entry.data.text ?? "";
      case "reaction":
        return entry.data.text ?? "";
    }
  }

  function excerpt(value: string, length = 120): string {
    const text = value.replace(/\s+/g, " ").trim();
    return text.length > length ? `${text.slice(0, length)}…` : text;
  }

  function reactionMeta(kind: ReactionKind) {
    return reactionKinds.find((item) => item.kind === kind) ?? reactionKinds[0];
  }

  function reactionLabel(kind: ReactionKind): string {
    return translate(reactionMeta(kind).label);
  }

  function entryTitle(entry: AnnotationEntry): string {
    switch (entry.type) {
      case "note":
        return entry.data.title?.trim() || translate("notes.untitledNote");
      case "marker":
        return entry.data.text?.trim()
          ? excerpt(entry.data.text)
          : translate("notes.untitledMarker");
      case "extract":
        return entry.data.citation?.trim() || translate("notes.untitledExtract");
      case "voice_note":
        return entry.data.text?.trim()
          ? excerpt(entry.data.text)
          : translate("notes.untitledVoice");
      case "reaction":
        return itemReactionTitle(entry.data.kind, entry.data.text);
    }
  }

  function itemReactionTitle(kind: ReactionKind, text: string | null): string {
    const label = reactionLabel(kind);
    return text?.trim() ? `${label} · ${excerpt(text)}` : label;
  }

  function kindLabel(entry: AnnotationEntry): string {
    switch (entry.type) {
      case "note":
        return translate("notes.typeNote");
      case "extract":
        return translate("notes.typeExtract");
      case "marker":
        return translate("notes.typeMarker");
      case "voice_note":
        return translate("notes.typeVoice");
      case "reaction":
        return translate("notes.typeReaction");
    }
  }

  function entryLocator(entry: AnnotationEntry): Locator {
    return {
      section: Math.max(0, entry.data.chapterIndex ?? 0),
      offset: Math.max(0, entry.data.offsetStart ?? 0),
    };
  }

  function openEntry(item: LibraryAnnotationEntry) {
    onRead?.(item.bookId, entryLocator(item.entry));
  }

  function audioUrl(path: string | null): string | null {
    return path ? convertFileSrc(path) : null;
  }

  function entryDate(entry: AnnotationEntry): string {
    const value = entry.data.createdAt;
    return formatDate(value, {
      day: "2-digit",
      month: "short",
      year: "numeric",
    });
  }

  function typeIcon(entry: AnnotationEntry) {
    switch (entry.type) {
      case "note":
        return "edit" as const;
      case "extract":
        return "bookmark" as const;
      case "marker":
        return "book" as const;
      case "voice_note":
        return "mic" as const;
      case "reaction":
        return "sparkles" as const;
      default:
        return "bookmark" as const;
    }
  }

  function startEditExtract(item: LibraryAnnotationEntry) {
    if (item.entry.type !== "extract") return;
    editingExtractId = item.entry.data.id;
    editNote = item.entry.data.note ?? "";
    editCitation = item.entry.data.citation ?? "";
  }

  function cancelEditExtract() {
    editingExtractId = null;
    editNote = "";
    editCitation = "";
  }

  async function saveEditExtract(item: LibraryAnnotationEntry) {
    if (item.entry.type !== "extract") return;
    savingEdit = true;
    try {
      const updated = await updateExtract(
        item.entry.data.id,
        editNote.trim() || null,
        editCitation.trim() || null,
      );
      entries = entries.map((entry) =>
        entry.entry.type === "extract" && entry.entry.data.id === updated.id
          ? { ...entry, entry: { type: "extract", data: updated } }
          : entry,
      );
      cancelEditExtract();
      notifySuccess(translate("notes.extractUpdated"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      savingEdit = false;
    }
  }

  function startEditVoice(item: LibraryAnnotationEntry) {
    if (item.entry.type !== "voice_note") return;
    editingVoiceId = item.entry.data.id;
    editVoiceText = item.entry.data.text;
    editVoiceTranscript = item.entry.data.transcript ?? "";
    editVoiceQuote = item.entry.data.quote ?? "";
  }

  function cancelEditVoice() {
    editingVoiceId = null;
    editVoiceText = "";
    editVoiceTranscript = "";
    editVoiceQuote = "";
  }

  async function saveEditVoice(item: LibraryAnnotationEntry) {
    if (item.entry.type !== "voice_note") return;
    savingEdit = true;
    try {
      const updated = await updateVoiceNote(
        item.entry.data.id,
        editVoiceText.trim(),
        editVoiceTranscript.trim() || null,
        editVoiceQuote.trim() || null,
      );
      entries = entries.map((entry) =>
        entry.entry.type === "voice_note" && entry.entry.data.id === updated.id
          ? { ...entry, entry: { type: "voice_note", data: updated } }
          : entry,
      );
      cancelEditVoice();
      notifySuccess(translate("notes.voiceUpdated"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      savingEdit = false;
    }
  }

  function startEditReaction(item: LibraryAnnotationEntry) {
    if (item.entry.type !== "reaction") return;
    editingReactionId = item.entry.data.id;
    editReactionKind = item.entry.data.kind;
    editReactionText = item.entry.data.text ?? "";
  }

  function cancelEditReaction() {
    editingReactionId = null;
    editReactionText = "";
  }

  async function saveEditReaction(item: LibraryAnnotationEntry) {
    if (item.entry.type !== "reaction") return;
    savingEdit = true;
    try {
      const updated = await updateReaction(
        item.entry.data.id,
        editReactionKind,
        editReactionText.trim() || null,
      );
      entries = entries.map((entry) =>
        entry.entry.type === "reaction" && entry.entry.data.id === updated.id
          ? { ...entry, entry: { type: "reaction", data: updated } }
          : entry,
      );
      cancelEditReaction();
      notifySuccess(translate("notes.reactionUpdated"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      savingEdit = false;
    }
  }

  async function remove(item: LibraryAnnotationEntry) {
    if (!window.confirm(translate("notes.deleteConfirm"))) return;
    deletingId = item.entry.data.id;
    try {
      await deleteAnnotation(item.entry.type, item.entry.data.id);
      entries = entries.filter((entry) => entry.entry.data.id !== item.entry.data.id);
      if (editingExtractId === item.entry.data.id) cancelEditExtract();
      if (editingVoiceId === item.entry.data.id) cancelEditVoice();
      if (editingReactionId === item.entry.data.id) cancelEditReaction();
      notifySuccess(translate("notes.deleted"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      deletingId = null;
    }
  }

  function clearFilters() {
    query = "";
    filter = "all";
  }
</script>

<section class="notes-view">
  <header class="hero">
    <div>
      <p class="eyebrow">{$t("notes.eyebrow")}</p>
      <h1 class="page-title">{$t("notes.title")}</h1>
      <p class="page-subtitle">{$t("notes.subtitle")}</p>
    </div>
    <div class="hero-count">
      <strong>{entries.length}</strong>
      <span>{$t("notes.count")}</span>
    </div>
  </header>

  <section class="toolbar">
    <div class="search-box">
      <Icon name="search" size={17} />
      <input
        type="search"
        bind:value={query}
        placeholder={$t("notes.searchPlaceholder")}
        onkeydown={(event) => {
          if (event.key === "Escape") query = "";
        }}
      />
      {#if query}
        <button
          class="clear"
          aria-label={$t("common.close")}
          onclick={() => (query = "")}
        >
          <Icon name="close" size={14} />
        </button>
      {/if}
    </div>
    <div class="filters" role="group" aria-label={$t("notes.filters")}>
      {#each ([
        ["all", "notes.filterAll"],
        ["note", "notes.filterNotes"],
        ["extract", "notes.filterExtracts"],
        ["marker", "notes.filterMarkers"],
        ["voice_note", "notes.filterVoice"],
        ["reaction", "notes.filterReactions"],
      ] as [NoteFilter, MessageKey][]) as [value, label] (value)}
        <button class:active={filter === value} onclick={() => (filter = value)}>
          {$t(label)}
          {#if counts[value] > 0}<span>{counts[value]}</span>{/if}
        </button>
      {/each}
    </div>
  </section>

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {:else if loading}
    <Spinner label={$t("common.loading")} />
  {:else if filtered.length === 0}
    {#if entries.length === 0}
      <EmptyState
        title={$t("notes.empty")}
        description={$t("notes.emptyHelp")}
        action={$t("notes.goLibrary")}
        onAction={() => onNavigate?.("library")}
        icon="book"
      />
    {:else}
      <EmptyState
        title={$t("notes.noResults")}
        description={$t("notes.noResultsHelp")}
        action={$t("notes.clearFilters")}
        onAction={clearFilters}
        icon="search"
      />
    {/if}
  {:else}
    <ul class="note-list">
      {#each filtered as item (item.entry.data.id)}
        <li>
          <article class="note-card">
            <header>
              <span class="type-icon"><Icon name={typeIcon(item.entry)} size={16} /></span>
              <div class="meta">
                <span class="kind">{kindLabel(item.entry)}</span>
                <button class="book" onclick={() => openEntry(item)}>
                  {item.bookTitle}
                </button>
              </div>
              <time datetime={item.entry.data.createdAt}>{entryDate(item.entry)}</time>
            </header>

            <button class="content" onclick={() => openEntry(item)}>
              <strong>{entryTitle(item.entry)}</strong>
              {#if item.entry.type === "note" && item.entry.data.body}
                <p>{item.entry.data.body}</p>
              {:else if item.entry.type === "marker" && item.entry.data.note}
                <p>{item.entry.data.note}</p>
              {:else if item.entry.type === "extract" && item.entry.data.note}
                <p>{item.entry.data.note}</p>
              {:else if item.entry.type === "voice_note"}
                <p>
                  {item.entry.data.transcript ??
                    item.entry.data.text ??
                    $t("notes.audioNote")}
                </p>
              {:else if item.entry.type === "reaction" && item.entry.data.text}
                <p>{item.entry.data.text}</p>
              {/if}
            </button>

            {#if item.entry.type === "voice_note" && audioUrl(item.entry.data.audioPath)}
              <audio
                class="voice-audio"
                controls
                preload="none"
                aria-label={`${$t("notes.audioNote")} · ${item.bookTitle}`}
                src={audioUrl(item.entry.data.audioPath) ?? undefined}
              ></audio>
            {/if}

            {#if item.entry.type === "voice_note" && editingVoiceId === item.entry.data.id}
              <div class="edit-box">
                <label>
                  <span>{$t("notes.editVoiceText")}</span>
                  <textarea bind:value={editVoiceText} rows="2" maxlength="2000"></textarea>
                </label>
                <label>
                  <span>{$t("notes.editVoiceTranscript")}</span>
                  <textarea bind:value={editVoiceTranscript} rows="2" maxlength="6000"></textarea>
                </label>
                <label>
                  <span>{$t("notes.editVoiceQuote")}</span>
                  <input bind:value={editVoiceQuote} maxlength="1000" />
                </label>
                <div class="edit-actions">
                  <Button
                    variant="primary"
                    size="small"
                    loading={savingEdit}
                    onclick={() => saveEditVoice(item)}
                  >
                    {$t("common.save")}
                  </Button>
                  <Button variant="ghost" size="small" onclick={cancelEditVoice}>
                    {$t("common.cancel")}
                  </Button>
                </div>
              </div>
            {/if}

            {#if item.entry.type === "extract" && editingExtractId === item.entry.data.id}
              <div class="edit-box">
                <label>
                  <span>{$t("notes.editNote")}</span>
                  <textarea bind:value={editNote} rows="2" maxlength="2000"></textarea>
                </label>
                <label>
                  <span>{$t("notes.editCitation")}</span>
                  <input bind:value={editCitation} maxlength="500" />
                </label>
                <div class="edit-actions">
                  <Button
                    variant="primary"
                    size="small"
                    loading={savingEdit}
                    onclick={() => saveEditExtract(item)}
                  >
                    {$t("common.save")}
                  </Button>
                  <Button variant="ghost" size="small" onclick={cancelEditExtract}>
                    {$t("common.cancel")}
                  </Button>
                </div>
              </div>
            {/if}

            {#if item.entry.type === "reaction" && editingReactionId === item.entry.data.id}
              <div class="edit-box">
                <label>
                  <span>{$t("notes.editReactionKind")}</span>
                  <select bind:value={editReactionKind}>
                    {#each reactionKinds as reaction (reaction.kind)}
                      <option value={reaction.kind}>
                        {reaction.emoji} {$t(reaction.label)}
                      </option>
                    {/each}
                  </select>
                </label>
                <label>
                  <span>{$t("notes.editReactionText")}</span>
                  <input bind:value={editReactionText} maxlength="1000" />
                </label>
                <div class="edit-actions">
                  <Button
                    variant="primary"
                    size="small"
                    loading={savingEdit}
                    onclick={() => saveEditReaction(item)}
                  >
                    {$t("common.save")}
                  </Button>
                  <Button variant="ghost" size="small" onclick={cancelEditReaction}>
                    {$t("common.cancel")}
                  </Button>
                </div>
              </div>
            {/if}

            <footer>
              <Button variant="ghost" size="small" onclick={() => openEntry(item)}>
                <Icon name="book" size={14} /> {$t("notes.open")}
              </Button>
              {#if item.entry.type === "extract"}
                <Button variant="ghost" size="small" onclick={() => startEditExtract(item)}>
                  <Icon name="edit" size={14} /> {$t("common.edit")}
                </Button>
              {:else if item.entry.type === "voice_note"}
                <Button variant="ghost" size="small" onclick={() => startEditVoice(item)}>
                  <Icon name="edit" size={14} /> {$t("common.edit")}
                </Button>
              {:else if item.entry.type === "reaction"}
                <Button variant="ghost" size="small" onclick={() => startEditReaction(item)}>
                  <Icon name="edit" size={14} /> {$t("common.edit")}
                </Button>
              {/if}
              <Button
                variant="danger"
                size="small"
                loading={deletingId === item.entry.data.id}
                onclick={() => remove(item)}
              >
                <Icon name="trash" size={14} /> {$t("common.delete")}
              </Button>
            </footer>
          </article>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .notes-view {
    display: flex;
    flex-direction: column;
    gap: 1.15rem;
  }

  .hero {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
  }

  .hero-count {
    display: flex;
    align-items: baseline;
    gap: 0.45rem;
    padding: 0.55rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: var(--muted);
  }

  .hero-count strong {
    color: var(--accent);
    font-family: var(--font-display);
    font-size: 1.25rem;
  }

  .hero-count span {
    font-size: 0.72rem;
  }

  .toolbar {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 15px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.15rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .search-box input {
    flex: 1;
    min-width: 0;
    min-height: 38px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.84rem;
  }

  .search-box input:focus {
    outline: none;
  }

  .clear {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
  }

  .clear:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .filters button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-height: 32px;
    padding: 0.28rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.7rem;
  }

  .filters button.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .filters button span {
    display: inline-grid;
    place-items: center;
    min-width: 17px;
    height: 17px;
    padding: 0 4px;
    border-radius: 999px;
    background: color-mix(in srgb, currentColor 12%, transparent);
    font-size: 0.58rem;
  }

  .note-list {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .note-card {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.8rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .note-card header {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }

  .type-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    border-radius: 9px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .kind {
    color: var(--muted);
    font-size: 0.62rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .book {
    overflow: hidden;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .book:hover {
    color: var(--accent);
  }

  time {
    color: var(--muted);
    font-size: 0.65rem;
    white-space: nowrap;
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .content strong {
    font-family: var(--font-display);
    font-size: 0.95rem;
    line-height: 1.35;
  }

  .content p {
    margin: 0;
    display: -webkit-box;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.8rem;
    line-height: 1.5;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }

  .voice-audio {
    width: 100%;
    height: 34px;
  }

  .edit-box {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    padding: 0.55rem;
    border: 1px solid var(--accent-line);
    border-radius: 9px;
    background: var(--accent-soft);
  }

  .edit-box label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    color: var(--muted);
    font-size: 0.66rem;
  }

  .edit-box input,
  .edit-box textarea,
  .edit-box select {
    width: 100%;
    padding: 0.38rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
  }

  .edit-box textarea {
    min-height: 56px;
    resize: vertical;
  }

  .edit-actions {
    display: flex;
    gap: 0.35rem;
  }

  .note-card footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.4rem;
    padding-top: 0.35rem;
    border-top: 1px solid var(--border);
  }

  @media (max-width: 640px) {
    .hero {
      align-items: flex-start;
    }

    .hero-count {
      padding: 0.4rem 0.6rem;
    }

    .note-card header {
      align-items: flex-start;
    }

    time {
      display: none;
    }
  }
</style>

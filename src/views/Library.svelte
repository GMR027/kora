<script module lang="ts">
  // Las portadas generadas son opcionales. Recordamos los intentos durante la
  // sesión y limitamos el trabajo para no bloquear la biblioteca al abrirla.
  const attemptedCovers = new Set<string>();
</script>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import {
    categoryCreate,
    categoryDelete,
    categoryList,
    categorySuggestionAccept,
    categorySuggestionAcceptAll,
    categorySuggestionDismiss,
    categorySuggestionListPending,
    categoryUpdate,
    exportNotes,
    libraryAssignCategory,
    libraryAudiobookBookIds,
    libraryClassify,
    libraryDelete,
    libraryEmptyTrash,
    libraryEnsureCover,
    libraryImport,
    libraryList,
    libraryPurgeBook,
    libraryReplaceCover,
    libraryRestore,
    librarySetFavorite,
    libraryTrash,
    libraryUnassignCategory,
    libraryUpdate,
    prepareSemanticModel,
    reindexLibrary,
    saveAppearance,
    searchSemantic,
    semanticStatus,
  } from "../lib/api";
  import { appearance, updateAppearance } from "../lib/appearance";
  import { formatBytes, t, translate } from "../lib/i18n";
  import { discardImportSource, stageImportSource } from "../lib/importSource";
  import { notifyError, notifyImportant, notifySuccess } from "../lib/toast";
  import type { AppTab } from "../lib/navigation";
  import Button from "../lib/components/Button.svelte";
  import CategorySuggestions from "../lib/components/CategorySuggestions.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import PdfConversionModal from "../lib/components/PdfConversionModal.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import type {
    BookSummary,
    Category,
    CategorySuggestion,
    Locator,
    PdfConversionResult,
    SemanticHit,
    SemanticProgress,
    SemanticStatus,
    TrashedBookSummary,
  } from "../lib/types";

  let {
    onRead,
    onListen,
    onNavigate,
  }: {
    onRead?: (bookId: string, locator?: Locator) => void;
    onListen?: (bookId: string, section?: number) => void;
    onNavigate?: (tab: AppTab) => void;
  } = $props();

  let books = $state<BookSummary[]>([]);
  let categories = $state<Category[]>([]);
  let loading = $state(true);
  let busy = $state<string | null>(null);
  let categoryName = $state("");
  let categoryColor = $state("#c2410c");
  let categoryDrafts = $state<Record<string, { name: string; color: string }>>({});
  let categoryBusyId = $state<string | null>(null);
  let metadataEditing = $state(false);
  let metadataTargetId = $state<string | null>(null);
  let metadataBusy = $state(false);
  let metadataDraft = $state({
    title: "",
    author: "",
    description: "",
    language: "",
    publisher: "",
  });
  let semanticProgress = $state<SemanticProgress | null>(null);
  let semanticMessage = $state<string | null>(null);
  let dropActive = $state(false);
  let query = $state("");
  let hits = $state<SemanticHit[]>([]);
  let semantic = $state<SemanticStatus | null>(null);
  let searching = $state(false);
  let filter = $state<string>("all");
  let sort = $state<"recent" | "lastRead" | "title" | "progress">("recent");
  let view = $state<"grid" | "list">("grid");
  let brokenCovers = $state<Record<string, boolean>>({});
  let menuBookId = $state<string | null>(null);
  let selectedBookId = $state<string | null>(null);
  let audiobookBookIds = $state<Set<string>>(new Set());
  let suggestions = $state<CategorySuggestion[]>([]);
  let suggestionBusyId = $state<string | null>(null);
  let classifyingBookId = $state<string | null>(null);
  let suggestionBookIds = $derived(
    new Set(suggestions.map((suggestion) => suggestion.bookId)),
  );
  let menuBook = $derived(
    books.find((book) => book.id === menuBookId) ?? null,
  );
  let trashOpen = $state(false);
  let trash = $state<TrashedBookSummary[]>([]);
  let trashLoading = $state(false);
  let trashBusyId = $state<string | null>(null);
  let conversionBook = $state<BookSummary | null>(null);

  let filtered = $derived.by(() => {
    let result = books;
    if (filter === "shared") result = result.filter((book) => book.readOnly);
    else if (filter === "reading")
      result = result.filter(
        (book) => book.progress > 0.001 && book.progress < 0.995,
      );
    else if (filter === "favorites")
      result = result.filter((book) => book.favorite);
    else if (filter === "finished")
      result = result.filter((book) => book.progress >= 0.995);
    else if (filter === "notStarted")
      result = result.filter((book) => book.progress <= 0.001);
    else if (filter === "audiobooks")
      result = result.filter((book) => audiobookBookIds.has(book.id));
    else if (filter === "epub") result = result.filter((book) => book.format === "epub");
    else if (filter === "pdf") result = result.filter((book) => book.format === "pdf");
    else if (filter === "uncategorized")
      result = result.filter((book) => book.categoryIds.length === 0);
    else if (filter === "suggestions")
      result = result.filter((book) => suggestionBookIds.has(book.id));
    else if (filter !== "all")
      result = result.filter((book) => book.categoryIds.includes(filter));

    if (sort === "title") {
      result = [...result].sort((left, right) =>
        left.title.localeCompare(right.title),
      );
    } else if (sort === "progress") {
      result = [...result].sort((left, right) => right.progress - left.progress);
    } else if (sort === "lastRead") {
      result = [...result].sort((left, right) => {
        const leftTime = left.lastReadAt ? new Date(left.lastReadAt).getTime() : 0;
        const rightTime = right.lastReadAt ? new Date(right.lastReadAt).getTime() : 0;
        return rightTime - leftTime;
      });
    }
    return result;
  });

  let normalizedQuery = $derived(query.trim().toLocaleLowerCase());
  let lexicalMatches = $derived.by(() => {
    if (normalizedQuery.length < 2) return [];
    return books.filter((book) => {
      const haystack = `${book.title} ${book.author ?? ""}`.toLocaleLowerCase();
      return haystack.includes(normalizedQuery);
    });
  });
  let assignedCategories = $derived(
    menuBook
      ? categories.filter((category) => menuBook?.categoryIds.includes(category.id))
      : [],
  );

  let installedCategories = $derived(categories);
  let hasShared = $derived(books.some((book) => book.readOnly));
  let booksLabel = $derived(
    books.length === 1
      ? translate("library.oneBook")
      : translate("library.booksCount", { count: books.length }),
  );
  let continueBook = $derived.by(() => {
    const pending = books.filter(
      (book) => book.progress > 0.001 && book.progress < 0.995,
    );
    pending.sort((left, right) => {
      const leftTime = left.lastReadAt
        ? new Date(left.lastReadAt).getTime()
        : new Date(left.addedAt).getTime();
      const rightTime = right.lastReadAt
        ? new Date(right.lastReadAt).getTime()
        : new Date(right.addedAt).getTime();
      return rightTime - leftTime;
    });
    return pending[0] ?? null;
  });
  let readingCount = $derived(
    books.filter((book) => book.progress > 0.001 && book.progress < 0.995)
      .length,
  );
  let finishedCount = $derived(
    books.filter((book) => book.progress >= 0.995).length,
  );
  let favoriteCount = $derived(books.filter((book) => book.favorite).length);
  let audiobookCount = $derived(audiobookBookIds.size);
  let detailsBook = $derived(
    books.find((book) => book.id === selectedBookId) ??
      continueBook ??
      filtered[0] ??
      books[0] ??
      null,
  );
  let suggestionsForDetails = $derived(
    detailsBook
      ? suggestions.filter((suggestion) => suggestion.bookId === detailsBook.id)
      : [],
  );

  onMount(() => {
    if ($appearance?.preferences.libraryLayout === "list") view = "list";
    void load();
    void loadSemantic();
    void loadTrash();

    let unlisten: (() => void) | undefined;
    let unlistenClassification: (() => void) | undefined;
    let unlistenSemantic: (() => void) | undefined;
    let unlistenSemanticModel: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          dropActive = true;
        } else if (payload.type === "leave") {
          dropActive = false;
        } else if (payload.type === "drop") {
          dropActive = false;
          const paths = payload.paths.filter((candidate) =>
            /\.(epub|pdf)$/i.test(candidate),
          );
          if (paths.length > 0) void importBooks(paths);
          else notifyError(new Error(translate("library.importHint")));
        }
      })
      .then((stop) => {
        unlisten = stop;
      })
      .catch(() => undefined);

    listen("classification::suggestions", () => {
      scheduleSuggestionsReload();
    })
      .then((stop) => {
        unlistenClassification = stop;
      })
      .catch(() => undefined);

    listen<SemanticProgress>("semantic::progress", (event) => {
      semanticProgress = event.payload;
      if (event.payload.status === "done" || event.payload.percent >= 100) {
        void loadSemantic();
        setTimeout(() => {
          semanticProgress = null;
        }, 1600);
      }
    })
      .then((stop) => {
        unlistenSemantic = stop;
      })
      .catch(() => undefined);

    listen<{ status: string; message: string }>("semantic::model", (event) => {
      semanticMessage = event.payload.message;
      if (event.payload.status === "ready") {
        void loadSemantic();
        setTimeout(() => {
          semanticMessage = null;
        }, 1800);
      }
    })
      .then((stop) => {
        unlistenSemanticModel = stop;
      })
      .catch(() => undefined);

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") menuBookId = null;
    };
    window.addEventListener("keydown", onKeydown);

    return () => {
      unlisten?.();
      unlistenClassification?.();
      unlistenSemantic?.();
      unlistenSemanticModel?.();
      if (suggestionsRefresh) clearTimeout(suggestionsRefresh);
      window.removeEventListener("keydown", onKeydown);
    };
  });

  async function load() {
    loading = true;
    try {
      [books, categories] = await Promise.all([libraryList(), categoryList()]);
      syncCategoryDrafts();
      void loadSuggestions();
      try {
        audiobookBookIds = new Set(await libraryAudiobookBookIds());
      } catch {
        audiobookBookIds = new Set();
      }
      void ensureCovers(books.slice());
    } catch (cause) {
      notifyError(cause);
    } finally {
      loading = false;
    }
  }

  async function ensureCovers(snapshot: BookSummary[]) {
    const pending = snapshot
      .filter((book) => !book.coverPath && !attemptedCovers.has(book.id))
      .slice(0, 6);
    for (const book of pending) {
      attemptedCovers.add(book.id);
      try {
        const updated = await libraryEnsureCover(book.id);
        books = books.map((item) => (item.id === updated.id ? updated : item));
      } catch {
        // La portada generada es opcional; el placeholder sigue siendo válido.
      }
    }
  }

  async function loadSemantic() {
    try {
      semantic = await semanticStatus();
    } catch {
      // La búsqueda semántica es opcional.
    }
  }

  async function loadSuggestions() {
    try {
      suggestions = await categorySuggestionListPending();
    } catch {
      suggestions = [];
    }
  }

  async function loadTrash() {
    trashLoading = true;
    try {
      trash = await libraryTrash();
    } catch {
      trash = [];
    } finally {
      trashLoading = false;
    }
  }

  function openTrash() {
    trashOpen = true;
    void loadTrash();
  }

  async function restoreTrashed(item: TrashedBookSummary) {
    trashBusyId = item.id;
    try {
      const restored = await libraryRestore(item.id);
      trash = trash.filter((entry) => entry.id !== item.id);
      books = [restored, ...books.filter((book) => book.id !== restored.id)];
      notifySuccess(translate("library.restored"), restored.title);
    } catch (cause) {
      notifyError(cause);
    } finally {
      trashBusyId = null;
    }
  }

  async function purgeTrashed(item: TrashedBookSummary) {
    if (!window.confirm(translate("library.purgeConfirm", { title: item.title }))) {
      return;
    }
    trashBusyId = item.id;
    try {
      await libraryPurgeBook(item.id);
      trash = trash.filter((entry) => entry.id !== item.id);
      notifySuccess(translate("library.purged"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      trashBusyId = null;
    }
  }

  async function emptyTrash() {
    if (!window.confirm(translate("library.emptyTrashConfirm"))) return;
    trashBusyId = "all";
    try {
      const purged = await libraryEmptyTrash();
      trash = [];
      notifySuccess(translate("library.emptyTrashDone", { count: purged }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      trashBusyId = null;
    }
  }

  let suggestionsRefresh: ReturnType<typeof setTimeout> | undefined;

  function scheduleSuggestionsReload() {
    if (suggestionsRefresh) clearTimeout(suggestionsRefresh);
    suggestionsRefresh = setTimeout(() => {
      suggestionsRefresh = undefined;
      void loadSuggestions();
    }, 400);
  }

  async function refreshLibraryLists() {
    [books, categories] = await Promise.all([libraryList(), categoryList()]);
    syncCategoryDrafts();
  }

  function syncCategoryDrafts() {
    const next = { ...categoryDrafts };
    const ids = new Set(categories.map((category) => category.id));
    for (const id of Object.keys(next)) {
      if (!ids.has(id)) delete next[id];
    }
    for (const category of categories) {
      if (!next[category.id]) {
        next[category.id] = {
          name: category.name,
          color: category.color ?? "#c2410c",
        };
      }
    }
    categoryDrafts = next;
  }

  function updateCategoryDraft(
    id: string,
    field: "name" | "color",
    value: string,
  ) {
    const current = categoryDrafts[id] ?? { name: "", color: "#c2410c" };
    categoryDrafts = {
      ...categoryDrafts,
      [id]: { ...current, [field]: value },
    };
  }

  async function detectCategories(bookId: string) {
    if (classifyingBookId) return;
    classifyingBookId = bookId;
    try {
      const force = !suggestions.some(
        (suggestion) => suggestion.bookId === bookId,
      );
      const found = await libraryClassify(bookId, force);
      await loadSuggestions();
      notifySuccess(
        found.length > 0
          ? translate("classification.detected", { count: found.length })
          : translate("classification.noneDetected"),
      );
    } catch (cause) {
      notifyError(cause);
    } finally {
      classifyingBookId = null;
    }
  }

  async function acceptSuggestion(suggestion: CategorySuggestion) {
    suggestionBusyId = suggestion.id;
    try {
      await categorySuggestionAccept(suggestion.id);
      suggestions = suggestions.filter((item) => item.id !== suggestion.id);
      if (suggestion.categoryId) {
        const categoryId = suggestion.categoryId;
        books = books.map((book) =>
          book.id === suggestion.bookId
            ? {
                ...book,
                categoryIds: [
                  ...new Set([...book.categoryIds, categoryId]),
                ],
              }
            : book,
        );
      } else {
        await refreshLibraryLists();
      }
      notifySuccess(translate("classification.accepted"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      suggestionBusyId = null;
    }
  }

  async function dismissSuggestion(suggestion: CategorySuggestion) {
    suggestionBusyId = suggestion.id;
    try {
      await categorySuggestionDismiss(suggestion.id);
      suggestions = suggestions.filter((item) => item.id !== suggestion.id);
      notifySuccess(translate("classification.dismissed"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      suggestionBusyId = null;
    }
  }

  async function acceptAllSuggestions(bookId: string) {
    try {
      await categorySuggestionAcceptAll(bookId);
      suggestions = suggestions.filter(
        (suggestion) => suggestion.bookId !== bookId,
      );
      await refreshLibraryLists();
      notifySuccess(translate("classification.acceptedAll"));
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function exportBookNotes(book: BookSummary) {
    try {
      const destination = await save({
        defaultPath: `${book.title.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 60) || "libro"}-notas.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!destination) return;
      const result = await exportNotes(book.id, "markdown", destination);
      notifySuccess(
        translate("library.notesExported", {
          count: result.entries,
          path: result.path,
        }),
      );
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function toggleFavorite(book: BookSummary) {
    try {
      const updated = await librarySetFavorite(book.id, !book.favorite);
      books = books.map((item) => (item.id === updated.id ? updated : item));
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function chooseFile() {
    try {
      const selected = await open({
        multiple: true,
        filters: [{ name: "EPUB/PDF", extensions: ["epub", "pdf"] }],
      });
      const paths =
        typeof selected === "string" ? [selected] : (selected ?? []);
      if (paths.length > 0) await importBooks(paths);
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function importBooks(sources: string[]) {
    const paths = [
      ...new Set(sources.map((source) => source.trim()).filter(Boolean)),
    ];
    if (paths.length === 0) return;

    busy = "import";
    let imported = 0;
    let failed = 0;
    let lastError: unknown = null;
    for (const source of paths) {
      let staged = source;
      try {
        staged = stageImportSource(source);
        const book = await libraryImport(staged);
        books = [book, ...books.filter((item) => item.id !== book.id)];
        imported += 1;
      } catch (cause) {
        failed += 1;
        lastError = cause;
      } finally {
        if (staged !== source) discardImportSource(staged);
      }
    }

    if (imported > 0) {
      notifyImportant(
        "success",
        translate("library.importedCount", { count: imported }),
        paths.length > 1
          ? translate("library.importedOf", { total: paths.length })
          : null,
        "library",
      );
    }
    if (failed > 0) {
      notifyError(
        failed === 1 && lastError
          ? lastError
          : new Error(translate("library.importFailedCount", { count: failed })),
      );
    }
    void loadSemantic();
    void loadSuggestions();
    busy = null;
  }

  async function addCategory() {
    const name = categoryName.trim();
    if (!name) return;
    busy = "category";
    try {
      const category = await categoryCreate(name, categoryColor);
      if (!categories.some((item) => item.id === category.id)) {
        categories = [...categories, category];
      }
      categoryDrafts = {
        ...categoryDrafts,
        [category.id]: {
          name: category.name,
          color: category.color ?? categoryColor,
        },
      };
      categoryName = "";
      notifySuccess(translate("library.categoryAdded", { name: category.name }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function saveCategory(category: Category) {
    const draft = categoryDrafts[category.id];
    if (!draft || !draft.name.trim()) return;
    categoryBusyId = category.id;
    try {
      const updated = await categoryUpdate(
        category.id,
        draft.name.trim(),
        draft.color || null,
      );
      categories = categories.map((item) =>
        item.id === updated.id ? updated : item,
      );
      categoryDrafts = {
        ...categoryDrafts,
        [updated.id]: {
          name: updated.name,
          color: updated.color ?? draft.color,
        },
      };
      notifySuccess(translate("library.categorySaved", { name: updated.name }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      categoryBusyId = null;
    }
  }

  async function deleteCategory(category: Category) {
    if (
      !window.confirm(
        translate("library.categoryDeleteConfirm", { name: category.name }),
      )
    ) {
      return;
    }
    categoryBusyId = category.id;
    try {
      await categoryDelete(category.id);
      categories = categories.filter((item) => item.id !== category.id);
      books = books.map((book) => ({
        ...book,
        categoryIds: book.categoryIds.filter((id) => id !== category.id),
      }));
      if (filter === category.id) filter = "all";
      notifySuccess(translate("library.categoryDeleted", { name: category.name }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      categoryBusyId = null;
    }
  }

  async function assignCategory(bookId: string, categoryId: string) {
    if (!categoryId) return;
    try {
      await libraryAssignCategory(bookId, categoryId);
      books = books.map((book) =>
        book.id === bookId
          ? { ...book, categoryIds: [...new Set([...book.categoryIds, categoryId])] }
          : book,
      );
    } catch (cause) {
      notifyError(cause);
    }
  }

  async function unassignCategory(bookId: string, categoryId: string) {
    try {
      await libraryUnassignCategory(bookId, categoryId);
      books = books.map((book) =>
        book.id === bookId
          ? {
              ...book,
              categoryIds: book.categoryIds.filter((id) => id !== categoryId),
            }
          : book,
      );
    } catch (cause) {
      notifyError(cause);
    }
  }

  function startMetadataEdit(book: BookSummary) {
    metadataDraft = {
      title: book.title,
      author: book.author ?? "",
      description: book.description ?? "",
      language: book.language ?? "",
      publisher: book.publisher ?? "",
    };
    metadataTargetId = book.id;
    metadataEditing = true;
  }

  function cancelMetadataEdit() {
    metadataEditing = false;
    metadataTargetId = null;
  }

  async function saveMetadata(book: BookSummary) {
    if (!metadataDraft.title.trim()) return;
    metadataBusy = true;
    try {
      const updated = await libraryUpdate(book.id, {
        title: metadataDraft.title.trim(),
        author: metadataDraft.author.trim() || null,
        description: metadataDraft.description.trim() || null,
        language: metadataDraft.language.trim() || null,
        publisher: metadataDraft.publisher.trim() || null,
      });
      books = books.map((item) => (item.id === updated.id ? updated : item));
      metadataEditing = false;
      metadataTargetId = null;
      notifySuccess(translate("library.metadataSaved", { title: updated.title }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      metadataBusy = false;
    }
  }

  function handleConverted(result: PdfConversionResult) {
    if (result.report.replaced) {
      books = books.map((item) =>
        item.id === result.book.id ? result.book : item,
      );
      selectedBookId = result.book.id;
    } else if (!books.some((item) => item.id === result.book.id)) {
      books = [result.book, ...books];
    }
  }

  async function removeBook(book: BookSummary) {
    if (!window.confirm(translate("library.trashConfirm", { title: book.title }))) {
      return;
    }
    busy = book.id;
    try {
      await libraryDelete(book.id);
      books = books.filter((item) => item.id !== book.id);
      await loadTrash();
      notifySuccess(translate("library.trashed"), translate("library.trashedHelp"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function chooseCover(book: BookSummary) {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: "Imagen", extensions: ["jpg", "jpeg", "png", "webp"] },
        ],
      });
      if (typeof selected !== "string") return;

      busy = book.id;
      const updated = await libraryReplaceCover(book.id, selected);
      books = books.map((item) => (item.id === updated.id ? updated : item));
      brokenCovers = { ...brokenCovers, [book.id]: false };
      notifySuccess(translate("library.cover"), book.title);
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function runSearch() {
    const text = query.trim();
    if (text.length < 2) {
      hits = [];
      return;
    }
    searching = true;
    try {
      hits = await searchSemantic(text, 20);
      if (hits.length === 0 && lexicalMatches.length === 0) {
        notifyError(new Error(translate("library.noResults")));
      }
      void loadSemantic();
    } catch (cause) {
      hits = [];
      if (lexicalMatches.length === 0) notifyError(cause);
    } finally {
      searching = false;
    }
  }

  async function prepareModel() {
    searching = true;
    try {
      semantic = await prepareSemanticModel();
      notifySuccess(translate("library.prepareModel"), translate("library.indexed", {
        indexed: semantic.indexedBooks,
        total: semantic.totalBooks,
        chunks: semantic.chunks,
      }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      searching = false;
    }
  }

  async function reindex() {
    searching = true;
    try {
      semantic = await reindexLibrary();
      notifySuccess(translate("library.reindex"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      searching = false;
    }
  }

  async function toggleView(next: "grid" | "list") {
    view = next;
    const state = $appearance;
    if (!state) return;
    try {
      const saved = await saveAppearance(
        state.preferences.themeId,
        state.preferences.texture,
        state.preferences.locale,
        state.preferences.onboardingDone,
        next,
      );
      updateAppearance(saved);
    } catch (cause) {
      notifyError(cause);
    }
  }

  function coverSrc(book: BookSummary): string | null {
    return book.coverPath ? convertFileSrc(book.coverPath) : null;
  }

  function closeMenu() {
    menuBookId = null;
  }

  function openHit(hit: SemanticHit) {
    onRead?.(hit.bookId, { section: hit.section, offset: hit.offset });
  }

  function originLabel(origin: BookSummary["origin"]): string | null {
    if (origin === "tejido") return "Tejido";
    if (origin === "transcribed") return translate("audiobooks.import");
    if (origin === "generated") return translate("audiobooks.generate");
    return null;
  }

  function categoryLabel(book: BookSummary): string | null {
    const first = book.categoryIds[0];
    return categories.find((category) => category.id === first)?.name ?? null;
  }

  function statusLabel(book: BookSummary): string {
    if (book.progress >= 0.995) return translate("library.finished");
    if (book.progress > 0.001) return translate("library.statReading");
    return translate("library.notStarted");
  }

  function lastReadLabel(book: BookSummary): string {
    return book.lastReadAt
      ? new Date(book.lastReadAt).toLocaleDateString()
      : translate("common.never");
  }

  function addedLabel(book: BookSummary): string {
    try {
      return new Intl.DateTimeFormat(document.documentElement.lang || "es", {
        day: "2-digit",
        month: "short",
        year: "numeric",
      }).format(new Date(book.addedAt));
    } catch {
      return "—";
    }
  }
</script>

<section class="library" class:drop-active={dropActive}>
  <header class="page-head">
    <div class="title-block">
      <p class="eyebrow">{$t("library.catalog")}</p>
      <h1 class="page-title">{$t("library.title")}</h1>
    </div>
    <div class="head-actions">
      <Button variant="primary" onclick={chooseFile} loading={busy === "import"}>
        <Icon name="plus" size={18} />
        {$t("library.import")}
        <span class="head-hint">(PDF / EPUB)</span>
      </Button>
      <Button variant="ghost" onclick={() => onNavigate?.("audiobooks")}>
        <Icon name="sparkles" size={18} />
        {$t("library.generateAudio")}
      </Button>
      <Button variant="ghost" onclick={openTrash}>
        <Icon name="trash" size={18} />
        {$t("library.trash")}
        {#if trash.length > 0}<span class="head-hint">({trash.length})</span>{/if}
      </Button>
      <span class="head-divider" aria-hidden="true"></span>
      <label class="sort-select">
        <span class="sr-only">{$t("library.sort")}</span>
        <select bind:value={sort}>
          <option value="recent">{$t("library.sortRecent")}</option>
          <option value="lastRead">{$t("library.sortLastRead")}</option>
          <option value="title">{$t("library.sortTitle")}</option>
          <option value="progress">{$t("library.sortProgress")}</option>
        </select>
      </label>
      <div class="view-toggle" role="group" aria-label={`${$t("library.grid")} / ${$t("library.list")}`}>
        <button
          class:active={view === "grid"}
          aria-label={$t("library.grid")}
          aria-pressed={view === "grid"}
          onclick={() => toggleView("grid")}
        >
          <Icon name="grid" size={17} />
        </button>
        <button
          class:active={view === "list"}
          aria-label={$t("library.list")}
          aria-pressed={view === "list"}
          onclick={() => toggleView("list")}
        >
          <Icon name="list" size={17} />
        </button>
      </div>
    </div>
  </header>

  <section class="search-zone" aria-label={$t("library.semantic")}>
    <div class="search-line">
      <div class="search-box">
        <span class="search-icon" aria-hidden="true"><Icon name="search" size={18} /></span>
        <input
          type="search"
          bind:value={query}
          placeholder={$t("library.searchPlaceholder")}
          aria-label={$t("library.semantic")}
          oninput={() => {
            if (hits.length > 0) hits = [];
          }}
          onkeydown={(event) => {
            if (event.key === "Enter") void runSearch();
          }}
        />
        {#if query}
          <button
            class="search-clear"
            aria-label={$t("common.close")}
            onclick={() => {
              query = "";
              hits = [];
            }}
          >
            <Icon name="close" size={15} />
          </button>
        {/if}
        <button class="search-go" onclick={runSearch} disabled={searching}>
          {#if searching}
            <span class="mini-spin" aria-hidden="true"></span>
          {:else}
            {$t("library.search")}
          {/if}
        </button>
      </div>
      <div class="filter-chips" role="group" aria-label={$t("library.categories")}>
        <button class:active={filter === "all"} onclick={() => (filter = "all")}>
          {$t("common.all")}
        </button>
        <button class:active={filter === "suggestions"} onclick={() => (filter = "suggestions")}>
          <Icon name="sparkles" size={13} /> {$t("classification.pending")}
          {#if suggestions.length}<span class="chip-count">{suggestions.length}</span>{/if}
        </button>
        <button class:active={filter === "reading"} onclick={() => (filter = "reading")}>
          {$t("library.readingNow")}
        </button>
        <button class:active={filter === "finished"} onclick={() => (filter = "finished")}>
          {$t("library.finished")}
        </button>
        <button class:active={filter === "notStarted"} onclick={() => (filter = "notStarted")}>
          {$t("library.notStarted")}
        </button>
        <button class:active={filter === "uncategorized"} onclick={() => (filter = "uncategorized")}>
          {$t("library.uncategorized")}
        </button>
        <button class:active={filter === "favorites"} onclick={() => (filter = "favorites")}>
          <Icon name="star" size={13} /> {$t("library.favorites")}
          {#if favoriteCount}<span class="chip-count">{favoriteCount}</span>{/if}
        </button>
        <button class:active={filter === "audiobooks"} onclick={() => (filter = "audiobooks")}>
          <Icon name="headphones" size={13} /> {$t("library.audiobooks")}
          {#if audiobookCount}<span class="chip-count">{audiobookCount}</span>{/if}
        </button>
        <button class:active={filter === "epub"} onclick={() => (filter = "epub")}>
          {$t("library.epubOnly")}
        </button>
        <button class:active={filter === "pdf"} onclick={() => (filter = "pdf")}>
          {$t("library.pdfOnly")}
        </button>
        <button class:active={filter === "shared"} onclick={() => (filter = "shared")}>
          {$t("library.shared")}
          {#if hasShared}<span class="chip-count">{books.filter((book) => book.readOnly).length}</span>{/if}
        </button>
        {#each installedCategories as category (category.id)}
          <button class:active={filter === category.id} onclick={() => (filter = category.id)}>
            {#if category.color}<i style:background={category.color}></i>{/if}
            {category.name}
          </button>
        {/each}
      </div>
    </div>
    <div class="semantic-line">
      {#if semanticProgress}
        <span class="semantic-progress">
          <span class="progress-track">
            <span style:width={`${Math.round(semanticProgress.percent)}%`}></span>
          </span>
          {semanticProgress.message || semanticProgress.title} ·
          {Math.round(semanticProgress.percent)}%
        </span>
      {:else if semanticMessage}
        <span>{semanticMessage}</span>
      {:else if semantic?.supported === false}
        <span>{$t("error.unsupported")}</span>
      {:else if semantic?.available}
        <span>
          {$t("library.indexed", {
            indexed: semantic.indexedBooks,
            total: semantic.totalBooks,
            chunks: semantic.chunks,
          })}
        </span>
        <button class="link" onclick={reindex} disabled={searching}>
          {$t("library.reindex")}
        </button>
      {:else}
        <span>{$t("library.semanticHelp")}</span>
        <button class="link" onclick={prepareModel} disabled={searching}>
          {$t("library.prepareModel")}
        </button>
      {/if}
    </div>
    {#if normalizedQuery.length >= 2 && lexicalMatches.length > 0}
      <div class="lexical-results">
        <span class="eyebrow">
          {$t("library.lexicalResults", { count: lexicalMatches.length })}
        </span>
        <div class="lexical-list">
          {#each lexicalMatches.slice(0, 8) as book (book.id)}
            <button class="lexical-chip" onclick={() => onRead?.(book.id)}>
              <strong>{book.title}</strong>
              <small>{book.author ?? $t("reader.unknownAuthor")}</small>
            </button>
          {/each}
        </div>
      </div>
    {/if}
    {#if hits.length > 0}
      <ul class="hits">
        {#each hits as hit, index (`${hit.bookId}-${hit.section}-${hit.offset}-${index}`)}
          <li in:fly={{ y: 6, duration: 140 }}>
            <button class="hit" onclick={() => openHit(hit)}>
              <div class="hit-head">
                <strong>{hit.bookTitle}{#if hit.sectionTitle} · {hit.sectionTitle}{/if}</strong>
                <small>{Math.round(hit.score * 100)}%</small>
              </div>
              <span>{hit.snippet}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if suggestions.length > 0 && filter !== "suggestions"}
    <div class="suggestion-banner">
      <span>
        <Icon name="sparkles" size={16} />
        {$t("classification.banner", { count: suggestions.length })}
      </span>
      <button class="link" onclick={() => (filter = "suggestions")}>
        {$t("classification.review")}
      </button>
    </div>
  {/if}

  {#if filter === "suggestions"}
    <section class="suggestion-inbox">
      <header>
        <div>
          <h2><Icon name="sparkles" size={17} /> {$t("classification.title")}</h2>
          <p>{$t("classification.inboxHelp")}</p>
        </div>
        {#if suggestions.length === 0}
          <span class="suggestion-empty">{$t("classification.empty")}</span>
        {/if}
      </header>
      {#if suggestions.length > 0}
        <CategorySuggestions
          {suggestions}
          busyId={suggestionBusyId}
          onaccept={acceptSuggestion}
          ondismiss={dismissSuggestion}
        />
      {/if}
    </section>
  {/if}

  <div class="stats" aria-label={$t("library.title")}>
    <article class="stat">
      <span class="stat-icon tone-primary"><Icon name="library" size={21} /></span>
      <div>
        <strong>{books.length}</strong>
        <span>{$t("library.statBooks")}</span>
      </div>
    </article>
    <article class="stat">
      <span class="stat-icon tone-tertiary"><Icon name="book" size={21} /></span>
      <div>
        <strong>{readingCount}</strong>
        <span>{$t("library.statReading")}</span>
      </div>
    </article>
    <article class="stat">
      <span class="stat-icon tone-secondary"><Icon name="check" size={21} /></span>
      <div>
        <strong>{finishedCount}</strong>
        <span>{$t("library.statFinished")}</span>
      </div>
    </article>
    <article class="stat">
      <span class="stat-icon tone-warm"><Icon name="flame" size={21} /></span>
      <div>
        <strong>{categories.length}</strong>
        <span>{$t("library.categories")}</span>
      </div>
    </article>
  </div>

  <button
    class="mobile-import"
    onclick={chooseFile}
    disabled={busy === "import"}
  >
    <span class="mobile-import-icon"><Icon name="download" size={19} /></span>
    <span class="mobile-import-copy">
      <strong>{$t("library.importBannerTitle")}</strong>
      <small>{$t("library.importBannerHelp")}</small>
    </span>
    <span class="mobile-import-action">{$t("library.load")}</span>
  </button>

  {#if continueBook}
    <div class="section-row">
      <span class="eyebrow">{$t("library.continueReading")}</span>
      <button class="link" onclick={() => onNavigate?.("diary")}>
        {$t("library.fullHistory")} <span aria-hidden="true">→</span>
      </button>
    </div>
    <section class="continue" aria-labelledby="continue-title">
      <button
        class="continue-cover"
        onclick={() => onRead?.(continueBook.id)}
        aria-label={`${$t("library.open")} ${continueBook.title}`}
      >
        <span class="cover-spine" aria-hidden="true"></span>
        {#if coverSrc(continueBook) && !brokenCovers[continueBook.id]}
          <img
            class="cover"
            src={coverSrc(continueBook)}
            alt=""
            onerror={() => (brokenCovers = { ...brokenCovers, [continueBook.id]: true })}
          />
        {:else}
          <span class="cover placeholder">
            <strong>{continueBook.title}</strong>
            <small>{continueBook.author ?? continueBook.format.toUpperCase()}</small>
          </span>
        {/if}
        <span class="continue-page">{continueBook.format.toUpperCase()} · {Math.round(continueBook.progress * 100)}%</span>
      </button>

      <div class="continue-body">
        <div class="continue-tag">
          <span class="pulse-dot" aria-hidden="true"></span>
          {$t("library.continueTag")}
          <span class="continue-format">{continueBook.format.toUpperCase()}</span>
        </div>
        <h2 id="continue-title">{continueBook.title}</h2>
        <p class="continue-author">{continueBook.author ?? "—"}</p>
        <p class="continue-quote">«{$t("library.continueHelp")}»</p>
        <div class="continue-progress-row">
          <span class="progress-label">
            {Math.round(continueBook.progress * 100)}%
            <span>{$t("library.progressRead", { percent: Math.round(continueBook.progress * 100) })}</span>
          </span>
          <span class="progress-date">{$t("library.added")} · {addedLabel(continueBook)}</span>
        </div>
        <div
          class="progress-track"
          aria-label={$t("library.progressRead", {
            percent: Math.round(continueBook.progress * 100),
          })}
        >
          <span style:width={`${Math.round(continueBook.progress * 100)}%`}></span>
        </div>
        <div class="continue-actions">
          <Button variant="primary" onclick={() => onRead?.(continueBook.id)}>
            <Icon name="book" size={17} />
            {$t("library.continueAction")}
          </Button>
          <Button variant="ghost" onclick={() => (selectedBookId = continueBook.id)}>
            <Icon name="clock" size={17} />
            {$t("library.details")}
          </Button>
        </div>
      </div>

      <div class="continue-side" aria-hidden="true">
        <span class="continue-ornament">99</span>
      </div>
    </section>
  {/if}

  {#if loading}
    <Spinner label={$t("common.loading")} />
  {:else if books.length === 0}
    <EmptyState
      title={$t("library.noBooks")}
      description={$t("library.noBooksHelp")}
      action={$t("library.import")}
      onAction={chooseFile}
      icon="library"
    />
  {:else}
    <div class="shelf-layout">
      <div class="shelf">
        <div class="shelf-head">
          <h2>
            {$t("library.shelf")}
            <span>{booksLabel}</span>
          </h2>
          <button class="link" onclick={() => { filter = "all"; query = ""; hits = []; }}>
            {$t("library.showAll")}
          </button>
        </div>

        {#if filtered.length === 0}
          <EmptyState
            title={$t("library.noResults")}
            description={filter === "shared" ? $t("library.noSharedHelp") : $t("library.noResultsHelp")}
            action={filter === "shared" ? $t("library.sharedAction") : undefined}
            onAction={filter === "shared" ? () => onNavigate?.("account") : undefined}
            icon={filter === "shared" ? "sync" : "search"}
          />
        {:else}
          <ul class="books {view}">
            {#each filtered as book (book.id)}
              <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
              <li
                class="book"
                class:selected={detailsBook?.id === book.id}
                in:fly={{ y: 8, duration: 160 }}
                onmouseenter={() => (selectedBookId = book.id)}
                onfocusin={() => (selectedBookId = book.id)}
              >
                <div class="cover-frame">
                  <button
                    class="cover-button"
                    onclick={() => onRead?.(book.id)}
                    aria-label={`${$t("library.open")} ${book.title}`}
                  >
                    {#if coverSrc(book) && !brokenCovers[book.id]}
                      <img
                        class="cover"
                        src={coverSrc(book)}
                        alt=""
                        loading="lazy"
                        onerror={() => (brokenCovers = { ...brokenCovers, [book.id]: true })}
                      />
                    {:else}
                      <span class="cover placeholder">
                        <strong>{book.title}</strong>
                        <small>{book.author ?? book.format.toUpperCase()}</small>
                      </span>
                    {/if}
                    <span class="cover-sheen" aria-hidden="true"></span>
                  </button>
                  <button
                    class="cover-favorite"
                    class:active={book.favorite}
                    aria-label={book.favorite ? $t("library.removeFavorite") : $t("library.addFavorite")}
                    aria-pressed={book.favorite}
                    title={book.favorite ? $t("library.removeFavorite") : $t("library.addFavorite")}
                    onclick={(event) => {
                      event.stopPropagation();
                      void toggleFavorite(book);
                    }}
                  >
                    <Icon name="star" size={14} />
                  </button>
                  <span class="cover-format">{book.format.toUpperCase()}</span>
                  {#if audiobookBookIds.has(book.id)}
                    <span class="cover-audio-badge">
                      <Icon name="headphones" size={11} /> {$t("library.audioBadge")}
                    </span>
                  {/if}
                  {#if book.readOnly}
                    <span class="cover-badge"><Icon name="lock" size={11} /> {$t("library.readOnly")}</span>
                  {:else if book.progress >= 0.995}
                    <span class="cover-badge done"><Icon name="check" size={11} /> {$t("library.finished")}</span>
                  {/if}
                  {#if book.progress > 0 && book.progress < 0.995}
                    <span class="cover-progress-pill">{Math.round(book.progress * 100)}%</span>
                  {/if}
                </div>

                <div class="book-main">
                  {#if categoryLabel(book)}
                    <p class="book-genre">{categoryLabel(book)}</p>
                  {/if}
                  <div class="book-title-row">
                    <button class="title-button" onclick={() => onRead?.(book.id)}>
                      {book.title}
                    </button>
                    <button
                      class="book-menu-button"
                      aria-label={$t("library.options")}
                      aria-haspopup="dialog"
                      aria-expanded={menuBookId === book.id}
                      onclick={() => (menuBookId = book.id)}
                    >
                      <Icon name="more" size={18} />
                    </button>
                  </div>
                  <p class="author">{book.author ?? "—"}</p>
                  <div class="book-foot">
                    <span class="book-state" class:done={book.progress >= 0.995}>
                      {#if book.progress >= 0.995}
                        <Icon name="check" size={13} /> {$t("library.finished")}
                      {:else if book.progress > 0}
                        {$t("library.progressRead", { percent: Math.round(book.progress * 100) })}
                      {:else}
                        {$t("library.notStarted")}
                      {/if}
                    </span>
                    <span class="book-format-label">{book.format.toUpperCase()}</span>
                  </div>
                  {#if book.progress > 0 && book.progress < 0.995}
                    <div
                      class="reading-progress"
                      aria-label={$t("library.progressRead", {
                        percent: Math.round(book.progress * 100),
                      })}
                    >
                      <span style:width={`${Math.round(book.progress * 100)}%`}></span>
                    </div>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      {#if detailsBook}
        <aside class="details" aria-label={$t("library.details")}>
          <header class="details-head">
            <span class="details-icon"><Icon name="clock" size={18} /></span>
            <div>
              <strong>{$t("library.details")}</strong>
              <span>{$t("library.selected")}</span>
            </div>
          </header>

          <div class="details-book">
            <span class="details-cover">
              {#if coverSrc(detailsBook) && !brokenCovers[detailsBook.id]}
                <img src={coverSrc(detailsBook)} alt="" />
              {:else}
                <span class="details-placeholder">{detailsBook.title.slice(0, 1)}</span>
              {/if}
            </span>
            <div>
              {#if categoryLabel(detailsBook)}<p class="book-genre">{categoryLabel(detailsBook)}</p>{/if}
              <strong>{detailsBook.title}</strong>
              <span class="details-author">{detailsBook.author ?? "—"}</span>
              <div class="details-pills">
                <span>{detailsBook.format.toUpperCase()}</span>
                {#if detailsBook.readOnly}<span>{$t("library.readOnly")}</span>{/if}
                {#if originLabel(detailsBook.origin)}<span>{originLabel(detailsBook.origin)}</span>{/if}
              </div>
            </div>
          </div>

          {#if metadataEditing && metadataTargetId === detailsBook.id}
            <div class="metadata-form">
              <label>
                <span>{$t("library.titleField")}</span>
                <input bind:value={metadataDraft.title} maxlength="300" />
              </label>
              <label>
                <span>{$t("library.authorField")}</span>
                <input bind:value={metadataDraft.author} maxlength="200" />
              </label>
              <div class="metadata-row">
                <label>
                  <span>{$t("common.language")}</span>
                  <input bind:value={metadataDraft.language} maxlength="40" />
                </label>
                <label>
                  <span>{$t("library.publisher")}</span>
                  <input bind:value={metadataDraft.publisher} maxlength="200" />
                </label>
              </div>
              <label>
                <span>{$t("library.description")}</span>
                <textarea bind:value={metadataDraft.description} rows="4" maxlength="8000"></textarea>
              </label>
              <div class="metadata-actions">
                <Button
                  variant="primary"
                  size="small"
                  loading={metadataBusy}
                  disabled={!metadataDraft.title.trim()}
                  onclick={() => saveMetadata(detailsBook)}
                >
                  {$t("common.save")}
                </Button>
                <Button variant="ghost" size="small" onclick={cancelMetadataEdit}>
                  {$t("common.cancel")}
                </Button>
              </div>
            </div>
          {:else}
            <dl class="details-grid">
              <div>
                <dt>{$t("library.format")}</dt>
                <dd>{detailsBook.format.toUpperCase()}</dd>
              </div>
              <div>
                <dt>{$t("library.progress")}</dt>
                <dd>{Math.round(detailsBook.progress * 100)}%</dd>
              </div>
              <div>
                <dt>{$t("library.status")}</dt>
                <dd>{statusLabel(detailsBook)}</dd>
              </div>
              <div>
                <dt>{$t("library.size")}</dt>
                <dd>{formatBytes(detailsBook.sizeBytes)}</dd>
              </div>
              <div>
                <dt>{$t("library.added")}</dt>
                <dd>{addedLabel(detailsBook)}</dd>
              </div>
              <div>
                <dt>{$t("library.lastRead")}</dt>
                <dd>{lastReadLabel(detailsBook)}</dd>
              </div>
              {#if detailsBook.language}
                <div>
                  <dt>{$t("common.language")}</dt>
                  <dd>{detailsBook.language}</dd>
                </div>
              {/if}
              {#if detailsBook.publisher}
                <div>
                  <dt>{$t("library.publisher")}</dt>
                  <dd>{detailsBook.publisher}</dd>
                </div>
              {/if}
              <div class="wide">
                <dt>{$t("library.category")}</dt>
                <dd>{categoryLabel(detailsBook) ?? $t("library.uncategorized")}</dd>
              </div>
              {#if detailsBook.description}
                <div class="wide">
                  <dt>{$t("library.description")}</dt>
                  <dd class="details-description">{detailsBook.description}</dd>
                </div>
              {/if}
            </dl>
            <button class="link details-edit" onclick={() => startMetadataEdit(detailsBook)}>
              <Icon name="edit" size={14} /> {$t("library.editMetadata")}
            </button>
          {/if}

          {#if suggestionsForDetails.length > 0}
            <div class="details-suggestions">
              <div class="details-suggestions-head">
                <strong>{$t("classification.title")}</strong>
                <button
                  class="link"
                  onclick={() => acceptAllSuggestions(detailsBook.id)}
                >
                  {$t("classification.acceptAll")}
                </button>
              </div>
              <CategorySuggestions
                suggestions={suggestionsForDetails}
                busyId={suggestionBusyId}
                onaccept={acceptSuggestion}
                ondismiss={dismissSuggestion}
              />
            </div>
          {/if}

          <div class="details-quote">
            <span>{$t("library.lastRead")}</span>
            <p>«{$t("library.continueHelp")}»</p>
          </div>

          <Button variant="primary" onclick={() => onRead?.(detailsBook.id)}>
            <Icon name="book" size={17} />
            {$t("library.openReader")}
          </Button>
          {#if onListen}
            <Button variant="ghost" onclick={() => onListen?.(detailsBook.id, 0)}>
              <Icon name="headphones" size={17} />
              {$t("library.listen")}
            </Button>
          {/if}
          <Button variant="ghost" onclick={() => exportBookNotes(detailsBook)}>
            <Icon name="download" size={17} />
            {$t("library.exportNotes")}
          </Button>
          <Button
            variant="ghost"
            loading={classifyingBookId === detailsBook.id}
            disabled={classifyingBookId !== null && classifyingBookId !== detailsBook.id}
            onclick={() => detectCategories(detailsBook.id)}
          >
            <Icon name="sparkles" size={17} />
            {$t("classification.detect")}
          </Button>
          <Button variant="ghost" onclick={() => chooseCover(detailsBook)}>
            <Icon name="folder" size={17} />
            {$t("library.cover")}
          </Button>
          <div class="details-actions">
            <button class="link" onclick={() => (menuBookId = detailsBook.id)}>
              <Icon name="more" size={15} /> {$t("library.options")}
            </button>
            <span class="details-version">Kora</span>
          </div>
        </aside>
      {/if}
    </div>
  {/if}

  <details class="category-admin">
    <summary>
      <span><Icon name="list" size={16} /> {$t("library.manageCategories")}</span>
    </summary>
    <div class="category-form">
      <input
        bind:value={categoryName}
        placeholder={$t("library.newCategory")}
        maxlength="60"
        onkeydown={(event) => {
          if (event.key === "Enter") void addCategory();
        }}
      />
      <input
        class="category-color"
        type="color"
        bind:value={categoryColor}
        aria-label={$t("library.categoryColor")}
        title={$t("library.categoryColor")}
      />
      <Button variant="ghost" onclick={addCategory} loading={busy === "category"}>
        {$t("common.add")}
      </Button>
    </div>
    {#if categories.length > 0}
      <ul class="category-list">
        {#each categories as category (category.id)}
          <li>
            <input
              class="category-color"
              type="color"
              value={categoryDrafts[category.id]?.color ?? category.color ?? "#c2410c"}
              oninput={(event) =>
                updateCategoryDraft(category.id, "color", event.currentTarget.value)}
              aria-label={$t("library.categoryColor")}
            />
            <input
              class="category-name-input"
              value={categoryDrafts[category.id]?.name ?? category.name}
              maxlength="60"
              oninput={(event) =>
                updateCategoryDraft(category.id, "name", event.currentTarget.value)}
              onkeydown={(event) => {
                if (event.key === "Enter") void saveCategory(category);
              }}
            />
            <Button
              variant="ghost"
              size="small"
              loading={categoryBusyId === category.id}
              onclick={() => saveCategory(category)}
            >
              {$t("common.save")}
            </Button>
            <Button
              variant="danger"
              size="small"
              loading={categoryBusyId === category.id}
              onclick={() => deleteCategory(category)}
            >
              {$t("common.delete")}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </details>

  {#if menuBook}
    <div
      class="menu-backdrop"
      role="presentation"
      onclick={closeMenu}
    ></div>
    <div
      class="book-menu-sheet"
      role="dialog"
      aria-modal="true"
      aria-label={`${$t("library.options")}: ${menuBook.title}`}
    >
      <header>
        <div>
          <p class="eyebrow">{$t("library.options")}</p>
          <strong>{menuBook.title}</strong>
        </div>
        <button class="menu-close" aria-label={$t("common.close")} onclick={closeMenu}>
          <Icon name="close" size={18} />
        </button>
      </header>
      <div class="menu-actions">
        <button
          onclick={() => {
            closeMenu();
            onRead?.(menuBook.id);
          }}
        >
          <Icon name="book" size={17} /> {$t("library.open")}
        </button>
        {#if onListen}
          <button
            onclick={() => {
              const target = menuBook;
              closeMenu();
              onListen?.(target.id, 0);
            }}
          >
            <Icon name="headphones" size={17} /> {$t("library.listen")}
          </button>
        {/if}
        {#if menuBook.format === "pdf" && !menuBook.readOnly && !menuBook.receivedShareId}
          <button
            onclick={() => {
              const target = menuBook;
              closeMenu();
              conversionBook = target;
            }}
          >
            <Icon name="book" size={17} /> {$t("library.convertPdf")}
          </button>
        {/if}
        <button
          onclick={() => {
            closeMenu();
            void chooseCover(menuBook);
          }}
        >
          <Icon name="folder" size={17} /> {$t("library.cover")}
        </button>
        <label class="menu-field">
          <span>{$t("library.addCategory")}</span>
          <select
            value=""
            onchange={(event) => {
              const value = event.currentTarget.value;
              event.currentTarget.value = "";
              void assignCategory(menuBook.id, value);
            }}
          >
            <option value="">{$t("library.addCategory")}</option>
            {#each categories.filter((category) => !menuBook.categoryIds.includes(category.id)) as category (category.id)}
              <option value={category.id}>{category.name}</option>
            {/each}
          </select>
        </label>
        {#if assignedCategories.length > 0}
          <label class="menu-field">
            <span>{$t("library.removeCategory")}</span>
            <select
              value=""
              onchange={(event) => {
                const value = event.currentTarget.value;
                event.currentTarget.value = "";
                void unassignCategory(menuBook.id, value);
              }}
            >
              <option value="">{$t("library.removeCategory")}</option>
              {#each assignedCategories as category (category.id)}
                <option value={category.id}>{category.name}</option>
              {/each}
            </select>
          </label>
        {/if}
        <button
          onclick={() => {
            const target = menuBook.id;
            closeMenu();
            void detectCategories(target);
          }}
        >
          <Icon name="sparkles" size={17} /> {$t("classification.detect")}
        </button>
        <button
          class="danger"
          onclick={() => {
            closeMenu();
            void removeBook(menuBook);
          }}
          disabled={busy === menuBook.id}
        >
          <Icon name="close" size={17} /> {$t("library.delete")}
        </button>
      </div>
    </div>
  {/if}

  {#if trashOpen}
    <div class="menu-backdrop" role="presentation" onclick={() => (trashOpen = false)}></div>
    <div
      class="book-menu-sheet trash-sheet"
      role="dialog"
      aria-modal="true"
      aria-label={$t("library.trash")}
    >
      <header>
        <div>
          <p class="eyebrow">{$t("library.trash")}</p>
          <strong>{$t("library.trashTitle")}</strong>
        </div>
        <button
          class="menu-close"
          aria-label={$t("common.close")}
          onclick={() => (trashOpen = false)}
        >
          <Icon name="close" size={18} />
        </button>
      </header>

      <p class="trash-help">{$t("library.trashHelp")}</p>

      {#if trashLoading}
        <Spinner label={$t("common.loading")} />
      {:else if trash.length === 0}
        <p class="trash-empty">{$t("library.trashEmpty")}</p>
      {:else}
        <ul class="trash-list">
          {#each trash as item (item.id)}
            <li>
              <span class="trash-cover">
                {#if coverSrc(item) && !brokenCovers[item.id]}
                  <img
                    src={coverSrc(item)}
                    alt=""
                    onerror={() => (brokenCovers = { ...brokenCovers, [item.id]: true })}
                  />
                {:else}
                  <Icon name="book" size={18} />
                {/if}
              </span>
              <div class="trash-copy">
                <strong>{item.title}</strong>
                <small>
                  {item.author ?? $t("reader.unknownAuthor")} · {formatBytes(item.sizeBytes)} ·
                  {$t("library.trashedOn", {
                    date: new Date(item.deletedAt).toLocaleDateString(),
                  })}
                </small>
              </div>
              <div class="trash-actions">
                <Button
                  variant="subtle"
                  size="small"
                  loading={trashBusyId === item.id}
                  onclick={() => restoreTrashed(item)}
                >
                  {$t("library.restore")}
                </Button>
                <Button
                  variant="danger"
                  size="small"
                  loading={trashBusyId === item.id}
                  onclick={() => purgeTrashed(item)}
                >
                  {$t("library.deleteForever")}
                </Button>
              </div>
            </li>
          {/each}
        </ul>
        <div class="trash-footer">
          <Button
            variant="danger"
            size="small"
            loading={trashBusyId === "all"}
            onclick={emptyTrash}
          >
            <Icon name="trash" size={15} /> {$t("library.emptyTrash")}
          </Button>
        </div>
      {/if}
    </div>
  {/if}

  {#if dropActive}
    <div class="drop-overlay" role="status">
      <div>
        <span class="drop-icon"><Icon name="download" size={30} /></span>
        <strong>{$t("library.importHint")}</strong>
        <span>{$t("library.importPlaceholder")}</span>
      </div>
    </div>
  {/if}

  <PdfConversionModal
    open={conversionBook !== null}
    book={conversionBook}
    onClose={() => (conversionBook = null)}
    onConverted={handleConverted}
  />
</section>


<style>
  .library {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .page-head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 18px;
  }

  .title-block {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .page-title {
    margin-top: 6px;
    font-size: clamp(2rem, 3.2vw, 2.75rem);
  }

  .head-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }

  .head-hint {
    margin-left: 2px;
    font-size: 0.6875rem;
    font-weight: 500;
    opacity: 0.72;
  }

  .head-divider {
    width: 1px;
    height: 24px;
    margin: 0 4px;
    background: color-mix(in srgb, var(--border-warm) 45%, var(--border));
  }

  .sort-select {
    position: relative;
    display: flex;
    align-items: center;
  }

  .sort-select select {
    min-height: 38px;
    padding: 0.45rem 1.9rem 0.45rem 0.75rem;
    border: 0;
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.75rem;
    font-weight: 600;
    appearance: none;
    cursor: pointer;
  }

  .sort-select::after {
    content: "";
    position: absolute;
    right: 0.7rem;
    width: 6px;
    height: 6px;
    border-right: 1.5px solid var(--muted);
    border-bottom: 1.5px solid var(--muted);
    transform: rotate(45deg) translateY(-1px);
    pointer-events: none;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 12px;
    background: var(--surface-2);
  }

  .view-toggle button {
    display: grid;
    place-items: center;
    width: 32px;
    min-width: 32px;
    height: 32px;
    min-height: 32px;
    padding: 0;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .view-toggle button:hover {
    color: var(--text);
  }

  .view-toggle button.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-xs);
  }

  .search-zone {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .search-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 380px;
    min-width: 0;
    max-width: 640px;
    min-height: 44px;
    padding: 3px 3px 3px 12px;
    border: 1px solid transparent;
    border-radius: 12px;
    background: var(--surface-2);
  }

  .search-box:focus-within {
    border-color: var(--accent-line);
    background: var(--surface);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .search-icon {
    display: inline-flex;
    flex: 0 0 auto;
    color: var(--muted);
  }

  .search-box input {
    flex: 1;
    min-width: 0;
    min-height: 36px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.9rem;
  }

  .search-box input:focus {
    outline: 0;
  }

  .search-clear {
    display: grid;
    place-items: center;
    width: 28px;
    min-width: 28px;
    height: 28px;
    min-height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
  }

  .search-go {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 36px;
    padding: 0.35rem 0.95rem;
    border: 0;
    border-radius: 9px;
    background: linear-gradient(100deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    font: inherit;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .search-go:hover:not(:disabled) {
    background: linear-gradient(100deg, var(--accent-2), var(--accent-3));
  }

  .mini-spin {
    width: 13px;
    height: 13px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .filter-chips {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 2px;
    scrollbar-width: none;
  }

  .filter-chips::-webkit-scrollbar {
    display: none;
  }

  .filter-chips button {
    flex: 0 0 auto;
    min-height: 32px;
    padding: 0.3rem 0.8rem;
    border: 0;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.75rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .filter-chips button:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .filter-chips button.active {
    background: var(--text);
    color: var(--surface);
  }

  .filter-chips i {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 4px;
    border-radius: 50%;
  }

  .chip-count {
    margin-left: 4px;
    padding: 0 4px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 26%, transparent);
    font-size: 0.6rem;
  }

  .filter-chips button.active .chip-count {
    background: color-mix(in srgb, var(--surface) 22%, transparent);
  }

  .semantic-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    min-height: 16px;
    padding-left: 2px;
    color: var(--muted);
    font-size: 0.6875rem;
  }

  .semantic-progress {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .semantic-progress .progress-track {
    width: 110px;
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-2);
  }

  .semantic-progress .progress-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 180ms ease;
  }

  .lexical-results {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 7px;
  }

  .lexical-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .lexical-chip {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-height: 30px;
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
  }

  .lexical-chip:hover {
    border-color: var(--accent-line);
    background: var(--surface-2);
  }

  .lexical-chip strong {
    font-weight: 600;
  }

  .lexical-chip small {
    color: var(--muted);
  }

  .link {
    min-height: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: inherit;
    font-weight: 600;
    text-decoration: none;
  }

  .link:hover {
    text-decoration: underline;
  }

  .hits {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 8px;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow-sm);
  }

  .hit {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    min-height: 0;
    padding: 0.55rem 0.6rem;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .hit:hover {
    background: var(--surface-2);
  }

  .hit-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .hit-head strong {
    font-size: 0.78rem;
    font-weight: 600;
  }

  .hit-head small {
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 700;
  }

  .hit span {
    color: var(--muted);
    font-size: 0.75rem;
    line-height: 1.5;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .stat {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 14px 16px;
    border: 0;
    border-radius: 16px;
    background: var(--surface-2);
  }

  .stat-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
    border-radius: 12px;
    background: var(--surface-4);
  }

  .stat-icon.tone-primary {
    color: var(--accent);
  }

  .stat-icon.tone-tertiary {
    color: var(--accent-3);
  }

  .stat-icon.tone-secondary {
    color: #545f73;
  }

  .stat-icon.tone-warm {
    background: #ffdcc3;
    color: #6e3900;
  }

  .stat div {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .stat strong {
    font-family: var(--font-sans);
    font-size: 1.25rem;
    font-weight: 600;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .stat span:last-child {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.6875rem;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .section-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 2px 2px -10px;
  }

  .continue {
    position: relative;
    display: grid;
    grid-template-columns: 188px minmax(0, 1fr) 80px;
    align-items: center;
    gap: 30px;
    overflow: hidden;
    padding: 26px 30px;
    border: 0;
    border-radius: 24px;
    background:
      radial-gradient(420px 280px at 92% 2%, rgba(245, 158, 11, 0.12), transparent 70%),
      radial-gradient(300px 260px at 4% 100%, rgba(125, 83, 31, 0.09), transparent 72%),
      var(--surface-3);
    box-shadow: var(--shadow-sm);
  }

  .continue-cover {
    position: relative;
    display: block;
    width: 188px;
    height: 276px;
    min-height: 0;
    padding: 0;
    border: 0;
    border-radius: 3px 12px 12px 3px;
    background: var(--surface-4);
    box-shadow:
      0 20px 34px -18px rgba(49, 24, 12, 0.38),
      0 2px 0 rgba(255, 255, 255, 0.6) inset;
    overflow: hidden;
    transform: perspective(900px) rotateY(1.6deg);
    transition: transform 220ms ease;
  }

  .continue-cover:hover {
    transform: perspective(900px) rotateY(0deg) translateY(-2px);
  }

  .cover-spine {
    position: absolute;
    inset: 0 auto 0 0;
    z-index: 2;
    width: 13px;
    background: linear-gradient(90deg, rgba(0, 0, 0, 0.35), rgba(255, 255, 255, 0.12) 60%, transparent);
    pointer-events: none;
  }

  .continue-cover .cover {
    width: 100%;
    height: 100%;
    border: 0;
    border-radius: inherit;
    object-fit: cover;
  }

  .continue-page {
    position: absolute;
    right: 10px;
    bottom: 10px;
    z-index: 3;
    padding: 0.3rem 0.5rem;
    border-radius: 5px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    color: var(--text);
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    box-shadow: var(--shadow-xs);
  }

  .continue-body {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
  }

  .continue-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0.22rem 0.6rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent-2);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 13%, transparent);
  }

  .continue-format {
    margin-left: 2px;
    padding-left: 7px;
    border-left: 1px solid var(--accent-line);
    color: var(--muted);
    font-size: 0.58rem;
  }

  .continue-body h2 {
    margin: 10px 0 2px;
    font-family: var(--font-display);
    font-size: clamp(1.7rem, 3vw, 2.35rem);
    font-weight: 500;
    line-height: 1.08;
    letter-spacing: -0.025em;
  }

  .continue-author {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .continue-quote {
    max-width: 62ch;
    margin: 12px 0 16px;
    color: color-mix(in srgb, var(--muted) 92%, var(--text));
    font-family: var(--font-display);
    font-size: 1rem;
    font-style: italic;
    line-height: 1.5;
  }

  .continue-progress-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    margin-bottom: 6px;
  }

  .progress-label {
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
  }

  .progress-label span {
    margin-left: 5px;
    color: var(--muted);
    font-size: 0.6875rem;
    font-weight: 500;
  }

  .progress-date {
    color: var(--muted);
    font-size: 0.65rem;
  }

  .progress-track {
    width: 100%;
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 12%, var(--surface));
    box-shadow: inset 0 1px 2px rgba(49, 24, 12, 0.08);
  }

  .progress-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .continue-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 16px;
  }

  .continue-side {
    align-self: stretch;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    pointer-events: none;
  }

  .continue-ornament {
    color: color-mix(in srgb, var(--accent) 12%, transparent);
    font-family: var(--font-display);
    font-size: 4.4rem;
    font-style: italic;
    line-height: 0.8;
  }

  .shelf-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 292px;
    align-items: start;
    gap: 22px;
  }

  .shelf {
    min-width: 0;
  }

  .shelf-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .shelf-head h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.55rem;
    font-weight: 500;
    letter-spacing: -0.02em;
  }

  .shelf-head h2 span {
    padding: 0.14rem 0.45rem;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--muted);
    font-family: var(--font-sans);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0;
  }

  .books {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(148px, 1fr));
    gap: 14px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .book {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 8px;
    border: 1px solid transparent;
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
    transition:
      border-color 160ms ease,
      box-shadow 160ms ease,
      transform 160ms ease;
  }

  .book:hover,
  .book.selected {
    border-color: color-mix(in srgb, var(--accent) 22%, var(--border));
    box-shadow: var(--shadow-sm);
  }

  .book:hover {
    transform: translateY(-2px);
  }

  .cover-frame {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
  }

  .cover-frame::before {
    content: "";
    position: absolute;
    inset: 0 auto 0 0;
    z-index: 2;
    width: 8px;
    border-radius: 6px 0 0 6px;
    background: linear-gradient(90deg, rgba(18, 10, 5, 0.28), transparent);
    pointer-events: none;
  }

  .cover-button {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: 0;
    overflow: hidden;
    border: 0;
    border-radius: 6px;
    background: var(--surface-3);
    box-shadow:
      0 1px 2px rgba(49, 24, 12, 0.12),
      0 12px 24px -20px rgba(49, 24, 12, 0.45);
  }

  .cover {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: inherit;
    object-fit: cover;
  }

  .cover.placeholder {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.4rem;
    padding: 1rem;
    background:
      linear-gradient(145deg, rgba(141, 75, 0, 0.08), transparent 60%),
      var(--surface-3);
    color: var(--text);
    text-align: left;
  }

  .cover.placeholder strong {
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 500;
    line-height: 1.15;
  }

  .cover.placeholder small {
    color: var(--muted);
    font-size: 0.62rem;
  }

  .cover-sheen {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: linear-gradient(105deg, transparent 38%, rgba(255, 255, 255, 0.1) 48%, transparent 60%);
    pointer-events: none;
  }

  .cover-format {
    position: absolute;
    top: 6px;
    left: 11px;
    z-index: 3;
    padding: 0.16rem 0.36rem;
    border: 1px solid color-mix(in srgb, var(--border) 56%, transparent);
    border-radius: 5px;
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    color: var(--text);
    font-size: 0.5rem;
    font-weight: 700;
    letter-spacing: 0.07em;
  }

  .cover-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    left: auto;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    max-width: calc(100% - 62px);
    padding: 0.16rem 0.36rem;
    border-radius: 5px;
    background: color-mix(in srgb, var(--text) 68%, transparent);
    color: var(--surface);
    font-size: 0.5rem;
    font-weight: 600;
  }

  .cover-badge.done {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .cover-progress-pill {
    position: absolute;
    right: 6px;
    bottom: 6px;
    z-index: 3;
    padding: 0.18rem 0.4rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    color: var(--accent);
    font-size: 0.56rem;
    font-weight: 700;
    box-shadow: var(--shadow-xs);
  }

  .book-main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 8px 2px 2px;
  }

  .book-genre {
    margin: 0 0 3px;
    overflow: hidden;
    color: var(--accent);
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-overflow: ellipsis;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .book-title-row {
    display: flex;
    align-items: flex-start;
    gap: 4px;
  }

  .title-button {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 500;
    line-height: 1.2;
    letter-spacing: -0.015em;
    text-align: left;
  }

  .title-button:hover {
    color: var(--accent);
  }

  .book-menu-button {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 26px;
    min-width: 26px;
    height: 26px;
    min-height: 26px;
    padding: 0;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    opacity: 0;
  }

  .book:hover .book-menu-button,
  .book-menu-button:focus-visible {
    opacity: 1;
  }

  .book-menu-button:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .author {
    overflow: hidden;
    margin: 2px 0 6px;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .book-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-top: auto;
  }

  .book-state {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.62rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .book-state.done {
    color: var(--ok);
    font-weight: 600;
  }

  .book-format-label {
    flex: 0 0 auto;
    color: color-mix(in srgb, var(--muted) 80%, transparent);
    font-size: 0.54rem;
    font-weight: 700;
    letter-spacing: 0.06em;
  }

  .reading-progress {
    width: 100%;
    height: 3px;
    margin-top: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .reading-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
  }

  .details {
    position: sticky;
    top: calc(var(--topbar-height) + 18px);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: var(--shadow-sm);
  }

  .details-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }

  .details-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
    color: var(--accent);
  }

  .details-head div {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }

  .details-head strong {
    font-family: var(--font-display);
    font-size: 1.1rem;
    font-weight: 500;
    line-height: 1.15;
    letter-spacing: -0.015em;
  }

  .details-head span:last-child {
    color: var(--muted);
    font-size: 0.625rem;
  }

  .details-book {
    display: flex;
    gap: 11px;
    align-items: flex-start;
    padding-bottom: 2px;
  }

  .details-cover {
    display: block;
    flex: 0 0 auto;
    width: 58px;
    height: 84px;
    overflow: hidden;
    border-radius: 4px 7px 7px 4px;
    background: var(--surface-3);
    box-shadow: 0 6px 14px -8px rgba(49, 24, 12, 0.45);
  }

  .details-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .details-placeholder {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    color: var(--muted);
    font-family: var(--font-display);
    font-size: 1.4rem;
  }

  .details-book > div {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .details-book strong {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 1.02rem;
    font-weight: 500;
    line-height: 1.18;
    letter-spacing: -0.01em;
  }

  .details-author {
    margin-top: 2px;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .details-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 7px;
  }

  .details-pills span {
    padding: 0.18rem 0.4rem;
    border: 1px solid var(--accent-line);
    border-radius: 5px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .details-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 9px 10px;
    margin: 2px 0 0;
    padding: 12px 0;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .details-grid div {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .details-grid .wide {
    grid-column: 1 / -1;
  }

  .details-grid dt {
    color: var(--muted);
    font-size: 0.58rem;
    font-weight: 500;
  }

  .details-grid dd {
    margin: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 0.7rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .details-grid dd.details-description {
    overflow: visible;
    white-space: normal;
    font-weight: 400;
    line-height: 1.45;
  }

  .details-edit {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    padding: 0;
  }

  .metadata-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .metadata-form label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--muted);
    font-size: 0.65rem;
  }

  .metadata-form input,
  .metadata-form textarea {
    width: 100%;
    min-height: 34px;
    padding: 0.4rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.75rem;
  }

  .metadata-form textarea {
    min-height: 74px;
    resize: vertical;
  }

  .metadata-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .metadata-actions {
    display: flex;
    gap: 6px;
  }

  .metadata-actions :global(.ui-button) {
    width: auto;
  }

  .details-quote {
    padding: 10px 12px;
    border-left: 2px solid var(--accent-2);
    border-radius: 0 8px 8px 0;
    background: color-mix(in srgb, var(--accent) 5%, var(--surface-2));
  }

  .details-quote span {
    display: block;
    color: var(--muted);
    font-size: 0.55rem;
    font-weight: 600;
    letter-spacing: 0.11em;
    text-transform: uppercase;
  }

  .details-quote p {
    margin: 4px 0 0;
    color: var(--muted);
    font-family: var(--font-display);
    font-size: 0.82rem;
    font-style: italic;
    line-height: 1.4;
  }

  .details :global(.ui-button) {
    justify-content: flex-start;
    width: 100%;
  }

  .details-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 3px;
  }

  .details-actions .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .details-version {
    color: color-mix(in srgb, var(--muted) 60%, transparent);
    font-size: 0.55rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .category-admin {
    align-self: flex-start;
    padding: 0;
    color: var(--muted);
    font-size: 0.75rem;
  }

  .category-admin summary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0.35rem 0.6rem;
    border-radius: 8px;
    cursor: pointer;
    list-style: none;
  }

  .category-admin summary:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .category-admin summary::-webkit-details-marker {
    display: none;
  }

  .category-admin summary span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .category-form {
    display: flex;
    gap: 6px;
    margin-top: 7px;
  }

  .category-form input {
    min-width: 180px;
  }

  .category-form .category-color,
  .category-list .category-color {
    flex: 0 0 auto;
    width: 32px;
    min-width: 32px;
    height: 32px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }

  .category-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 560px;
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }

  .category-list li {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .category-name-input {
    flex: 1;
    min-width: 140px;
    min-height: 34px;
    padding: 0.35rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.75rem;
  }

  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: rgba(28, 16, 8, 0.38);
    backdrop-filter: blur(2px);
  }

  .book-menu-sheet {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 90;
    width: min(430px, calc(100vw - 32px));
    max-height: calc(100vh - 40px);
    overflow-y: auto;
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--surface);
    box-shadow: var(--shadow-lg);
    transform: translate(-50%, -50%);
  }

  .book-menu-sheet header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
  }

  .book-menu-sheet header strong {
    display: block;
    margin-top: 3px;
    font-family: var(--font-display);
    font-size: 1.2rem;
    font-weight: 500;
    line-height: 1.2;
  }

  .menu-close {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    min-width: 32px;
    height: 32px;
    min-height: 32px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
  }

  .trash-sheet {
    width: min(640px, calc(100vw - 32px));
  }

  .trash-help {
    margin: 0 0 12px;
    color: var(--muted);
    font-size: 0.75rem;
    line-height: 1.5;
  }

  .trash-empty {
    padding: 28px 12px;
    border: 1px dashed var(--border);
    border-radius: 12px;
    color: var(--muted);
    text-align: center;
    font-size: 0.8rem;
  }

  .trash-list {
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .trash-list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
  }

  .trash-cover {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 36px;
    height: 48px;
    overflow: hidden;
    border-radius: 6px;
    background: var(--surface-3);
    color: var(--muted);
  }

  .trash-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .trash-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .trash-copy strong,
  .trash-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .trash-copy strong {
    font-size: 0.82rem;
  }

  .trash-copy small {
    color: var(--muted);
    font-size: 0.66rem;
  }

  .trash-actions {
    display: flex;
    flex: 0 0 auto;
    gap: 5px;
  }

  .trash-footer {
    display: flex;
    justify-content: flex-end;
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  @media (max-width: 600px) {
    .trash-list li {
      align-items: flex-start;
      flex-wrap: wrap;
    }

    .trash-actions {
      width: 100%;
      justify-content: flex-end;
    }
  }

  .menu-actions {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .menu-actions > button,
  .menu-field {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 42px;
    width: 100%;
    padding: 0.5rem 0.65rem;
    border: 0;
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 500;
    text-align: left;
  }

  .menu-actions > button:hover {
    background: var(--surface-3);
  }

  .menu-actions .danger {
    color: var(--danger);
  }

  .menu-field {
    justify-content: space-between;
    gap: 12px;
    cursor: default;
  }

  .menu-field select {
    flex: 1;
    min-width: 0;
    max-width: 62%;
    min-height: 32px;
    background: var(--surface);
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 95;
    display: grid;
    place-items: center;
    padding: 24px;
    background: color-mix(in srgb, var(--bg) 78%, transparent);
  }

  .drop-overlay > div {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-width: 360px;
    padding: 34px;
    border: 2px dashed var(--accent-line);
    border-radius: 20px;
    background: var(--surface);
    text-align: center;
    box-shadow: var(--shadow-lg);
  }

  .drop-icon {
    display: grid;
    place-items: center;
    width: 58px;
    height: 58px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .drop-overlay strong {
    font-family: var(--font-display);
    font-size: 1.2rem;
    font-weight: 500;
  }

  .drop-overlay span:last-child {
    color: var(--muted);
    font-size: 0.78rem;
  }

  .books.list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .books.list .book {
    flex-direction: row;
    align-items: center;
    gap: 12px;
    padding: 8px;
  }

  .books.list .cover-frame {
    flex: 0 0 56px;
    width: 56px;
    height: 80px;
    aspect-ratio: auto;
  }

  .books.list .cover-frame::before,
  .books.list .cover-format,
  .books.list .cover-badge,
  .books.list .cover-progress-pill {
    display: none;
  }

  .books.list .book-main {
    flex: 1;
    padding: 2px 4px;
  }

  .books.list .title-button {
    font-size: 1rem;
  }

  .books.list .author {
    margin-bottom: 2px;
  }

  .books.list .reading-progress {
    max-width: 180px;
  }

  .books.list .book-menu-button {
    opacity: 1;
  }

  @media (max-width: 1240px) {
    .filter-chips {
      flex-basis: 100%;
      justify-content: flex-start;
    }
  }

  @media (max-width: 1180px) {
    .shelf-layout {
      grid-template-columns: minmax(0, 1fr) 264px;
      gap: 16px;
    }

    .continue {
      grid-template-columns: 160px minmax(0, 1fr);
      gap: 24px;
      padding: 22px;
    }

    .continue-cover {
      width: 160px;
      height: 238px;
    }

    .continue-side {
      display: none;
    }
  }

  @media (max-width: 1040px) {
    .shelf-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .details {
      display: none;
    }
  }

  @media (max-width: 900px) {
    .library {
      gap: 18px;
    }

    .page-head {
      align-items: flex-start;
    }

    .head-actions {
      width: 100%;
    }

    .head-actions :global(.ui-button:first-child) {
      flex: 1;
    }

    .head-hint,
    .head-divider,
    .sort-select {
      display: none;
    }

    .search-line {
      flex-direction: column;
      align-items: stretch;
      gap: 10px;
    }

    .search-box {
      flex: 0 0 auto;
      width: 100%;
      max-width: none;
    }

    .filter-chips {
      flex: 0 0 auto;
      width: calc(100% + 32px);
      justify-content: flex-start;
      margin: 0 -16px;
      padding: 0 16px 2px;
    }

    .stats {
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 0;
      padding: 10px 4px;
      border: 1px solid var(--border);
      border-radius: 14px;
      background: var(--surface);
      box-shadow: var(--shadow-xs);
    }

    .stat {
      flex-direction: column;
      align-items: center;
      gap: 1px;
      padding: 1px 3px;
      border: 0;
      border-right: 1px solid var(--border);
      border-radius: 0;
      background: transparent;
      text-align: center;
    }

    .stat:last-child {
      border-right: 0;
    }

    .stat-icon {
      display: none;
    }

    .stat div {
      align-items: center;
      gap: 0;
    }

    .stat strong {
      font-size: 1.18rem;
      line-height: 1.1;
    }

    .stat span {
      font-size: 0.55rem;
      line-height: 1.2;
    }

    .continue {
      grid-template-columns: 88px minmax(0, 1fr);
      gap: 14px;
      align-items: start;
      padding: 14px;
      border-radius: 18px;
    }

    .continue-cover {
      width: 88px;
      height: 132px;
    }

    .continue-body h2 {
      margin-top: 6px;
      font-size: 1.22rem;
      line-height: 1.12;
    }

    .continue-author {
      font-size: 0.7rem;
    }

    .continue-quote {
      display: none;
    }

    .continue-progress-row {
      flex-direction: column;
      gap: 2px;
      margin-top: 6px;
    }

    .continue-actions {
      width: 100%;
      margin-top: 10px;
    }

    .continue-actions :global(.ui-button) {
      flex: 1;
      padding: 0.45rem 0.55rem;
      font-size: 0.72rem;
    }

    .shelf-head h2 {
      font-size: 1.32rem;
    }

    .books {
      gap: 10px;
    }

    .book {
      padding: 7px;
      border-radius: 13px;
    }

    .title-button {
      font-size: 0.96rem;
    }

    .book-menu-button {
      opacity: 1;
    }

    .book-menu-sheet {
      top: auto;
      bottom: 0;
      left: 0;
      width: 100%;
      max-height: 80vh;
      border-radius: 20px 20px 0 0;
      transform: none;
    }
  }

  @media (max-width: 430px) {
    .head-actions :global(.ui-button:nth-child(2)) {
      width: 40px;
      min-width: 40px;
      padding: 0;
      overflow: hidden;
      font-size: 0;
      gap: 0;
    }
  }

  .cover-favorite {
    position: absolute;
    bottom: 6px;
    left: 11px;
    z-index: 3;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    min-width: 32px;
    min-height: 32px;
    padding: 0;
    border: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    border-radius: 50%;
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    color: var(--muted);
    box-shadow: var(--shadow-xs);
  }

  .cover-favorite:hover,
  .cover-favorite.active {
    border-color: var(--accent-line);
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .cover-favorite.active :global(svg) {
    fill: currentColor;
  }

  .filter-chips button :global(svg) {
    vertical-align: -2px;
    margin-right: 3px;
  }

  .filter-chips button.active :global(svg) {
    color: currentColor;
  }

  .mobile-import {
    display: none;
  }

  @media (max-width: 900px) {
    .mobile-import {
      display: flex;
      align-items: center;
      gap: 10px;
      width: 100%;
      padding: 10px 11px;
      border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--border));
      border-radius: 14px;
      background: var(--surface);
      color: var(--text);
      text-align: left;
      box-shadow: var(--shadow-xs);
    }

    .mobile-import-icon {
      display: grid;
      place-items: center;
      flex: 0 0 auto;
      width: 38px;
      height: 38px;
      border-radius: 10px;
      background: var(--accent-soft);
      color: var(--accent);
    }

    .mobile-import-copy {
      display: flex;
      flex: 1;
      flex-direction: column;
      min-width: 0;
    }

    .mobile-import-copy strong {
      font-size: 0.8rem;
      font-weight: 650;
    }

    .mobile-import-copy small {
      color: var(--muted);
      font-size: 0.63rem;
    }

    .mobile-import-action {
      flex: 0 0 auto;
      padding: 0.34rem 0.62rem;
      border-radius: 999px;
      background: var(--surface-3);
      color: var(--accent);
      font-size: 0.68rem;
      font-weight: 650;
    }

    .mobile-import:disabled {
      opacity: 0.55;
    }
  }

  @media (max-width: 900px) {
    .page-head {
      order: 0;
    }

    .search-zone {
      order: 1;
    }

    .section-row {
      order: 2;
    }

    .continue {
      order: 3;
    }

    .stats {
      order: 4;
    }

    .mobile-import {
      order: 5;
    }

    .shelf-layout {
      order: 6;
    }

    .library > :global(.spinner-wrap),
    .library > :global(.empty-state) {
      order: 7;
    }
  }

  .cover-audio-badge {
    position: absolute;
    top: 7px;
    left: 7px;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 3px 6px;
    border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 86%, transparent);
    color: var(--accent);
    font-size: 0.54rem;
    font-weight: 700;
    line-height: 1;
  }

  .suggestion-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin: 0.9rem 0 0;
    padding: 0.7rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--gold) 35%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--surface) 88%, var(--gold) 12%);
    font-size: 0.84rem;
  }

  .suggestion-banner > span {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .suggestion-inbox {
    margin: 0.9rem 0 0;
    padding: 0.95rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .suggestion-inbox > header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.75rem;
  }

  .suggestion-inbox h2 {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    margin: 0;
    font-size: 1rem;
  }

  .suggestion-inbox p {
    margin: 0.25rem 0 0;
    color: var(--muted);
    font-size: 0.8rem;
    line-height: 1.4;
  }

  .suggestion-empty {
    color: var(--muted);
    font-size: 0.82rem;
  }

  .details-suggestions {
    margin: 0.25rem 0 0.75rem;
    padding: 0.75rem;
    border: 1px dashed color-mix(in srgb, var(--gold) 40%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--surface) 94%, var(--gold) 6%);
  }

  .details-suggestions-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 0.55rem;
    font-size: 0.82rem;
  }
</style>

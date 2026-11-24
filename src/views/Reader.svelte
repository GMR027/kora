<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy, onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import AnnotationPanel from "../lib/components/AnnotationPanel.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import PdfView from "./PdfView.svelte";
  import { appearance } from "../lib/appearance";
  import { t, translate } from "../lib/i18n";
  import { readerOverlayOpen } from "../lib/readerOverlay";
  import {
    addMarker,
    addNote,
    aiChat,
    aiStatus,
    errorMessage,
    getMirrorStatus,
    getPeers,
    joinMirror,
    leaveMirror,
    libraryList,
    listAnnotations,
    practiceCancel,
    practiceDownloadModel,
    practiceHistory,
    practiceModels as practiceModelList,
    practiceStart,
    practiceStop,
    readerDeleteAnnotation,
    readerGetChapter,
    readerApplyToAll,
    readerOpen,
    readerSaveSettings,
    readerUpdateAnnotation,
    readerUpdateProgress,
    sessionEnd,
    sessionStart,
    startMirror,
    stopMirror,
    ttsAssignCharacterVoice,
    ttsCharacterVoices,
    ttsCharacters,
    ttsDownloadVoice,
    ttsPause,
    ttsPreferences,
    ttsResume,
    ttsSavePreferences,
    ttsSetPitch,
    ttsSetRate,
    ttsStart,
    ttsStatus as ttsStatusCommand,
    ttsStop,
    ttsVoices as ttsVoiceList,
  } from "../lib/api";
  import type {
    AiMessage,
    AiStatus,
    Annotation,
    AnnotationAnchor,
    AnnotationKind,
    BookSummary,
    ChapterContent,
    CharacterInfo,
    Extract,
    Locator,
    MirrorPositionEvent,
    MirrorStatus,
    MirrorTtsEvent,
    ModelDownloadProgress,
    PeerInfo,
    PracticeHistoryEntry,
    PracticeResult,
    Reaction,
    ReactionKind,
    ReaderMode,
    ReaderSession,
    ReaderSettings,
    SessionKind,
    SyncHandoffEvent,
    TtsPhraseEvent,
    TtsPreferences,
    TtsSessionEvent,
    TtsStatus,
    VoiceDescriptor,
    WhisperModelInfo,
  } from "../lib/types";

  let {
    bookId = null,
    initialLocator = null,
    autoListen = false,
    onExit,
    onSettings,
  }: {
    bookId?: string | null;
    initialLocator?: Locator | null;
    autoListen?: boolean;
    onExit?: () => void;
    onSettings?: () => void;
  } = $props();

  const colors = [
    "#ffd166",
    "#8ecae6",
    "#b7e4c7",
    "#ffb4a2",
    "#cdb4db",
    "#f4a261",
    "#90be6d",
    "#e9c46a",
  ];
  const quickReactions: { kind: ReactionKind; emoji: string; label: string }[] = [
    { kind: "like", emoji: "👍", label: "Me gusta" },
    { kind: "surprise", emoji: "😮", label: "Sorpresa" },
    { kind: "tension", emoji: "😬", label: "Tensión" },
    { kind: "boredom", emoji: "😴", label: "Aburrimiento" },
    { kind: "emotion", emoji: "🥹", label: "Emoción" },
  ];
  const fontStacks: Record<string, string> = {
    newsreader: '"Newsreader", "Iowan Old Style", "Palatino Linotype", Georgia, serif',
    literata: '"Literata", "Newsreader", Georgia, serif',
    garamond: '"EB Garamond", "Garamond", "Palatino Linotype", serif',
    atkinson: '"Atkinson Hyperlegible", "Plus Jakarta Sans", sans-serif',
    jakarta: '"Plus Jakarta Sans", ui-sans-serif, system-ui, sans-serif',
    mono: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
    opendyslexic: '"OpenDyslexic", "Atkinson Hyperlegible", sans-serif',
    serif: '"Newsreader", "Iowan Old Style", "Palatino Linotype", Georgia, serif',
    sans: '"Plus Jakarta Sans", ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
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

  interface DecorationMark {
    id: string;
    kind: "extract" | "reaction";
    rects: MarkRect[];
    title: string;
    emoji: string | null;
    offset: number;
    /** Borde izquierdo de la línea donde empieza la cita o la emoción. */
    anchorLeft: number;
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
  let extracts = $state<Extract[]>([]);
  let reactions = $state<Reaction[]>([]);
  let marks = $state<Mark[]>([]);
  let decorations = $state<DecorationMark[]>([]);
  let chapterProgress = $state(0);
  let bookProgress = $state(0);
  let loading = $state(false);
  let opening = $state(false);
  let error = $state<string | null>(null);
  let failedBookId = $state<string | null>(null);
  let tocOpen = $state(false);
  let settingsOpen = $state(false);
  let selection = $state<SelectionInfo | null>(null);
  let activeAnnotation = $state<Annotation | null>(null);
  let annotationNote = $state("");
  let annotationColor = $state(colors[0]);
  let extractNotice = $state<string | null>(null);
  let annotationsOpen = $state(false);
  let ttsPanelOpen = $state(false);
  let excerptTts = $state(false);
  let annotationsTab = $state<"marker" | "note" | "extract" | "reaction">("marker");
  let annotationsRefresh = $state(0);
  let voiceRecordToken = $state(0);
  let panelAnchor = $state<AnnotationAnchor | null>(null);
  let handoff = $state<SyncHandoffEvent | null>(null);
  let mirror = $state<MirrorStatus | null>(null);
  let mirrorPeers = $state<PeerInfo[]>([]);
  let mirrorOpen = $state(false);
  let mirrorBusy = $state(false);
  let spreadEnabled = $state(false);
  let pdfPage = $state(0);
  let aiOpen = $state(false);
  let aiTab = $state<"analysis" | "chat">("analysis");
  let aiStatusState = $state<AiStatus | null>(null);
  let aiBusy = $state<string | null>(null);
  let aiError = $state<string | null>(null);
  let aiSummary = $state("");
  let aiTerms = $state("");
  let aiCharacters = $state("");
  let aiChatMessages = $state<AiMessage[]>([]);
  let aiChatInput = $state("");
  let aiNotice = $state<string | null>(null);
  let aiScrollEl = $state<HTMLElement | null>(null);
  let sleepMinutes = $state(0);
  let sleepRemaining = $state(0);
  let sleepTimer: ReturnType<typeof setInterval> | undefined;
  let sleepNotice = $state<string | null>(null);
  let isFullscreen = $state(false);
  let pageIndex = $state(1);
  let pageCount = $state(1);
  let dimImages = $state(localStorage.getItem("kora.reader.dimImages") === "1");
  let focusLine = $state(localStorage.getItem("kora.reader.focusLine") === "1");
  let focusTop = $state(0);
  let focusLeft = $state(0);
  let focusWidth = $state(0);
  let focusVisible = $state(false);
  let searchQuery = $state("");
  let searchResults = $state<
    { section: number; offset: number; snippet: string; title: string }[]
  >([]);
  let searchBusy = $state(false);
  let searchProgress = $state(0);
  let searchError = $state<string | null>(null);
  let tocTab = $state<"index" | "search">("index");
  let pageJump = $state("");
  let termPanelOpen = $state(false);
  let termQuery = $state("");
  let termContext = $state("");
  let termResult = $state("");
  let termBusy = $state(false);
  let termError = $state<string | null>(null);
  let termTab = $state<"definition" | "context">("definition");
  let termNotice = $state<string | null>(null);

  let ttsStatus = $state<TtsStatus | null>(null);
  let ttsRate = $state(1);
  let ttsPitch = $state(1);
  let ttsVoices = $state<VoiceDescriptor[]>([]);
  let ttsVoiceId = $state("");
  let ttsPreferencesState = $state<TtsPreferences | null>(null);
  let ttsError = $state<string | null>(null);
  let ttsPhrase = $state<{ section: number; start: number; end: number } | null>(
    null,
  );
  let ttsRects = $state<MarkRect[]>([]);
  let characterPanelOpen = $state(false);
  let voicePickerOpen = $state(false);
  let characters = $state<CharacterInfo[]>([]);
  let characterVoices = $state<Record<string, string>>({});
  let practiceOpen = $state(false);
  let practiceRecording = $state(false);
  let practiceResult = $state<PracticeResult | null>(null);
  let practiceWords = $derived(
    practiceResult
      ? practiceResult.diff.filter((word) => word.kind !== "extra").length
      : 0,
  );
  let practiceWpm = $derived(
    practiceResult && practiceResult.durationMs > 0
      ? Math.round(practiceWords / (practiceResult.durationMs / 60_000))
      : 0,
  );
  let practiceError = $state<string | null>(null);
  let practiceHistoryEntries = $state<PracticeHistoryEntry[]>([]);
  let whisperModels = $state<WhisperModelInfo[]>([]);
  let practiceModelId = $state("tiny");
  let practiceDownload = $state<number | null>(null);
  let practiceSeconds = $state(0);
  let autoAdvance = $state(true);
  let downloadingVoiceId = $state<string | null>(null);

  let ttsSettingsTimer: ReturnType<typeof setTimeout> | undefined;
  let practiceTimer: ReturnType<typeof setInterval> | undefined;
  let ttsFrame = 0;
  let unlistenHandoff: UnlistenFn | undefined;
  let unlistenMirrorPosition: UnlistenFn | undefined;
  let unlistenMirrorTts: UnlistenFn | undefined;
  let unlistenPhraseStart: UnlistenFn | undefined;
  let unlistenPhraseEnd: UnlistenFn | undefined;
  let unlistenTtsState: UnlistenFn | undefined;
  let unlistenPracticeDownload: UnlistenFn | undefined;
  let disposed = false;
  let activeSessionId: string | null = null;
  let activeSessionKind: SessionKind | null = null;
  let sessionRequest = 0;
  let visitedSections = $state(new Set<number>());
  let autoListenConsumed = false;
  let openRequest = 0;
  let chapterRequest = 0;

  let scrollerEl = $state<HTMLElement | null>(null);
  let contentEl = $state<HTMLElement | null>(null);
  let progressTimer: ReturnType<typeof setTimeout> | undefined;
  let settingsTimer: ReturnType<typeof setTimeout> | undefined;
  let settingsRevision = 0;
  let settingsFrame = 0;
  let scrollFrame = 0;

  let ttsState = $derived(ttsStatus?.state ?? "idle");
  let ttsActive = $derived(ttsState === "playing" || ttsState === "paused");

  $effect(() => {
    // El motor TTS es global: si reproduce otro libro u otro audiolibro, el
    // lector no debe mostrar controles de pausa para la obra equivocada.
    if (ttsStatus?.bookId && session?.bookId && ttsStatus.bookId !== session.bookId) {
      ttsStatus = null;
      excerptTts = false;
    }
  });

  $effect(() => {
    if (ttsActive && !excerptTts) ttsPanelOpen = true;
  });

  $effect(() => {
    if (
      tocOpen ||
      settingsOpen ||
      aiOpen ||
      annotationsOpen ||
      termPanelOpen ||
      mirrorOpen ||
      characterPanelOpen ||
      practiceOpen ||
      activeAnnotation
    ) {
      voicePickerOpen = false;
    }
  });

  $effect(() => {
    // Cerrar el panel mientras se graba debe apagar el micrófono, no dejar
    // una captura huérfana en segundo plano.
    if (!practiceOpen && practiceRecording) {
      practiceRecording = false;
      clearInterval(practiceTimer);
      void practiceCancel();
    }
  });
  let installedVoices = $derived(
    ttsVoices.filter(
      (voice) =>
        voice.installed && (voice.language === "es" || voice.language === "en"),
    ),
  );
  let downloadableVoices = $derived(
    ttsVoices.filter(
      (voice) =>
        !voice.installed && (voice.language === "es" || voice.language === "en"),
    ),
  );
  let currentTtsVoice = $derived(
    installedVoices.find((voice) => voice.id === ttsVoiceId) ?? null,
  );
  let ttsLanguage = $derived.by(() => {
    const language = (session?.document.language ?? "").toLowerCase();
    if (language.startsWith("en")) return "en";
    return "es";
  });

  let sectionCount = $derived(session ? session.document.sections.length : 0);
  let pdfPageCount = $derived(session?.pdf?.pageCount ?? 0);
  let isPdf = $derived(session?.document.format === "pdf");
  let atBookStart = $derived(
    chapterIndex <= 0 && (chapterProgress <= 0.001 || pageCount <= 1),
  );
  let atBookEnd = $derived(
    sectionCount > 0 &&
      chapterIndex >= sectionCount - 1 &&
      (chapterProgress >= 0.999 || pageCount <= 1),
  );
  let annotationSections = $derived(
    session
      ? session.document.sections.map((section, index) => ({
          index,
          title: section.title ?? `Sección ${index + 1}`,
        }))
      : [],
  );
  let annotationAnchor = $derived.by<AnnotationAnchor>(() => {
    if (panelAnchor) return panelAnchor;
    if (selection) {
      return {
        section: chapterIndex,
        offsetStart: selection.start,
        offsetEnd: selection.end,
        text: selection.text,
      };
    }
    return {
      section: chapterIndex,
      offsetStart: 0,
      offsetEnd: 0,
      text: "",
    };
  });
  let themeClass = $derived(`theme-${settings?.theme ?? "light"}`);
  let surfaceTexture = $derived(
    settings?.texture === "inherit"
      ? ($appearance?.preferences.texture ?? "none")
      : (settings?.texture ?? "none"),
  );
  let surfaceStyle = $derived.by(() => {
    if (!settings) return "";
    const family =
      fontStacks[settings.fontFace] ??
      fontStacks[settings.fontFamily] ??
      fontStacks.newsreader;
    return [
      `--reader-font-size:${settings.fontSize}px`,
      `--reader-line-height:${settings.lineHeight}`,
      `--reader-margin:${settings.margin}px`,
      `--reader-font-family:${family}`,
      `--reader-zoom:${settings.zoom}`,
      `--reader-column-width:${settings.columnWidth}px`,
      `--reader-text-align:${settings.textAlign}`,
      `--reader-paragraph-indent:${settings.paragraphIndent > 0 ? "1.6em" : "0px"}`,
      `--reader-hyphens:${settings.hyphenate ? "auto" : "manual"}`,
    ].join(";");
  });

  $effect(() => {
    readerOverlayOpen.set(aiOpen);
  });

  onDestroy(() => {
    readerOverlayOpen.set(false);
  });

  onMount(() => {
    disposed = false;
    void loadLibrary();
    void initTtsEvents();
    void initSyncEvents();
    void refreshMirror();
    void refreshTtsStatus();

    const handleResize = () => {
      const fraction = chapterProgress;
      const index = chapterIndex;
      void measure().then(() => {
        if (disposed || chapterIndex !== index) return;
        scrollToFraction(fraction);
        scheduleMarks();
        scheduleTtsRects();
      });
    };

    window.addEventListener("resize", handleResize);
    window.addEventListener("keydown", onKeydown);

    return () => {
      disposed = true;
      openRequest += 1;
      chapterRequest += 1;
      sessionRequest += 1;
      window.removeEventListener("resize", handleResize);
      window.removeEventListener("keydown", onKeydown);
      clearTimeout(progressTimer);
      clearTimeout(settingsTimer);
      clearTimeout(ttsSettingsTimer);
      clearInterval(practiceTimer);
      clearInterval(sleepTimer);
      cancelAnimationFrame(settingsFrame);
      cancelAnimationFrame(scrollFrame);
      cancelAnimationFrame(ttsFrame);
      unlistenHandoff?.();
      unlistenMirrorPosition?.();
      unlistenMirrorTts?.();
      unlistenPhraseStart?.();
      unlistenPhraseEnd?.();
      unlistenTtsState?.();
      unlistenPracticeDownload?.();
      if (practiceRecording) void practiceCancel();
      if (!isPdf) {
        if (mirror?.hosting) void stopMirror().catch(() => {});
        else if (mirror?.active) void leaveMirror().catch(() => {});
      }
      void persistProgress();
      void saveSettings();
      void endReadingSession();
      void ttsStop().catch(() => {});
      if (isFullscreen) void getCurrentWindow().setFullscreen(false);
    };
  });

  async function initTtsEvents() {
    unlistenPhraseStart = await listen<TtsPhraseEvent>(
      "tts::phrase_start",
      (event) => {
        if (event.payload.bookId !== session?.bookId || isPdf) return;
        if (event.payload.section !== chapterIndex) return;
        ttsPhrase = {
          section: event.payload.section,
          start: event.payload.offsetStart,
          end: event.payload.offsetEnd,
        };
        scheduleTtsRects(true);
      },
    );
    if (disposed) {
      unlistenPhraseStart?.();
      return;
    }

    unlistenPhraseEnd = await listen<TtsPhraseEvent>(
      "tts::phrase_end",
      (event) => {
        if (event.payload.bookId !== session?.bookId || isPdf) return;
        ttsPhrase = null;
        ttsRects = [];
      },
    );
    if (disposed) {
      unlistenPhraseEnd?.();
      return;
    }

    unlistenTtsState = await listen<TtsSessionEvent>("tts::state", (event) => {
      if (event.payload.bookId !== session?.bookId || isPdf) return;
      const wasExcerpt = excerptTts;
      ttsStatus = {
        bookId: event.payload.bookId,
        section: event.payload.section,
        state: event.payload.state,
        rate: ttsRate,
        pitch: ttsPitch,
        chunkIndex: null,
        error: event.payload.error,
        excerpt: Boolean(event.payload.excerpt),
      };
      if (event.payload.error) ttsError = event.payload.error;
      if (
        event.payload.state === "playing" ||
        event.payload.state === "paused"
      ) {
        excerptTts = Boolean(event.payload.excerpt);
      }
      if (
        event.payload.state === "error" ||
        event.payload.state === "stopped"
      ) {
        excerptTts = false;
        ttsPhrase = null;
        ttsRects = [];
        if (session) void startReadingSession("reading");
      }
      if (event.payload.state === "finished") {
        excerptTts = false;
        ttsPhrase = null;
        ttsRects = [];
        if (session) void startReadingSession("reading");
        if (autoAdvance && !wasExcerpt) void advanceTts(event.payload.section);
      }
    });
    if (disposed) {
      unlistenTtsState?.();
      return;
    }

    unlistenPracticeDownload = await listen<ModelDownloadProgress>(
      "tts-practice-download-progress",
      (event) => {
        if (event.payload.done) {
          practiceDownload = null;
          if (event.payload.error) practiceError = event.payload.error;
          void loadWhisperModels();
        } else if (event.payload.total > 0) {
          practiceDownload =
            (event.payload.downloaded / event.payload.total) * 100;
        }
      },
    );
    if (disposed) {
      unlistenPracticeDownload?.();
    }
  }

  async function refreshTtsStatus() {
    try {
      ttsStatus = await ttsStatusCommand();
      excerptTts = Boolean(
        ttsStatus?.excerpt &&
          (ttsStatus.state === "playing" || ttsStatus.state === "paused"),
      );
    } catch {
      ttsStatus = null;
      excerptTts = false;
    }
  }

  let lastOpenKey = "";

  $effect(() => {
    const key = `${bookId ?? ""}:${initialLocator?.section ?? ""}:${initialLocator?.offset ?? ""}`;
    if (bookId && key !== lastOpenKey && bookId !== failedBookId) {
      lastOpenKey = key;
      void openBook(bookId, initialLocator);
    }
  });

  async function endReadingSession() {
    const id = activeSessionId;
    activeSessionId = null;
    activeSessionKind = null;
    if (!id) return;
    try {
      await sessionEnd(id, visitedSections.size || null);
    } catch {
      // Una sesión perdida no debe romper el lector.
    }
  }

  async function startReadingSession(kind: SessionKind) {
    if (!session || activeSessionKind === kind) return;
    const request = ++sessionRequest;
    await endReadingSession();
    if (disposed || request !== sessionRequest || !session) return;
    try {
      const started = await sessionStart(
        session.bookId,
        kind,
        chapterIndex,
        bookProgress,
      );
      if (disposed || request !== sessionRequest) {
        try {
          await sessionEnd(started.id, null);
        } catch {
          // La sesión efímera ya no existe.
        }
        return;
      }
      activeSessionId = started.id;
      activeSessionKind = kind;
    } catch {
      // El diario es best-effort desde la UI.
    }
  }

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

  function coverSrc(book: BookSummary): string | null {
    return book.coverPath ? convertFileSrc(book.coverPath) : null;
  }

  async function openBook(id: string, locator: Locator | null = null) {
    const request = ++openRequest;
    chapterRequest += 1;
    sessionRequest += 1;

    if (session) {
      await persistProgress();
      if (disposed || request !== openRequest) return;
      await saveSettings();
      if (disposed || request !== openRequest) return;
      await endReadingSession();
      if (disposed || request !== openRequest) return;
      clearTimeout(settingsTimer);
      settingsRevision += 1;
    }

    if (ttsActive && ttsStatus?.bookId !== id) {
      try {
        ttsStatus = await ttsStop();
      } catch {
        ttsStatus = null;
      }
      excerptTts = false;
      ttsPanelOpen = false;
      ttsPhrase = null;
      ttsRects = [];
      if (disposed || request !== openRequest) return;
    }

    if (mirror?.active && mirror.bookId !== id) {
      try {
        mirror = mirror.hosting ? await stopMirror() : await leaveMirror();
      } catch {
        // El espejo de otro libro se cierra igualmente al cambiar de obra.
        mirror = null;
      }
      if (disposed || request !== openRequest) return;
    }

    opening = true;
    loading = true;
    chapter = null;
    error = null;
    handoff = null;
    ttsError = null;
    extractNotice = null;
    ttsPanelOpen = false;
    selection = null;
    activeAnnotation = null;
    panelAnchor = null;
    tocOpen = false;
    settingsOpen = false;
    termPanelOpen = false;
    characterPanelOpen = false;
    voicePickerOpen = false;
    mirrorOpen = false;
    aiOpen = false;
    annotationsOpen = false;
    if (practiceRecording) {
      practiceRecording = false;
      clearInterval(practiceTimer);
      void practiceCancel();
    }
    practiceOpen = false;
    searchQuery = "";
    searchResults = [];
    searchError = null;
    searchProgress = 0;
    resetAiConversation();

    try {
      const opened = await readerOpen(id, locator);
      if (disposed || request !== openRequest) return;
      session = opened;
      settings = opened.settings;
      selectedBookId = id;
      failedBookId = null;
      bookProgress = opened.progress;
      chapterIndex = Math.min(
        opened.locator.section,
        Math.max(opened.document.sections.length - 1, 0),
      );
      visitedSections = new Set([chapterIndex]);
      try {
        await loadBookDecorations(id);
      } catch (cause) {
        // Sin anotaciones el libro sigue siendo legible; no bloqueamos la
        // apertura por un fallo al cargar las capas decorativas.
        console.error("No se pudieron cargar las anotaciones", cause);
      }
      if (disposed || request !== openRequest) return;
      ttsPhrase = null;
      ttsRects = [];
      autoListenConsumed = false;
      await loadTtsForBook(id);
      if (disposed || request !== openRequest) return;

      if (opened.document.format === "pdf") {
        chapter = null;
        pdfPage = opened.locator.section;
        chapterIndex = opened.locator.section;
        chapterProgress = opened.pdf?.pageCount
          ? (pdfPage + 1) / opened.pdf.pageCount
          : 0;
      } else {
        await loadChapter(chapterIndex, opened.locator.offset);
        if (disposed || request !== openRequest) return;
        if (!chapter) {
          failedBookId = id;
          session = null;
          return;
        }
        if (autoListen && !autoListenConsumed) {
          autoListenConsumed = true;
          ttsPanelOpen = true;
          await tick();
          if (disposed || request !== openRequest) return;
          await startTts();
        }
      }
      if (disposed || request !== openRequest) return;
      void startReadingSession("reading");
    } catch (cause) {
      if (disposed || request !== openRequest) return;
      error = errorMessage(cause);
      failedBookId = id;
      session = null;
      chapter = null;
      pdfPage = 0;
    } finally {
      if (request === openRequest) {
        opening = false;
        loading = false;
      }
    }
  }

  function resetAiConversation() {
    aiSummary = "";
    aiTerms = "";
    aiCharacters = "";
    aiChatMessages = [];
    aiChatInput = "";
    aiError = null;
    aiNotice = null;
    aiBusy = null;
  }

  function chapterPlainText(limit = 7_000): string {
    if (!chapter?.html) return "";
    const doc = new DOMParser().parseFromString(chapter.html, "text/html");
    const text = (doc.body.textContent ?? "").replace(/\s+/g, " ").trim();
    return text.slice(0, limit);
  }

  function aiLanguage(): string {
    return document.documentElement.lang.startsWith("en") ? "English" : "Spanish";
  }

  function aiContext(): string {
    if (!session) return "";
    const limit = aiStatusState?.provider === "local" ? 1_000 : 7_000;
    return [
      `Libro: ${session.document.title}`,
      `Autor: ${session.document.author ?? "desconocido"}`,
      `Capítulo: ${chapter?.title ?? chapterLabel(chapterIndex)}`,
      "Fragmento:",
      `"""${chapterPlainText(limit)}"""`,
    ].join("\n");
  }

  function aiSystemPrompt(): string {
    return [
      "Eres el asistente de lectura local de Kora.",
      "Trabajas únicamente con el fragmento del capítulo que se te entrega.",
      "No inventes hechos, no cites fuentes externas y no adelantes acontecimientos posteriores al capítulo.",
      "Si la respuesta no está en el texto, dilo con claridad.",
      `Responde en ${aiLanguage()}, con prosa clara y concisa.`,
      "Empieza directamente con la respuesta, sin repetir la tarea ni el título.",
    ].join(" ");
  }

  async function refreshAiStatus() {
    try {
      aiStatusState = await aiStatus();
      aiError = null;
    } catch (cause) {
      aiError = errorMessage(cause);
    }
  }

  async function ensureAiReady(): Promise<boolean> {
    if (!aiStatusState?.available) await refreshAiStatus();
    if (!aiStatusState?.available) {
      aiError = translate("reader.aiUnavailable");
      return false;
    }
    aiError = null;
    return true;
  }

  function closeReaderPanels() {
    tocOpen = false;
    settingsOpen = false;
    termPanelOpen = false;
    practiceOpen = false;
    activeAnnotation = null;
    characterPanelOpen = false;
    voicePickerOpen = false;
    mirrorOpen = false;
    aiOpen = false;
    annotationsOpen = false;
    panelAnchor = null;
    selection = null;
    window.getSelection()?.removeAllRanges();
  }

  async function openTermLookup() {
    if (!session) return;
    const anchor =
      panelAnchor ??
      (selection
        ? {
            section: chapterIndex,
            offsetStart: selection.start,
            offsetEnd: selection.end,
            text: selection.text,
          }
        : null);
    const selected = anchor?.text.trim() ?? "";
    if (!anchor || !selected) return;
    termQuery = selected.length > 120 ? `${selected.slice(0, 120)}…` : selected;
    const doc = chapter
      ? new DOMParser().parseFromString(chapter.html, "text/html")
      : null;
    const text = doc?.body.textContent ?? "";
    const start = Math.max(0, anchor.offsetStart - 220);
    const end = Math.min(text.length, anchor.offsetEnd + 220);
    termContext = `${start > 0 ? "…" : ""}${text
      .slice(start, end)
      .replace(/\s+/g, " ")
      .trim()}${end < text.length ? "…" : ""}`;
    termResult = "";
    termError = null;
    termNotice = null;
    termTab = "definition";
    termPanelOpen = true;
    selection = null;
    panelAnchor = null;
    settingsOpen = false;
    tocOpen = false;
    annotationsOpen = false;
    aiOpen = false;
    window.getSelection()?.removeAllRanges();
    await lookupTerm();
  }

  async function lookupTerm() {
    if (!session || termBusy) return;
    termBusy = true;
    termError = null;
    termResult = "";
    try {
      if (!(await ensureAiReady())) {
        termError = translate("reader.termAiOffline");
        return;
      }
      const reply = await aiChat([
        {
          role: "system",
          content: [
            "Eres un lexicógrafo local de Kora.",
            "Responde en el idioma del lector, con precisión y sin inventar datos.",
            "Si no conoces la etimología, escribe exactamente «Etimología no disponible».",
            "No cites fuentes externas.",
          ].join(" "),
        },
        {
          role: "user",
          content: `${aiContext()}\n\nConsulta lexicográfica del término o expresión: «${termQuery}».\nDevuelve, en este orden y con encabezados breves:\n1. Definición contextual.\n2. Etimología (si la conoces).\n3. Sinónimos.\n4. Uso en este pasaje.`,
        },
      ]);
      termResult = reply.content;
    } catch (cause) {
      termError = errorMessage(cause);
    } finally {
      termBusy = false;
    }
  }

  async function copyTermLookup() {
    const text = `${termQuery}\n\n${termResult}`;
    if (await copyText(text)) {
      termNotice = translate("reader.termCopied");
    } else {
      termNotice = null;
    }
  }

  async function saveTermAsNote() {
    if (!session || !termResult) return;
    try {
      await addNote({
        bookId: session.bookId,
        chapterIndex,
        offsetStart: null,
        offsetEnd: null,
        text: termQuery,
        title: `${translate("reader.termTitle")} · ${termQuery}`,
        contentJson: JSON.stringify({
          ops: [{ insert: `${termResult}\n` }],
        }),
        plainText: termResult,
      });
      termNotice = translate("reader.termSaved");
      annotationsRefresh += 1;
    } catch (cause) {
      termError = errorMessage(cause);
    }
  }

  async function runAiAnalysis(kind: "summary" | "terms" | "characters") {
    if (!session || isPdf || aiBusy) return;
    aiError = null;
    aiNotice = null;
    aiBusy = kind;
    try {
      if (!(await ensureAiReady())) return;
      const context = aiContext();
      if (!context) return;
      const task =
        kind === "summary"
          ? "Resume la escena en 3 o 4 frases. Capta la acción, el tono y la tensión principal. No repitas la tarea ni el título."
          : kind === "terms"
            ? "Extrae entre 4 y 6 términos, símbolos o referencias clave. Para cada uno, da una definición breve basada en el fragmento. No repitas la tarea."
            : "Enumera los personajes que aparecen o se mencionan. Para cada uno, indica en una línea su papel en la escena. No repitas la tarea.";
      const reply = await aiChat([
        { role: "system", content: aiSystemPrompt() },
        {
          role: "user",
          content: `${context}\n\nTarea: ${task}`,
        },
      ]);
      if (kind === "summary") aiSummary = reply.content;
      else if (kind === "terms") aiTerms = reply.content;
      else aiCharacters = reply.content;
    } catch (cause) {
      aiError = errorMessage(cause);
    } finally {
      aiBusy = null;
    }
  }

  async function sendAiMessage() {
    const text = aiChatInput.trim();
    if (!text || !session || isPdf || aiBusy === "chat") return;
    const previous = aiChatMessages.slice(-8);
    aiChatMessages = [...aiChatMessages, { role: "user", content: text }];
    aiChatInput = "";
    aiError = null;
    aiBusy = "chat";
    try {
      if (!(await ensureAiReady())) return;
      const reply = await aiChat([
        { role: "system", content: `${aiSystemPrompt()}\n\n${aiContext()}` },
        ...previous,
        { role: "user", content: text },
      ]);
      aiChatMessages = [
        ...aiChatMessages,
        { role: "assistant", content: reply.content },
      ];
      await tick();
      aiScrollEl?.scrollTo({ top: aiScrollEl.scrollHeight, behavior: "smooth" });
    } catch (cause) {
      aiError = errorMessage(cause);
    } finally {
      aiBusy = null;
    }
  }

  async function saveAiAnalysis() {
    if (!session || !aiSummary) return;
    try {
      await addNote({
        bookId: session.bookId,
        chapterIndex,
        offsetStart: null,
        offsetEnd: null,
        text: null,
        title: `${translate("reader.aiTitle")} · ${chapter?.title ?? chapterLabel(chapterIndex)}`,
        contentJson: JSON.stringify({
          ops: [{ insert: `${aiSummary}\n` }],
        }),
        plainText: aiSummary,
      });
      aiNotice = translate("reader.aiNoteSaved");
      annotationsRefresh += 1;
    } catch (cause) {
      aiError = errorMessage(cause);
    }
  }

  function openAiPanel() {
    aiOpen = !aiOpen;
    if (aiOpen) {
      settingsOpen = false;
      tocOpen = false;
      annotationsOpen = false;
      }
    if (aiOpen && !aiStatusState) void refreshAiStatus();
    if (aiOpen && aiStatusState && !aiStatusState.available) {
      void refreshAiStatus();
    }
  }

  function formatSleep(seconds: number): string {
    const safe = Math.max(0, seconds);
    return `${Math.floor(safe / 60)}:${String(safe % 60).padStart(2, "0")}`;
  }

  function startSleepTimer(minutes: number) {
    clearInterval(sleepTimer);
    sleepMinutes = minutes;
    sleepNotice = null;
    if (!minutes) {
      sleepRemaining = 0;
      return;
    }
    const end = Date.now() + minutes * 60_000;
    sleepRemaining = minutes * 60;
    sleepTimer = setInterval(() => {
      const left = Math.max(0, Math.ceil((end - Date.now()) / 1000));
      sleepRemaining = left;
      if (left <= 0) {
        clearInterval(sleepTimer);
        sleepMinutes = 0;
        sleepRemaining = 0;
        void stopTts();
        sleepNotice = translate("reader.sleepDone");
      }
    }, 1000);
  }

  async function toggleFullscreen() {
    try {
      const next = !isFullscreen;
      const fraction = chapterProgress;
      const index = chapterIndex;
      await getCurrentWindow().setFullscreen(next);
      isFullscreen = next;
      await tick();
      await measure();
      if (chapterIndex === index) scrollToFraction(fraction);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function addBookmark() {
    if (!session || !chapter) return;
    const offset = currentReadingOffset();
    try {
      const annotation = await addMarker({
        bookId: session.bookId,
        chapterIndex,
        offsetStart: offset,
        offsetEnd: offset,
        kind: "bookmark",
        color: colors[0],
        text: chapter.title,
        note: null,
      });
      annotations = [...annotations, annotation];
      annotationsRefresh += 1;
      extractNotice = translate("reader.bookmarkSaved");
      window.setTimeout(() => (extractNotice = null), 2400);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function copyText(value: string): Promise<boolean> {
    try {
      await navigator.clipboard.writeText(value);
      return true;
    } catch {
      const area = document.createElement("textarea");
      area.value = value;
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.appendChild(area);
      area.select();
      const copied = document.execCommand("copy");
      area.remove();
      return copied;
    }
  }

  async function copyQuote() {
    if (!session) return;
    const text = (panelAnchor?.text ?? selection?.text ?? "").trim();
    if (!text) return;
    const citation = [
      text,
      `— ${session.document.title}${session.document.author ? `, ${session.document.author}` : ""}`,
    ].join("\n\n");
    const copied = await copyText(citation);
    if (copied) {
      extractNotice = translate("reader.copiedQuote");
      window.setTimeout(() => (extractNotice = null), 2400);
    }
  }

  /**
   * Texto buscable con un mapa de posiciones normalizado → offset UTF-16 del
   * capítulo. El HTML de un capítulo contiene saltos de línea e indentación
   * que una búsqueda sobre el texto colapsado perdía; eso devolvía offsets más
   * pequeños y «Saltar aquí» llevaba a un punto anterior al real.
   */
  function searchableText(html: string): { text: string; offsets: number[] } {
    const doc = new DOMParser().parseFromString(html, "text/html");
    const source = doc.body.textContent ?? "";
    let text = "";
    const offsets: number[] = [];
    let previousWasSpace = true;

    for (let index = 0; index < source.length; ) {
      const char = String.fromCodePoint(source.codePointAt(index) ?? 0);
      if (/\s/u.test(char)) {
        if (!previousWasSpace) {
          text += " ";
          offsets.push(index);
          previousWasSpace = true;
        }
      } else {
        const lower = char.toLowerCase();
        text += lower;
        for (let unit = 0; unit < lower.length; unit += 1) {
          offsets.push(index);
        }
        previousWasSpace = false;
      }
      index += char.length;
    }

    if (text.endsWith(" ")) {
      text = text.slice(0, -1);
      offsets.pop();
    }
    return { text, offsets };
  }

  function normalizeQuery(query: string): string {
    return query.replace(/\s+/gu, " ").trim().toLowerCase();
  }

  async function searchInBook() {
    if (!session || isPdf || searchBusy) return;
    const query = searchQuery.trim();
    if (query.length < 2) {
      searchError = translate("reader.searchShort");
      return;
    }
    searchBusy = true;
    searchError = null;
    searchResults = [];
    searchProgress = 0;
    const needle = normalizeQuery(query);
    const bookId = session.bookId;
    const sections = session.document.sections;
    const activeChapter = chapter;
    const activeChapterIndex = chapterIndex;
    try {
      for (let index = 0; index < sections.length; index += 1) {
        if (disposed || searchResults.length >= 60) break;
        const content =
          index === activeChapterIndex && activeChapter
            ? activeChapter
            : await readerGetChapter(bookId, index);
        if (disposed) return;
        if (!needle) break;
        const raw = new DOMParser().parseFromString(content.html, "text/html")
          .body.textContent ?? "";
        const { text, offsets } = searchableText(content.html);
        let found = text.indexOf(needle);
        while (found >= 0 && searchResults.length < 60) {
          const start = offsets[found] ?? 0;
          const end = (offsets[found + needle.length - 1] ?? start) + 1;
          const snippetStart = Math.max(0, start - 80);
          const snippetEnd = Math.min(raw.length, end + 120);
          searchResults = [
            ...searchResults,
            {
              section: index,
              offset: start,
              snippet: `${snippetStart > 0 ? "…" : ""}${raw
                .slice(snippetStart, snippetEnd)
                .replace(/\s+/g, " ")
                .trim()}${snippetEnd < raw.length ? "…" : ""}`,
              title: sections[index]?.title ?? chapterLabel(index),
            },
          ];
          found = text.indexOf(needle, found + needle.length);
        }
        searchProgress = Math.round(((index + 1) / sections.length) * 100);
        if (index % 3 === 0) {
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
      }
      if (searchResults.length === 0) {
        searchError = translate("reader.searchNoResults");
      }
    } catch (cause) {
      searchError = errorMessage(cause);
    } finally {
      searchBusy = false;
      searchProgress = 0;
    }
  }

  function jumpToChapterPage() {
    if (String(pageJump).trim() === "") return;
    const target = Number(pageJump);
    if (!scrollerEl || !Number.isFinite(target)) return;
    const page = Math.max(1, Math.min(pageCount, Math.round(target)));
    scrollToPage(page);
    pageIndex = page;
    pageJump = "";
  }

  async function openSearchHit(hit: { section: number; offset: number }) {
    if (!session) return;
    tocOpen = false;
    if (hit.section === chapterIndex) {
      scrollToOffset(hit.offset);
      return;
    }
    await loadChapter(hit.section, hit.offset);
  }

  function setImageDim(value: boolean) {
    dimImages = value;
    localStorage.setItem("kora.reader.dimImages", value ? "1" : "0");
  }

  function setFocusLine(value: boolean) {
    focusLine = value;
    localStorage.setItem("kora.reader.focusLine", value ? "1" : "0");
    if (!value) focusVisible = false;
  }

  function trackFocusLine(event: MouseEvent) {
    if (!focusLine) return;
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    focusTop = event.clientY - 24;
    focusLeft = rect.left;
    focusWidth = rect.width;
    focusVisible = event.clientY > rect.top + 20;
  }

  function updatePageMetrics() {
    if (!scrollerEl || !chapter) {
      pageIndex = 1;
      pageCount = 1;
      return;
    }
    const paged = settings?.mode === "paged";
    const size = Math.max(
      1,
      paged ? scrollerEl.clientWidth : scrollerEl.clientHeight,
    );
    const total = paged ? scrollerEl.scrollWidth : scrollerEl.scrollHeight;
    const max = Math.max(0, total - size);
    // `ceil` mantiene la última página parcial; el desplazamiento real de esa
    // página se limita a `max` en scrollToPage().
    pageCount = Math.max(1, Math.ceil(total / size));
    if (max <= 0) {
      pageIndex = 1;
    } else {
      const position = paged ? scrollerEl.scrollLeft : scrollerEl.scrollTop;
      pageIndex = Math.min(
        pageCount,
        Math.max(
          1,
          Math.round((position / max) * (pageCount - 1)) + 1,
        ),
      );
    }
  }

  function scrollToPage(page: number) {
    if (!scrollerEl) return;
    const paged = settings?.mode === "paged";
    const size = Math.max(
      1,
      paged ? scrollerEl.clientWidth : scrollerEl.clientHeight,
    );
    const max = Math.max(
      0,
      (paged ? scrollerEl.scrollWidth : scrollerEl.scrollHeight) - size,
    );
    const clamped = Math.max(1, Math.min(pageCount, page));
    pageIndex = clamped;
    const position =
      clamped >= pageCount ? max : Math.min(max, (clamped - 1) * size);
    if (max > 0) {
      chapterProgress = Math.min(1, Math.max(0, position / max));
      bookProgress = sectionCount
        ? Math.min(1, (chapterIndex + chapterProgress) / sectionCount)
        : 0;
    }
    if (paged) {
      scrollerEl.scrollTo({ left: position, behavior: "smooth" });
    } else {
      scrollerEl.scrollTo({ top: position, behavior: "smooth" });
    }
  }

  /**
   * Lleva el scroll al punto exacto del texto. Usar `offset / charCount` como
   * fracción de píxeles no coincidía con la maquetación real (títulos,
   * imágenes, espacios), así que las búsquedas y anotaciones caían antes o
   * después del pasaje.
   */
  function scrollToOffset(offset: number) {
    if (!scrollerEl || !contentEl) return;
    if (offset <= 0) {
      scrollToFraction(0);
      return;
    }
    if (chapter && chapter.charCount > 0 && offset >= chapter.charCount) {
      scrollToFraction(1);
      return;
    }
    const point = pointAt(contentEl, Math.max(0, offset));
    if (!point) {
      scrollToFraction(
        chapter && chapter.charCount > 0 ? offset / chapter.charCount : 0,
      );
      return;
    }

    const range = document.createRange();
    range.setStart(point.node, point.offset);
    range.collapse(true);
    const rect = range.getBoundingClientRect();
    if (!rect || (rect.width === 0 && rect.height === 0)) {
      scrollToFraction(
        chapter && chapter.charCount > 0 ? offset / chapter.charCount : 0,
      );
      return;
    }

    const surface = scrollerEl.getBoundingClientRect();
    if (settings?.mode === "paged") {
      const size = Math.max(1, scrollerEl.clientWidth);
      const relative = rect.left - surface.left + scrollerEl.scrollLeft;
      const page = Math.min(pageCount, Math.max(1, Math.floor(relative / size) + 1));
      scrollToPage(page);
      return;
    }

    const max = Math.max(0, scrollerEl.scrollHeight - scrollerEl.clientHeight);
    const target =
      scrollerEl.scrollTop +
      (rect.top - surface.top) -
      scrollerEl.clientHeight * 0.25;
    scrollerEl.scrollTop = Math.min(max, Math.max(0, target));
  }

  function handlePdfProgress(page: number, percent: number) {
    pdfPage = page;
    chapterIndex = page;
    chapterProgress = percent;
    bookProgress = percent;
    visitedSections = new Set([...visitedSections, page]);
  }

  function handlePdfZoom(zoom: number) {
    if (!settings) return;
    settings.zoom = zoom;
    scheduleSettingsSave();
  }

  async function loadChapter(index: number, offset = 0) {
    if (!session) return;
    const request = ++chapterRequest;

    ttsPhrase = null;
    ttsRects = [];
    resetAiConversation();
    loading = true;
    error = null;
    try {
      const content = await readerGetChapter(session.bookId, index);
      if (disposed || request !== chapterRequest) return;
      chapter = content;
      chapterIndex = index;
      visitedSections = new Set([...visitedSections, index]);
      await tick();
      if (disposed || request !== chapterRequest) return;
      if (
        contentEl &&
        contentEl.textContent &&
        contentEl.textContent.length < content.charCount
      ) {
        console.warn(
          "Kora: faltan nodos de texto en el capítulo",
          contentEl.textContent.length,
          content.charCount,
        );
      }
      await measure();
      if (disposed || request !== chapterRequest) return;
      scrollToOffset(offset);
      updateProgressFromScroll();
      scheduleMarks();
      document.fonts?.ready.then(() => scheduleMarks());
    } catch (cause) {
      if (request === chapterRequest) error = errorMessage(cause);
    } finally {
      if (request === chapterRequest) loading = false;
    }
  }

  async function goToChapter(index: number, fromEnd = false) {
    if (!session || index < 0 || index >= sectionCount) return;
    if (index === chapterIndex) {
      tocOpen = false;
      return;
    }

    await persistProgress();
    selection = null;
    activeAnnotation = null;
    tocOpen = false;
    await loadChapter(index, fromEnd ? Number.MAX_SAFE_INTEGER : 0);
  }

  async function measure() {
    if (!scrollerEl) return;
    // El ancho útil es el del escenario, no el de la hoja: la hoja está
    // limitada por el ancho de columna y nunca alcanzaría el umbral.
    const availableWidth =
      scrollerEl.parentElement?.clientWidth ?? scrollerEl.clientWidth;
    const inset = Math.max(settings?.margin ?? 48, 28);
    const columnSpace = (availableWidth - 4 * inset) / 2;
    const nextSpread =
      settings?.mode === "paged" &&
      availableWidth >= 880 &&
      columnSpace >= 320;
    if (nextSpread !== spreadEnabled) {
      spreadEnabled = nextSpread;
      await tick();
    }
    updatePageMetrics();
  }

  function scrollToFraction(fraction: number) {
    if (!scrollerEl) return;

    const clamped = Math.min(1, Math.max(0, fraction));
    const paged = settings?.mode === "paged";
    const size = Math.max(
      1,
      paged ? scrollerEl.clientWidth : scrollerEl.clientHeight,
    );
    const total = paged ? scrollerEl.scrollWidth : scrollerEl.scrollHeight;
    const max = Math.max(0, total - size);

    if (paged) {
      const pages = Math.max(1, Math.ceil(total / size));
      if (pages <= 1 || max <= 0) {
        scrollerEl.scrollLeft = 0;
        return;
      }
      const page = Math.min(
        pages - 1,
        Math.max(0, Math.round(clamped * (pages - 1))),
      );
      scrollerEl.scrollLeft = page >= pages - 1 ? max : page * size;
    } else {
      scrollerEl.scrollTop = clamped * max;
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

    // Si el capítulo entero cabe en pantalla, se ha visto completo: evita
    // que el progreso del libro se quede en 0% y que «Siguiente» no detecte
    // el final en capítulos cortos.
    chapterProgress = max > 0 ? Math.min(1, Math.max(0, position / max)) : 1;
    bookProgress = sectionCount
      ? Math.min(1, (chapterIndex + chapterProgress) / sectionCount)
      : 0;
    updatePageMetrics();
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

    const offset = currentReadingOffset();
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
      updatePageMetrics();
      if (direction > 0 && pageIndex >= pageCount) {
        await goToChapter(chapterIndex + 1);
        return;
      }
      if (direction < 0 && pageIndex <= 1) {
        await goToChapter(chapterIndex - 1, true);
        return;
      }
      scrollToPage(pageIndex + direction);
      return;
    }

    const max = scrollerEl.scrollHeight - scrollerEl.clientHeight;
    const atEnd = scrollerEl.scrollTop >= max - 2;
    const atStart = scrollerEl.scrollTop <= 2;

    if (direction > 0 && atEnd) {
      await goToChapter(chapterIndex + 1);
      return;
    }
    if (direction < 0 && atStart) {
      await goToChapter(chapterIndex - 1, true);
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

    const panelOpen =
      tocOpen ||
      settingsOpen ||
      termPanelOpen ||
      practiceOpen ||
      activeAnnotation !== null ||
      characterPanelOpen ||
      voicePickerOpen ||
      mirrorOpen ||
      aiOpen ||
      annotationsOpen;
    if (panelOpen) {
      if (event.key === "Escape") closeReaderPanels();
      return;
    }

    if (event.key === "ArrowRight" || event.key === "PageDown") {
      event.preventDefault();
      void pageStep(1);
    } else if (event.key === "ArrowLeft" || event.key === "PageUp") {
      event.preventDefault();
      void pageStep(-1);
    } else if (event.key === "Escape") {
      closeReaderPanels();
    }
  }

  function onContentMouseUp() {
    window.setTimeout(captureSelection, 0);
  }

  function closeSelectionBar() {
    selection = null;
    window.getSelection()?.removeAllRanges();
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

    let start = offsetAt(contentEl, range.startContainer, range.startOffset);
    let end = offsetAt(contentEl, range.endContainer, range.endOffset);
    const rawText = current.toString();
    // `trim()` recorta el texto que se guarda; los offsets deben recortarse
    // igual para que la marca no empiece o acabe en un espacio invisible.
    const leading = rawText.length - rawText.trimStart().length;
    const trailing = rawText.length - rawText.trimEnd().length;
    start += leading;
    end -= trailing;
    const text = rawText.trim().slice(0, 2000);
    end = start + text.length;

    if (end <= start || !text) {
      selection = null;
      return;
    }

    const rect = range.getBoundingClientRect();
    panelAnchor = null;
    selection = {
      start,
      end,
      text,
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
    let boundary: { node: Text; offset: number } | null = null;

    while ((node = walker.nextNode())) {
      const text = node.textContent ?? "";
      if (target < total + text.length) {
        return {
          node: node as Text,
          offset: Math.max(0, Math.min(text.length, target - total)),
        };
      }
      if (target === total + text.length) {
        // Un offset justo en el límite entre dos nodos debe apuntar al inicio
        // del siguiente. Quedarse en el final del nodo anterior (a menudo un
        // salto de línea sin caja) hacía que «Ir» o la búsqueda no supieran
        // dónde colocar el scroll.
        boundary = { node: node as Text, offset: text.length };
      }
      total += text.length;
    }

    return boundary;
  }

  /**
   * Offset UTF-16 del punto de lectura actual (la línea que queda a un cuarto
   * de la altura visible, igual que usa `scrollToOffset`). Guardar
   * `chapterProgress * charCount` hacía que el progreso se desplazara al
   * reabrir el libro, porque los píxeles no equivalen a caracteres.
   */
  function readingOffset(fallback = 0): number {
    if (!scrollerEl || !contentEl || !chapter) return fallback;
    const surface = scrollerEl.getBoundingClientRect();
    const paged = settings?.mode === "paged";
    const x = surface.left + (paged ? surface.width * 0.25 : surface.width / 2);
    const y = surface.top + (paged ? 40 : surface.height * 0.25);
    const caretDocument = document as Document & {
      caretRangeFromPoint?: (x: number, y: number) => Range | null;
      caretPositionFromPoint?: (
        x: number,
        y: number,
      ) => { offsetNode: Node; offset: number } | null;
    };

    try {
      let node: Node | null = null;
      let offset = 0;
      if (typeof caretDocument.caretRangeFromPoint === "function") {
        const range = caretDocument.caretRangeFromPoint(x, y);
        node = range?.startContainer ?? null;
        offset = range?.startOffset ?? 0;
      } else if (typeof caretDocument.caretPositionFromPoint === "function") {
        const position = caretDocument.caretPositionFromPoint(x, y);
        node = position?.offsetNode ?? null;
        offset = position?.offset ?? 0;
      }
      if (!node || !contentEl.contains(node)) return fallback;
      const element =
        node.nodeType === Node.ELEMENT_NODE
          ? (node as Element)
          : node.parentElement;
      if (element?.closest(".annotation-layer, .tts-layer")) return fallback;
      return Math.min(
        chapter.charCount,
        Math.max(0, offsetAt(contentEl, node, offset)),
      );
    } catch {
      return fallback;
    }
  }

  function currentReadingOffset(): number {
    const fallback = Math.round(chapterProgress * (chapter?.charCount ?? 0));
    if (!scrollerEl || !chapter) return fallback;
    const paged = settings?.mode === "paged";
    const max = Math.max(
      0,
      (paged ? scrollerEl.scrollWidth : scrollerEl.scrollHeight) -
        (paged ? scrollerEl.clientWidth : scrollerEl.clientHeight),
    );
    const position = paged ? scrollerEl.scrollLeft : scrollerEl.scrollTop;
    if (max <= 0) return fallback;
    if (position <= 2) return 0;
    if (position >= max - 2) return chapter.charCount;
    return readingOffset(fallback);
  }

  function scheduleMarks() {
    requestAnimationFrame(() => recomputeMarks());
  }


  function textRects(
    offsetStart: number,
    offsetEnd: number,
    rootRect: DOMRect,
  ): MarkRect[] {
    if (!contentEl) return [];

    const start = pointAt(contentEl, offsetStart);
    const end = pointAt(contentEl, offsetEnd);
    if (!start || !end) return [];

    const range = document.createRange();
    range.setStart(start.node, start.offset);
    range.setEnd(end.node, end.offset);

    const parent = start.node.parentElement ?? contentEl;
    const fontSize = parseFloat(getComputedStyle(parent).fontSize) || 16;

    return Array.from(range.getClientRects())
      .filter((rect) => rect.width > 0 && rect.height > 0)
      .map((rect) => {
        // Los rects del rango incluyen el interlineado completo; el resaltado
        // se ajusta al em-box real del texto y se centra en la línea.
        const height = Math.min(rect.height, fontSize);
        return {
          left: rect.left - rootRect.left,
          top: rect.top - rootRect.top + (rect.height - height) / 2,
          width: rect.width,
          height,
        };
      });
  }

  /**
   * Punto de inicio de la línea que contiene el offset indicado. Se usa para
   * anclar las marcas de cita y emoción en el margen izquierdo de esa línea,
   * nunca encima del texto.
   */
  function lineStartLeft(offsetStart: number, rootRect: DOMRect): number | null {
    if (!contentEl) return null;
    const start = pointAt(contentEl, offsetStart);
    if (!start) return null;

    let block: HTMLElement | null = start.node.parentElement;
    while (block && block !== contentEl) {
      const display = getComputedStyle(block).display;
      if (
        display === "block" ||
        display === "list-item" ||
        display === "flex" ||
        display === "grid"
      ) {
        break;
      }
      block = block.parentElement;
    }
    const target = block ?? contentEl;
    const style = getComputedStyle(target);
    const box = target.getBoundingClientRect();
    const padding = parseFloat(style.paddingLeft) || 0;
    const border = parseFloat(style.borderLeftWidth) || 0;
    return box.left - rootRect.left + border + padding;
  }

  function recomputeMarks() {
    if (!contentEl) {
      marks = [];
      decorations = [];
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

      const rects = textRects(
        annotation.offsetStart,
        annotation.offsetEnd,
        rootRect,
      );

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

    const nextDecorations: DecorationMark[] = [];
    const decorationSlots = new Map<string, number>();
    const slotFor = (anchorLeft: number, rect: MarkRect) => {
      const key = `${Math.round(anchorLeft)}:${Math.round(rect.top + rect.height / 2)}`;
      const index = decorationSlots.get(key) ?? 0;
      decorationSlots.set(key, index + 1);
      return index * 24;
    };

    for (const extract of extracts) {
      if (
        extract.chapterIndex !== chapterIndex ||
        extract.offsetStart == null ||
        extract.offsetEnd == null
      ) {
        continue;
      }
      const rects = textRects(extract.offsetStart, extract.offsetEnd, rootRect);
      if (rects.length > 0) {
        const anchorLeft =
          lineStartLeft(extract.offsetStart, rootRect) ?? rects[0].left;
        nextDecorations.push({
          id: extract.id,
          kind: "extract",
          rects,
          title: extract.note ?? extract.text ?? "Cita",
          emoji: null,
          offset: slotFor(anchorLeft, rects[0]),
          anchorLeft,
        });
      }
    }

    for (const reaction of reactions) {
      if (
        reaction.chapterIndex !== chapterIndex ||
        reaction.offsetStart == null ||
        reaction.offsetEnd == null
      ) {
        continue;
      }
      const rects = textRects(reaction.offsetStart, reaction.offsetEnd, rootRect);
      if (rects.length > 0) {
        const meta = quickReactions.find((item) => item.kind === reaction.kind);
        const anchorLeft =
          lineStartLeft(reaction.offsetStart, rootRect) ?? rects[0].left;
        nextDecorations.push({
          id: reaction.id,
          kind: "reaction",
          rects,
          title: meta?.label ?? "Reacción",
          emoji: meta?.emoji ?? "💬",
          offset: slotFor(anchorLeft, rects[0]),
          anchorLeft,
        });
      }
    }

    marks = next;
    decorations = nextDecorations;
  }

  async function markSelection(kind: AnnotationKind, color: string) {
    if (!session) return;

    const anchor =
      panelAnchor ??
      (selection
        ? {
            section: chapterIndex,
            offsetStart: selection.start,
            offsetEnd: selection.end,
            text: selection.text,
          }
        : null);
    if (!anchor || !anchor.text.trim()) return;

    // Al marcar desde el panel de anotaciones conservamos el pasaje para que
    // «Extraer», «Reaccionar» o «Nueva nota» sigan disponibles. Si la marca
    // viene de la barra de selección, sí se limpia la selección.
    const keepPanelAnchor = panelAnchor !== null;

    try {
      const annotation = await addMarker({
        bookId: session.bookId,
        chapterIndex: anchor.section,
        offsetStart: anchor.offsetStart,
        offsetEnd: anchor.offsetEnd,
        kind,
        color,
        text: anchor.text,
        note: null,
      });

      annotations = [...annotations, annotation];
      annotationsRefresh += 1;
      selection = null;
      if (!keepPanelAnchor) panelAnchor = null;
      window.getSelection()?.removeAllRanges();
      await tick();
      scheduleMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function captureAnchor(): AnnotationAnchor {
    if (selection) {
      return {
        section: chapterIndex,
        offsetStart: selection.start,
        offsetEnd: selection.end,
        text: selection.text,
      };
    }
    return {
      section: chapterIndex,
      offsetStart: 0,
      offsetEnd: 0,
      text: "",
    };
  }

  function openAnnotations(
    tab: "marker" | "note" | "extract" | "reaction",
  ) {
    panelAnchor = captureAnchor();
    annotationsTab = tab;
    annotationsOpen = true;
    settingsOpen = false;
    aiOpen = false;
    tocOpen = false;
    termPanelOpen = false;
    mirrorOpen = false;
    characterPanelOpen = false;
    practiceOpen = false;
    activeAnnotation = null;
    annotationsRefresh += 1;
  }

  function closeAnnotations() {
    annotationsOpen = false;
    panelAnchor = null;
    selection = null;
    window.getSelection()?.removeAllRanges();
  }

  async function loadBookDecorations(bookId: string) {
    const entries = await listAnnotations(bookId, "all");
    const nextAnnotations: Annotation[] = [];
    const nextExtracts: Extract[] = [];
    const nextReactions: Reaction[] = [];

    for (const entry of entries) {
      if (entry.type === "marker") nextAnnotations.push(entry.data);
      else if (entry.type === "extract") nextExtracts.push(entry.data);
      else if (entry.type === "reaction") nextReactions.push(entry.data);
    }

    annotations = nextAnnotations;
    extracts = nextExtracts;
    reactions = nextReactions;
  }

  async function refreshMarkers() {
    if (!session) return;
    await loadBookDecorations(session.bookId);
    await tick();
    scheduleMarks();
  }

  async function jumpToAnnotation(section: number, offset: number) {
    if (!session) return;
    if (section === chapterIndex) {
      scrollToOffset(offset);
      return;
    }
    await loadChapter(section, offset);
  }

  function openAnnotation(annotation: Annotation) {
    activeAnnotation = annotation;
    annotationNote = annotation.note ?? "";
    annotationColor = annotation.color ?? colors[0];
    selection = null;
    annotationsOpen = false;
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
      annotationsRefresh += 1;
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
      annotationsRefresh += 1;
      await tick();
      scheduleMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function changeMode(mode: ReaderMode) {
    if (!settings) return;

    const fraction = chapterProgress;
    const index = chapterIndex;
    settings.mode = mode;
    await tick();
    await measure();
    if (chapterIndex !== index) return;
    scrollToFraction(fraction);
    scheduleMarks();
    scheduleSettingsSave();
  }

  function settingsChanged() {
    const fraction = chapterProgress;
    const index = chapterIndex;
    cancelAnimationFrame(settingsFrame);
    settingsFrame = requestAnimationFrame(() => {
      void measure().then(() => {
        if (!scrollerEl || disposed || chapterIndex !== index) return;
        scrollToFraction(fraction);
        updateProgressFromScroll();
        scheduleMarks();
      });
    });
    scheduleSettingsSave();
  }

  function scheduleSettingsSave() {
    settingsRevision += 1;
    clearTimeout(settingsTimer);
    settingsTimer = setTimeout(() => {
      void saveSettings();
    }, 500);
  }

  async function saveSettings() {
    if (!session || !settings) return;
    const revision = settingsRevision;

    try {
      const saved = await readerSaveSettings(session.bookId, {
        fontSize: settings.fontSize,
        lineHeight: settings.lineHeight,
        margin: settings.margin,
        fontFamily: settings.fontFamily,
        fontFace: settings.fontFace,
        columnWidth: settings.columnWidth,
        textAlign: settings.textAlign,
        paragraphIndent: settings.paragraphIndent,
        hyphenate: settings.hyphenate,
        theme: settings.theme,
        texture: settings.texture,
        mode: settings.mode,
        zoom: settings.zoom,
      });
      // Si el usuario siguió ajustando mientras se guardaba, no pisamos los
      // valores nuevos con la respuesta antigua.
      if (revision === settingsRevision) settings = saved;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function applySettingsToAll() {
    if (!settings) return;
    if (!window.confirm(translate("reader.applyToAllConfirm"))) return;
    try {
      const count = await readerApplyToAll({
        fontSize: settings.fontSize,
        lineHeight: settings.lineHeight,
        margin: settings.margin,
        fontFamily: settings.fontFamily,
        fontFace: settings.fontFace,
        columnWidth: settings.columnWidth,
        textAlign: settings.textAlign,
        paragraphIndent: settings.paragraphIndent,
        hyphenate: settings.hyphenate,
        theme: settings.theme,
        texture: settings.texture,
        mode: settings.mode,
        zoom: settings.zoom,
      });
      extractNotice = translate("reader.appliedToAll", { count });
      window.setTimeout(() => (extractNotice = null), 3000);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function chapterLabel(index: number): string {
    return (
      session?.document.sections[index]?.title ??
      translate("reader.chapterN", { number: index + 1 })
    );
  }

  async function loadTtsForBook(id: string) {
    try {
      const [preferences, voices] = await Promise.all([
        ttsPreferences(id),
        ttsVoiceList(),
      ]);
      ttsPreferencesState = preferences;
      ttsVoices = voices;
      ttsRate = preferences.rate;
      ttsPitch = preferences.pitch;
      const installed = voices.filter(
        (voice) =>
          voice.installed &&
          (voice.language === "es" || voice.language === "en"),
      );
      ttsVoiceId =
        preferences.voiceId &&
        installed.some((voice) => voice.id === preferences.voiceId)
          ? preferences.voiceId
          : installed.find((voice) => voice.language === ttsLanguage)?.id ??
            installed[0]?.id ??
            "";
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  async function initSyncEvents() {
    unlistenHandoff = await listen<SyncHandoffEvent>("sync::handoff", (event) => {
      if (isPdf) return;
      if (event.payload.handoff.bookId === session?.bookId) {
        handoff = event.payload;
      }
    });
    unlistenMirrorPosition = await listen<MirrorPositionEvent>(
      "mirror::position",
      async (event) => {
        if (isPdf || event.payload.book_id !== session?.bookId) return;
        await jumpToAnnotation(event.payload.chapter, event.payload.offset);
      },
    );
    unlistenMirrorTts = await listen<MirrorTtsEvent>(
      "mirror::tts_state",
      async (event) => {
        if (isPdf || event.payload.book_id !== session?.bookId) return;
        await applyMirrorTts(event.payload);
      },
    );
  }

  async function refreshMirror() {
    try {
      mirror = await getMirrorStatus();
    } catch {
      // El espejo es opcional; no bloquea el lector.
    }
  }

  async function openMirrorPanel() {
    mirrorOpen = !mirrorOpen;
    if (mirrorOpen) {
      settingsOpen = false;
      tocOpen = false;
      aiOpen = false;
      annotationsOpen = false;
      termPanelOpen = false;
      characterPanelOpen = false;
      practiceOpen = false;
      activeAnnotation = null;
      await refreshMirror();
      try {
        mirrorPeers = await getPeers();
      } catch {
        mirrorPeers = [];
      }
    }
  }

  async function toggleHostMirror() {
    if (!session) return;
    mirrorBusy = true;
    try {
      mirror = mirror?.hosting ? await stopMirror() : await startMirror(session.bookId);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      mirrorBusy = false;
    }
  }

  async function joinMirrorPeer(peer: PeerInfo) {
    if (!session) return;
    mirrorBusy = true;
    try {
      mirror = await joinMirror(peer.id, session.bookId);
      extractNotice = `Siguiendo el espejo de ${peer.name}.`;
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      mirrorBusy = false;
    }
  }

  async function leaveMirrorSession() {
    try {
      mirror = await leaveMirror();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function applyMirrorTts(event: MirrorTtsEvent) {
    if (!session || mirror?.hosting) return;
    try {
      if (event.state === "playing") {
        if (ttsState !== "playing") {
          excerptTts = event.end != null;
          ttsStatus = await ttsStart(session.bookId, {
            section: event.section,
            offset: event.offset,
            end: event.end ?? undefined,
          });
          void startReadingSession("listening");
        }
      } else if (event.state === "paused") {
        excerptTts = event.end != null;
        if (ttsState === "playing") ttsStatus = await ttsPause();
      } else if (ttsState === "playing" || ttsState === "paused") {
        excerptTts = false;
        ttsStatus = await ttsStop();
        ttsPhrase = null;
        ttsRects = [];
      }
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  async function resumeHandoff() {
    if (!session || !handoff || isPdf) return;
    const target = handoff.handoff;
    handoff = null;
    await loadChapter(target.chapter, target.offset);
  }

  function ensureTtsVoice(): boolean {
    if (installedVoices.length > 0) return true;
    ttsError = "Descarga una voz en el panel de voces para leer.";
    characterPanelOpen = true;
    return false;
  }

  async function startTts() {
    if (!session || isPdf) return;
    if (!ensureTtsVoice()) return;

    ttsError = null;
    excerptTts = false;
    const offset =
      ttsPhrase?.section === chapterIndex
        ? ttsPhrase.start
        : chapterProgress >= 0.999
          ? 0
          : currentReadingOffset();

    try {
      ttsStatus = await ttsStart(session.bookId, {
        section: chapterIndex,
        offset,
      });
      void startReadingSession("listening");
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  async function listenSelection() {
    if (!session || isPdf) return;
    const anchor = annotationAnchor;
    if (anchor.offsetEnd <= anchor.offsetStart || !anchor.text.trim()) return;
    if (!ensureTtsVoice()) return;

    ttsError = null;
    excerptTts = true;
    ttsPanelOpen = false;
    try {
      ttsStatus = await ttsStart(session.bookId, {
        section: anchor.section,
        offset: anchor.offsetStart,
        end: anchor.offsetEnd,
      });
      void startReadingSession("listening");
    } catch (cause) {
      excerptTts = false;
      ttsError = errorMessage(cause);
    }
  }

  async function toggleSelectionTts() {
    if (excerptTts && ttsState === "playing") {
      await stopTts();
      return;
    }
    if (excerptTts && ttsState === "paused") {
      await toggleTts();
      return;
    }
    await listenSelection();
  }

  async function toggleTts() {
    try {
      if (ttsState === "playing") {
        ttsStatus = await ttsPause();
      } else if (ttsState === "paused") {
        ttsStatus = await ttsResume();
      } else {
        await startTts();
      }
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  async function stopTts() {
    excerptTts = false;
    try {
      ttsStatus = await ttsStop();
    } catch {
      ttsStatus = null;
    }
    ttsPhrase = null;
    ttsRects = [];
    if (session) void startReadingSession("reading");
  }

  function setTtsRate(value: number) {
    ttsRate = value;
    clearTimeout(ttsSettingsTimer);
    ttsSettingsTimer = setTimeout(() => {
      void ttsSetRate(value).catch((cause) => {
        ttsError = errorMessage(cause);
      });
      void saveTtsPreferences();
    }, 220);
  }

  function setTtsVoice(voiceId: string) {
    ttsVoiceId = voiceId;
    void saveTtsPreferences();
  }

  function setTtsPitch(value: number) {
    ttsPitch = value;
    void ttsSetPitch(value).catch((cause) => {
      ttsError = errorMessage(cause);
    });
    void saveTtsPreferences();
  }

  function setTtsPauseScale(value: number) {
    if (!ttsPreferencesState) return;
    ttsPreferencesState = { ...ttsPreferencesState, pauseScale: value };
    void saveTtsPreferences();
  }

  function setTtsExpressive(value: boolean) {
    if (!ttsPreferencesState) return;
    ttsPreferencesState = { ...ttsPreferencesState, expressive: value };
    void saveTtsPreferences();
  }

  async function saveTtsPreferences() {
    if (!session || !ttsPreferencesState) return;
    try {
      ttsPreferencesState = await ttsSavePreferences(session.bookId, {
        voiceId: ttsVoiceId || null,
        rate: ttsRate,
        pitch: ttsPitch,
        pauseScale: ttsPreferencesState.pauseScale,
        expressive: ttsPreferencesState.expressive,
        multiVoice: ttsPreferencesState.multiVoice,
      });
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  function scheduleTtsRects(autoScroll = false) {
    cancelAnimationFrame(ttsFrame);
    ttsFrame = requestAnimationFrame(() => recomputeTtsRects(autoScroll));
  }

  function recomputeTtsRects(autoScroll = false) {
    if (
      !contentEl ||
      !ttsPhrase ||
      ttsPhrase.section !== chapterIndex ||
      ttsPhrase.end <= ttsPhrase.start
    ) {
      ttsRects = [];
      return;
    }

    const start = pointAt(contentEl, ttsPhrase.start);
    const end = pointAt(contentEl, ttsPhrase.end);
    if (!start || !end) {
      ttsRects = [];
      return;
    }

    const range = document.createRange();
    range.setStart(start.node, start.offset);
    range.setEnd(end.node, end.offset);

    const root = contentEl.getBoundingClientRect();
    const rects = Array.from(range.getClientRects())
      .filter((rect) => rect.width > 0 && rect.height > 0)
      .map((rect) => ({
        left: rect.left - root.left,
        top: rect.top - root.top,
        width: rect.width,
        height: rect.height,
      }));
    ttsRects = rects;

    if (autoScroll && rects.length > 0 && scrollerEl && settings?.mode !== "paged") {
      const bounding = range.getBoundingClientRect();
      const surface = scrollerEl.getBoundingClientRect();
      if (
        bounding.top < surface.top + 70 ||
        bounding.bottom > surface.bottom - 70
      ) {
        scrollerEl.scrollBy({
          top: bounding.top - surface.top - surface.height * 0.35,
          behavior: "smooth",
        });
      }
    }
  }

  async function advanceTts(section: number) {
    if (!session || isPdf) return;
    const next = section + 1;
    if (next >= sectionCount) return;
    await goToChapter(next);
    await tick();
    if (chapterIndex !== next) return;
    try {
      ttsStatus = await ttsStart(session.bookId, { section: next, offset: 0 });
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  function openVoicePicker() {
    voicePickerOpen = true;
    settingsOpen = false;
    tocOpen = false;
    aiOpen = false;
    annotationsOpen = false;
    termPanelOpen = false;
    mirrorOpen = false;
    characterPanelOpen = false;
    practiceOpen = false;
    activeAnnotation = null;
  }

  async function openCharacterPanel() {
    characterPanelOpen = true;
    settingsOpen = false;
    tocOpen = false;
    aiOpen = false;
    annotationsOpen = false;
    termPanelOpen = false;
    mirrorOpen = false;
    practiceOpen = false;
    activeAnnotation = null;
    if (!session) return;
    try {
      const [detected, assigned] = await Promise.all([
        ttsCharacters(session.bookId, chapterIndex),
        ttsCharacterVoices(session.bookId),
      ]);
      characters = detected.filter((item) => item.name !== "Narrador");
      characterVoices = Object.fromEntries(
        assigned.map((voice) => [voice.character, voice.voiceId]),
      );
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  async function downloadVoice(voice: VoiceDescriptor) {
    downloadingVoiceId = voice.id;
    ttsError = null;
    try {
      await ttsDownloadVoice(voice.id);
      ttsVoices = await ttsVoiceList();
      ttsVoiceId = voice.id;
      await saveTtsPreferences();
    } catch (cause) {
      ttsError = errorMessage(cause);
    } finally {
      downloadingVoiceId = null;
    }
  }

  async function assignCharacterVoice(character: string, voiceId: string) {    if (!session || !voiceId) return;
    try {
      await ttsAssignCharacterVoice(session.bookId, character, voiceId);
      characterVoices = { ...characterVoices, [character]: voiceId };
    } catch (cause) {
      ttsError = errorMessage(cause);
    }
  }

  async function openPracticePanel() {
    practiceOpen = true;
    settingsOpen = false;
    tocOpen = false;
    aiOpen = false;
    annotationsOpen = false;
    termPanelOpen = false;
    mirrorOpen = false;
    characterPanelOpen = false;
    activeAnnotation = null;
    practiceError = null;
    practiceResult = null;
    await loadWhisperModels();
    await loadPracticeHistory();
  }

  async function loadPracticeHistory() {
    if (!session) return;
    try {
      practiceHistoryEntries = await practiceHistory(session.bookId);
    } catch {
      practiceHistoryEntries = [];
    }
  }

  async function loadWhisperModels() {
    try {
      whisperModels = await practiceModelList();
      if (!whisperModels.some((model) => model.id === practiceModelId && model.installed)) {
        practiceModelId =
          whisperModels.find((model) => model.installed)?.id ?? practiceModelId;
      }
    } catch (cause) {
      practiceError = errorMessage(cause);
    }
  }

  async function startPractice() {
    if (!session || !selection) {
      practiceError = "Selecciona un pasaje en el texto para practicar.";
      return;
    }
    if (!whisperModels.some((model) => model.id === practiceModelId && model.installed)) {
      practiceError = "Descarga un modelo Whisper antes de grabar.";
      return;
    }

    practiceError = null;
    practiceResult = null;
    if (ttsState === "playing") {
      try {
        ttsStatus = await ttsPause();
      } catch {
        // La práctica puede continuar aunque falle la pausa del TTS.
      }
    }
    try {
      await practiceStart(
        session.bookId,
        chapterIndex,
        selection.start,
        selection.end,
        selection.text,
        ttsLanguage,
      );
      if (disposed || !practiceOpen) {
        void practiceCancel();
        return;
      }
      practiceRecording = true;
      practiceSeconds = 0;
      clearInterval(practiceTimer);
      practiceTimer = setInterval(() => {
        practiceSeconds += 1;
      }, 1000);
    } catch (cause) {
      practiceError = errorMessage(cause);
    }
  }

  async function stopPractice() {
    if (!practiceRecording) return;
    practiceRecording = false;
    clearInterval(practiceTimer);
    try {
      practiceResult = await practiceStop(practiceModelId);
      await loadPracticeHistory();
    } catch (cause) {
      practiceError = errorMessage(cause);
      void practiceCancel();
    }
  }

  async function cancelPractice() {
    practiceRecording = false;
    clearInterval(practiceTimer);
    try {
      await practiceCancel();
    } catch {
      /* la grabación ya había terminado */
    }
  }

  async function downloadWhisperModel() {
    practiceError = null;
    practiceDownload = 0;
    try {
      await practiceDownloadModel(practiceModelId);
      await loadWhisperModels();
    } catch (cause) {
      practiceDownload = null;
      practiceError = errorMessage(cause);
    }
  }

  function practiceClock(seconds: number): string {
    const minutes = Math.floor(seconds / 60);
    const rest = seconds % 60;
    return `${minutes}:${String(rest).padStart(2, "0")}`;
  }
</script>

<section class="reader">
  <header class="toolbar">
    <div class="toolbar-context">
      <div class="heading">
      {#if onExit}
        <button
          class="bar-back"
          aria-label={$t("nav.library")}
          title={$t("nav.library")}
          onclick={onExit}
        >
          <span class="bar-back-icon" aria-hidden="true">←</span>
          <span class="bar-back-label">{$t("nav.library")}</span>
        </button>
      {/if}
      <div class="heading-copy">
        <h1>{session?.document.title ?? $t("reader.title")}</h1>
        <p class="subtitle">
          {#if session}
            {session.document.author ?? $t("reader.unknownAuthor")}
            {#if chapter}
              · {chapter.title}
            {/if}
          {:else}
            {$t("reader.pickBook")}
          {/if}
        </p>
      </div>
    </div>

    {#if session}
      <div class="progress" aria-label={$t("reader.readingProgress")}>
        <div class="progress-meta">
          <span>{$t("reader.bookProgress", { percent: Math.round(bookProgress * 100) })}</span>
          {#if isPdf}
            <span>{$t("reader.pageOf", { page: pdfPage + 1, total: pdfPageCount })}</span>
          {:else}
            {#if settings?.mode === "paged"}
              <span>{$t("reader.pageOf", { page: pageIndex, total: pageCount })}</span>
            {:else}
              <span>{$t("reader.chapterProgress", { percent: Math.round(chapterProgress * 100) })}</span>
            {/if}
          {/if}
        </div>
        <span class="progress-track">
          <span class="progress-fill" style:width={`${bookProgress * 100}%`}></span>
        </span>
      </div>
      {/if}
    </div>

    {#if session}
      <div class="toolbar-actions">
        <div class="controls">
        {#if !isPdf}
          <button
            class="ghost"
            class:active={tocOpen && tocTab === "index"}
            aria-expanded={tocOpen && tocTab === "index"}
            aria-label={$t("reader.contents")}
            title={$t("reader.contents")}
            onclick={() => {
              if (tocOpen && tocTab === "index") {
                tocOpen = false;
                return;
              }
              tocTab = "index";
              tocOpen = true;
              settingsOpen = false;
              aiOpen = false;
              annotationsOpen = false;
            }}
          >
            <Icon name="list" size={17} />
            <span class="control-label">{$t("reader.contents")}</span>
          </button>
          <button
            class="ghost"
            class:active={tocOpen && tocTab === "search"}
            aria-expanded={tocOpen && tocTab === "search"}
            aria-label={$t("reader.searchBook")}
            title={$t("reader.searchBook")}
            onclick={() => {
              tocTab = "search";
              tocOpen = true;
                        settingsOpen = false;
              aiOpen = false;
              annotationsOpen = false;
            }}
          >
            <Icon name="search" size={17} />
            <span class="control-label">{$t("reader.searchBook")}</span>
          </button>
        {/if}
        <button
          class="ghost type-button"
          class:active={settingsOpen}
          aria-expanded={settingsOpen}
          aria-label={$t("reader.settings")}
          title={$t("reader.settings")}
          onclick={() => {
            settingsOpen = !settingsOpen;
            if (settingsOpen) {
              tocOpen = false;
              aiOpen = false;
              annotationsOpen = false;
                      }
          }}
        >
          Aa
        </button>
        {#if !isPdf}
          <button
            class="ghost"
            class:active={ttsPanelOpen || ttsActive}
            aria-expanded={ttsPanelOpen}
            aria-label={$t("reader.listen")}
            title={$t("reader.listen")}
            onclick={() => (ttsPanelOpen = !ttsPanelOpen)}
          >
            <Icon name="headphones" size={17} />
            <span class="control-label">{$t("reader.listen")}</span>
          </button>
          <button
            class="ghost"
            aria-label={$t("reader.addBookmark")}
            title={$t("reader.addBookmark")}
            onclick={addBookmark}
          >
            <Icon name="bookmark" size={17} />
          </button>
          <button
            class="ghost control-fullscreen"
            class:active={isFullscreen}
            aria-pressed={isFullscreen}
            aria-label={$t("reader.fullscreen")}
            title={$t("reader.fullscreen")}
            onclick={toggleFullscreen}
          >
            <Icon name="expand" size={17} />
          </button>
          <button
            class="ghost"
            class:active={aiOpen}
            aria-expanded={aiOpen}
            aria-label={$t("reader.ai")}
            title={$t("reader.ai")}
            onclick={openAiPanel}
          >
            <Icon name="sparkles" size={17} />
            <span class="control-label">{$t("reader.ai")}</span>
          </button>
          <button
            class="ghost"
            class:active={annotationsOpen}
            aria-expanded={annotationsOpen}
            aria-label={$t("reader.annotations")}
            title={$t("reader.annotations")}
            onclick={() => openAnnotations("marker")}
          >
            <Icon name="diary" size={17} />
            <span class="control-label">{$t("reader.annotations")}</span>
          </button>
          <button
            class="ghost control-metrics"
            aria-label={$t("reader.metrics")}
            title={$t("reader.metrics")}
            onclick={openPracticePanel}
          >
            <Icon name="chart" size={17} />
            <span class="control-label">{$t("reader.metrics")}</span>
          </button>
          <button
            class="ghost control-mirror"
            class:active={mirrorOpen}
            aria-expanded={mirrorOpen}
            aria-label={$t("reader.mirror")}
            title={$t("reader.mirror")}
            onclick={openMirrorPanel}
          >
            <Icon name="sync" size={17} />
            <span class="control-label">{$t("reader.mirror")}</span>
          </button>
        {/if}
        </div>
      </div>
    {/if}
  </header>

  {#if session && !isPdf && selection}
    <div
      class="selection-bar"
      role="region"
      aria-label={$t("reader.selectionActions")}
    >
      <span class="selection-bar-quote" title={selection.text}>
        {selection.text}
      </span>
      <div class="selection-bar-colors">
        <span class="selection-bar-label">{$t("reader.highlight")}</span>
        {#each colors as color (color)}
          <button
            class="selection-color"
            style:background={color}
            aria-label={$t("reader.highlightWith", { color })}
            title={$t("reader.highlightWith", { color })}
            onclick={() => {
              annotationColor = color;
              void markSelection("highlight", color);
            }}
          ></button>
        {/each}
      </div>
      <button class="ghost small" onclick={() => openAnnotations("note")}>
        <Icon name="edit" size={14} />
        {$t("reader.note")}
      </button>
      <button
        class="ghost small"
        onclick={() => void markSelection("underline", annotationColor)}
      >
        {$t("reader.underline")}
      </button>
      <button class="ghost small" onclick={toggleSelectionTts}>
        <Icon name="headphones" size={14} />
        {#if excerptTts && ttsState === "playing"}
          {$t("reader.stop")}
        {:else if excerptTts && ttsState === "paused"}
          {$t("reader.resume")}
        {:else}
          {$t("reader.listenSelection")}
        {/if}
      </button>
      <button class="ghost small" onclick={copyQuote}>
        {$t("reader.copyQuote")}
      </button>
      <button class="ghost small" onclick={openTermLookup}>
        {$t("reader.termAction")}
      </button>
      <button
        class="selection-bar-close"
        aria-label={$t("common.close")}
        title={$t("common.close")}
        onclick={closeSelectionBar}
      >
        <Icon name="close" size={14} />
      </button>
    </div>
  {/if}

  {#if session}
    <div class="mobile-reading-status">
      <span class="mobile-reading-copy">
        <Icon name="sparkles" size={13} />
        <span>
          {session.document.title}{#if chapter} · {chapter.title}{/if}
        </span>
      </span>
      <span class="mobile-reading-score">
        <strong>{$t("reader.bookProgress", { percent: Math.round(bookProgress * 100) })}</strong>
        <button
          class="mobile-reading-metrics"
          aria-label={$t("reader.metrics")}
          title={$t("reader.metrics")}
          onclick={openPracticePanel}
        >
          <Icon name="chart" size={13} />
        </button>
      </span>
      <span class="mobile-reading-track" aria-hidden="true">
        <span style:width={`${bookProgress * 100}%`}></span>
      </span>
    </div>
  {/if}

  {#if handoff}
    <div class="handoff" role="status">
      <span>
        {$t("reader.handoffAt", {
          peer: handoff.peerName,
          chapter: handoff.handoff.chapter + 1,
          offset: handoff.handoff.offset,
        })}
      </span>
      <button class="primary small" onclick={resumeHandoff}>{$t("common.resume")}</button>
      <button class="ghost small" onclick={() => (handoff = null)}>{$t("common.discard")}</button>
    </div>
  {/if}

  {#if session && !isPdf && ttsPanelOpen}
    <div class="tts-bar">
      <header class="tts-head">
        <span class="tts-heading">
          <span class="tts-head-icon"><Icon name="headphones" size={16} /></span>
          <strong>{$t("reader.listen")}</strong>
        </span>
        <button
          class="tts-close"
          aria-label={$t("common.close")}
          title={$t("common.close")}
          onclick={() => (ttsPanelOpen = false)}
        >
          <Icon name="close" size={16} />
        </button>
      </header>

      <div class="tts-main">
        <button class="primary tts-play" onclick={toggleTts}>
          <Icon
            name={ttsState === "playing" ? "pause" : "play"}
            size={16}
          />
          {ttsState === "playing"
            ? $t("reader.pause")
            : ttsState === "paused"
              ? $t("reader.resume")
              : $t("reader.listen")}
        </button>
        {#if ttsActive}
          <button class="ghost tts-stop" onclick={stopTts}>
            <Icon name="stop" size={14} />
            {$t("reader.stop")}
          </button>
        {/if}
        {#if excerptTts}
          <span class="tts-excerpt">{$t("reader.excerptLabel")}</span>
        {/if}
      </div>

      <div class="tts-sliders">
        <label class="tts-control" title={$t("reader.readingSpeed")}>
          <span class="tts-control-head">
            <span>{$t("reader.speed")}</span>
            <strong>{ttsRate.toFixed(1)}×</strong>
          </span>
          <input
            type="range"
            min="0.5"
            max="3"
            step="0.1"
            value={ttsRate}
            oninput={(event) => setTtsRate(Number(event.currentTarget.value))}
            aria-label={$t("reader.readingSpeed")}
          />
        </label>
        <label class="tts-control" title={$t("reader.voiceTone")}>
          <span class="tts-control-head">
            <span>{$t("reader.tone")}</span>
            <strong>{ttsPitch.toFixed(2)}</strong>
          </span>
          <input
            type="range"
            min="0.8"
            max="1.2"
            step="0.02"
            value={ttsPitch}
            oninput={(event) => setTtsPitch(Number(event.currentTarget.value))}
            aria-label={$t("reader.voiceTone")}
          />
        </label>
      </div>

      <div class="tts-voice-row">
        <button
          class="tts-voice"
          onclick={openVoicePicker}
          aria-label={$t("reader.readingVoice")}
        >
          {#if currentTtsVoice}
            <span class="tts-voice-lang">{currentTtsVoice.language.toUpperCase()}</span>
            <span class="tts-voice-label">
              {currentTtsVoice.name}{currentTtsVoice.cloned
                ? ` · ${$t("reader.cloned")}`
                : ""}
            </span>
          {:else}
            <span class="tts-voice-label">{$t("reader.noVoices")}</span>
          {/if}
          <Icon name="chevron" size={13} />
        </button>
        <div class="tts-circle-action">
          <button
            class="tts-circle"
            aria-label={$t("reader.voices")}
            title={$t("reader.voices")}
            onclick={openCharacterPanel}
          >
            <Icon name="account" size={19} />
          </button>
          <span>{$t("reader.voices")}</span>
        </div>
        <div class="tts-circle-action">
          <button
            class="tts-circle tts-circle-accent"
            aria-label={$t("reader.practice")}
            title={$t("reader.practice")}
            onclick={openPracticePanel}
          >
            <Icon name="mic" size={19} />
          </button>
          <span>{$t("reader.practiceShort")}</span>
        </div>
      </div>

      <div class="tts-footer">
        <label class="tts-auto">
          <input type="checkbox" bind:checked={autoAdvance} />
          {$t("reader.autoAdvance")}
        </label>
        <label class="tts-sleep" title={$t("reader.sleepTimer")}>
          <Icon name="clock" size={14} />
          <select
            value={sleepMinutes}
            onchange={(event) => startSleepTimer(Number(event.currentTarget.value))}
            aria-label={$t("reader.sleepTimer")}
          >
            <option value={0}>{$t("reader.sleepOff")}</option>
            <option value={15}>{$t("reader.sleep15")}</option>
            <option value={30}>{$t("reader.sleep30")}</option>
            <option value={60}>{$t("reader.sleep60")}</option>
          </select>
          {#if sleepRemaining > 0}
            <strong>{formatSleep(sleepRemaining)}</strong>
          {/if}
        </label>
      </div>

      {#if sleepNotice}
        <span class="sleep-notice">{$t("reader.sleepDone")}</span>
      {/if}
      {#if ttsError}
        <span class="tts-error" role="alert">{ttsError}</span>
      {/if}
    </div>
    {#if installedVoices.find((voice) => voice.id === ttsVoiceId)?.cloned}
      <p class="tts-watermark">{$t("reader.clonedWatermark")}</p>
    {/if}
  {/if}

  {#if aiOpen && session && !isPdf}
    <aside class="panel ai-panel" aria-label={$t("reader.aiTitle")}>
      <header class="ai-panel-head">
        <div class="ai-title">
          <span class="ai-mark" aria-hidden="true"><Icon name="sparkles" size={17} /></span>
          <div>
            <h2>{$t("reader.aiTitle")}</h2>
            <p>{$t("reader.aiSubtitle")}</p>
          </div>
        </div>
        <button class="ghost small" aria-label={$t("common.close")} onclick={() => (aiOpen = false)}>
          <Icon name="close" size={16} />
        </button>
      </header>

      <div class="ai-tabs" role="tablist">
        <button
          class:active={aiTab === "analysis"}
          role="tab"
          aria-selected={aiTab === "analysis"}
          onclick={() => (aiTab = "analysis")}
        >
          {$t("reader.aiAnalysis")}
        </button>
        <button
          class:active={aiTab === "chat"}
          role="tab"
          aria-selected={aiTab === "chat"}
          onclick={() => (aiTab = "chat")}
        >
          {$t("reader.aiChat")}
        </button>
      </div>

      {#if !aiStatusState}
        <p class="ai-thinking">{$t("reader.aiChecking")}</p>
      {:else if !aiStatusState.available}
        <div class="ai-offline">
          <span class="ai-offline-icon"><Icon name="sparkles" size={22} /></span>
          <strong>{$t("reader.aiOfflineTitle")}</strong>
          <p>{$t("reader.aiOfflineHelp")}</p>
          {#if onSettings}
            <button class="primary small" onclick={onSettings}>
              {$t("reader.aiOpenSettings")}
            </button>
          {/if}
          <button class="ghost small" onclick={refreshAiStatus}>
            {$t("common.retry")}
          </button>
        </div>
      {:else}
        <div class="ai-meta">
          <span>
            {aiStatusState.provider === "local"
              ? $t("settings.aiLocal")
              : aiStatusState.provider === "ollama"
                ? "Ollama"
                : "llama.cpp"}
          </span>
          <code>{aiStatusState.model}</code>
          <span>{$t("reader.aiLocalBadge")}</span>
        </div>

        {#if aiTab === "analysis"}
          <div class="ai-tools">
            <button
              class="ai-tool"
              disabled={aiBusy !== null}
              onclick={() => runAiAnalysis("summary")}
            >
              <span><Icon name="book" size={16} /></span>
              <strong>{$t("reader.aiSummary")}</strong>
              <small>{$t("reader.aiSummaryHint")}</small>
            </button>
            <button
              class="ai-tool"
              disabled={aiBusy !== null}
              onclick={() => runAiAnalysis("terms")}
            >
              <span><Icon name="search" size={16} /></span>
              <strong>{$t("reader.aiTerms")}</strong>
              <small>{$t("reader.aiTermsHint")}</small>
            </button>
            <button
              class="ai-tool"
              disabled={aiBusy !== null}
              onclick={() => runAiAnalysis("characters")}
            >
              <span><Icon name="account" size={16} /></span>
              <strong>{$t("reader.aiCharacters")}</strong>
              <small>{$t("reader.aiCharactersHint")}</small>
            </button>
          </div>

          {#if aiError}
            <p class="ai-error" role="alert">{aiError}</p>
          {/if}
          {#if aiBusy}
            <p class="ai-thinking">
              <span class="ai-dot" aria-hidden="true"></span>
              {$t("reader.aiThinking")}
            </p>
          {/if}

          {#if aiSummary}
            <article class="ai-result">
              <header>
                <strong>{$t("reader.aiSummary")}</strong>
                <button class="ai-save" onclick={saveAiAnalysis}>
                  <Icon name="plus" size={14} /> {$t("reader.aiSaveNote")}
                </button>
              </header>
              <p>{aiSummary}</p>
            </article>
          {/if}
          {#if aiTerms}
            <article class="ai-result">
              <header><strong>{$t("reader.aiTerms")}</strong></header>
              <p>{aiTerms}</p>
            </article>
          {/if}
          {#if aiCharacters}
            <article class="ai-result">
              <header><strong>{$t("reader.aiCharacters")}</strong></header>
              <p>{aiCharacters}</p>
            </article>
          {/if}
          {#if aiNotice}
            <p class="ai-notice" role="status">{$t("reader.aiNoteSaved")}</p>
          {/if}
          {#if !aiSummary && !aiTerms && !aiCharacters && !aiBusy}
            <p class="ai-hint">{$t("reader.aiAnalysisHelp")}</p>
          {/if}
        {:else}
          <div class="ai-chat" bind:this={aiScrollEl}>
            {#if aiChatMessages.length === 0}
              <div class="ai-chat-empty">
                <span><Icon name="sparkles" size={22} /></span>
                <p>{$t("reader.aiChatHelp")}</p>
              </div>
            {/if}
            {#each aiChatMessages as message, index (index)}
              <div class="ai-bubble {message.role}">
                <span>{message.role === "user" ? $t("reader.aiYou") : $t("reader.aiAssistant")}</span>
                <p>{message.content}</p>
              </div>
            {/each}
            {#if aiBusy === "chat"}
              <p class="ai-thinking">
                <span class="ai-dot" aria-hidden="true"></span>
                {$t("reader.aiThinking")}
              </p>
            {/if}
          </div>
          {#if aiError}
            <p class="ai-error" role="alert">{aiError}</p>
          {/if}
          <div class="ai-composer">
            <textarea
              bind:value={aiChatInput}
              rows="2"
              placeholder={$t("reader.aiPlaceholder")}
              aria-label={$t("reader.aiPlaceholder")}
              onkeydown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void sendAiMessage();
                }
              }}
            ></textarea>
            <button
              class="primary small"
              disabled={!aiChatInput.trim() || aiBusy === "chat"}
              onclick={sendAiMessage}
              aria-label={$t("reader.aiSend")}
            >
              <Icon name="chevron" size={16} />
            </button>
          </div>
          {#if aiChatMessages.length > 0}
            <button
              class="ai-clear"
              onclick={() => {
                aiChatMessages = [];
                aiError = null;
              }}
            >
              {$t("reader.aiClear")}
            </button>
          {/if}
        {/if}
      {/if}
    </aside>
  {/if}

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {/if}

  {#if !session}
    <div class="picker">
      <div class="picker-head">
        <span class="picker-icon"><Icon name="book" size={22} /></span>
        <div>
          <h2>{$t("library.open")}</h2>
          <p class="hint">{$t("reader.pickerHelp")}</p>
        </div>
      </div>
      <div class="picker-row">
        <select bind:value={selectedBookId} aria-label={$t("reader.bookToRead")}>
          <option value="">{$t("reader.selectBook")}</option>
          {#each books as book (book.id)}
            <option value={book.id}>{book.title} · {book.format.toUpperCase()}</option>
          {/each}
        </select>
        <button
          class="primary"
          onclick={openSelected}
          disabled={!selectedBookId || opening}
        >
          {opening ? $t("reader.opening") : $t("reader.read")}
        </button>
      </div>
      {#if books.length === 0}
        <p class="hint">{$t("reader.noBooks")}</p>
      {:else}
        <div class="recent">
          <p class="eyebrow">{$t("reader.recent")}</p>
          <div class="recent-row">
            {#each books.slice(0, 5) as book (book.id)}
              <button class="recent-book" onclick={() => openBook(book.id)}>
                <span class="recent-cover">
                  {#if coverSrc(book)}
                    <img src={coverSrc(book)} alt="" loading="lazy" />
                  {:else}
                    <span class="recent-placeholder">{book.title.slice(0, 1)}</span>
                  {/if}
                  {#if book.progress > 0}
                    <span class="recent-progress" aria-hidden="true">
                      <span style:width={`${Math.round(book.progress * 100)}%`}></span>
                    </span>
                  {/if}
                </span>
                <span class="recent-title">{book.title}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {:else if isPdf && settings}
    {#key session.bookId}
      <PdfView
        {session}
        {settings}
        onprogress={handlePdfProgress}
        onzoom={handlePdfZoom}
      />
    {/key}
  {:else}
    <div class="reader-stage">
      <span class="book-spine" class:visible={spreadEnabled} aria-hidden="true"></span>
      <div
        class="surface {themeClass}"
        style={surfaceStyle}
        data-texture={settings ? surfaceTexture : undefined}
        role="region"
        aria-label={$t("reader.title")}
        bind:this={scrollerEl}
        class:paged={settings?.mode === "paged"}
        class:spread={spreadEnabled}
        class:dim-images={dimImages}
        onscroll={onScroll}
        onmousemove={trackFocusLine}
        onmouseleave={() => (focusVisible = false)}
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
                      aria-label={$t("reader.openAnnotation")}
                    ></button>
                  {/each}
                {/each}

                {#each decorations as mark (mark.id)}
                  {@const rect = mark.rects[0]}
                  <button
                    class="text-mark kind-{mark.kind}"
                    style:left={`${mark.anchorLeft - 13 - mark.offset}px`}
                    style:top={`${rect.top + rect.height / 2}px`}
                    onclick={() => openAnnotations(mark.kind)}
                    title={mark.title}
                    aria-label={mark.title}
                  >
                    {#if mark.kind === "extract"}
                      <Icon name="bookmark" size={12} />
                    {:else}
                      <span aria-hidden="true">{mark.emoji}</span>
                    {/if}
                  </button>
                {/each}
              </div>

              <div class="tts-layer" aria-hidden="true">
                {#each ttsRects as rect, index (index)}
                  <span
                    class="tts-phrase"
                    style:left={`${rect.left}px`}
                    style:top={`${rect.top}px`}
                    style:width={`${rect.width}px`}
                    style:height={`${rect.height}px`}
                  ></span>
                {/each}
              </div>
            </article>
          {/key}
        {:else if loading}
          <p class="loading">{$t("reader.loadingChapter")}</p>
        {/if}
      </div>
    </div>

    {#if focusLine && focusVisible}
      <div
        class="focus-ruler"
        style:top={`${focusTop}px`}
        style:left={`${focusLeft}px`}
        style:width={`${focusWidth}px`}
        aria-hidden="true"
      ></div>
    {/if}

    <nav
      class="reader-pager"
      class:spread={spreadEnabled}
      aria-label={$t("reader.pageNavigation")}
    >
      <button
        class="pager-step chapter-step"
        onclick={() => goToChapter(chapterIndex - 1, true)}
        disabled={chapterIndex <= 0 || loading}
        title={$t("reader.previousChapter")}
        aria-label={$t("reader.previousChapter")}
      >
        <span class="step-icon" aria-hidden="true">«</span>
        <span class="step-label">{$t("reader.chapter")}</span>
      </button>
      <button
        class="pager-step page-step"
        onclick={() => pageStep(-1)}
        disabled={loading || atBookStart}
        title={$t("reader.previousPage")}
        aria-label={$t("reader.previousPage")}
      >
        <span class="step-icon" aria-hidden="true">‹</span>
        <span class="step-label">{$t("reader.previousShort")}</span>
      </button>

      <div class="pager-status">
        <strong>{chapterIndex + 1} / {sectionCount}</strong>
        <span class="pager-title">{chapterLabel(chapterIndex)}</span>
        <span class="pager-track" aria-hidden="true">
          <span style:width={`${chapterProgress * 100}%`}></span>
        </span>
      </div>

      <button
        class="pager-step page-step primary"
        onclick={() => pageStep(1)}
        disabled={loading || atBookEnd}
        title={$t("reader.nextPage")}
        aria-label={$t("reader.nextPage")}
      >
        <span class="step-label">{$t("reader.nextShort")}</span>
        <span class="step-icon" aria-hidden="true">›</span>
      </button>
      <button
        class="pager-step chapter-step"
        onclick={() => goToChapter(chapterIndex + 1)}
        disabled={chapterIndex >= sectionCount - 1 || loading}
        title={$t("reader.nextChapter")}
        aria-label={$t("reader.nextChapter")}
      >
        <span class="step-label">{$t("reader.chapter")}</span>
        <span class="step-icon" aria-hidden="true">»</span>
      </button>
    </nav>
  {/if}

  {#if extractNotice}
    <p class="extract-notice" role="status">
      {extractNotice}
      <button class="ghost small" onclick={() => (extractNotice = null)}>{$t("common.close")}</button>
    </p>
  {/if}

  {#if tocOpen || settingsOpen || termPanelOpen || practiceOpen || activeAnnotation || characterPanelOpen || voicePickerOpen || mirrorOpen || aiOpen || annotationsOpen}
    <button
      class="panel-backdrop"
      aria-label={$t("common.close")}
      onclick={closeReaderPanels}
    ></button>
  {/if}

  {#if termPanelOpen && session}
    <aside class="panel term-panel" aria-label={$t("reader.termTitle")}>
      <header class="modal-head">
        <div class="modal-heading">
          <span class="modal-mark"><Icon name="search" size={18} /></span>
          <div>
            <h2>{$t("reader.termTitle")}</h2>
            <p>{$t("reader.termSubtitle")}</p>
          </div>
        </div>
        <button
          class="modal-close"
          aria-label={$t("common.close")}
          onclick={() => (termPanelOpen = false)}
        >
          <Icon name="close" size={17} />
        </button>
      </header>

      <div class="term-query">
        <strong>{termQuery}</strong>
        <span>{chapter?.title ?? chapterLabel(chapterIndex)}</span>
      </div>

      <div class="modal-tabs" role="tablist">
        <button
          class:active={termTab === "definition"}
          role="tab"
          aria-selected={termTab === "definition"}
          onclick={() => (termTab = "definition")}
        >
          {$t("reader.termDefinition")}
        </button>
        <button
          class:active={termTab === "context"}
          role="tab"
          aria-selected={termTab === "context"}
          onclick={() => (termTab = "context")}
        >
          {$t("reader.termContext")}
        </button>
      </div>

      <div class="modal-body">
        {#if termTab === "context"}
          <p class="term-context">{termContext}</p>
        {:else if termBusy}
          <p class="empty">{$t("reader.termSearching")}</p>
        {:else if termError}
          <p class="error" role="alert">{termError}</p>
          <button class="ghost small" onclick={lookupTerm}>{$t("common.retry")}</button>
        {:else if termResult}
          <p class="term-result">{termResult}</p>
        {:else}
          <p class="empty">{$t("reader.termEmpty")}</p>
        {/if}
        {#if termNotice}
          <p class="term-notice" role="status">{termNotice}</p>
        {/if}
      </div>

      <footer class="modal-foot term-foot">
        <button class="ghost small" onclick={copyTermLookup} disabled={!termResult}>
          {$t("reader.termCopy")}
        </button>
        <button class="primary small" onclick={saveTermAsNote} disabled={!termResult}>
          {$t("reader.termSave")}
        </button>
      </footer>
    </aside>
  {/if}

  {#if session && !isPdf}
    <AnnotationPanel
      bookId={session.bookId}
      bookTitle={session.document.title}
      format="epub"
      {sectionCount}
      sections={annotationSections}
      currentSection={chapterIndex}
      {annotations}
      anchor={annotationAnchor}
      open={annotationsOpen}
      initialTab={annotationsTab}
      refreshToken={annotationsRefresh}
      recordToken={voiceRecordToken}
      onClose={closeAnnotations}
      onJump={jumpToAnnotation}
      onRefreshMarkers={refreshMarkers}
      onAddMarker={addBookmark}
      onMarkSelection={markSelection}
      onCopyQuote={copyQuote}
      onLookup={openTermLookup}
      onListenSelection={listenSelection}
      onError={(message) => (error = message)}
    />
  {/if}

  {#if mirrorOpen && session && !isPdf}
    <aside class="panel mirror-panel">
      <header>
        <h2>{$t("reader.mirror")}</h2>
        <button class="ghost small" onclick={() => (mirrorOpen = false)}>{$t("common.close")}</button>
      </header>
      <p class="hint">
        {mirror?.active
          ? mirror.hosting
            ? $t("reader.mirrorSharing", { count: mirror.followers.length })
            : $t("reader.mirrorFollowing")
          : $t("reader.mirrorIdle")}
      </p>
      <div class="editor-actions">
        <button class="primary small" onclick={toggleHostMirror} disabled={mirrorBusy}>
          {mirror?.hosting ? $t("reader.stopMirror") : $t("reader.shareBook")}
        </button>
        {#if mirror?.active && !mirror.hosting}
          <button class="ghost small" onclick={leaveMirrorSession}>{$t("reader.leaveMirror")}</button>
        {/if}
      </div>
      <h3>{$t("reader.devices")}</h3>
      {#if mirrorPeers.length === 0}
        <p class="hint">{$t("reader.noPeers")}</p>
      {:else}
        <div class="mirror-peers">
          {#each mirrorPeers as peer (peer.id)}
            <div class="mirror-peer">
              <span>{peer.name}</span>
              {#if peer.trusted}
                <button
                  class="ghost small"
                  onclick={() => joinMirrorPeer(peer)}
                  disabled={mirrorBusy}
                >
                  {$t("reader.join")}
                </button>
              {:else}
                <span class="hint">{$t("reader.unpaired")}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </aside>
  {/if}

  {#if tocOpen && session && !isPdf}
    <aside class="panel navigation-panel" aria-label={$t("reader.indexTitle")}>
      <header class="modal-head">
        <div class="modal-heading">
          <span class="modal-mark"><Icon name="book" size={18} /></span>
          <div>
            <h2>{$t("reader.indexTitle")}</h2>
            <p>{$t("reader.indexSubtitle")}</p>
          </div>
        </div>
        <button
          class="modal-close"
          aria-label={$t("common.close")}
          onclick={() => (tocOpen = false)}
        >
          <Icon name="close" size={17} />
        </button>
      </header>

      <div class="modal-tabs" role="tablist">
        <button
          class:active={tocTab === "index"}
          role="tab"
          aria-selected={tocTab === "index"}
          onclick={() => (tocTab = "index")}
        >
          {$t("reader.bookStructure", { count: sectionCount })}
        </button>
        <button
          class:active={tocTab === "search"}
          role="tab"
          aria-selected={tocTab === "search"}
          onclick={() => (tocTab = "search")}
        >
          {$t("reader.searchResultsTab", { count: searchResults.length })}
        </button>
      </div>

      <form
        class="modal-search"
        onsubmit={(event) => {
          event.preventDefault();
          tocTab = "search";
          void searchInBook();
        }}
      >
        <Icon name="search" size={16} />
        <input
          bind:value={searchQuery}
          placeholder={$t("reader.searchDeepPlaceholder")}
          aria-label={$t("reader.searchBook")}
        />
        {#if searchResults.length > 0}
          <span class="search-count">
            {$t("reader.hits", { count: searchResults.length })}
          </span>
        {/if}
        <button
          class="primary small"
          type="submit"
          disabled={searchBusy || searchQuery.trim().length < 2}
        >
          {searchBusy ? `${searchProgress}%` : $t("library.search")}
        </button>
      </form>

      {#if searchError}
        <p class="search-error" role="status">{searchError}</p>
      {:else if searchBusy}
        <div class="search-progress"><span style:width={`${searchProgress}%`}></span></div>
      {/if}

      <div class="modal-body">
        {#if tocTab === "index"}
          <ol class="toc-list">
            {#each session.document.sections as section, index (section.id)}
              <li>
                <button
                  class:active={index === chapterIndex}
                  onclick={() => {
                    void goToChapter(index);
                    tocOpen = false;
                  }}
                >
                  <span class="toc-number">{index + 1}</span>
                  <span class="toc-copy">
                    <strong>{section.title ?? $t("reader.chapterN", { number: index + 1 })}</strong>
                    <small>
                      {#if index === chapterIndex}
                        {$t("reader.currentChapter")}
                      {:else if visitedSections.has(index)}
                        {$t("reader.visitedChapter")}
                      {:else}
                        {$t("reader.chapterN", { number: index + 1 })}
                      {/if}
                    </small>
                  </span>
                  {#if index === chapterIndex}
                    <span class="toc-badge">{$t("reader.current")}</span>
                  {/if}
                </button>
              </li>
            {/each}
          </ol>
        {:else}
          {#if searchResults.length === 0}
            <p class="empty">
              {searchBusy
                ? $t("reader.searchingBook")
                : $t("reader.searchDeepHelp")}
            </p>
          {:else}
            <ul class="search-results">
              {#each searchResults as result, index (index)}
                <li>
                  <button onclick={() => openSearchHit(result)}>
                    <span class="result-meta">
                      <span class="result-chapter">{result.title}</span>
                      {#if result.section === chapterIndex}
                        <span class="result-current">{$t("reader.current")}</span>
                      {/if}
                    </span>
                    <span class="result-snippet">{result.snippet}</span>
                    <span class="result-open">
                      {$t("reader.jumpHere")} <span aria-hidden="true">→</span>
                    </span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      </div>

      <footer class="modal-foot">
        <label class="page-jump">
          <span>{$t("reader.directPage")}</span>
          <input
            type="number"
            min="1"
            max={pageCount}
            bind:value={pageJump}
            placeholder={String(pageIndex)}
            onkeydown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                jumpToChapterPage();
              }
            }}
          />
          <small>/ {pageCount}</small>
          <button class="primary small" onclick={jumpToChapterPage}>{$t("reader.go")}</button>
        </label>
      </footer>
    </aside>
  {/if}

  {#if settingsOpen && settings}
    <aside class="panel settings settings-panel">
      <header class="modal-head">
        <div class="modal-heading">
          <span class="modal-mark"><Icon name="settings" size={18} /></span>
          <div>
            <h2>{$t("reader.settings")}</h2>
            <p>{$t("reader.settingsSubtitle")}</p>
          </div>
        </div>
        <button
          class="modal-close"
          aria-label={$t("common.close")}
          onclick={() => (settingsOpen = false)}
        >
          <Icon name="close" size={17} />
        </button>
      </header>

      <label>
        <span>{$t("reader.fontSize")} <strong>{settings.fontSize}px</strong></span>
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
        <span>{$t("reader.lineHeight")} <strong>{settings.lineHeight.toFixed(1)}</strong></span>
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
        <span>{$t("reader.margins")} <strong>{settings.margin}px</strong></span>
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
        <span>{$t("reader.zoom")} <strong>{settings.zoom.toFixed(1)}×</strong></span>
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
        <span>{$t("reader.fontFace")}</span>
        <select bind:value={settings.fontFace} onchange={settingsChanged}>
          <option value="newsreader">Newsreader</option>
          <option value="literata">Literata</option>
          <option value="garamond">EB Garamond</option>
          <option value="atkinson">Atkinson Hyperlegible</option>
          <option value="jakarta">Plus Jakarta Sans</option>
          <option value="mono">Monoespaciada</option>
          <option value="opendyslexic">OpenDyslexic</option>
        </select>
      </label>

      <label>
        <span>{$t("reader.columnWidth")} <strong>{settings.columnWidth}px</strong></span>
        <input
          type="range"
          min="520"
          max="1000"
          step="20"
          bind:value={settings.columnWidth}
          oninput={settingsChanged}
        />
      </label>

      <label>
        <span>{$t("reader.alignment")}</span>
        <div class="segmented">
          <button
            class:active={settings.textAlign === "left"}
            onclick={() => {
              if (settings) {
                settings.textAlign = "left";
                settingsChanged();
              }
            }}
          >
            {$t("reader.alignLeft")}
          </button>
          <button
            class:active={settings.textAlign === "justify"}
            onclick={() => {
              if (settings) {
                settings.textAlign = "justify";
                settingsChanged();
              }
            }}
          >
            {$t("reader.alignJustify")}
          </button>
        </div>
      </label>

      <label class="check-row">
        <input
          type="checkbox"
          checked={settings.paragraphIndent > 0}
          onchange={(event) => {
            if (settings) {
              settings.paragraphIndent = event.currentTarget.checked ? 1 : 0;
              settingsChanged();
            }
          }}
        />
        <span>{$t("reader.paragraphIndent")}</span>
      </label>

      <label class="check-row">
        <input
          type="checkbox"
          bind:checked={settings.hyphenate}
          onchange={settingsChanged}
        />
        <span>{$t("reader.hyphenate")}</span>
      </label>

      <label class="check-row">
        <input
          type="checkbox"
          checked={dimImages}
          onchange={(event) => setImageDim(event.currentTarget.checked)}
        />
        <span>{$t("reader.dimImages")}</span>
      </label>

      <label class="check-row">
        <input
          type="checkbox"
          checked={focusLine}
          onchange={(event) => setFocusLine(event.currentTarget.checked)}
        />
        <span>{$t("reader.focusLine")}</span>
      </label>

      <label>
        <span>{$t("common.theme")}</span>
        <select bind:value={settings.theme} onchange={settingsChanged}>
          <option value="inherit">{$t("reader.inherit")}</option>
          <option value="light">{$t("theme.light")}</option>
          <option value="dark">{$t("theme.dark")}</option>
          <option value="sepia">{$t("theme.sepia")}</option>
          <option value="contrast">{$t("theme.contrast")}</option>
        </select>
      </label>

      <label>
        <span>{$t("common.texture")}</span>
        <select bind:value={settings.texture} onchange={settingsChanged}>
          <option value="inherit">{$t("reader.inheritTexture")}</option>
          <option value="none">{$t("texture.none")}</option>
          <option value="paper">{$t("texture.paper")}</option>
          <option value="parchment">{$t("texture.parchment")}</option>
          <option value="soft-dark">{$t("texture.softDark")}</option>
          <option value="linen">{$t("texture.linen")}</option>
          <option value="grid">{$t("texture.grid")}</option>
          <option value="dots">{$t("texture.dots")}</option>
          <option value="waves">{$t("texture.waves")}</option>
          <option value="stripes">{$t("texture.stripes")}</option>
          <option value="grain">{$t("texture.grain")}</option>
          <option value="gingham">{$t("texture.gingham")}</option>
        </select>
      </label>

      <label>
        <span>{$t("reader.mode")}</span>
        <select
          value={settings.mode}
          onchange={(event) =>
            changeMode((event.currentTarget as HTMLSelectElement).value as ReaderMode)}
        >
          <option value="scroll">{$t("reader.scroll")}</option>
          <option value="paged">{$t("reader.paged")}</option>
        </select>
        <small class="hint">{$t("reader.modeHelp")}</small>
      </label>

      <button class="primary small apply-all" onclick={applySettingsToAll}>
        <Icon name="library" size={15} /> {$t("reader.applyToAll")}
      </button>
    </aside>
  {/if}

  {#if characterPanelOpen && session}
    <aside class="panel characters">
      <header>
        <h2>{$t("reader.characterVoices")}</h2>
        <button class="ghost small" onclick={() => (characterPanelOpen = false)}>
          {$t("common.close")}
        </button>
      </header>
      <p class="hint">{$t("reader.narratorHelp")}</p>
      {#if characters.length === 0}
        <p class="empty">{$t("reader.noDialoguesChapter")}</p>
      {:else}
        <ul class="character-list">
          {#each characters as character (character.name)}
            <li>
              <div>
                <strong>{character.name}</strong>
                <span>{$t("reader.phrases", { count: character.lines })}</span>
                <small>{character.sample}</small>
              </div>
              <select
                value={characterVoices[character.name] ?? ""}
                onchange={(event) =>
                  assignCharacterVoice(character.name, event.currentTarget.value)}
                aria-label={$t("reader.voiceFor", { name: character.name })}
              >
                <option value="">{$t("reader.defaultVoice")}</option>
                {#each installedVoices as voice (voice.id)}
                  <option value={voice.id}>{voice.name}</option>
                {/each}
              </select>
            </li>
          {/each}
        </ul>
      {/if}

      {#if downloadableVoices.length > 0}
        <div class="voice-downloads">
          <span class="hint">{$t("reader.downloadVoices")}</span>
          {#each downloadableVoices as voice (voice.id)}
            <button
              class="ghost small"
              onclick={() => downloadVoice(voice)}
              disabled={downloadingVoiceId !== null}
            >
              {downloadingVoiceId === voice.id
                ? $t("reader.downloading")
                : `${voice.language.toUpperCase()} · ${voice.name}`}
            </button>
          {/each}
        </div>
      {/if}

      <div class="tts-tuning">
        <label>
          <span>{$t("reader.sentencePauses")}</span>
          <input
            type="range"
            min="0"
            max="2"
            step="0.1"
            value={ttsPreferencesState?.pauseScale ?? 1}
            oninput={(event) =>
              setTtsPauseScale(Number(event.currentTarget.value))}
          />
        </label>
        <label class="tts-auto">
          <input
            type="checkbox"
            checked={ttsPreferencesState?.expressive ?? true}
            onchange={(event) =>
              setTtsExpressive(event.currentTarget.checked)}
          />
          {$t("reader.expressive")}
        </label>
        <p class="hint">{$t("reader.pausesHelp")}</p>
      </div>
    </aside>
  {/if}

  {#if voicePickerOpen && session}
    <aside class="panel voice-picker" aria-label={$t("reader.readingVoice")}>
      <header>
        <h2>{$t("reader.readingVoice")}</h2>
        <button class="ghost small" onclick={() => (voicePickerOpen = false)}>
          {$t("common.close")}
        </button>
      </header>
      <p class="hint">{$t("reader.voicePickerHelp")}</p>
      {#if installedVoices.length === 0}
        <p class="empty">{$t("reader.noVoices")}</p>
      {:else}
        <ul class="voice-list">
          {#each installedVoices as voice (voice.id)}
            <li>
              <button
                class="voice-option"
                class:active={voice.id === ttsVoiceId}
                onclick={() => {
                  setTtsVoice(voice.id);
                  voicePickerOpen = false;
                }}
              >
                <span class="voice-lang">{voice.language.toUpperCase()}</span>
                <span class="voice-name">
                  {voice.name}{voice.cloned ? ` · ${$t("reader.cloned")}` : ""}
                </span>
                {#if voice.id === ttsVoiceId}
                  <Icon name="check" size={15} />
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </aside>
  {/if}

  {#if practiceOpen && session}
    <aside class="panel practice practice-metrics-panel">
      <header class="modal-head">
        <div class="modal-heading">
          <span class="modal-mark"><Icon name="voice" size={18} /></span>
          <div>
            <h2>{$t("reader.practice")}</h2>
            <p>{$t("reader.practiceSubtitle")}</p>
          </div>
        </div>
        <button
          class="modal-close"
          aria-label={$t("common.close")}
          onclick={() => (practiceOpen = false)}
        >
          <Icon name="close" size={17} />
        </button>
      </header>

      {#if practiceResult}
        <div class="practice-metrics">
          <article class="metric-card">
            <span>{$t("reader.accuracyLabel")}</span>
            <strong>{Math.round(practiceResult.accuracy * 100)}%</strong>
            <small>{$t("reader.practiceAccuracyHint")}</small>
          </article>
          <article class="metric-card">
            <span>{$t("reader.rhythmLabel")}</span>
            <strong>{practiceWpm}</strong>
            <small>{$t("reader.wordsPerMinute")}</small>
          </article>
        </div>
      {/if}

      <div class="practice-passage">
        <span class="quote-label">{$t("reader.passageLabel")}</span>
        {#if selection}
          <blockquote>{selection.text}</blockquote>
        {:else}
          <p class="empty">{$t("reader.noPassage")}</p>
        {/if}
      </div>

      <p class="hint">{$t("reader.practiceHelp")}</p>

      <div class="practice-row">
        <select bind:value={practiceModelId} aria-label={$t("reader.recognitionModel")}>
          {#each whisperModels as model (model.id)}
            <option value={model.id}>
              {model.name}{model.installed ? "" : ` (${$t("reader.notDownloaded")})`}
            </option>
          {/each}
        </select>
        {#if !(whisperModels.find((model) => model.id === practiceModelId)?.installed ?? false)}
          <button class="ghost small" onclick={downloadWhisperModel}>
            {$t("reader.downloadModel")}
          </button>
        {/if}
      </div>

      {#if practiceDownload !== null}
        <progress max="100" value={practiceDownload}></progress>
      {/if}

      <div class="practice-row">
        {#if practiceRecording}
          <button class="primary small" onclick={stopPractice}>
            {$t("reader.stopCompare", { time: practiceClock(practiceSeconds) })}
          </button>
          <button class="ghost small" onclick={cancelPractice}>{$t("common.cancel")}</button>
        {:else}
          <button class="primary small" onclick={startPractice}>
            {$t("reader.recordReading")}
          </button>
        {/if}
      </div>

      {#if practiceError}
        <p class="notice" role="alert">{practiceError}</p>
      {/if}

      {#if practiceResult}
        <div class="practice-result">
          <p class="practice-session-meta">
            {$t("reader.accuracy", {
              seconds: (practiceResult.durationMs / 1000).toFixed(1),
              model: practiceResult.model,
            })}
          </p>
          <span class="quote-label">{$t("reader.transcriptLabel")}</span>
          <p class="transcript">{practiceResult.transcript || $t("reader.noTranscript")}</p>
          <div class="diff">
            {#each practiceResult.diff as word, index (index)}
              <span class="diff-word {word.kind}">
                {#if word.kind === "extra"}
                  +{word.actual}
                {:else if word.kind === "substitution"}
                  {word.expected} → {word.actual}
                {:else if word.kind === "missing"}
                  {word.expected}
                {:else}
                  {word.expected}
                {/if}
              </span>
            {/each}
          </div>
        </div>
      {/if}

      <div class="practice-history">
        <div class="practice-history-head">
          <strong>{$t("reader.practiceHistory")}</strong>
          <span class="hint">
            {practiceHistoryEntries.length}/50
          </span>
        </div>
        {#if practiceHistoryEntries.length === 0}
          <p class="hint">{$t("reader.practiceHistoryEmpty")}</p>
        {:else}
          <ul>
            {#each practiceHistoryEntries.slice(0, 6) as attempt (attempt.id)}
              <li>
                <span class="practice-history-date">
                  {new Date(attempt.createdAt).toLocaleDateString()}
                </span>
                <strong>{Math.round(attempt.accuracy * 100)}%</strong>
                <span class="practice-history-text">{attempt.transcript}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </aside>
  {/if}

  {#if activeAnnotation}
    <aside class="panel annotation-editor note-style">
      <header class="modal-head">
        <div class="modal-heading">
          <span class="modal-mark"><Icon name="bookmark" size={18} /></span>
          <div>
            <h2>{$t("reader.annotation")}</h2>
            <p>{chapter?.title ?? chapterLabel(chapterIndex)}</p>
          </div>
        </div>
        <button
          class="modal-close"
          aria-label={$t("common.close")}
          onclick={() => (activeAnnotation = null)}
        >
          <Icon name="close" size={17} />
        </button>
      </header>
      {#if activeAnnotation.text}
        <div class="quote-card">
          <span class="quote-label">{$t("reader.selectedText")}</span>
          <blockquote>{activeAnnotation.text}</blockquote>
        </div>
      {/if}
      <label class="note-field">
        <span>{$t("reader.annotationNote")}</span>
        <textarea bind:value={annotationNote} rows="4" placeholder={$t("reader.writeNote")}></textarea>
      </label>
      <div class="color-row">
        <span>{$t("reader.markerColors")}</span>
        <div class="colors">
          {#each colors as color (color)}
            <button
              class="swatch"
              class:selected={annotationColor === color}
              style:background={color}
              aria-label={$t("reader.color", { color })}
              onclick={() => (annotationColor = color)}
            ></button>
          {/each}
        </div>
      </div>
      <footer class="modal-foot editor-actions">
        <button class="primary small" onclick={saveAnnotation}>{$t("reader.saveNote")}</button>
        <button class="danger small" onclick={deleteAnnotation}>{$t("common.delete")}</button>
      </footer>
    </aside>
  {/if}
</section>

<style>
  .reader {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100vh;
    gap: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .toolbar {
    position: relative;
    z-index: 20;
    flex: 0 0 auto;
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: 0.75rem;
    min-height: 56px;
    padding: 0.3rem 0.75rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    background: color-mix(in srgb, var(--surface) 97%, transparent);
  }

  .toolbar-context {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
    min-width: 0;
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
  }

  .selection-bar {
    position: absolute;
    top: 3.6rem;
    left: 50%;
    z-index: 19;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.55rem;
    width: min(64rem, calc(100% - 1.5rem));
    padding: 0.4rem 0.85rem;
    border: 1px solid color-mix(in srgb, var(--accent) 32%, var(--border));
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
    box-shadow: var(--shadow-lg);
    transform: translateX(-50%);
  }

  .selection-bar-quote {
    flex: 1 1 16rem;
    min-width: 8rem;
    overflow: hidden;
    color: var(--muted);
    font-family: var(--font-reading);
    font-size: 0.76rem;
    font-style: italic;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .selection-bar-colors {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .selection-bar-label {
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .selection-color {
    width: 20px;
    min-width: 20px;
    height: 20px;
    min-height: 20px;
    padding: 0;
    border: 1px solid color-mix(in srgb, #000 22%, transparent);
    border-radius: 50%;
  }

  .selection-color:hover {
    transform: scale(1.1);
  }

  .selection-bar-close {
    display: grid;
    place-items: center;
    width: 28px;
    min-width: 28px;
    height: 28px;
    min-height: 28px;
    padding: 0;
    border-color: transparent;
    background: transparent;
    color: var(--muted);
  }

  .selection-bar-close:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  @media (max-width: 760px) {
    .selection-bar {
      top: 3.3rem;
      width: calc(100% - 1rem);
      padding: 0.4rem 0.55rem;
      flex-wrap: nowrap;
      overflow-x: auto;
      scrollbar-width: none;
    }

    .selection-bar::-webkit-scrollbar {
      display: none;
    }

    .selection-bar-quote {
      display: none;
    }
  }

  .heading {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    flex: 1;
    min-width: 0;
    max-width: none;
  }

  .heading-copy {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .bar-back {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 36px;
    min-width: 36px;
    height: 36px;
    min-height: 36px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .bar-back:hover {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  h1 {
    margin: 0;
    overflow: hidden;
    font-family: var(--font-sans);
    font-size: 0.92rem;
    font-weight: 650;
    line-height: 1.25;
    letter-spacing: -0.01em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .subtitle {
    margin: 0.02rem 0 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .controls {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: nowrap;
    gap: 2px;
    padding: 3px;
    border: 1px solid color-mix(in srgb, var(--border) 85%, transparent);
    border-radius: 14px;
    background: var(--surface-2);
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 42%, transparent);
  }

  .controls button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    width: 36px;
    min-width: 36px;
    min-height: 36px;
    height: 36px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.76rem;
    font-weight: 500;
    box-shadow: none;
  }

  .controls button:hover {
    border-color: var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .controls button.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }



  .controls .type-button {
    width: 36px;
    min-width: 36px;
    padding: 0;
    font-family: var(--font-display);
    font-size: 0.94rem;
    font-weight: 600;
  }

  .controls .control-metrics {
    display: none;
  }

  .control-label {
    display: none;
  }

  .tts-bar {
    position: absolute;
    top: calc(56px + 0.5rem);
    left: 50%;
    z-index: 35;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    width: min(34rem, calc(100vw - 1.5rem));
    padding: 0.9rem;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--surface);
    box-shadow: var(--shadow-lg);
    font-size: 0.82rem;
    transform: translateX(-50%);
  }

  .tts-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .tts-heading {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    font-family: var(--font-display);
    font-size: 1.02rem;
    font-weight: 600;
  }

  .tts-head-icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent);
  }

  .tts-close {
    display: grid;
    place-items: center;
    width: 44px;
    min-width: 44px;
    height: 44px;
    min-height: 44px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
  }

  .tts-close:hover {
    border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
    color: var(--text);
  }

  .tts-main {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }

  .tts-play {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    min-height: 46px;
    padding: 0.5rem 1.25rem;
    border-radius: 999px;
  }

  .tts-stop {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    min-height: 46px;
    padding: 0.5rem 1rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
  }

  .tts-sliders {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }

  .tts-control {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.55rem 0.65rem 0.45rem;
    border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
    border-radius: 12px;
    background: var(--surface-2);
  }

  .tts-control-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
    color: var(--muted);
    font-size: 0.64rem;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tts-control-head strong {
    color: var(--accent);
    font-size: 0.72rem;
    letter-spacing: 0;
    text-transform: none;
  }

  .tts-control input[type="range"] {
    width: 100%;
    margin: 0;
    accent-color: var(--accent);
  }

  .tts-voice-row {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
  }

  .tts-voice {
    flex: 1;
    min-width: 0;
    min-height: 52px;
    font: inherit;
    font-size: 0.78rem;
    padding: 0.35rem 0.55rem;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    text-align: left;
    cursor: pointer;
  }

  .tts-voice :global(svg) {
    flex: 0 0 auto;
    color: var(--muted);
  }

  .tts-voice-lang {
    flex: 0 0 auto;
    min-width: 2rem;
    padding: 0.18rem 0.32rem;
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 700;
    text-align: center;
  }

  .tts-voice-label {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .tts-circle-action {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    flex: 0 0 auto;
  }

  .tts-circle-action > span {
    color: var(--muted);
    font-size: 0.58rem;
    font-weight: 600;
  }

  .tts-circle {
    display: grid;
    place-items: center;
    width: 52px;
    min-width: 52px;
    height: 52px;
    min-height: 52px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text);
  }

  .tts-circle:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  }

  .tts-circle-accent {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
    background: color-mix(in srgb, var(--accent) 12%, var(--surface));
    color: var(--accent);
  }

  .tts-footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .voice-list {
    list-style: none;
    margin: 0.6rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .voice-option {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.6rem;
    border: 1px solid transparent;
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    text-align: left;
    cursor: pointer;
  }

  .voice-option:hover {
    border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
  }

  .voice-option.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--surface-2));
  }

  .voice-option :global(svg) {
    flex: 0 0 auto;
    color: var(--accent);
  }

  .voice-lang {
    flex: 0 0 auto;
    min-width: 2rem;
    padding: 0.12rem 0.3rem;
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 700;
    text-align: center;
  }

  .voice-name {
    flex: 1;
    min-width: 0;
  }

  .tts-auto {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.76rem;
  }

  .handoff {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.75rem;
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--accent) 55%, var(--border));
    background: color-mix(in srgb, var(--accent) 9%, var(--surface));
    font-size: 0.82rem;
  }

  .handoff span {
    flex: 1;
    min-width: 12rem;
  }

  .mirror-panel h3 {
    margin: 0.4rem 0 0.2rem;
    font-size: 0.86rem;
  }

  .mirror-peers {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .mirror-peer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.4rem 0.5rem;
    border-radius: 8px;
    background: var(--surface-2);
    font-size: 0.82rem;
  }

  .extract-notice {
    position: fixed;
    right: 1.2rem;
    bottom: 1.2rem;
    z-index: 40;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0;
    padding: 0.55rem 0.7rem;
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, #22c55e 55%, var(--border));
    background: var(--surface);
    font-size: 0.8rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  }

  .tts-excerpt {
    padding: 0.18rem 0.5rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tts-error {
    color: var(--warn);
    font-size: 0.78rem;
  }

  .tts-watermark {
    margin: 0;
    padding: 0.35rem 0.6rem;
    border-radius: 8px;
    border: 1px dashed var(--border);
    color: var(--muted);
    font-size: 0.74rem;
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
    flex: 0 0 auto;
    flex-direction: column;
    gap: 0.25rem;
    width: 9.5rem;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    box-shadow: none;
    color: var(--muted);
    font-size: 0.6rem;
  }

  .progress-meta {
    display: flex;
    justify-content: space-between;
    gap: 0.4rem;
  }

  .progress-track {
    width: 100%;
    height: 3px;
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
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    width: min(42rem, calc(100% - 3rem));
    max-width: 42rem;
    margin: auto;
    padding: clamp(1.4rem, 4vw, 2.2rem);
    border: 1px solid var(--border);
    border-radius: 20px;
    background:
      radial-gradient(
        520px 240px at 50% 0%,
        color-mix(in srgb, var(--accent) 9%, transparent),
        transparent 72%
      ),
      var(--surface);
    box-shadow: var(--shadow-md);
  }

  .picker-head {
    display: flex;
    align-items: flex-start;
    gap: 0.8rem;
  }

  .picker-head > div {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .picker-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 44px;
    height: 44px;
    border: 1px solid color-mix(in srgb, var(--accent) 20%, var(--border));
    border-radius: 13px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .picker h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .recent {
    margin-top: 0.7rem;
    padding-top: 0.9rem;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .recent .eyebrow {
    margin-bottom: 0.5rem;
  }

  .recent-row {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 0.55rem;
  }

  .recent-book {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
    min-height: 0;
    padding: 0;
    border: 0;
    background: transparent;
    text-align: left;
  }

  .recent-cover {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 64%, transparent);
    border-radius: 7px;
    background: var(--surface-2);
    box-shadow: 0 6px 14px -10px var(--shadow-color);
    transition:
      transform 160ms ease,
      box-shadow 160ms ease;
  }

  .recent-cover img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .recent-placeholder {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    font-family: var(--font-display);
    font-size: 1.5rem;
    font-weight: 650;
  }

  .recent-progress {
    position: absolute;
    right: 0.25rem;
    bottom: 0.25rem;
    left: 0.25rem;
    height: 3px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.34);
    overflow: hidden;
  }

  .recent-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: #fff;
  }

  .recent-title {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.66rem;
    line-height: 1.3;
  }

  .recent-book:hover .recent-cover {
    transform: translateY(-2px);
    box-shadow: 0 12px 20px -14px var(--shadow-color);
  }

  .picker-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .picker select {
    flex: 1;
    min-width: 14rem;
    min-height: 42px;
    padding: 0.55rem 0.7rem;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.88rem;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .reader-stage {
    position: relative;
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
    width: 100%;
    justify-content: center;
    align-items: stretch;
    padding: clamp(0.5rem, 1.4vh, 0.9rem) clamp(0.5rem, 1.2vw, 1rem);
    background:
      radial-gradient(
        1000px 500px at 50% 0%,
        color-mix(in srgb, var(--accent) 5%, transparent),
        transparent 72%
      ),
      color-mix(in srgb, var(--surface-2) 55%, var(--bg));
    overflow: hidden;
  }

  .book-spine {
    display: none;
    position: absolute;
    top: clamp(0.5rem, 1.4vh, 0.9rem);
    bottom: clamp(0.5rem, 1.4vh, 0.9rem);
    left: 50%;
    z-index: 3;
    width: 38px;
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, var(--reader-text) 7%, transparent) 38%,
      color-mix(in srgb, var(--reader-text) 15%, transparent) 50%,
      color-mix(in srgb, var(--reader-text) 7%, transparent) 62%,
      transparent
    );
    pointer-events: none;
    transform: translateX(-50%);
  }

  .book-spine.visible {
    display: block;
  }

  .surface {
    position: relative;
    width: 100%;
    max-width: 1560px;
    height: 100%;
    min-height: 0;
    aspect-ratio: auto;
    border: 1px solid color-mix(in srgb, var(--reader-border) 72%, transparent);
    border-radius: var(--radius-lg);
    background-color: var(--reader-bg);
    background-image: var(--app-texture, none);
    background-position: var(--app-texture-position, 0 0);
    background-size: var(--app-texture-size, auto);
    background-repeat: repeat;
    color: var(--reader-text);
    --texture-ink: var(--reader-text);
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-overflow-scrolling: touch;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--reader-text) 24%, transparent) transparent;
    box-shadow:
      0 24px 60px -38px color-mix(in srgb, #2a0e07 40%, transparent),
      0 10px 30px -24px color-mix(in srgb, var(--gold) 28%, transparent),
      inset 0 0 0 1px color-mix(in srgb, var(--reader-text) 3%, transparent);
    transition:
      background-color 160ms ease,
      color 160ms ease,
      border-color 160ms ease;
  }

  .surface.paged {
    overflow-x: auto;
    overflow-y: hidden;
  }

  .surface::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  .surface::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: color-mix(in srgb, var(--reader-text) 22%, transparent);
  }

  .surface::-webkit-scrollbar-track {
    background: transparent;
  }

  .chapter {
    position: relative;
    max-width: var(--reader-column-width, 840px);
    margin: 0 auto;
    padding: clamp(2rem, 4.5vh, 3.5rem) clamp(1.5rem, 5vw, 4rem);
    font-family: var(--reader-font-family);
    font-size: calc(var(--reader-font-size) * var(--reader-zoom));
    line-height: var(--reader-line-height);
    color: var(--reader-text);
    text-align: var(--reader-text-align, left);
    hyphens: var(--reader-hyphens, manual);
    overflow-wrap: anywhere;
  }

  .surface.paged .chapter {
    max-width: none;
    margin: 0;
    height: 100%;
    padding: clamp(1.5rem, 4vh, 2.5rem) var(--reader-margin);
    column-count: 1;
    column-gap: calc(2 * var(--reader-margin));
    column-fill: auto;
  }

  .surface.paged.spread .chapter {
    column-count: 2;
    padding-inline: max(var(--reader-margin), 1.75rem);
    column-gap: calc(2 * max(var(--reader-margin), 1.75rem));
  }

  .chapter :global(p) {
    margin: 0 0 1em;
    text-indent: var(--reader-paragraph-indent, 0px);
  }

  .chapter :global(p:first-of-type) {
    text-indent: 0;
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

  .annotation-layer,
  .tts-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: visible;
  }

  .tts-phrase {
    position: absolute;
    border-radius: 3px;
    background: color-mix(in srgb, var(--accent) 28%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
    transition:
      left 90ms linear,
      top 90ms linear,
      width 90ms linear,
      height 90ms linear;
  }

  .annotation {
    position: absolute;
    /* El botón global de la app impone min-height: 40px; aquí la caja debe
       ceñirse al alto real del texto o el resaltado se desborda por abajo. */
    min-height: 0;
    min-width: 0;
    border: 0;
    padding: 0;
    border-radius: 3px;
    pointer-events: auto;
    cursor: pointer;
    background: color-mix(in srgb, var(--mark-color) 40%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--mark-color) 58%, transparent);
    mix-blend-mode: multiply;
    transition:
      filter 120ms ease,
      box-shadow 120ms ease;
  }

  .surface.theme-dark .annotation,
  .surface.theme-contrast .annotation {
    mix-blend-mode: screen;
    background: color-mix(in srgb, var(--mark-color) 32%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--mark-color) 75%, transparent);
  }

  .annotation.kind-underline {
    background: transparent;
    box-shadow: none;
    border-radius: 0;
  }

  /* La línea se dibuja pegada a la base del texto, no al fondo del
     interlineado completo, para que no quede separada entre líneas. */
  .annotation.kind-underline::after {
    content: "";
    position: absolute;
    right: 0;
    bottom: 22%;
    left: 0;
    height: max(1px, 0.09em);
    border-radius: 999px;
    background: var(--mark-color);
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
    filter: brightness(1.08);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--mark-color) 80%, transparent);
  }

  .text-mark {
    position: absolute;
    z-index: 3;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    min-height: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface);
    color: var(--accent);
    font-size: 0.8rem;
    line-height: 1;
    box-shadow: var(--shadow-xs);
    transform: translate(-50%, -50%);
    pointer-events: auto;
    cursor: pointer;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }

  .text-mark.kind-extract {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .text-mark.kind-reaction {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    background: color-mix(in srgb, var(--surface) 88%, var(--accent));
  }

  .surface.theme-dark .text-mark.kind-reaction,
  .surface.theme-contrast .text-mark.kind-reaction {
    background: color-mix(in srgb, var(--surface) 78%, var(--accent));
  }

  .text-mark:hover {
    transform: translate(-50%, -50%) scale(1.1);
    box-shadow: var(--shadow-sm);
  }

  .reader-pager {
    position: relative;
    left: auto;
    bottom: auto;
    z-index: 15;
    flex: 0 0 auto;
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    width: 100%;
    max-width: none;
    padding: 0.3rem 0.75rem calc(0.3rem + env(safe-area-inset-bottom));
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    border-radius: 0;
    background: color-mix(in srgb, var(--surface) 98%, transparent);
    box-shadow: none;
    transform: none;
  }

  .pager-step {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    min-height: 36px;
    padding: 0.3rem 0.8rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      transform 150ms ease,
      border-color 150ms ease,
      background 150ms ease,
      box-shadow 150ms ease;
  }

  .pager-step:hover:not(:disabled) {
    transform: translateY(-1px);
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    background: color-mix(in srgb, var(--accent) 9%, var(--surface));
    box-shadow: 0 8px 18px -12px color-mix(in srgb, #000 40%, transparent);
  }

  .pager-step.primary {
    background: linear-gradient(
      180deg,
      var(--accent),
      color-mix(in srgb, var(--accent) 86%, #000)
    );
    border-color: color-mix(in srgb, var(--accent) 82%, #000);
    color: var(--accent-contrast);
  }

  .pager-step.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 88%, #000);
  }

  .pager-step:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .pager-step .step-icon {
    font-size: 1.25rem;
    line-height: 0.8;
  }

  .chapter-step {
    color: var(--muted);
    font-weight: 500;
    background: transparent;
  }

  .pager-status {
    min-width: 10rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.14rem;
    padding: 0 0.4rem;
    text-align: center;
  }

  .pager-status strong {
    font-size: 0.7rem;
    letter-spacing: 0.02em;
  }

  .pager-title {
    max-width: 12rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: 0.62rem;
  }

  .pager-track {
    width: 7rem;
    height: 3px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .pager-track > span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 160ms ease;
  }

  @media (max-width: 640px) {
    .pager-step .step-label {
      display: none;
    }

    .pager-step {
      min-width: 38px;
      padding: 0.35rem 0.55rem;
    }

    .pager-status {
      min-width: 6.5rem;
    }
  }

  .swatch {
    flex: none;
    width: 24px;
    height: 24px;
    min-height: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    padding: 0;
  }

  .swatch.selected {
    border-color: var(--text);
  }

  .note-input {
    width: 11rem;
    min-height: 34px;
    font: inherit;
    font-size: 0.8rem;
    padding: 0.35rem 0.55rem;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
  }

  .panel {
    position: fixed;
    z-index: 40;
    top: 4.85rem;
    right: 1rem;
    width: min(24rem, calc(100vw - 2rem));
    max-height: min(78vh, 48rem);
    overflow-y: auto;
    padding: 1rem;
    border-radius: 18px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .panel header {
    position: sticky;
    top: -1rem;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin: -1rem -1rem 0.75rem;
    padding: 0.85rem 1rem 0.65rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    border-radius: 18px 18px 0 0;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
  }

  .panel h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.03rem;
    font-weight: 650;
  }

  .character-list {
    list-style: none;
    margin: 0.6rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .character-list li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.5rem;
    align-items: center;
    padding-bottom: 0.55rem;
    border-bottom: 1px solid var(--border);
  }

  .character-list li > div {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .character-list strong {
    font-size: 0.86rem;
  }

  .character-list span,
  .character-list small {
    color: var(--muted);
    font-size: 0.74rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .character-list select,
  .practice select {
    font: inherit;
    font-size: 0.76rem;
    padding: 0.3rem 0.45rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    max-width: 9rem;
  }

  .voice-downloads {
    margin-top: 0.7rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    align-items: center;
  }

  .tts-tuning {
    margin-top: 0.8rem;
    padding-top: 0.7rem;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.8rem;
  }

  .tts-tuning label:not(.tts-auto) {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    color: var(--muted);
  }

  .tts-tuning input[type="range"] {
    accent-color: var(--accent);
  }

  .practice {
    width: min(28rem, calc(100vw - 2.5rem));
  }

  .practice blockquote {
    margin: 0.5rem 0;
    padding-left: 0.7rem;
    border-left: 3px solid var(--border);
    color: var(--muted);
    font-size: 0.82rem;
    max-height: 7rem;
    overflow-y: auto;
  }

  .practice-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
    align-items: center;
    margin-top: 0.6rem;
  }

  .practice progress {
    width: 100%;
    margin-top: 0.5rem;
    accent-color: var(--accent);
  }

  .practice-result {
    margin-top: 0.8rem;
    border-top: 1px solid var(--border);
    padding-top: 0.7rem;
    font-size: 0.82rem;
  }

  .practice-result .transcript {
    color: var(--muted);
    font-style: italic;
  }

  .diff {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    line-height: 1.5;
  }

  .diff-word {
    padding: 0.1rem 0.3rem;
    border-radius: 5px;
    background: var(--surface-2);
    font-size: 0.78rem;
  }

  .diff-word.match {
    background: color-mix(in srgb, #22c55e 18%, transparent);
  }

  .diff-word.substitution {
    background: color-mix(in srgb, #f59e0b 22%, transparent);
  }

  .diff-word.missing {
    background: color-mix(in srgb, #ef4444 20%, transparent);
    text-decoration: line-through;
  }

  .diff-word.extra {
    background: color-mix(in srgb, #3b82f6 18%, transparent);
  }

  .practice-history {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    margin-top: 0.8rem;
    padding-top: 0.7rem;
    border-top: 1px solid var(--border);
  }

  .practice-history-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.8rem;
  }

  .practice-history ul {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .practice-history li {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr);
    align-items: baseline;
    gap: 0.4rem;
    padding: 0.35rem 0.45rem;
    border-radius: 7px;
    background: var(--surface-2);
    font-size: 0.72rem;
  }

  .practice-history-date {
    color: var(--muted);
  }

  .practice-history li strong {
    color: var(--accent);
  }

  .practice-history-text {
    overflow: hidden;
    color: var(--muted);
    text-overflow: ellipsis;
    white-space: nowrap;
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
    --reader-bg: #fbf9f5;
    --reader-text: #1b1c1a;
    --reader-muted: #554336;
    --reader-border: #e4e2de;
  }

  .theme-dark {
    --reader-bg: #151311;
    --reader-text: #e8e1dd;
    --reader-muted: #d8c3ad;
    --reader-border: #534434;
  }

  .theme-sepia {
    --reader-bg: #f6f1e8;
    --reader-text: #33221c;
    --reader-muted: #7a6658;
    --reader-border: #d8c4a9;
  }

  .theme-contrast {
    --reader-bg: #000000;
    --reader-text: #ffffff;
    --reader-muted: #d8d8d8;
    --reader-border: #ffffff;
  }

  .theme-inherit {
    --reader-bg: var(--bg);
    --reader-text: var(--text);
    --reader-muted: var(--muted);
    --reader-border: var(--border);
  }

  @media (max-width: 900px) {
    .toolbar {
      flex-direction: column;
      align-items: stretch;
      gap: 0.35rem;
      min-height: 0;
      padding: 0.35rem 0.75rem 0.45rem;
    }

    .toolbar-context {
      width: 100%;
      gap: 0.55rem;
    }

    .toolbar-actions {
      width: 100%;
    }

    .controls {
      flex: 1 1 auto;
      width: auto;
      min-width: 0;
      justify-content: space-between;
    }

    .controls .control-metrics {
      display: inline-flex;
    }

    .controls .control-mirror {
      display: none;
    }

    .progress {
      display: flex;
      flex: 0 0 auto;
      align-items: center;
      width: auto;
      margin-left: auto;
      padding: 0.22rem 0.55rem;
      border: 1px solid var(--border);
      border-radius: 999px;
      background: var(--surface-2);
    }

    .progress-meta {
      gap: 0;
      font-size: 0.62rem;
    }

    .progress-meta span:first-child {
      color: var(--accent);
      font-weight: 650;
    }

    .progress-meta span:first-child::before {
      content: "";
      display: inline-block;
      width: 6px;
      height: 6px;
      margin-right: 4px;
      border-radius: 50%;
      background: var(--accent);
      vertical-align: 0.08em;
    }

    .progress-meta span:not(:first-child) {
      display: none;
    }

    .progress-track {
      display: none;
    }
  }

  @media (max-width: 760px) {
    .toolbar {
      min-height: 52px;
      gap: 0.45rem;
      padding-inline: 0.5rem;
    }

    .subtitle {
      display: block;
    }

    .control-mirror {
      display: none;
    }

    .chapter {
      padding: 1.6rem clamp(1.1rem, 6vw, 2.2rem);
    }

    .reader-stage {
      padding: 0.35rem;
    }

    .panel {
      top: auto;
      right: 0.75rem;
      bottom: 0.75rem;
      left: 0.75rem;
      width: auto;
      max-height: 60vh;
    }

    .reader-pager {
      gap: 0.2rem;
      padding-inline: 0.4rem;
    }

    .pager-status {
      min-width: 7rem;
    }

    .pager-title {
      max-width: 8rem;
    }
  }

  /* ── Ajustes de apariencia Lumina Reader ───────────────────────────── */
  .reader {
    height: calc(100vh - var(--topbar-height));
    min-height: 0;
    background: var(--bg);
  }

  .toolbar {
    min-height: 52px;
    padding: 0.3rem 24px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow: none;
  }

  .heading {
    gap: 0.7rem;
  }

  .bar-back {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    width: auto;
    min-width: 0;
    height: 32px;
    min-height: 32px;
    padding: 0 0.55rem;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .bar-back:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .bar-back-icon {
    font-size: 1.15rem;
    line-height: 1;
  }

  .heading-copy h1 {
    font-family: var(--font-sans);
    font-size: 0.82rem;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .subtitle {
    font-size: 0.65rem;
  }

  .progress {
    flex: 0 1 230px;
    width: 230px;
    margin-left: auto;
  }

  .progress-meta {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    color: var(--muted);
    font-size: 0.625rem;
    white-space: nowrap;
  }

  .progress-meta span:last-child {
    color: var(--accent);
    font-weight: 600;
  }

  .progress-track {
    height: 3px;
    margin-top: 3px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 12%, var(--surface-3));
  }

  .progress-fill {
    background: var(--accent);
  }

  .controls {
    gap: 2px;
    padding: 3px;
    border-radius: 14px;
    background: var(--surface-2);
  }

  .controls button {
    width: 36px;
    min-width: 36px;
    height: 36px;
    min-height: 36px;
    padding: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font-size: 0.76rem;
  }

  .controls button:hover {
    background: var(--surface-3);
  }

  .controls button.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .reader-stage {
    padding: 0;
    background: var(--bg);
  }

  .book-spine {
    display: none !important;
  }

  .surface {
    max-width: var(--reader-column-width, 840px);
    margin: 0 auto;
    border: 0;
    border-radius: 0;
    background-color: var(--reader-bg);
    box-shadow: none;
  }

  /* La hoja doble necesita el doble de ancho que una columna de lectura. */
  .surface.paged.spread {
    max-width: min(calc(var(--reader-column-width, 840px) * 2), 100%);
  }

  .surface.paged .chapter {
    padding: 42px clamp(30px, 5vw, 64px) 34px;
    column-gap: calc(var(--reader-margin) * 2);
  }

  .chapter {
    max-width: 760px;
    padding: 48px clamp(24px, 5vw, 56px) 56px;
  }

  .chapter :global(h1),
  .chapter :global(h2),
  .chapter :global(h3) {
    font-family: var(--font-display);
    font-weight: 500;
    letter-spacing: -0.02em;
  }

  .chapter :global(h1) {
    margin: 0.2em 0 0.6em;
    font-size: 2.2em;
    font-style: italic;
    line-height: 1.12;
  }

  .chapter :global(h2) {
    font-size: 1.6em;
  }

  .chapter :global(h3) {
    font-size: 1.22em;
  }

  .chapter :global(> h1:first-child),
  .chapter :global(> h2:first-child) {
    padding-bottom: 0.3em;
  }

  .chapter :global(hr) {
    width: 54px;
    margin: 1.2em 0 1.6em;
    border: 0;
    border-top: 2px solid var(--accent-2);
  }

  .chapter :global(blockquote) {
    border-left: 3px solid var(--accent-2);
    border-radius: 0 8px 8px 0;
    background: color-mix(in srgb, var(--accent) 5%, transparent);
    padding: 0.8em 1.1em;
    color: var(--muted);
  }

  .reader-pager {
    justify-content: space-between;
    width: min(var(--reader-column-width, 840px), 100%);
    margin: 0 auto;
    padding: 8px 24px calc(10px + env(safe-area-inset-bottom));
    border-top: 0;
    background: transparent;
    backdrop-filter: none;
  }

  .reader-pager.spread {
    width: min(calc(var(--reader-column-width, 840px) * 2), 100%);
  }

  .pager-step {
    gap: 5px;
    min-height: 32px;
    padding: 0.25rem 0.6rem;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .pager-step:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text);
    transform: none;
    box-shadow: none;
  }

  .pager-step.primary {
    background: transparent;
    color: var(--accent);
  }

  .pager-step.primary:hover:not(:disabled) {
    background: var(--accent-soft);
  }

  .pager-step .step-icon {
    font-size: 1.1rem;
  }

  .chapter-step .step-label {
    display: none;
  }

  .pager-status {
    min-width: 0;
    flex: 1;
    flex-direction: row;
    justify-content: center;
    gap: 7px;
    color: var(--muted);
    font-size: 0.65rem;
  }

  .pager-status strong {
    color: var(--text);
    font-size: 0.65rem;
    font-weight: 600;
  }

  .pager-title {
    max-width: 42ch;
    overflow: hidden;
    font-size: 0.65rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pager-track {
    width: 72px;
    height: 3px;
    margin-left: 4px;
    border-radius: 999px;
    background: var(--surface-4);
  }

  .pager-track span {
    background: var(--accent);
  }

  .tts-bar {
    position: absolute;
    top: auto;
    bottom: 54px;
    z-index: 32;
    width: calc(100% - 20px);
    max-height: 60vh;
    padding: 12px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 18px;
    background: var(--surface);
    box-shadow: 0 18px 44px -24px rgba(49, 24, 12, 0.34);
    font-size: 0.76rem;
  }

  .tts-bar .primary {
    border-radius: 999px;
    background: linear-gradient(110deg, var(--accent-2), var(--accent-3));
    color: #fff;
  }

  .tts-sliders {
    grid-template-columns: 1fr 1fr;
  }

  .tts-voice {
    font-size: 0.7rem;
  }

  .tts-circle {
    width: 48px;
    min-width: 48px;
    height: 48px;
    min-height: 48px;
  }

  .tts-circle-action > span {
    font-size: 0.55rem;
  }

  .tts-auto {
    font-size: 0.68rem;
  }

  .panel {
    top: calc(var(--topbar-height) + 62px);
    right: 22px;
    border-radius: 16px;
    border-color: var(--border);
    background: var(--surface);
    box-shadow: var(--shadow-lg);
  }

  .panel header {
    border-radius: 16px 16px 0 0;
  }

  .panel h2 {
    font-weight: 500;
  }

  .picker {
    max-width: 720px;
    margin: 40px auto;
    border-radius: 20px;
    background: var(--surface);
    box-shadow: var(--shadow-sm);
  }

  @media (max-width: 900px) {
    .reader {
      height: calc(100vh - 68px - 72px - env(safe-area-inset-bottom));
    }

    .toolbar {
      min-height: 0;
      align-items: stretch;
      padding: 0.35rem 14px 0.45rem;
    }

    .bar-back-label {
      display: none;
    }

    .bar-back {
      width: 40px;
      min-width: 40px;
      height: 40px;
      min-height: 40px;
      padding: 0;
      justify-content: center;
      border: 1px solid var(--border);
      border-radius: 50%;
      background: var(--surface-2);
    }

    .progress {
      flex: 0 0 auto;
      width: auto;
      min-width: 0;
      margin-left: auto;
      padding: 0.22rem 0.55rem;
      border: 1px solid var(--border);
      border-radius: 999px;
      background: var(--surface-2);
    }

    .heading-copy h1 {
      max-width: none;
    }

    .surface {
      max-width: none;
    }

    .surface.paged .chapter {
      padding: 28px 22px 24px;
    }

    .chapter {
      padding: 28px 20px 40px;
    }

    .chapter :global(h1) {
      font-size: 1.8em;
    }

    .reader-pager {
      padding: 6px 10px calc(8px + env(safe-area-inset-bottom));
    }

    .pager-status {
      flex-direction: column;
      gap: 2px;
    }

    .pager-title,
    .pager-track {
      display: none;
    }

    .chapter-step {
      display: none;
    }

    .tts-bar {
      bottom: 52px;
      width: calc(100% - 20px);
      max-height: 60vh;
      overflow-y: auto;
    }
  }

  /* ── IA local ───────────────────────────────────────────────────────── */
  .ai-panel {
    top: calc(var(--topbar-height) + 62px);
    right: 22px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: min(404px, calc(100vw - 32px));
    max-height: min(80vh, 54rem);
    padding: 14px;
    overflow: hidden;
  }

  .ai-panel-head {
    position: static;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    margin: 0;
    padding: 0 0 8px;
    border: 0;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: transparent;
  }

  .ai-title {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }

  .ai-mark {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .ai-title h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.08rem;
    font-weight: 500;
    letter-spacing: -0.01em;
  }

  .ai-title p {
    margin: 1px 0 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.58rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-tabs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
    padding: 3px;
    border-radius: 9px;
    background: var(--surface-2);
  }

  .ai-tabs button {
    min-height: 30px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
  }

  .ai-tabs button.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-xs);
  }

  .ai-meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px 8px;
    color: var(--muted);
    font-size: 0.6rem;
  }

  .ai-meta span:last-child {
    margin-left: auto;
    padding: 0.15rem 0.4rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 700;
    text-transform: uppercase;
  }

  .ai-meta code {
    max-width: 65%;
    overflow: hidden;
    color: var(--text);
    font-size: 0.6rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-tools {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }

  .ai-tool {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    min-width: 0;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .ai-tool:hover:not(:disabled) {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .ai-tool:disabled {
    opacity: 0.55;
  }

  .ai-tool > span {
    color: var(--accent);
  }

  .ai-tool strong {
    font-size: 0.68rem;
    font-weight: 600;
    line-height: 1.2;
  }

  .ai-tool small {
    color: var(--muted);
    font-size: 0.55rem;
    line-height: 1.25;
  }

  .ai-hint {
    margin: 6px 2px;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.45;
  }

  .ai-thinking {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 4px 2px;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .ai-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent-2);
    animation: ai-pulse 1s ease-in-out infinite alternate;
  }

  @keyframes ai-pulse {
    to {
      opacity: 0.25;
    }
  }

  .ai-result {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .ai-result header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .ai-result header strong {
    color: var(--accent);
    font-size: 0.62rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .ai-save {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.55rem;
  }

  .ai-save:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .ai-result p {
    margin: 0;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.82rem;
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .ai-notice {
    margin: 0;
    color: var(--ok);
    font-size: 0.62rem;
  }

  .ai-error {
    margin: 0;
    padding: 7px 8px;
    border-radius: 7px;
    background: color-mix(in srgb, var(--danger) 8%, var(--surface));
    color: var(--danger);
    font-size: 0.65rem;
    line-height: 1.4;
  }

  .ai-chat {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-height: 130px;
    max-height: 38vh;
    overflow-y: auto;
    padding: 2px;
    overscroll-behavior: contain;
  }

  .ai-chat-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 24px 16px;
    color: var(--muted);
    text-align: center;
  }

  .ai-chat-empty span {
    color: var(--accent);
  }

  .ai-chat-empty p {
    margin: 0;
    font-size: 0.68rem;
    line-height: 1.45;
  }

  .ai-bubble {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 92%;
    padding: 7px 9px;
    border-radius: 9px;
    background: var(--surface-2);
  }

  .ai-bubble.user {
    align-self: flex-end;
    background: var(--accent-soft);
  }

  .ai-bubble > span {
    color: var(--muted);
    font-size: 0.52rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .ai-bubble p {
    margin: 0;
    font-size: 0.72rem;
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .ai-composer {
    display: flex;
    align-items: flex-end;
    gap: 5px;
  }

  .ai-composer textarea {
    flex: 1;
    min-height: 44px;
    max-height: 90px;
    resize: vertical;
    padding: 7px 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
    line-height: 1.35;
  }

  .ai-composer textarea:focus {
    outline: 0;
    border-color: var(--accent-line);
    background: var(--surface);
  }

  .ai-composer .primary {
    display: grid;
    place-items: center;
    width: 36px;
    min-width: 36px;
    height: 36px;
    padding: 0;
    border-radius: 50%;
  }

  .ai-clear {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.58rem;
    text-decoration: underline;
  }

  .ai-offline {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    padding: 22px 14px;
    text-align: center;
  }

  .ai-offline-icon {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .ai-offline strong {
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 500;
  }

  .ai-offline p {
    max-width: 28ch;
    margin: 0;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.45;
  }

  @media (max-width: 900px) {
    .ai-panel {
      top: auto;
      right: 0;
      bottom: calc(64px + env(safe-area-inset-bottom));
      left: 0;
      width: 100%;
      max-height: 68vh;
      border-radius: 20px 20px 0 0;
    }

    .ai-tools {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .ai-chat {
      max-height: 30vh;
    }
  }

  .settings .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 3px;
    padding: 3px;
    border-radius: 8px;
    background: var(--surface-2);
  }

  .settings .segmented button {
    min-height: 30px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.68rem;
    font-weight: 600;
  }

  .settings .segmented button.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .settings .check-row {
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;
  }

  .settings .check-row input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .settings .check-row span {
    color: var(--text);
    font-size: 0.76rem;
  }

  .modal-head {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    background: color-mix(in srgb, var(--surface) 96%, transparent);
  }

  .modal-heading {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .modal-mark {
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

  .modal-heading h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.06rem;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .modal-heading p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.64rem;
    line-height: 1.3;
  }

  .modal-close {
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

  .modal-close:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .modal-tabs {
    display: flex;
    flex: 0 0 auto;
    gap: 6px;
    overflow-x: auto;
    padding: 10px 14px 7px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    scrollbar-width: none;
  }

  .modal-tabs::-webkit-scrollbar {
    display: none;
  }

  .modal-tabs button {
    flex: 0 0 auto;
    min-height: 31px;
    padding: 0.28rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.66rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .modal-tabs button.active {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .modal-search {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 6px;
    margin: 10px 14px 2px;
    padding: 4px 4px 4px 10px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .modal-search:focus-within {
    border-color: var(--accent-line);
    background: var(--surface);
  }

  .modal-search input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  .modal-search input:focus {
    outline: 0;
  }

  .search-count {
    flex: 0 0 auto;
    padding: 2px 6px;
    border-radius: 6px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.58rem;
    font-weight: 650;
    white-space: nowrap;
  }

  .search-progress {
    height: 3px;
    margin: 7px 14px 0;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .search-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 120ms ease;
  }

  .search-error {
    margin: 7px 14px 0;
    color: var(--warn);
    font-size: 0.68rem;
  }

  .modal-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 14px 12px;
    overscroll-behavior: contain;
  }

  .toc-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .toc-list button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    text-align: left;
    transition:
      border-color 150ms ease,
      background 150ms ease;
  }

  .toc-list button:hover {
    border-color: var(--accent-line);
  }

  .toc-list button.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .toc-number {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.64rem;
    font-weight: 650;
  }

  .toc-list button.active .toc-number {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .toc-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .toc-copy strong {
    overflow: hidden;
    font-size: 0.78rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toc-copy small {
    color: var(--muted);
    font-size: 0.6rem;
  }

  .toc-badge {
    flex: 0 0 auto;
    padding: 2px 6px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-contrast);
    font-size: 0.54rem;
    font-weight: 650;
  }

  .search-results {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .search-results button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    text-align: left;
    transition:
      border-color 150ms ease,
      background 150ms ease;
  }

  .search-results button:hover {
    border-color: var(--accent-line);
    background: color-mix(in srgb, var(--accent) 4%, var(--surface));
  }

  .result-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
  }

  .result-chapter {
    padding: 2px 6px;
    border-radius: 6px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.58rem;
    font-weight: 650;
  }

  .result-current {
    padding: 2px 6px;
    border-radius: 6px;
    border: 1px solid var(--accent-line);
    color: var(--accent);
    font-size: 0.54rem;
    font-weight: 600;
  }

  .result-snippet {
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.8rem;
    line-height: 1.5;
  }

  .result-open {
    align-self: flex-end;
    color: var(--accent);
    font-size: 0.6rem;
    font-weight: 650;
  }

  .modal-foot {
    flex: 0 0 auto;
    padding: 10px 14px calc(10px + env(safe-area-inset-bottom));
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: color-mix(in srgb, var(--surface-2) 70%, var(--surface));
  }

  .page-jump {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 0.64rem;
  }

  .page-jump > span {
    flex: 1;
    min-width: 0;
  }

  .page-jump input {
    width: 58px;
    min-height: 32px;
    padding: 0.2rem 0.3rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 650;
    text-align: center;
  }

  .page-jump small {
    font-size: 0.6rem;
    white-space: nowrap;
  }

  .navigation-panel {
    top: calc(var(--topbar-height) + 56px);
    left: 50%;
    right: auto;
    display: flex;
    flex-direction: column;
    width: min(560px, calc(100vw - 32px));
    height: min(84vh, 46rem);
    max-height: min(84vh, 46rem);
    padding: 0;
    overflow: hidden;
    border-radius: 24px;
    transform: translateX(-50%);
  }

  .tts-sleep {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0.2rem 0.35rem;
    border-radius: 7px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.65rem;
  }

  .tts-sleep select {
    min-height: 26px;
    padding: 0 0.2rem;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.65rem;
  }

  .tts-sleep strong {
    padding-left: 3px;
    color: var(--accent);
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
  }

  .sleep-notice {
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 600;
  }

  @media (max-width: 900px) {
    .book-search {
      top: auto;
      bottom: calc(64px + env(safe-area-inset-bottom));
      left: 0;
      right: 0;
      width: 100%;
      max-height: 70vh;
      border-radius: 20px 20px 0 0;
      transform: none;
    }
  }

  .surface.dim-images :global(img) {
    filter: brightness(0.72) sepia(0.12) saturate(0.9);
  }

  .focus-ruler {
    position: fixed;
    z-index: 18;
    height: 2.4em;
    border-top: 1px solid color-mix(in srgb, var(--accent-2) 45%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--accent-2) 45%, transparent);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 4%, transparent),
      color-mix(in srgb, var(--accent) 8%, transparent)
    );
    pointer-events: none;
  }

  .settings .apply-all {
    justify-content: center;
    width: 100%;
    margin-top: 3px;
  }

  .mobile-reading-status {
    display: none;
  }

  @media (max-width: 900px) {
    .mobile-reading-status {
      display: none;
    }

    .mobile-reading-copy {
      display: flex;
      align-items: center;
      gap: 5px;
      min-width: 0;
      color: var(--muted);
      font-size: 0.62rem;
      font-weight: 550;
    }

    .mobile-reading-copy :global(svg) {
      flex: 0 0 auto;
      color: var(--accent);
    }

    .mobile-reading-copy span {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .mobile-reading-status strong {
      color: var(--accent);
      font-size: 0.6rem;
      font-weight: 650;
      white-space: nowrap;
    }

    .mobile-reading-track {
      grid-column: 1 / -1;
      height: 3px;
      overflow: hidden;
      border-radius: 999px;
      background: var(--surface-3);
    }

    .mobile-reading-track span {
      display: block;
      height: 100%;
      border-radius: inherit;
      background: linear-gradient(90deg, var(--accent), var(--accent-2));
      transition: width 160ms ease;
    }

    .controls {
      max-width: none;
      width: 100%;
      overflow-x: auto;
      padding: 3px;
      scrollbar-width: none;
    }

    .controls::-webkit-scrollbar {
      display: none;
    }

    .controls button {
      width: 40px;
      min-width: 40px;
      height: 40px;
      min-height: 40px;
      border-radius: 10px;
    }

    .controls .control-fullscreen {
      display: none;
    }
  }

  @media (max-width: 900px) {
    .navigation-panel {
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      height: 88vh;
      max-height: 88vh;
      border-bottom: 0;
      border-radius: 22px 22px 0 0;
      transform: none;
    }

    .navigation-panel::before {
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

    .navigation-panel .modal-head {
      padding-top: 20px;
    }
  }

  .term-panel {
    top: calc(var(--topbar-height) + 56px);
    left: 50%;
    right: auto;
    display: flex;
    flex-direction: column;
    width: min(540px, calc(100vw - 32px));
    max-height: min(82vh, 44rem);
    padding: 0;
    overflow: hidden;
    border-radius: 24px;
    transform: translateX(-50%);
  }

  .term-query {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    margin: 12px 16px 2px;
    padding: 9px 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
  }

  .term-query strong {
    font-family: var(--font-display);
    font-size: 1.08rem;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .term-query span {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.6rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .term-result,
  .term-context {
    margin: 0;
    padding: 2px 2px 8px;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.84rem;
    line-height: 1.65;
    white-space: pre-wrap;
  }

  .term-context {
    padding: 10px 12px;
    border-left: 3px solid var(--accent);
    border-radius: 0 10px 10px 0;
    background: color-mix(in srgb, var(--accent) 5%, var(--surface));
    font-style: italic;
  }

  .term-notice {
    margin: 8px 2px 0;
    color: var(--accent);
    font-size: 0.66rem;
    font-weight: 650;
  }

  .term-foot {
    display: flex;
    justify-content: flex-end;
    gap: 7px;
  }

  @media (max-width: 900px) {
    .term-panel {
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      max-height: 88vh;
      border-bottom: 0;
      border-radius: 22px 22px 0 0;
      transform: none;
    }

    .term-panel::before {
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

    .term-panel .modal-head {
      padding-top: 20px;
    }
  }

  .practice-metrics-panel {
    width: min(32rem, calc(100vw - 2rem));
    max-height: min(86vh, 50rem);
    padding: 0 0 14px;
    overflow-y: auto;
    border-radius: 22px;
  }

  .practice-metrics-panel .modal-head {
    position: sticky;
    top: 0;
    z-index: 2;
    margin: 0;
    border-radius: 22px 22px 0 0;
    background: var(--surface);
  }

  .practice-metrics {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    padding: 12px 16px 4px;
  }

  .metric-card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface-2);
  }

  .metric-card > span,
  .practice-passage .quote-label,
  .practice-result .quote-label {
    color: var(--muted);
    font-size: 0.56rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .metric-card strong {
    color: var(--accent);
    font-family: var(--font-sans);
    font-size: 1.75rem;
    font-weight: 650;
    line-height: 1.1;
  }

  .metric-card small {
    color: var(--muted);
    font-size: 0.6rem;
  }

  .practice-passage {
    margin: 8px 16px 0;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border));
    border-left: 3px solid var(--accent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 4%, var(--surface));
  }

  .practice-passage blockquote {
    margin: 5px 0 0;
    padding: 0;
    border: 0;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.84rem;
    font-style: italic;
    line-height: 1.55;
    max-height: 8rem;
    overflow-y: auto;
  }

  .practice-metrics-panel > .hint {
    padding: 0 16px;
  }

  .practice-metrics-panel > .practice-row {
    padding: 0 16px;
  }

  .practice-metrics-panel .practice-result {
    margin: 12px 16px 0;
  }

  .practice-metrics-panel .transcript {
    margin: 5px 0 8px;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font-style: italic;
  }

  .practice-session-meta {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 0.68rem;
  }

  @media (max-width: 900px) {
    .practice-metrics-panel {
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      max-height: 90vh;
      border-bottom: 0;
      border-radius: 22px 22px 0 0;
    }

    .practice-metrics-panel::before {
      content: "";
      position: absolute;
      top: 7px;
      left: 50%;
      z-index: 3;
      width: 38px;
      height: 4px;
      border-radius: 999px;
      background: var(--border-strong);
      transform: translateX(-50%);
    }

    .practice-metrics-panel .modal-head {
      padding-top: 20px;
    }
  }

  .note-style {
    width: min(32rem, calc(100vw - 2rem));
    max-height: min(86vh, 48rem);
    padding: 0;
    overflow-y: auto;
    border-radius: 22px;
  }

  .note-style .modal-head {
    position: sticky;
    top: 0;
    z-index: 2;
    margin: 0;
    border-radius: 22px 22px 0 0;
    background: var(--surface);
  }

  .note-style .quote-card {
    margin: 12px 16px 4px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border));
    border-left: 3px solid var(--accent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 4%, var(--surface));
  }

  .note-style .quote-label {
    display: block;
    margin-bottom: 4px;
    color: var(--accent);
    font-size: 0.56rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .note-style blockquote {
    margin: 0;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.85rem;
    font-style: italic;
    line-height: 1.55;
  }

  .note-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin: 10px 16px 0;
    color: var(--muted);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .note-field textarea {
    min-height: 7.5rem;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.85rem;
    font-weight: 400;
    line-height: 1.55;
    letter-spacing: 0;
    text-transform: none;
    resize: vertical;
  }

  .note-field textarea:focus {
    outline: 2px solid color-mix(in srgb, var(--accent) 22%, transparent);
    border-color: var(--accent-line);
  }

  .color-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 12px 16px;
    color: var(--muted);
    font-size: 0.64rem;
    font-weight: 600;
  }

  .color-row .colors {
    display: flex;
    gap: 8px;
  }

  .color-row .swatch {
    width: 26px;
    height: 26px;
    border: 2px solid var(--surface);
    box-shadow: 0 0 0 1px var(--border);
  }

  .color-row .swatch.selected {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .note-style .editor-actions {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 10px 16px calc(10px + env(safe-area-inset-bottom));
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: color-mix(in srgb, var(--surface-2) 70%, var(--surface));
  }

  .note-style .editor-actions .primary {
    flex: 1;
    justify-content: center;
    min-height: 40px;
    border-radius: 12px;
  }

  .note-style .editor-actions .danger {
    min-height: 40px;
    border-radius: 12px;
  }

  @media (max-width: 900px) {
    .note-style {
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      max-height: 92vh;
      border-bottom: 0;
      border-radius: 22px 22px 0 0;
    }

    .note-style::before {
      content: "";
      position: absolute;
      top: 7px;
      left: 50%;
      z-index: 3;
      width: 38px;
      height: 4px;
      border-radius: 999px;
      background: var(--border-strong);
      transform: translateX(-50%);
    }

    .note-style .modal-head {
      padding-top: 20px;
    }
  }

  .settings-panel {
    width: min(30rem, calc(100vw - 2rem));
    max-height: min(88vh, 50rem);
    padding: 0 14px 14px;
    border-radius: 22px;
  }

  .settings-panel .modal-head {
    position: sticky;
    top: 0;
    z-index: 2;
    margin: 0 -14px 4px;
    border-radius: 22px 22px 0 0;
    background: var(--surface);
  }

  @media (max-width: 900px) {
    .settings-panel {
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      max-height: 90vh;
      border-bottom: 0;
      border-radius: 22px 22px 0 0;
    }

    .settings-panel::before {
      content: "";
      position: absolute;
      top: 7px;
      left: 50%;
      z-index: 3;
      width: 38px;
      height: 4px;
      border-radius: 999px;
      background: var(--border-strong);
      transform: translateX(-50%);
    }

    .settings-panel .modal-head {
      padding-top: 20px;
    }
  }

  .navigation-panel .modal-head,
  .term-panel .modal-head {
    position: sticky;
    top: 0;
    z-index: 2;
    margin: 0;
    border-radius: 24px 24px 0 0;
    background: var(--surface);
  }

  .navigation-panel,
  .term-panel,
  .practice-metrics-panel,
  .note-style,
  .settings-panel,
  .annotation-editor {
    z-index: 70;
  }

  @media (max-width: 900px) {
    .navigation-panel .modal-head,
    .term-panel .modal-head {
      border-radius: 22px 22px 0 0;
    }
  }

  .panel-backdrop {
    position: fixed;
    inset: 0;
    z-index: 39;
    padding: 0;
    border: 0;
    background: color-mix(in srgb, #1a1008 38%, transparent);
    backdrop-filter: blur(2px);
  }

  .mobile-reading-score {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .mobile-reading-metrics {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--accent);
  }
</style>

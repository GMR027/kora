<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import AudiobookAlbumAddModal from "../lib/components/AudiobookAlbumAddModal.svelte";
  import AudiobookAlbumCard from "../lib/components/AudiobookAlbumCard.svelte";
  import AudiobookAlbumEditorModal from "../lib/components/AudiobookAlbumEditorModal.svelte";
  import AudiobookAlbumModal from "../lib/components/AudiobookAlbumModal.svelte";
  import AudiobookAlbumPickerModal from "../lib/components/AudiobookAlbumPickerModal.svelte";
  import AudiobookCard from "../lib/components/AudiobookCard.svelte";
  import AudiobookChapterEditor from "../lib/components/AudiobookChapterEditor.svelte";
  import AudiobookChaptersModal from "../lib/components/AudiobookChaptersModal.svelte";
  import AudiobookGeneratorModal from "../lib/components/AudiobookGeneratorModal.svelte";
  import AudiobookImportModal from "../lib/components/AudiobookImportModal.svelte";
  import AudiobookKaraokeModal from "../lib/components/AudiobookKaraokeModal.svelte";
  import AudiobookShareModal from "../lib/components/AudiobookShareModal.svelte";
  import AudiobookTranscribeModal from "../lib/components/AudiobookTranscribeModal.svelte";
  import AudiobookTreatmentModal from "../lib/components/AudiobookTreatmentModal.svelte";
  import AudiobookVoiceMarksModal from "../lib/components/AudiobookVoiceMarksModal.svelte";
  import Button from "../lib/components/Button.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import { audiobookPlayerHidden } from "../lib/playerUi";
  import {
    addAudiobooksToAlbum,
    applyAudiobookTreatment,
    audiobookCapabilities,
    audiobookChapters,
    audiobookKaraoke,
    audiobookLocator,
    audiobookQueueStatus,
    audiobookSetFavorite,
    audiobookVoiceMarkers,
    audiobookWaveform,
    aiStatus,
    cancelAudiobookGeneration,
    categoryList,
    clearAudiobookQueue,
    createAudiobookAlbum,
    createAudiobookShare,
    deleteAnnotation,
    deleteAudiobook,
    deleteAudiobookAlbum,
    enqueueAudiobook,
    errorMessage,
    exportAudiobook,
    exportBook,
    getAudiobookProgress,
    importAudioAudiobook,
    importExternalAudiobook,
    libraryClassify,
    libraryList,
    listAllExtracts,
    listAudiobookAlbums,
    listAudiobooks,
    moveQueuedAudiobook,
    pauseAudiobook,
    playAudiobook,
    practiceDownloadModel,
    practiceModels,
    pauseAudiobookGeneration,
    readerDeleteExtract,
    readerOpen,
    reanalyzeAudiobookChapters,
    removeAudiobookFromAlbum,
    removeQueuedAudiobook,
    reorderAudiobookAlbum,
    resumeAudiobookGeneration,
    retryAudiobook,
    saveVoiceMarkerPhrase,
    seekAudiobook,
    sessionEnd,
    sessionStart,
    setAudiobookSpeed,
    setAudiobookKeepScreenOn,
    setAudiobookSleepTimer,
    startShare,
    startVoiceMarkerListening,
    stopAudiobook,
    stopVoiceMarkerListening,
    transcribeAudiobookLater,
    ttsEngines,
    ttsVoices,
    updateAudiobookAlbum,
    updateAudiobookChapters,
    voiceMarkerStatus,
    weaveExtracts,
  } from "../lib/api";
  import {
    formatBytes,
    formatDuration,
    t,
    translate,
    type MessageKey,
  } from "../lib/i18n";
  import {
    discardImportSource,
    probeAndroidAudio,
    stageImportSource,
  } from "../lib/importSource";
  import {
    advanceAlbumPlayback,
    albumPlayback,
    clearAlbumQueue,
    startAlbumQueue,
  } from "../lib/albumQueue";
  import {
    lineIndexAt,
    prepareKaraoke,
    wordIndexAt,
  } from "../lib/karaoke";
  import { notifyError, notifySuccess } from "../lib/toast";
  import type { AppTab } from "../lib/navigation";
  import type {
    AiStatus,
    AndroidAudioMetadata,
    AudioImportMode,
    AudioImportOptions,
    AudioTreatment,
    AudioWaveform,
    AudiobookActiveGeneration,
    AudiobookAlbumInfo,
    AudiobookCapabilities,
    AudiobookCastInput,
    AudiobookChapterInfo,
    AudiobookGenerationView,
    AudiobookInfo,
    AudiobookProgressEvent,
    AudiobookQueueItem,
    AudiobookScope,
    BookSummary,
    Category,
    ChapterEditInput,
    ChapterKaraoke,
    EngineInfo,
    Extract,
    KaraokePrecision,
    Locator,
    ModelDownloadProgress,
    NarrationSettings,
    PlaybackSnapshot,
    SharePermission,
    ShareView,
    VoiceDescriptor,
    VoiceMarkerStatus,
    VoiceNote,
    WhisperModelInfo,
  } from "../lib/types";

  let {
    onRead,
    onNavigate,
    initialBookId = "",
    autoOpenGenerator = false,
    onIntentConsumed,
  }: {
    onRead?: (bookId: string, locator?: Locator) => void;
    onNavigate?: (tab: AppTab) => void;
    initialBookId?: string;
    autoOpenGenerator?: boolean;
    onIntentConsumed?: () => void;
  } = $props();

  type FilterId =
    | "all"
    | "favorites"
    | "generated"
    | "external"
    | "tejido"
    | "processing";
  type SortId = "recent" | "title" | "duration" | "progress";

  let books = $state<BookSummary[]>([]);
  let voices = $state<VoiceDescriptor[]>([]);
  let engines = $state<EngineInfo[]>([]);
  let audiobooks = $state<AudiobookInfo[]>([]);
  let albums = $state<AudiobookAlbumInfo[]>([]);
  let categories = $state<Category[]>([]);
  let extracts = $state<Extract[]>([]);
  let viewMode = $state<"audiobooks" | "albums">("audiobooks");
  let albumQuery = $state("");
  let albumEditorOpen = $state(false);
  let albumEditing = $state<AudiobookAlbumInfo | null>(null);
  let albumEditorBusy = $state(false);
  let albumEditorError = $state<string | null>(null);
  let albumDetailId = $state<string | null>(null);
  let albumAddOpen = $state(false);
  let albumBusy = $state<string | null>(null);
  let albumPickerFor = $state<AudiobookInfo | null>(null);
  let albumPickerBusy = $state(false);
  let albumPickerError = $state<string | null>(null);
  let convertItem = $state<AudiobookInfo | null>(null);
  let convertBusy = $state(false);
  let intentHandled = false;
  let playback = $state<PlaybackSnapshot | null>(null);
  let progress = $state<AudiobookProgressEvent | null>(null);
  let generationActive = $state<AudiobookActiveGeneration | null>(null);
  let queue = $state<AudiobookQueueItem[]>([]);
  let generationPaused = $state(false);
  let cancelling = $state(false);
  let capabilities = $state<AudiobookCapabilities | null>(null);
  let aiState = $state<AiStatus | null>(null);
  let whisperModels = $state<WhisperModelInfo[]>([]);
  let whisperProgress = $state<Record<string, number>>({});

  let sections = $state<{ index: number; title: string }[]>([]);
  let chapters = $state<AudiobookChapterInfo[]>([]);
  let modalChapters = $state<AudiobookChapterInfo[]>([]);
  let modalChapterId = $state<string | null>(null);
  let modalChaptersLoading = $state(false);
  let speed = $state(1);
  let chapterAudioId: string | null = null;

  let selectedBookId = $state("");
  let voiceId = $state("");

  let weaveTitle = $state("");
  let selectedExtracts = $state<string[]>([]);
  let extractFilter = $state("");

  let filter = $state<FilterId>("all");
  let categoryFilter = $state<string>("all");
  let classifyingId = $state<string | null>(null);
  let query = $state("");
  let sort = $state<SortId>("recent");

  let generateOpen = $state(false);
  let importOpen = $state(false);
  let importInitialKind = $state<"audio" | "transcribe">("audio");
  let chaptersOpen = $state(false);
  let chapterEditorOpen = $state(false);
  let chapterEditorId = $state<string | null>(null);
  let chapterEditorChapters = $state<ChapterEditInput[]>([]);
  let savingChapters = $state(false);
  let chapterEditorError = $state<string | null>(null);
  let treatmentOpen = $state(false);
  let treatmentInitialId = $state<string | null>(null);
  let treating = $state(false);

  let shareOpen = $state(false);
  let shareItem = $state<AudiobookInfo | null>(null);
  let shareResult = $state<ShareView | null>(null);
  let shareError = $state<string | null>(null);
  let sharing = $state(false);
  let favoriteBusy = $state<string | null>(null);

  let voiceMarksOpen = $state(false);
  let voiceMarkers = $state<VoiceNote[]>([]);
  let voiceMarkersFor = $state<string | null>(null);
  let voiceMarkersLoading = $state(false);
  let voiceStatus = $state<VoiceMarkerStatus | null>(null);
  let voiceToggling = $state(false);

  let waveform = $state<AudioWaveform | null>(null);
  let waveformKey = $state<string | null>(null);
  let waveformLoading = $state(false);

  let karaoke = $state<ChapterKaraoke | null>(null);
  let karaokeKey = $state<string | null>(null);
  let karaokeLoading = $state(false);
  let karaokeOpen = $state(false);
  let playbackAt = $state(Date.now());
  let frameClock = $state(Date.now());
  let playerExpanded = $state(false);
  let sheetDrag = $state(0);
  let sheetDragging = $state(false);
  let sheetDragStart = 0;

  const KARAOKE_PRECISION_KEYS: Record<KaraokePrecision, MessageKey> = {
    aligned: "audiobooks.karaokeAligned",
    timed: "audiobooks.karaokeTimed",
    estimated: "audiobooks.karaokeEstimated",
  };

  let busy = $state<string | null>(null);
  let downloadingId = $state<string | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  let scrubbing = $state(false);
  let seekValue = $state(0);
  let unlisten: UnlistenFn | undefined;
  let unlistenDownload: UnlistenFn | undefined;
  let unlistenQueue: UnlistenFn | undefined;
  let unlistenVoice: UnlistenFn | undefined;
  let unlistenVoiceError: UnlistenFn | undefined;
  let progressHideTimer: ReturnType<typeof setTimeout> | undefined;
  let listeningSessionId: string | null = null;
  let listeningBookId: string | null = null;

  let sleepMinutes = $state(0);
  let sleepRemaining = $state(0);
  let sleepTimer: ReturnType<typeof setInterval> | undefined;
  let keepScreenOn = $state(false);

  let hiddenProgress = $state(false);
  let activeTaskStartedAt = $state<number | null>(null);
  let activeTaskTitleOverride = $state<string | null>(null);
  let activeTaskVoiceOverride = $state<string | null>(null);
  let activeTaskCoverOverride = $state<string | null>(null);
  let clock = $state(Date.now());

  let installedVoices = $derived(voices.filter((voice) => voice.installed));
  let booksById = $derived(new Map(books.map((book) => [book.id, book])));
  let audiobooksById = $derived(new Map(audiobooks.map((item) => [item.id, item])));
  let albumDetail = $derived(
    albums.find((album) => album.id === albumDetailId) ?? null,
  );
  let playingAlbumId = $derived($albumPlayback?.albumId ?? null);
  let filteredAlbums = $derived.by(() => {
    const needle = albumQuery.trim().toLowerCase();
    if (!needle) return albums;
    return albums.filter((album) =>
      `${album.title} ${album.description ?? ""}`
        .toLowerCase()
        .includes(needle),
    );
  });

  function albumItems(album: AudiobookAlbumInfo): AudiobookInfo[] {
    return album.audiobookIds
      .map((id) => audiobooksById.get(id))
      .filter((item): item is AudiobookInfo => Boolean(item));
  }

  function albumCovers(album: AudiobookAlbumInfo): (string | null)[] {
    return albumItems(album).map((item) => item.coverPath);
  }

  function albumDuration(album: AudiobookAlbumInfo): number {
    return albumItems(album).reduce(
      (sum, item) => sum + Math.max(0, item.durationMs),
      0,
    );
  }

  function albumPercent(album: AudiobookAlbumInfo): number {
    const items = albumItems(album);
    const total = items.reduce((sum, item) => sum + Math.max(0, item.durationMs), 0);
    if (total <= 0) return 0;
    const listened = items.reduce(
      (sum, item) =>
        sum + Math.min(1, Math.max(0, item.percent)) * Math.max(0, item.durationMs),
      0,
    );
    return listened / total;
  }

  function playableAlbumItems(album: AudiobookAlbumInfo): AudiobookInfo[] {
    return albumItems(album).filter((item) => item.status === "done");
  }

  let availableEngines = $derived(engines.filter((engine) => engine.available));
  let engineLabel = $derived(
    availableEngines.length > 0
      ? availableEngines.map((engine) => engine.name).join(" · ")
      : $t("audiobooks.engineUnavailable"),
  );
  let platformSupported = $derived(capabilities?.platformSupported ?? true);
  let generationSupported = $derived(
    capabilities?.generationSupported ??
      ((capabilities?.platformSupported ?? true) &&
        (capabilities?.ffmpeg ?? true)),
  );
  let nativeMux = $derived(capabilities?.nativeMux ?? false);
  let audioImportSupported = $derived(
    capabilities?.audioImportSupported ?? true,
  );
  let nativeAudioImport = $derived(capabilities?.nativeAudioImport ?? false);
  let transcriptionSupported = $derived(
    capabilities?.transcriptionSupported ?? true,
  );
  let backgroundPlayback = $derived(capabilities?.backgroundPlayback ?? false);
  let ffmpegReady = $derived(capabilities?.ffmpeg ?? true);
  let whisperBase = $derived(
    whisperModels.find((model) => model.id === "base") ?? null,
  );
  let whisperTiny = $derived(
    whisperModels.find((model) => model.id === "tiny") ?? null,
  );
  let whisperDownloadPercent = $derived(whisperProgress["base"] ?? null);
  let whisperTinyDownloadPercent = $derived(whisperProgress["tiny"] ?? null);
  let voiceListening = $derived(voiceStatus?.listening ?? false);
  let voiceSupported = $derived(voiceStatus?.supported ?? true);
  let voiceModelReady = $derived(voiceStatus?.modelReady ?? false);
  let voicePhrase = $derived(
    voiceStatus?.phrase?.trim() || translate("audiobooks.voiceCommandDefault"),
  );
  let voiceModelLabel = $derived(
    voiceStatus?.modelId
      ? `Whisper ${voiceStatus.modelId.charAt(0).toUpperCase()}${voiceStatus.modelId.slice(1)}`
      : "Whisper",
  );
  let voiceMarkersCount = $derived(voiceMarkers.length);
  let voiceDownloadProgress = $derived(
    voiceModelReady ? null : whisperProgress["tiny"] ?? whisperProgress["base"] ?? null,
  );

  let filterTabs = $derived.by((): { id: FilterId; label: string; count: number }[] => [
    { id: "all", label: $t("audiobooks.filterAll"), count: audiobooks.length },
    {
      id: "favorites",
      label: $t("audiobooks.filterFavorites"),
      count: audiobooks.filter((item) => item.favorite).length,
    },
    {
      id: "generated",
      label: $t("audiobooks.filterGenerated"),
      count: audiobooks.filter((item) => item.origin === "generated").length,
    },
    {
      id: "external",
      label: $t("audiobooks.filterImported"),
      count: audiobooks.filter((item) => item.origin === "external").length,
    },
    {
      id: "tejido",
      label: $t("audiobooks.filterTejido"),
      count: audiobooks.filter((item) => item.origin === "tejido").length,
    },
    {
      id: "processing",
      label: $t("audiobooks.filterProcessing"),
      count: audiobooks.filter(
        (item) => item.status === "pending" || item.status === "running",
      ).length,
    },
  ]);

  let filteredAudiobooks = $derived.by(() => {
    let result = audiobooks;
    if (filter === "favorites") {
      result = result.filter((item) => item.favorite);
    } else if (filter === "processing") {
      result = result.filter(
        (item) => item.status === "pending" || item.status === "running",
      );
    } else if (filter !== "all") {
      result = result.filter((item) => item.origin === filter);
    }
    if (categoryFilter !== "all") {
      result = result.filter((item) =>
        item.categoryIds.includes(categoryFilter),
      );
    }

    const needle = query.trim().toLowerCase();
    if (needle) {
      result = result.filter((item) =>
        `${item.bookTitle} ${item.author ?? ""} ${item.voiceName ?? ""}`
          .toLowerCase()
          .includes(needle),
      );
    }

    if (sort === "title") {
      result = [...result].sort((left, right) =>
        left.bookTitle.localeCompare(right.bookTitle),
      );
    } else if (sort === "duration") {
      result = [...result].sort((left, right) => right.durationMs - left.durationMs);
    } else if (sort === "progress") {
      result = [...result].sort((left, right) => right.percent - left.percent);
    } else {
      result = [...result].sort(
        (left, right) =>
          new Date(right.createdAt).getTime() - new Date(left.createdAt).getTime(),
      );
    }
    return result;
  });

  let filteredExtracts = $derived(
    extracts.filter((extract) =>
      extractFilter.trim()
        ? `${extract.text} ${extract.note ?? ""}`
            .toLowerCase()
            .includes(extractFilter.trim().toLowerCase())
        : true,
    ),
  );

  let playerItem = $derived(
    playback
      ? (audiobooks.find((item) => item.id === playback?.audiobookId) ?? null)
      : null,
  );
  let playerCover = $derived(
    playerItem
      ? (playerItem.coverPath ?? booksById.get(playerItem.bookId)?.coverPath ?? null)
      : null,
  );
  let waveformPercent = $derived.by(() => {
    if (!playback || playback.durationMs <= 0) return 0;
    const position = scrubbing ? seekValue : playback.positionMs;
    return Math.min(100, Math.max(0, (position / playback.durationMs) * 100));
  });
  let playheadMs = $derived.by(() => {
    if (!playback) return 0;
    if (!playback.playing) return playback.positionMs;
    const elapsed = Math.max(0, frameClock - playbackAt);
    return Math.min(playback.durationMs, playback.positionMs + elapsed * speed);
  });
  let karaokePrepared = $derived(
    karaoke ? prepareKaraoke(karaoke.lines) : [],
  );
  let karaokePositionMs = $derived(
    karaoke ? Math.max(0, playheadMs - karaoke.startMs) : 0,
  );
  let karaokeLineIndex = $derived(
    lineIndexAt(karaokePrepared, karaokePositionMs),
  );
  let karaokeLine = $derived(
    karaokeLineIndex >= 0
      ? (karaokePrepared[karaokeLineIndex] ?? null)
      : null,
  );
  let karaokeWordIndex = $derived(
    wordIndexAt(karaokeLine, karaokePositionMs),
  );
  let karaokePrevious = $derived(
    karaokeLineIndex > 0 ? karaokePrepared[karaokeLineIndex - 1] : null,
  );
  let karaokeNext = $derived(
    karaokeLineIndex >= 0
      ? (karaokePrepared[karaokeLineIndex + 1] ?? null)
      : null,
  );
  let karaokePrecisionKey = $derived(
    karaoke ? KARAOKE_PRECISION_KEYS[karaoke.precision] : null,
  );

  let chapterEditorItem = $derived(
    chapterEditorId
      ? (audiobooks.find((item) => item.id === chapterEditorId) ?? null)
      : null,
  );
  let chapterEditorPlayhead = $derived(
    playback && chapterEditorId && playback.audiobookId === chapterEditorId
      ? playback.positionMs
      : null,
  );

  let activeTaskItem = $derived(
    progress?.audiobookId
      ? (audiobooks.find((item) => item.id === progress?.audiobookId) ?? null)
      : null,
  );
  let activeTaskCover = $derived(
    activeTaskItem?.coverPath ?? activeTaskCoverOverride,
  );
  let activeTaskTitle = $derived(
    activeTaskItem?.bookTitle ??
      activeTaskTitleOverride ??
      booksById.get(selectedBookId)?.title ??
      $t("audiobooks.title"),
  );
  let activeTaskVoice = $derived(
    activeTaskItem?.voiceName ??
      activeTaskVoiceOverride ??
      installedVoices.find((voice) => voice.id === voiceId)?.name ??
      "",
  );
  let progressPercent = $derived(
    Math.round(Math.min(100, Math.max(0, progress?.percent ?? 0))),
  );
  let elapsedSeconds = $derived(
    activeTaskStartedAt
      ? Math.max(0, Math.floor((clock - activeTaskStartedAt) / 1000))
      : 0,
  );
  let etaSeconds = $derived.by(() => {
    if (
      !progress ||
      !activeTaskStartedAt ||
      progress.percent < 3 ||
      progress.percent >= 100
    ) {
      return 0;
    }
    const elapsed = Math.max(0, (clock - activeTaskStartedAt) / 1000);
    return Math.max(
      0,
      Math.round((elapsed * (100 - progress.percent)) / progress.percent),
    );
  });
  let activeTaskVisible = $derived(progress !== null && !hiddenProgress);
  let canControlGeneration = $derived(
    generationActive !== null ||
      busy === "import" ||
      busy === "reanalyze" ||
      convertBusy,
  );
  let isGenerationPaused = $derived(
    generationPaused || generationActive?.paused === true,
  );

  onMount(() => {
    try {
      keepScreenOn = localStorage.getItem("kora.audiobook.keepScreenOn") === "true";
    } catch {
      keepScreenOn = false;
    }
    if (keepScreenOn) {
      void setAudiobookKeepScreenOn(true).catch(() => undefined);
    }
    const onVisibility = () => {
      if (document.visibilityState === "visible") void pollPlayback();
    };
    document.addEventListener("visibilitychange", onVisibility);
    void load();
    void initProgress().catch((cause) => {
      error = errorMessage(cause);
    });
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      unlisten?.();
      unlistenDownload?.();
      unlistenQueue?.();
      unlistenVoice?.();
      unlistenVoiceError?.();
      if (progressHideTimer) clearTimeout(progressHideTimer);
      clearInterval(sleepTimer);
      void endListeningSession();
      void stopVoiceMarkerListening().catch(() => undefined);
    };
  });

  // Sondeo y animación solo mientras suena audio. En pausa el estado ya llega
  // con la respuesta de pause/seek.
  $effect(() => {
    if (!playback?.playing) return;
    const timer = setTimeout(() => void pollPlayback(), 900);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    if (!playback?.playing) return;
    const timer = setInterval(() => {
      frameClock = Date.now();
    }, 120);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (!(playback && playerExpanded)) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  $effect(() => {
    if (!activeTaskStartedAt) return;
    const timer = setInterval(() => {
      clock = Date.now();
    }, 1000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    const id = playback?.audiobookId ?? null;
    if (id === voiceMarkersFor) return;
    voiceMarkersFor = id;
    voiceMarkers = [];
    if (id) void loadVoiceMarkers(id);
  });

  async function endListeningSession() {
    const id = listeningSessionId;
    listeningSessionId = null;
    listeningBookId = null;
    if (!id) return;
    try {
      await sessionEnd(id, null);
    } catch {
      // El diario es best-effort.
    }
  }

  async function startListeningSession(bookId: string) {
    await endListeningSession();
    // Marca el intento para que el sondeo no reintente cada segundo si el
    // diario no está disponible.
    listeningBookId = bookId;
    try {
      const started = await sessionStart(bookId, "listening", null, null);
      listeningSessionId = started.id;
    } catch {
      // El diario es best-effort.
    }
  }

  async function load() {
    try {
      const [
        bookList,
        voiceList,
        engineList,
        audiobookList,
        extractList,
        current,
        caps,
        generation,
        voiceState,
        categoryListResult,
        albumList,
      ] = await Promise.all([
        libraryList(),
        ttsVoices(),
        ttsEngines(),
        listAudiobooks(),
        listAllExtracts(),
        getAudiobookProgress(),
        audiobookCapabilities().catch(() => null),
        audiobookQueueStatus().catch(() => null),
        voiceMarkerStatus().catch(() => null),
        categoryList().catch(() => [] as Category[]),
        listAudiobookAlbums().catch(() => [] as AudiobookAlbumInfo[]),
      ]);
      books = bookList;
      voices = voiceList;
      engines = engineList;
      audiobooks = audiobookList;
      albums = albumList;
      categories = categoryListResult;
      extracts = extractList;
      playback = current;
      playbackAt = Date.now();
      frameClock = playbackAt;
      if (!loaded) {
        playerExpanded = current?.playing ?? false;
      }
      syncKaraoke(current);
      capabilities = caps;
      if (voiceState) voiceStatus = voiceState;
      if (generation) applyGenerationState(generation);
      seekValue = current?.positionMs ?? 0;
      void loadChapters(current?.audiobookId);
      const currentItem = current
        ? (audiobookList.find((item) => item.id === current.audiobookId) ?? null)
        : null;
      void loadWaveform(currentItem);

      const installed = voiceList.filter((voice) => voice.installed);
      if (!installed.some((voice) => voice.id === voiceId)) {
        voiceId = installed[0]?.id ?? "";
      }

      if (bookList.length === 0) {
        selectedBookId = "";
        sections = [];
      } else if (!bookList.some((book) => book.id === selectedBookId)) {
        selectedBookId = bookList[0].id;
        void loadSections(bookList[0].id);
      }
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      loaded = true;
    }
  }

  $effect(() => {
    if (!autoOpenGenerator || intentHandled || !loaded) {
      return;
    }
    intentHandled = true;
    onIntentConsumed?.();
    if (books.length === 0) {
      error = translate("audiobooks.empty");
      return;
    }
    if (!generationSupported) {
      notice = translate("audiobooks.ffmpegMissing");
      return;
    }
    const bookId =
      initialBookId && books.some((book) => book.id === initialBookId)
        ? initialBookId
        : books[0].id;
    selectedBookId = bookId;
    void loadSections(bookId);
    openGenerator();
  });

  function categoryLabelFor(item: AudiobookInfo): string | null {
    const names = item.categoryIds
      .map((id) => categories.find((category) => category.id === id)?.name)
      .filter((name): name is string => Boolean(name));
    return names.length > 0 ? names.slice(0, 2).join(" · ") : null;
  }

  async function classifyAudiobook(item: AudiobookInfo) {
    if (classifyingId) return;
    classifyingId = item.id;
    notice = null;
    error = null;
    try {
      const found = await libraryClassify(item.bookId, true);
      notice =
        found.length > 0
          ? `${translate("classification.detected", { count: found.length })} · ${translate("classification.reviewInLibrary")}`
          : translate("classification.noneDetected");
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      classifyingId = null;
    }
  }

  function applyGenerationState(view: AudiobookGenerationView) {
    generationActive = view.active;
    queue = view.queue;
    generationPaused = view.active?.paused ?? false;
    if (!view.active) return;

    if (activeTaskTitleOverride !== view.active.title) {
      activeTaskCoverOverride = null;
    }
    activeTaskTitleOverride = view.active.title;
    activeTaskVoiceOverride = view.active.voiceName;
    const terminal =
      !progress ||
      progress.phase === "done" ||
      progress.phase === "error" ||
      progress.phase === "cancelled";
    const sameJob =
      Boolean(view.active.audiobookId) &&
      Boolean(progress?.audiobookId) &&
      view.active.audiobookId === progress?.audiobookId;
    if (terminal && !sameJob) {
      progress = {
        audiobookId: view.active.audiobookId ?? "",
        phase: view.active.paused ? "paused" : "synthesizing",
        chapter: 0,
        totalChapters: 0,
        percent: 0,
        message: translate("audiobooks.preparingGeneration"),
      };
      activeTaskStartedAt =
        new Date(view.active.startedAt).getTime() || Date.now();
      hiddenProgress = false;
    }
  }

  async function refreshQueue() {
    try {
      applyGenerationState(await audiobookQueueStatus());
    } catch {
      // La cola no debe romper la vista si la app se está cerrando.
    }
  }

  async function loadWaveform(item: AudiobookInfo | null) {
    if (!item) {
      waveform = null;
      waveformKey = null;
      waveformLoading = false;
      return;
    }
    const key = `${item.id}:${item.audioHash ?? ""}`;
    if (key === waveformKey) return;
    waveformKey = key;
    waveformLoading = true;
    try {
      const data = await audiobookWaveform(item.id, 600);
      if (waveformKey === key) waveform = data;
    } catch {
      if (waveformKey === key) waveform = null;
    } finally {
      if (waveformKey === key) waveformLoading = false;
    }
  }

  function openTreatment(item?: AudiobookInfo | null) {
    error = null;
    notice = null;
    treatmentInitialId =
      item?.id ??
      playback?.audiobookId ??
      audiobooks.find((entry) => entry.status === "done")?.id ??
      null;
    treatmentOpen = true;
  }

  async function applyTreatment(id: string, treatment: AudioTreatment) {
    error = null;
    notice = null;
    treating = true;
    try {
      if (playback?.audiobookId === id) {
        await stopVoiceIfListening();
        await endListeningSession();
        playback = await stopAudiobook();
        audiobookPlayerHidden.set(false);
        playerExpanded = false;
        waveform = null;
        waveformKey = null;
      }
      const updated = await applyAudiobookTreatment(id, treatment);
      treatmentOpen = false;
      notice = translate("audiobooks.treatmentNotice", {
        title: updated.bookTitle,
      });
      await load();
      void loadWaveform(updated);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      treating = false;
    }
  }

  async function toggleGenerationPause() {
    const pausing = !isGenerationPaused;
    try {
      const view = pausing
        ? await pauseAudiobookGeneration()
        : await resumeAudiobookGeneration();
      applyGenerationState(view);
      if (
        progress &&
        progress.phase !== "done" &&
        progress.phase !== "error" &&
        progress.phase !== "cancelled"
      ) {
        progress = {
          ...progress,
          phase: pausing ? "paused" : "synthesizing",
        };
      }
      notice = translate(
        pausing ? "audiobooks.generationPaused" : "audiobooks.generationResumed",
      );
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function cancelGeneration() {
    if (!window.confirm(translate("audiobooks.cancelGenerationConfirm"))) return;
    cancelling = true;
    try {
      applyGenerationState(await cancelAudiobookGeneration());
      await load();
    } catch (cause) {
      cancelling = false;
      error = errorMessage(cause);
    }
  }

  async function removeQueueItem(item: AudiobookQueueItem) {
    try {
      applyGenerationState(await removeQueuedAudiobook(item.id));
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function moveQueueItem(item: AudiobookQueueItem, direction: -1 | 1) {
    try {
      applyGenerationState(await moveQueuedAudiobook(item.id, direction));
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function clearGenerationQueue() {
    if (!window.confirm(translate("audiobooks.clearQueueConfirm"))) return;
    try {
      applyGenerationState(await clearAudiobookQueue());
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function initProgress() {
    unlisten = await listen<AudiobookProgressEvent>(
      "audiobook::progress",
      async (event) => {
        if (progressHideTimer) {
          clearTimeout(progressHideTimer);
          progressHideTimer = undefined;
        }
        const payload = event.payload;
        const previous = progress;
        progress = payload;
        if (
          !activeTaskStartedAt ||
          previous?.phase === "done" ||
          previous?.phase === "error" ||
          previous?.phase === "cancelled"
        ) {
          activeTaskStartedAt = Date.now();
          hiddenProgress = false;
        }
        clock = Date.now();
        if (
          payload.phase === "done" ||
          payload.phase === "error" ||
          payload.phase === "cancelled"
        ) {
          cancelling = false;
          if (payload.phase === "done") {
            notice = payload.message;
          } else if (payload.phase === "cancelled") {
            notice = payload.message || translate("audiobooks.generationCancelled");
          } else {
            error = payload.message;
          }
          await load();
          if (progressHideTimer) clearTimeout(progressHideTimer);
          progressHideTimer = setTimeout(() => {
            if (
              progress?.phase === "done" ||
              progress?.phase === "error" ||
              progress?.phase === "cancelled"
            ) {
              progress = null;
              activeTaskStartedAt = null;
              activeTaskTitleOverride = null;
              activeTaskVoiceOverride = null;
              activeTaskCoverOverride = null;
            }
          }, 2500);
        }
      },
    );

    unlistenQueue = await listen("audiobook::queue", () => {
      void refreshQueue();
    });

    unlistenDownload = await listen<ModelDownloadProgress>(
      "tts-practice-download-progress",
      (event) => {
        const payload = event.payload;
        if (payload.done) {
          const next = { ...whisperProgress };
          delete next[payload.modelId];
          whisperProgress = next;
          if (payload.error) {
            error = payload.error;
          } else {
            void practiceModels()
              .then((models) => (whisperModels = models))
              .catch(() => undefined);
            void audiobookCapabilities()
              .then((caps) => (capabilities = caps))
              .catch(() => undefined);
            void refreshVoiceStatus();
          }
        } else if (payload.total > 0) {
          whisperProgress = {
            ...whisperProgress,
            [payload.modelId]: (payload.downloaded / payload.total) * 100,
          };
        }
      },
    );

    unlistenVoice = await listen<VoiceNote>(
      "audiobook::voice_marker",
      (event) => {
        const marker = event.payload;
        if (!marker.audiobookId || marker.audiobookId !== voiceMarkersFor) {
          return;
        }
        if (!voiceMarkers.some((entry) => entry.id === marker.id)) {
          voiceMarkers = [...voiceMarkers, marker].sort(
            (left, right) => left.positionMs - right.positionMs,
          );
        }
        notice = translate("audiobooks.voiceMarkerSaved", {
          time: formatDuration(marker.positionMs),
        });
      },
    );

    unlistenVoiceError = await listen<string>(
      "audiobook::voice_marker_error",
      (event) => {
        error = event.payload;
        void refreshVoiceStatus();
      },
    );
  }

  async function loadSections(bookId: string) {
    sections = [];
    if (!bookId) return;
    try {
      const session = await readerOpen(bookId);
      sections = session.document.sections.map((item, index) => ({
        index,
        title: item.title ?? translate("audiobooks.sectionPage") + ` ${index + 1}`,
      }));
    } catch {
      sections = [];
    }
  }

  async function onBookChange(value: string) {
    selectedBookId = value;
    await loadSections(value);
  }

  async function loadChapters(audiobookId: string | null | undefined) {
    if (!audiobookId) {
      chapters = [];
      chapterAudioId = null;
      return;
    }
    if (chapterAudioId === audiobookId) return;
    chapterAudioId = audiobookId;
    try {
      chapters = await audiobookChapters(audiobookId);
    } catch {
      chapters = [];
    }
  }

  function syncKaraoke(current: PlaybackSnapshot | null) {
    if (!current) {
      karaoke = null;
      karaokeKey = null;
      karaokeLoading = false;
      karaokeOpen = false;
      return;
    }
    void loadKaraoke(current.audiobookId, current.chapterIndex);
  }

  async function loadKaraoke(audiobookId: string, chapterIndex: number) {
    const key = `${audiobookId}:${chapterIndex}`;
    if (key === karaokeKey) return;
    karaokeKey = key;
    karaokeLoading = true;
    if (
      karaoke &&
      (karaoke.chapterIndex !== chapterIndex ||
        karaoke.lines.length === 0)
    ) {
      karaoke = null;
    }
    try {
      const data = await audiobookKaraoke(audiobookId, chapterIndex);
      if (karaokeKey === key) karaoke = data;
    } catch {
      if (karaokeKey === key) karaoke = null;
    } finally {
      if (karaokeKey === key) karaokeLoading = false;
    }
  }

  async function loadModalChapters(audiobookId: string) {
    modalChapterId = audiobookId;
    modalChaptersLoading = true;
    try {
      modalChapters = await audiobookChapters(audiobookId);
    } catch (cause) {
      modalChapters = [];
      error = errorMessage(cause);
      chaptersOpen = false;
    } finally {
      modalChaptersLoading = false;
    }
  }

  function formatSleep(seconds: number) {
    return formatClock(seconds);
  }

  function formatClock(totalSeconds: number) {
    const seconds = Math.max(0, Math.round(totalSeconds));
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const rest = seconds % 60;
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(rest).padStart(2, "0")}`
      : `${minutes}:${String(rest).padStart(2, "0")}`;
  }

  function startSleepTimer(minutes: number) {
    clearInterval(sleepTimer);
    sleepMinutes = minutes;
    sleepRemaining = 0;
    void setAudiobookSleepTimer(minutes).catch(() => undefined);
    if (!minutes) return;
    const end = Date.now() + minutes * 60_000;
    sleepRemaining = minutes * 60;
    sleepTimer = setInterval(() => {
      const left = Math.max(0, Math.ceil((end - Date.now()) / 1000));
      sleepRemaining = left;
      if (left <= 0) {
        clearInterval(sleepTimer);
        sleepMinutes = 0;
        sleepRemaining = 0;
        // En Android el servicio pausa por su cuenta; el temporizador JS solo
        // actualiza la vista al volver a primer plano.
        if (backgroundPlayback) {
          void pollPlayback();
        } else {
          void stopPlayback();
        }
        notice = translate("audiobooks.sleepDone");
      }
    }, 1000);
  }

  function skip(seconds: number) {
    if (!playback) return;
    const target = Math.max(
      0,
      Math.min(playback.durationMs, playback.positionMs + seconds * 1000),
    );
    void onSeek(target);
  }

  function applyPlayback(snapshot: PlaybackSnapshot | null) {
    if (!snapshot) {
      syncKaraoke(null);
      return;
    }
    playbackAt = Date.now();
    frameClock = playbackAt;
    syncKaraoke(snapshot);
    const percent =
      snapshot.durationMs > 0
        ? Math.min(1, Math.max(0, snapshot.positionMs / snapshot.durationMs))
        : 0;
    audiobooks = audiobooks.map((item) =>
      item.id === snapshot.audiobookId
        ? {
            ...item,
            positionMs: snapshot.positionMs,
            percent,
            lastListenedAt: new Date().toISOString(),
            progressChapterIndex: snapshot.chapterIndex,
            progressChapterTitle: snapshot.chapterTitle,
            progressSection: snapshot.section,
            progressOffset: snapshot.offsetStart,
          }
        : item,
    );
  }

  async function changeSpeed(value: number) {
    speed = value;
    try {
      playback = (await setAudiobookSpeed(value)) ?? playback;
      playbackAt = Date.now();
      frameClock = playbackAt;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function toggleKeepScreenOn() {
    keepScreenOn = !keepScreenOn;
    try {
      localStorage.setItem(
        "kora.audiobook.keepScreenOn",
        String(keepScreenOn),
      );
    } catch {
      // El ajuste es local; si el WebView no permite almacenamiento se ignora.
    }
    void setAudiobookKeepScreenOn(keepScreenOn).catch((cause) => {
      error = errorMessage(cause);
    });
  }

  async function openItemText(item: AudiobookInfo) {
    if (item.audioOnly) {
      notice = translate("audiobooks.audioOnlyNoText");
      return;
    }
    const positionMs =
      playback?.audiobookId === item.id ? playback.positionMs : item.positionMs;
    let fallbackSection = item.progressSection;
    let fallbackOffset = item.progressOffset;
    if (playback?.audiobookId === item.id) {
      fallbackSection = playback.section;
      fallbackOffset = playback.offsetStart;
    }
    let locator = null;
    try {
      locator = await audiobookLocator(item.id, positionMs);
    } catch {
      // Si no hay texto asociado (PDF escaneado, manifiesto antiguo) se abre
      // al menos el capítulo donde se quedó la escucha.
    }
    onRead?.(item.bookId, {
      section: locator?.section ?? fallbackSection,
      offset: locator?.offsetStart ?? fallbackOffset,
    });
  }

  function openGenerator() {
    error = null;
    notice = null;
    generateOpen = true;
  }

  function openImport(kind: "audio" | "transcribe" = "audio") {
    error = null;
    notice = null;
    importInitialKind = kind;
    importOpen = true;
    void aiStatus()
      .then((status) => (aiState = status))
      .catch(() => undefined);
    void practiceModels()
      .then((models) => (whisperModels = models))
      .catch(() => undefined);
  }

  function applyAlbum(updated: AudiobookAlbumInfo) {
    albums = albums.map((album) => (album.id === updated.id ? updated : album));
  }

  function openAlbumEditor(album: AudiobookAlbumInfo | null = null) {
    albumEditing = album;
    albumEditorError = null;
    albumEditorOpen = true;
  }

  async function saveAlbum(input: { title: string; description: string | null }) {
    albumEditorBusy = true;
    albumEditorError = null;
    const editing = albumEditing;
    const creating = !editing;
    try {
      const saved = editing
        ? await updateAudiobookAlbum(
            editing.id,
            input.title,
            input.description,
          )
        : await createAudiobookAlbum(input.title, input.description);
      if (creating) {
        albums = [...albums, saved];
        albumDetailId = saved.id;
        notice = translate("albums.created");
        viewMode = "albums";
      } else {
        applyAlbum(saved);
        notice = translate("albums.updated");
      }
      albumEditorOpen = false;
      albumEditing = null;
    } catch (cause) {
      albumEditorError = errorMessage(cause);
    } finally {
      albumEditorBusy = false;
    }
  }

  function openAlbum(album: AudiobookAlbumInfo) {
    albumDetailId = album.id;
  }

  async function removeAlbum(album: AudiobookAlbumInfo) {
    if (
      !window.confirm(
        translate("albums.deleteConfirm", { title: album.title }),
      )
    ) {
      return;
    }
    albumBusy = album.id;
    try {
      await deleteAudiobookAlbum(album.id);
      albums = albums.filter((entry) => entry.id !== album.id);
      if (albumDetailId === album.id) albumDetailId = null;
      notice = translate("albums.deleted");
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      albumBusy = null;
    }
  }

  async function addAlbumItems(ids: string[]) {
    const album = albumDetail;
    if (!album || ids.length === 0) {
      albumAddOpen = false;
      return;
    }
    albumBusy = album.id;
    try {
      applyAlbum(await addAudiobooksToAlbum(album.id, ids));
      albumAddOpen = false;
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      albumBusy = null;
    }
  }

  async function removeAlbumItem(
    album: AudiobookAlbumInfo,
    item: AudiobookInfo,
  ) {
    albumBusy = album.id;
    try {
      applyAlbum(await removeAudiobookFromAlbum(album.id, item.id));
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      albumBusy = null;
    }
  }

  async function moveAlbumItem(
    album: AudiobookAlbumInfo,
    item: AudiobookInfo,
    direction: -1 | 1,
  ) {
    const items = albumItems(album);
    const index = items.findIndex((entry) => entry.id === item.id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= items.length) return;
    const ids = items.map((entry) => entry.id);
    [ids[index], ids[target]] = [ids[target], ids[index]];
    albumBusy = album.id;
    try {
      applyAlbum(await reorderAudiobookAlbum(album.id, ids));
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      albumBusy = null;
    }
  }

  function openAlbumPicker(item: AudiobookInfo) {
    albumPickerError = null;
    albumPickerFor = item;
  }

  async function saveAlbumPicker(albumIds: string[]) {
    const item = albumPickerFor;
    if (!item) return;
    const current = new Set(
      albums
        .filter((album) => album.audiobookIds.includes(item.id))
        .map((album) => album.id),
    );
    const desired = new Set(albumIds);
    const toAdd = albums.filter(
      (album) => desired.has(album.id) && !current.has(album.id),
    );
    const toRemove = albums.filter(
      (album) => !desired.has(album.id) && current.has(album.id),
    );
    if (toAdd.length === 0 && toRemove.length === 0) {
      albumPickerFor = null;
      return;
    }
    albumPickerBusy = true;
    albumPickerError = null;
    try {
      for (const album of toAdd) {
        applyAlbum(await addAudiobooksToAlbum(album.id, [item.id]));
      }
      for (const album of toRemove) {
        applyAlbum(await removeAudiobookFromAlbum(album.id, item.id));
      }
      albumPickerFor = null;
    } catch (cause) {
      albumPickerError = errorMessage(cause);
    } finally {
      albumPickerBusy = false;
    }
  }

  async function startAlbumPlayback(
    album: AudiobookAlbumInfo,
    items: AudiobookInfo[],
    index: number,
  ) {
    if (items.length === 0) {
      error = translate("albums.emptyItems");
      return;
    }
    startAlbumQueue(
      album.id,
      items.map((item) => item.id),
      index,
    );
    try {
      const first = items[index] ?? items[0];
      playback = await playAudiobook(first.id);
      applyPlayback(playback);
      playerExpanded = true;
      seekValue = playback?.positionMs ?? 0;
      await startListeningSession(first.bookId);
      await loadChapters(first.id);
    } catch (cause) {
      clearAlbumQueue();
      error = errorMessage(cause);
    }
  }

  async function playAlbum(album: AudiobookAlbumInfo) {
    const items = playableAlbumItems(album);
    if (items.length === 0) {
      error = translate("albums.emptyItems");
      return;
    }
    await startAlbumPlayback(album, items, 0);
  }

  async function playAlbumItem(album: AudiobookAlbumInfo, item: AudiobookInfo) {
    const items = playableAlbumItems(album);
    const index = items.findIndex((entry) => entry.id === item.id);
    if (index < 0) {
      error = translate("audiobooks.statusFailed");
      return;
    }
    await startAlbumPlayback(album, items, index);
  }

  function openConvert(item: AudiobookInfo) {
    if (convertBusy || item.status !== "done") return;
    error = null;
    notice = null;
    if (item.audioOnly) {
      if (!nativeAudioImport && !ffmpegReady) {
        notice = translate("audiobooks.ffmpegMissing");
        return;
      }
      convertItem = item;
      if (whisperModels.length === 0) {
        void practiceModels()
          .then((models) => (whisperModels = models))
          .catch(() => undefined);
      }
      return;
    }
    const book = booksById.get(item.bookId);
    if (book?.format === "epub") {
      void exportAudiobookText(item, book);
      return;
    }
    notice = translate("audiobooks.pdfSourceHelp");
  }

  async function exportAudiobookText(
    item: AudiobookInfo,
    book: BookSummary,
  ) {
    try {
      const destination = await save({
        defaultPath: `${safeName(item.bookTitle)}.epub`,
        filters: [{ name: "EPUB", extensions: ["epub"] }],
      });
      if (typeof destination !== "string") return;
      const path = await exportBook(book.id, destination);
      notice = translate("audiobooks.exportEpubDone", { path });
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function confirmConvert(modelId: "tiny" | "base") {
    const item = convertItem;
    if (!item) return;
    convertItem = null;
    convertBusy = true;
    error = null;
    notice = null;
    try {
      const updated = await transcribeAudiobookLater(item.id, modelId);
      notice = translate("audiobooks.convertDone", {
        title: updated.bookTitle,
      });
      await load();
      if (modalChapterId === updated.id) {
        void loadModalChapters(updated.id);
      }
    } catch (cause) {
      if (isCancelledError(cause)) {
        notice = translate("audiobooks.generationCancelled");
      } else {
        error = errorMessage(cause);
      }
    } finally {
      convertBusy = false;
    }
  }

  async function openCurrentText() {
    const item = playback
      ? (audiobooks.find((entry) => entry.id === playback?.audiobookId) ?? null)
      : playerItem;
    if (item?.audioOnly) {
      notice = translate("audiobooks.audioOnlyNoText");
      return;
    }
    if (item) {
      await openItemText(item);
    }
  }

  async function openChapterEditor(id: string) {
    error = null;
    try {
      const list = await audiobookChapters(id);
      chapterEditorId = id;
      chapterEditorChapters = list.map((chapter) => ({
        startMs: chapter.startMs,
        endMs: chapter.endMs,
        title: chapter.title,
      }));
      chapterEditorError = null;
      chapterEditorOpen = true;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function saveChapters(chapters: ChapterEditInput[]) {
    if (!chapterEditorId) return;
    savingChapters = true;
    chapterEditorError = null;
    try {
      const updated = await updateAudiobookChapters(chapterEditorId, chapters);
      chapterEditorOpen = false;
      notice = translate("audiobooks.chapterSavedNotice");
      await load();
      if (modalChapterId === updated.id) {
        void loadModalChapters(updated.id);
      }
    } catch (cause) {
      chapterEditorError = errorMessage(cause);
    } finally {
      savingChapters = false;
    }
  }

  async function reanalyzeChapters(id: string, mode: AudioImportMode) {
    error = null;
    notice = null;
    busy = "reanalyze";
    hiddenProgress = false;
    activeTaskTitleOverride = translate("audiobooks.reanalyze");
    activeTaskVoiceOverride = "Análisis de capítulos";
    activeTaskCoverOverride = null;
    try {
      const updated = await reanalyzeAudiobookChapters(id, {
        mode,
        language: null,
        modelId: null,
      });
      notice = translate("audiobooks.reanalyzedNotice", {
        title: updated.bookTitle,
        count: updated.chapterCount,
      });
      await load();
      if (modalChapterId === updated.id) {
        void loadModalChapters(updated.id);
      }
    } catch (cause) {
      if (isCancelledError(cause)) {
        notice = translate("audiobooks.generationCancelled");
      } else {
        error = errorMessage(cause);
      }
    } finally {
      busy = null;
    }
  }

  async function generate(payload: {
    bookId: string;
    scope: AudiobookScope;
    voiceId: string;
    narration: NarrationSettings;
    cast: AudiobookCastInput | null;
  }) {
    error = null;
    notice = null;
    if (!payload.bookId || !payload.voiceId) {
      error = translate("audiobooks.chooseBookVoice");
      return;
    }
    if (payload.scope.kind === "selection" && payload.scope.end <= payload.scope.start) {
      error = translate("audiobooks.selectionInvalid");
      return;
    }

    const title =
      booksById.get(payload.bookId)?.title ?? translate("audiobooks.title");
    busy = "generate";
    hiddenProgress = false;
    selectedBookId = payload.bookId;
    voiceId = payload.voiceId;
    activeTaskTitleOverride = title;
    activeTaskVoiceOverride =
      installedVoices.find((voice) => voice.id === payload.voiceId)?.name ?? null;
    activeTaskCoverOverride = booksById.get(payload.bookId)?.coverPath ?? null;
    try {
      const view = await enqueueAudiobook(
        payload.bookId,
        payload.scope,
        payload.voiceId,
        payload.narration,
        payload.cast,
      );
      applyGenerationState(view);
      generateOpen = false;
      notice = translate("audiobooks.queuedNotice", { title });
      // El backend crea el registro en estado pendiente antes de sintetizar.
      // Recargamos para que aparezca en el filtro «En proceso».
      await load();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function importAudiobook(
    kind: "audio" | "transcribe",
    detection: AudioImportMode,
    modelId: "tiny" | "base" | null = null,
  ) {
    error = null;
    notice = null;
    if (!audioImportSupported) {
      error = translate("audiobooks.importUnavailable");
      return;
    }
    if (kind === "transcribe" && !transcriptionSupported) {
      error = translate("audiobooks.transcribeUnavailable");
      return;
    }
    let source: string | string[] | null = null;
    try {
      source = await open({
        multiple: false,
        filters: nativeAudioImport
          ? [{ name: "Audio", extensions: ["audio/*"] }]
          : [
              {
                name: "Audiolibro",
                extensions: ["m4b", "m4a", "mp3", "aac", "flac", "ogg", "opus"],
              },
            ],
      });
    } catch (cause) {
      error = errorMessage(cause);
      return;
    }
    if (typeof source !== "string") return;

    let staged = source;
    let metadata: AndroidAudioMetadata | null = null;
    busy = "import";
    try {
      staged = stageImportSource(source);
      if (nativeAudioImport) {
        metadata = probeAndroidAudio(staged);
        if (!metadata) {
          throw new Error(translate("audiobooks.metadataUnavailable"));
        }
      }
    } catch (cause) {
      if (staged !== source) discardImportSource(staged);
      error = errorMessage(cause);
      busy = null;
      return;
    }

    hiddenProgress = false;
    activeTaskTitleOverride =
      staged.split(/[\\/]/).pop() ?? translate("audiobooks.import");
    activeTaskVoiceOverride =
      kind === "transcribe"
        ? modelId === "tiny"
          ? "Whisper Tiny"
          : translate("audiobooks.whisperBase")
        : "Análisis de capítulos";
    activeTaskCoverOverride = null;
    importOpen = false;
    try {
      const imported =
        kind === "transcribe" && nativeAudioImport
          ? await importAudioAudiobook(
              staged,
              {
                mode: detection,
                language: metadata?.language ?? null,
                modelId,
              } satisfies AudioImportOptions,
              metadata,
              true,
            )
          : kind === "transcribe"
            ? await importExternalAudiobook(staged)
            : await importAudioAudiobook(
                staged,
                {
                  mode: detection,
                  language: metadata?.language ?? null,
                  modelId: null,
                } satisfies AudioImportOptions,
                metadata,
              );
      notice =
        kind === "transcribe"
          ? translate("audiobooks.importNotice", { title: imported.bookTitle })
          : translate("audiobooks.audioImportedNotice", {
              title: imported.bookTitle,
              count: imported.chapterCount,
            });
      await load();
    } catch (cause) {
      if (isCancelledError(cause)) {
        notice = translate("audiobooks.importCancelled");
      } else {
        error = errorMessage(cause);
      }
    } finally {
      if (staged !== source) discardImportSource(staged);
      busy = null;
    }
  }

  async function downloadWhisper(modelId = "base") {
    error = null;
    whisperProgress = { ...whisperProgress, [modelId]: 0 };
    try {
      await practiceDownloadModel(modelId);
      whisperModels = await practiceModels();
      capabilities = await audiobookCapabilities();
      voiceStatus = await voiceMarkerStatus().catch(() => voiceStatus);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      const next = { ...whisperProgress };
      delete next[modelId];
      whisperProgress = next;
    }
  }

  async function refreshVoiceStatus() {
    try {
      voiceStatus = await voiceMarkerStatus();
    } catch {
      // La escucha no debe romper la vista si el micrófono no está disponible.
    }
  }

  async function loadVoiceMarkers(audiobookId: string) {
    voiceMarkersLoading = true;
    try {
      const loaded = await audiobookVoiceMarkers(audiobookId);
      if (voiceMarkersFor === audiobookId) {
        voiceMarkers = loaded;
      }
    } catch (cause) {
      if (voiceMarkersFor === audiobookId) {
        voiceMarkers = [];
      }
      error = errorMessage(cause);
    } finally {
      voiceMarkersLoading = false;
    }
  }

  function openVoiceMarks() {
    error = null;
    notice = null;
    voiceMarksOpen = true;
    void refreshVoiceStatus();
    if (playback?.audiobookId) {
      void loadVoiceMarkers(playback.audiobookId);
    }
  }

  async function saveMarkerPhrase(next: string) {
    error = null;
    const wasListening = voiceListening;
    const audiobookId = playback?.audiobookId ?? null;
    if (wasListening) {
      await stopVoiceMarkerListening().catch(() => undefined);
    }
    voiceStatus = await saveVoiceMarkerPhrase(next);
    if (wasListening && audiobookId && voiceStatus.modelReady) {
      voiceStatus = await startVoiceMarkerListening(
        audiobookId,
        voiceStatus.phrase ?? next,
        null,
      );
    }
  }

  async function toggleVoiceListening() {
    if (voiceToggling) return;
    voiceToggling = true;
    error = null;
    try {
      if (voiceListening) {
        voiceStatus = await stopVoiceMarkerListening();
        return;
      }
      if (!playback) {
        throw new Error(translate("audiobooks.voiceNeedsPlayback"));
      }
      if (!voiceSupported) {
        throw new Error(translate("audiobooks.voiceDesktopOnly"));
      }
      if (!voiceModelReady) {
        throw new Error(translate("audiobooks.voiceWhisperMissing"));
      }
      voiceStatus = await startVoiceMarkerListening(
        playback.audiobookId,
        voicePhrase,
        null,
      );
      notice = translate("audiobooks.voiceListeningHint", {
        phrase: voicePhrase,
      });
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      voiceToggling = false;
    }
  }

  async function stopVoiceIfListening() {
    if (!voiceStatus?.listening) return;
    try {
      voiceStatus = await stopVoiceMarkerListening();
    } catch {
      // Al cerrar o cambiar de audiolibro no bloqueamos la interfaz.
    }
  }

  async function removeVoiceMarker(id: string) {
    error = null;
    await deleteAnnotation("voice_note", id);
    voiceMarkers = voiceMarkers.filter((marker) => marker.id !== id);
  }

  async function jumpToVoiceMarker(positionMs: number) {
    voiceMarksOpen = false;
    const item = playerItem;
    if (!item) return;
    try {
      if (playback?.audiobookId !== item.id || !playback.playing) {
        playback = await playAudiobook(item.id);
        audiobookPlayerHidden.set(false);
        playerExpanded = true;
        const bookItem = audiobooks.find((entry) => entry.id === item.id);
        if (bookItem) await startListeningSession(bookItem.bookId);
      }
      playback = await seekAudiobook(positionMs);
      applyPlayback(playback);
      seekValue = playback?.positionMs ?? positionMs;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function pollPlayback() {
    try {
      const current = await advanceAlbumPlayback(await getAudiobookProgress());
      playback = current;
      playbackAt = Date.now();
      frameClock = playbackAt;
      syncKaraoke(current);
      if (!current) {
        audiobookPlayerHidden.set(false);
        playerExpanded = false;
        sheetDrag = 0;
        waveform = null;
        waveformKey = null;
        waveformLoading = false;
      } else {
        void loadWaveform(
          audiobooks.find((item) => item.id === current.audiobookId) ?? null,
        );
      }
      if (!scrubbing) {
        seekValue = current?.positionMs ?? 0;
      }
      if (
        current &&
        audiobooks.some((item) => item.id === current.audiobookId)
      ) {
        const percent =
          current.durationMs > 0
            ? Math.min(1, Math.max(0, current.positionMs / current.durationMs))
            : 0;
        audiobooks = audiobooks.map((item) =>
          item.id === current.audiobookId
            ? {
                ...item,
                positionMs: current.positionMs,
                percent,
                progressChapterIndex: current.chapterIndex,
                progressChapterTitle: current.chapterTitle,
                progressSection: current.section,
                progressOffset: current.offsetStart,
              }
            : item,
        );
      }
      if (current?.audiobookId !== chapterAudioId) {
        void loadChapters(current?.audiobookId);
      }
      if (listeningSessionId && (!current || !current.playing)) {
        void endListeningSession();
      } else if (current?.playing) {
        const currentBookId =
          audiobooks.find((item) => item.id === current.audiobookId)?.bookId ??
          null;
        if (currentBookId && currentBookId !== listeningBookId) {
          void startListeningSession(currentBookId);
        }
      }
    } catch {
      // El sondeo no debe llenar la UI de errores.
    }
  }

  async function togglePlayback(item: AudiobookInfo) {
    error = null;
    if ($albumPlayback && !$albumPlayback.audiobookIds.includes(item.id)) {
      clearAlbumQueue();
    }
    try {
      if (playback?.audiobookId === item.id && playback.playing) {
        playback = await pauseAudiobook();
        applyPlayback(playback);
        await endListeningSession();
        await stopVoiceIfListening();
      } else {
        if (playback?.audiobookId && playback.audiobookId !== item.id) {
          await stopVoiceIfListening();
        }
        playback = await playAudiobook(item.id);
        applyPlayback(playback);
        audiobookPlayerHidden.set(false);
        playerExpanded = true;
        void loadWaveform(item);
        await startListeningSession(item.bookId);
        await loadChapters(item.id);
      }
      seekValue = playback?.positionMs ?? seekValue;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function onSeek(value: number) {
    seekValue = value;
    try {
      playback = await seekAudiobook(value);
      applyPlayback(playback);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function stopPlayback() {
    try {
      clearInterval(sleepTimer);
      sleepMinutes = 0;
      sleepRemaining = 0;
      void setAudiobookSleepTimer(0).catch(() => undefined);
      playback = await stopAudiobook();
      applyPlayback(playback);
      audiobookPlayerHidden.set(false);
      playerExpanded = false;
      sheetDrag = 0;
      waveform = null;
      waveformKey = null;
      await endListeningSession();
      await stopVoiceIfListening();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function retryAudiobookItem(item: AudiobookInfo) {
    busy = item.id;
    try {
      await retryAudiobook(item.id);
      notifySuccess(translate("audiobooks.retryDone"), item.bookTitle);
      await load();
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function removeAudiobook(item: AudiobookInfo) {
    if (!window.confirm(translate("audiobooks.confirmDelete", { title: item.bookTitle }))) {
      return;
    }
    try {
      if (generationActive?.audiobookId === item.id) {
        applyGenerationState(await cancelAudiobookGeneration());
      } else if (queue.some((entry) => entry.id === item.id)) {
        applyGenerationState(await removeQueuedAudiobook(item.id));
      }
      if (playback?.audiobookId === item.id) {
        await stopAudiobook().catch(() => undefined);
        await stopVoiceIfListening();
        playback = null;
        audiobookPlayerHidden.set(false);
        playerExpanded = false;
      }
      clearAlbumQueue();
      await deleteAudiobook(item.id);
      if (shareItem?.id === item.id) {
        shareOpen = false;
        shareItem = null;
        shareResult = null;
      }
      await load();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function toggleFavorite(item: AudiobookInfo) {
    if (favoriteBusy) return;
    favoriteBusy = item.id;
    error = null;
    try {
      const updated = await audiobookSetFavorite(item.id, !item.favorite);
      audiobooks = audiobooks.map((entry) =>
        entry.id === updated.id ? updated : entry,
      );
      notice = translate(
        updated.favorite
          ? "audiobooks.favoriteAdded"
          : "audiobooks.favoriteRemoved",
      );
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      favoriteBusy = null;
    }
  }

  function openShare(item: AudiobookInfo) {
    if (item.status !== "done") return;
    shareItem = item;
    shareResult = null;
    shareError = null;
    shareOpen = true;
  }

  async function shareAudiobook(
    pin: string,
    permission: SharePermission,
    expiresAt: string | null,
    maxUses: number | null,
  ) {
    if (!shareItem || sharing) return;
    sharing = true;
    shareError = null;
    try {
      const created = await createAudiobookShare(
        pin,
        shareItem.id,
        permission,
        expiresAt,
        maxUses,
      );
      shareResult = created;
      // Sin el anuncio mDNS el receptor no puede descubrir el dispositivo.
      await startShare().catch(() => undefined);
    } catch (cause) {
      shareError = errorMessage(cause);
      throw new Error(shareError);
    } finally {
      sharing = false;
    }
  }

  async function downloadAudiobook(item: AudiobookInfo) {
    error = null;
    notice = null;
    const extension = item.audioExt?.toLowerCase() || "m4b";
    let destination: string | null = null;
    try {
      destination = await save({
        defaultPath: `${safeName(item.bookTitle)}.${extension}`,
        filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      });
    } catch (cause) {
      error = errorMessage(cause);
      return;
    }
    if (typeof destination !== "string") return;

    downloadingId = item.id;
    try {
      const path = await exportAudiobook(item.id, destination);
      notice = translate("audiobooks.exported", { path });
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      downloadingId = null;
    }
  }

  function safeName(title: string) {
    return (
      title
        .replace(/[\\/:*?"<>|]/g, " ")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 80) || "audiolibro"
    );
  }

  function toggleExtract(id: string) {
    selectedExtracts = selectedExtracts.includes(id)
      ? selectedExtracts.filter((value) => value !== id)
      : [...selectedExtracts, id];
  }

  async function weave() {
    error = null;
    notice = null;
    if (!weaveTitle.trim()) {
      error = translate("audiobooks.weaveTitleRequired");
      return;
    }
    if (selectedExtracts.length === 0) {
      error = translate("audiobooks.weaveExtractsRequired");
      return;
    }
    if (!voiceId) {
      error = translate("audiobooks.weaveVoiceRequired");
      return;
    }

    busy = "weave";
    hiddenProgress = false;
    activeTaskTitleOverride = weaveTitle.trim();
    activeTaskVoiceOverride =
      installedVoices.find((voice) => voice.id === voiceId)?.name ?? null;
    activeTaskCoverOverride = null;
    try {
      const result = await weaveExtracts(
        selectedExtracts,
        weaveTitle.trim(),
        voiceId,
      );
      notice = translate("audiobooks.weaveNotice", { title: result.book.title });
      weaveTitle = "";
      selectedExtracts = [];
      await load();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = null;
    }
  }

  async function removeExtract(extract: Extract) {
    if (!window.confirm(translate("audiobooks.confirmDeleteExtract"))) return;
    try {
      await readerDeleteExtract(extract.id);
      selectedExtracts = selectedExtracts.filter((id) => id !== extract.id);
      await load();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function coverSrc(path: string | null): string | null {
    return path ? convertFileSrc(path) : null;
  }

  function isCancelledError(cause: unknown): boolean {
    return (
      typeof cause === "object" &&
      cause !== null &&
      "code" in cause &&
      (cause as { code?: unknown }).code === "cancelled"
    );
  }

  function openChapters(item: AudiobookInfo) {
    if (item.status !== "done") return;
    chaptersOpen = true;
    void loadModalChapters(item.id);
  }

  function openChaptersTool() {
    chaptersOpen = true;
    const candidate =
      (playback?.audiobookId
        ? audiobooks.find((item) => item.id === playback?.audiobookId)
        : null) ??
      audiobooks.find((item) => item.status === "done") ??
      null;
    if (candidate) {
      void loadModalChapters(candidate.id);
    } else {
      modalChapterId = null;
      modalChapters = [];
    }
  }

  async function playChapter(chapter: AudiobookChapterInfo) {
    const id = modalChapterId;
    if (!id) return;
    try {
      if (playback?.audiobookId !== id || !playback.playing) {
        await stopVoiceIfListening();
        playback = await playAudiobook(id);
        audiobookPlayerHidden.set(false);
        playerExpanded = true;
        const item = audiobooks.find((entry) => entry.id === id);
        if (item) await startListeningSession(item.bookId);
      }
      playback = await seekAudiobook(chapter.startMs);
      applyPlayback(playback);
      seekValue = playback?.positionMs ?? chapter.startMs;
      const item = audiobooks.find((entry) => entry.id === id);
      if (item) void loadWaveform(item);
      void loadChapters(id);
      chaptersOpen = false;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function togglePlayer() {
    const item = playerItem;
    if (!item) return;
    await togglePlayback(item);
  }

  function openPlayer() {
    audiobookPlayerHidden.set(false);
    playerExpanded = true;
  }

  function minimizePlayer() {
    audiobookPlayerHidden.set(false);
    playerExpanded = false;
    sheetDrag = 0;
    sheetDragging = false;
  }

  function hidePlayer() {
    audiobookPlayerHidden.set(true);
    playerExpanded = false;
    sheetDrag = 0;
    sheetDragging = false;
  }

  function onSheetPointerDown(event: PointerEvent) {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    const target = event.currentTarget as HTMLElement;
    sheetDragging = true;
    sheetDragStart = event.clientY;
    sheetDrag = 0;
    target.setPointerCapture(event.pointerId);
  }

  function onSheetPointerMove(event: PointerEvent) {
    if (!sheetDragging) return;
    sheetDrag = Math.max(0, event.clientY - sheetDragStart);
  }

  function onSheetPointerEnd(event: PointerEvent) {
    if (!sheetDragging) return;
    sheetDragging = false;
    const shouldMinimize = sheetDrag > 110;
    sheetDrag = 0;
    if (shouldMinimize) {
      minimizePlayer();
    } else {
      const target = event.currentTarget as HTMLElement;
      if (target.hasPointerCapture?.(event.pointerId)) {
        target.releasePointerCapture(event.pointerId);
      }
    }
  }

  function onPlayerKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && playerExpanded) {
      event.preventDefault();
      minimizePlayer();
    }
  }

  function voiceLabel(voice: VoiceDescriptor) {
    return `${voice.language.toUpperCase()} · ${voice.name}${voice.cloned ? ` · ${translate("audiobooks.voiceCloned")}` : ""}`;
  }

  function queueScopeLabel(scope: AudiobookScope) {
    if (scope.kind === "fullBook") return translate("audiobooks.fullBook");
    if (scope.kind === "chapter") {
      return translate("audiobooks.chapterNumber", { number: scope.section + 1 });
    }
    return translate("audiobooks.selectionScope");
  }

  function queueNarrationLabel(narration: NarrationSettings | null) {
    if (!narration) return "";
    const style =
      narration.style === "calm"
        ? translate("audiobooks.narrationCalm")
        : narration.style === "dramatic"
          ? translate("audiobooks.narrationDramatic")
          : narration.style === "lively"
            ? translate("audiobooks.narrationLively")
            : translate("audiobooks.narrationNeutral");
    const speed =
      Math.abs(narration.speed - 1) < 0.001
        ? "1×"
        : `${narration.speed.toFixed(2).replace(/0$/, "")}×`;
    const badge = translate("audiobooks.narrationBadge", { style, speed });
    return narration.multiVoice
      ? `${badge} · ${translate("audiobooks.multiVoiceShort")}`
      : badge;
  }

  function excerpt(text: string) {
    const clean = text.replace(/\s+/g, " ").trim();
    return clean.length > 160 ? `${clean.slice(0, 160)}…` : clean;
  }
</script>

{#snippet waveBars(classNames: string)}
  {#each waveform?.buckets ?? [] as peak, index (index)}
    {@const count = waveform?.buckets.length ?? 1}
    {@const width = 100 / count}
    {@const height = Math.max(3, peak * 96)}
    <rect
      class={classNames}
      x={index * width + width * 0.14}
      y={(100 - height) / 2}
      width={width * 0.72}
      height={height}
      rx={width * 0.2}
    />
  {/each}
{/snippet}

<svelte:window onkeydown={onPlayerKeydown} />

<section class="audiobooks">
  <header class="page-head">
    <div class="title-block">
      <p class="eyebrow engine-eyebrow">
        <span class="pulse-dot" aria-hidden="true"></span>
        {$t("audiobooks.engineLocal")} · {engineLabel}
      </p>
      <h1 class="page-title">{$t("audiobooks.titleLong")}</h1>
      <p class="page-subtitle">{$t("audiobooks.subtitle")}</p>
    </div>
    <div class="head-actions">
      {#if playback && $audiobookPlayerHidden}
        <Button variant="subtle" onclick={() => audiobookPlayerHidden.set(false)}>
          <Icon name="headphones" size={17} />
          {$t("audiobooks.showPlayer")}
        </Button>
      {/if}
      <Button
        variant="ghost"
        onclick={() => openImport("audio")}
        disabled={!audioImportSupported || busy !== null}
      >
        <Icon name="upload" size={17} />
        {$t("audiobooks.importShort")}
        <span class="format-chip">{$t("audiobooks.importFormatHint")}</span>
      </Button>
      <Button
        variant="primary"
        onclick={openGenerator}
        disabled={!generationSupported || busy !== null}
      >
        <Icon name="sparkles" size={17} />
        {$t("audiobooks.generateShort")}
      </Button>
    </div>
  </header>

  {#if nativeAudioImport}
    <p class="notice soft" role="status">{$t("audiobooks.mobileImportNotice")}</p>
  {:else if !platformSupported}
    <p class="notice soft" role="status">{$t("audiobooks.desktopOnly")}</p>
  {:else if capabilities && !capabilities.ffmpeg && !nativeMux}
    <p class="notice soft" role="status">
      {$t("audiobooks.ffmpegMissing")} · {$t("audiobooks.ffmpegMissingHelp")}
    </p>
  {/if}

  {#if activeTaskVisible && progress}
    <section
      class="active-task"
      class:error={progress.phase === "error"}
      aria-label={$t("audiobooks.activeTask")}
    >
      <div class="task-cover">
        {#if coverSrc(activeTaskCover)}
          <img src={coverSrc(activeTaskCover)} alt="" />
        {:else}
          <span><Icon name="headphones" size={22} /></span>
        {/if}
      </div>
      <div class="task-main">
        <div class="task-head">
          <span
            class="task-chip"
            class:error={progress.phase === "error"}
            class:paused={isGenerationPaused && !cancelling}
          >
            <span class="task-pip" class:paused={isGenerationPaused} aria-hidden="true"></span>
            {cancelling
              ? $t("audiobooks.cancelling")
              : progress.phase === "error"
                ? $t("audiobooks.statusFailed")
                : progress.phase === "done"
                  ? $t("audiobooks.statusReady")
                  : progress.phase === "cancelled"
                    ? $t("audiobooks.generationCancelled")
                    : isGenerationPaused
                      ? $t("audiobooks.paused")
                      : progress.message}
          </span>
          <strong>{activeTaskTitle}</strong>
          <span class="task-voice">
            {activeTaskVoice
              ? $t("audiobooks.voiceLine", { voice: activeTaskVoice })
              : engineLabel}
          </span>
        </div>
        <div class="task-progress">
          <div class="task-progress-head">
            <span>{$t("audiobooks.progressTitle")}</span>
            <strong>{progressPercent}%</strong>
          </div>
          <div class="task-track" aria-hidden="true">
            <span style={`width:${progressPercent}%`}></span>
          </div>
          <div class="task-progress-foot">
            <span>
              {#if progress.phase === "done"}
                {$t("audiobooks.completed")}
              {:else if progress.phase === "error"}
                {$t("audiobooks.statusFailed")}
              {:else if progress.phase === "cancelled"}
                {$t("audiobooks.generationCancelled")}
              {:else if progress.totalChapters > 0}
                {$t("audiobooks.progressBlock", {
                  phase: progress.phase,
                  chapter: progress.chapter,
                  total: progress.totalChapters,
                })}
              {:else}
                {progress.percent > 0
                  ? $t("audiobooks.elapsed", { time: formatClock(elapsedSeconds) })
                  : ""}
              {/if}
            </span>
            <span>
              {#if progress.phase !== "done" && progress.phase !== "error" && etaSeconds > 0}
                {$t("audiobooks.eta", { time: formatClock(etaSeconds) })}
              {:else if progress.percent > 0}
                {$t("audiobooks.elapsed", { time: formatClock(elapsedSeconds) })}
              {/if}
            </span>
          </div>
        </div>
      </div>
      <div class="task-actions">
        {#if canControlGeneration && !cancelling && progress.phase !== "done" && progress.phase !== "error" && progress.phase !== "cancelled"}
          <Button
            variant="subtle"
            size="small"
            onclick={toggleGenerationPause}
            ariaLabel={isGenerationPaused
              ? $t("audiobooks.resumeGeneration")
              : $t("audiobooks.pauseGeneration")}
            title={isGenerationPaused
              ? $t("audiobooks.resumeGeneration")
              : $t("audiobooks.pauseGeneration")}
          >
            <Icon name={isGenerationPaused ? "play" : "pause"} size={14} />
            {isGenerationPaused
              ? $t("audiobooks.resumeGeneration")
              : $t("audiobooks.pauseGeneration")}
          </Button>
          <Button
            variant="danger"
            size="small"
            onclick={cancelGeneration}
            ariaLabel={$t("audiobooks.cancelGeneration")}
            title={$t("audiobooks.cancelGeneration")}
          >
            <Icon name="close" size={14} />
            {$t("audiobooks.cancelGeneration")}
          </Button>
        {:else if cancelling}
          <span class="task-cancelling">{$t("audiobooks.cancelling")}</span>
        {/if}
        <Button variant="subtle" size="small" onclick={() => (hiddenProgress = true)}>
          {$t("audiobooks.hide")}
        </Button>
      </div>
    </section>
  {/if}

  {#if hiddenProgress && progress && progress.phase !== "done" && progress.phase !== "error" && progress.phase !== "cancelled"}
    <button
      class="task-restore"
      type="button"
      onclick={() => (hiddenProgress = false)}
    >
      <span class="task-pip" class:paused={isGenerationPaused} aria-hidden="true"></span>
      <span>{$t("audiobooks.activeTask")} · {activeTaskTitle}</span>
      <strong>{$t("audiobooks.showTask")}</strong>
    </button>
  {/if}

  {#if error && !generateOpen && !importOpen}
    <p class="notice" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="ok" role="status">{notice}</p>
  {/if}

  {#if queue.length > 0}
    <section class="queue-panel" aria-label={$t("audiobooks.queueTitle")}>
      <header class="queue-head">
        <span class="queue-icon"><Icon name="clock" size={20} /></span>
        <div class="queue-heading">
          <h2>{$t("audiobooks.queueTitle")}</h2>
          <p>{$t("audiobooks.queueHelp")}</p>
        </div>
        <span class="queue-count">
          {$t("audiobooks.queueCount", { count: queue.length })}
        </span>
        <Button variant="subtle" size="small" onclick={clearGenerationQueue}>
          {$t("audiobooks.queueClear")}
        </Button>
      </header>
      <ol class="queue-list">
        {#each queue as item, index (item.id)}
          <li>
            <span class="queue-index" aria-hidden="true">{index + 1}</span>
            <span class="queue-cover">
              {#if coverSrc(item.coverPath)}
                <img src={coverSrc(item.coverPath)} alt="" />
              {:else}
                <Icon name="headphones" size={15} />
              {/if}
            </span>
            <span class="queue-copy">
              <strong>{item.bookTitle}</strong>
              <small>
                {item.voiceName ?? $t("audiobooks.voice")} · {queueScopeLabel(item.scope)}
                {#if item.narration}
                  · {queueNarrationLabel(item.narration)}
                {/if}
              </small>
            </span>
            {#if index === 0 && generationActive}
              <span class="queue-next">{$t("audiobooks.queueNext")}</span>
            {/if}
            {#if index > 0}
              <button
                class="queue-move"
                type="button"
                onclick={() => moveQueueItem(item, -1)}
                title={$t("audiobooks.queueMoveUp")}
                aria-label={$t("audiobooks.queueMoveUp")}
              >
                ↑
              </button>
            {/if}
            {#if index < queue.length - 1}
              <button
                class="queue-move"
                type="button"
                onclick={() => moveQueueItem(item, 1)}
                title={$t("audiobooks.queueMoveDown")}
                aria-label={$t("audiobooks.queueMoveDown")}
              >
                ↓
              </button>
            {/if}
            <button
              class="queue-remove"
              type="button"
              onclick={() => removeQueueItem(item)}
              title={$t("audiobooks.queueRemove")}
              aria-label={$t("audiobooks.queueRemove")}
            >
              <Icon name="trash" size={15} />
            </button>
          </li>
        {/each}
      </ol>
    </section>
  {/if}

  <section class="catalog-toolbar" aria-label={$t("audiobooks.library")}>
    <div class="view-switch" role="group" aria-label={$t("audiobooks.library")}>
      <button
        class:active={viewMode === "audiobooks"}
        aria-pressed={viewMode === "audiobooks"}
        onclick={() => (viewMode = "audiobooks")}
      >
        <Icon name="headphones" size={14} />
        {$t("audiobooks.viewAudiobooks")}
        <span>{audiobooks.length}</span>
      </button>
      <button
        class:active={viewMode === "albums"}
        aria-pressed={viewMode === "albums"}
        onclick={() => (viewMode = "albums")}
      >
        <Icon name="grid" size={14} />
        {$t("audiobooks.viewAlbums")}
        <span>{albums.length}</span>
      </button>
    </div>

    {#if viewMode === "audiobooks"}
      <div class="filter-tabs" role="group" aria-label={$t("audiobooks.library")}>
        {#each filterTabs as tab (tab.id)}
          <button
            class:active={filter === tab.id}
            aria-pressed={filter === tab.id}
            onclick={() => (filter = tab.id)}
          >
            {#if tab.id === "favorites"}
              <Icon name="star" size={13} />
            {/if}
            {tab.label}
            <span>{tab.count}</span>
          </button>
        {/each}
      </div>
      <div class="toolbar-tools">
        <label class="search-field">
          <Icon name="search" size={17} />
          <input
            type="search"
            bind:value={query}
            placeholder={$t("audiobooks.searchPlaceholder")}
            aria-label={$t("audiobooks.searchPlaceholder")}
          />
          {#if query}
            <button
              class="clear-button"
              type="button"
              onclick={() => (query = "")}
              aria-label={$t("common.close")}
              title={$t("common.close")}
            >
              <Icon name="close" size={14} />
            </button>
          {/if}
        </label>
        <label class="sort-field">
          <Icon name="sort" size={16} />
          <select bind:value={sort} aria-label={$t("audiobooks.sort")}>
            <option value="recent">{$t("audiobooks.sortRecent")}</option>
            <option value="title">{$t("audiobooks.sortTitle")}</option>
            <option value="duration">{$t("audiobooks.sortDuration")}</option>
            <option value="progress">{$t("audiobooks.sortProgress")}</option>
          </select>
        </label>
        <label class="sort-field">
          <Icon name="sparkles" size={16} />
          <select bind:value={categoryFilter} aria-label={$t("library.categories")}>
            <option value="all">{$t("library.allCategories")}</option>
            {#each categories as category (category.id)}
              <option value={category.id}>{category.name}</option>
            {/each}
          </select>
        </label>
      </div>
    {:else}
      <div class="toolbar-tools">
        <label class="search-field">
          <Icon name="search" size={17} />
          <input
            type="search"
            bind:value={albumQuery}
            placeholder={$t("audiobooks.albumSearchPlaceholder")}
            aria-label={$t("audiobooks.albumSearchPlaceholder")}
          />
          {#if albumQuery}
            <button
              class="clear-button"
              type="button"
              onclick={() => (albumQuery = "")}
              aria-label={$t("common.close")}
              title={$t("common.close")}
            >
              <Icon name="close" size={14} />
            </button>
          {/if}
        </label>
        <button class="new-album" type="button" onclick={() => openAlbumEditor()}>
          <Icon name="plus" size={16} />
          {$t("audiobooks.createAlbum")}
        </button>
      </div>
    {/if}
  </section>

  {#if !loaded}
    <div class="loading-state">
      <Spinner label={$t("common.loading")} />
    </div>
  {:else if viewMode === "albums"}
    {#if albums.length === 0}
      <EmptyState
        title={$t("audiobooks.albumsEmpty")}
        description={$t("audiobooks.albumsEmptyHelp")}
        action={$t("audiobooks.createAlbum")}
        onAction={() => openAlbumEditor()}
        icon="library"
      />
    {:else if filteredAlbums.length === 0}
      <EmptyState
        title={$t("audiobooks.albumSearchPlaceholder")}
        description={$t("audiobooks.albumsEmptyHelp")}
        action={$t("audiobooks.filterAll")}
        onAction={() => (albumQuery = "")}
        icon="search"
      />
    {:else}
      <div class="album-grid">
        {#each filteredAlbums as album (album.id)}
          <AudiobookAlbumCard
            {album}
            covers={albumCovers(album)}
            durationMs={albumDuration(album)}
            percent={albumPercent(album)}
            playing={playingAlbumId === album.id && Boolean(playback?.playing)}
            onOpen={() => openAlbum(album)}
            onPlay={() => void playAlbum(album)}
            onEdit={() => openAlbumEditor(album)}
            onDelete={() => void removeAlbum(album)}
          />
        {/each}
        <button
          class="quick-add"
          type="button"
          onclick={() => openAlbumEditor()}
        >
          <span class="quick-add-icon"><Icon name="plus" size={24} /></span>
          <strong>{$t("audiobooks.createAlbum")}</strong>
          <p>{$t("audiobooks.albumsEmptyHelp")}</p>
          <span class="quick-add-link">{$t("albums.title")}</span>
        </button>
      </div>
    {/if}
  {:else if audiobooks.length === 0}
    <EmptyState
      title={$t("audiobooks.empty")}
      description={$t("audiobooks.emptyHelp")}
      action={generationSupported && busy === null
        ? $t("audiobooks.generate")
        : undefined}
      onAction={() => {
        if (generationSupported && busy === null) openGenerator();
      }}
      icon="headphones"
    />
  {:else if filteredAudiobooks.length === 0}
    <EmptyState
      title={query ? $t("audiobooks.emptySearch") : $t("audiobooks.filterEmpty")}
      description={$t("audiobooks.emptyHelp")}
      action={$t("audiobooks.filterAll")}
      onAction={() => {
        filter = "all";
        query = "";
      }}
      icon="search"
    />
  {:else}
    <div class="audiobook-grid">
      {#each filteredAudiobooks as item (item.id)}
        <AudiobookCard
          {item}
          playing={playback?.audiobookId === item.id && playback.playing}
          busy={downloadingId === item.id || busy === item.id}
          favoriteBusy={favoriteBusy === item.id}
          categoriesLabel={categoryLabelFor(item)}
          onPlay={() => togglePlayback(item)}
          onRead={() => void openItemText(item)}
          onDownload={() => downloadAudiobook(item)}
          onChapters={() => openChapters(item)}
          onDelete={() => removeAudiobook(item)}
          onFavorite={() => void toggleFavorite(item)}
          onShare={() => openShare(item)}
          onClassify={() => void classifyAudiobook(item)}
          onRetry={() => void retryAudiobookItem(item)}
          onAlbum={() => openAlbumPicker(item)}
          onConvert={() => openConvert(item)}
        />
      {/each}
      {#if filter === "all" && !query.trim()}
        <button
          class="quick-add"
          type="button"
          onclick={openGenerator}
          disabled={!generationSupported || busy !== null}
        >
          <span class="quick-add-icon"><Icon name="plus" size={24} /></span>
          <strong>{$t("audiobooks.quickAdd")}</strong>
          <p>{$t("audiobooks.quickAddHelp")}</p>
          <span class="quick-add-link">{$t("audiobooks.quickAddAction")}</span>
        </button>
      {/if}
    </div>
  {/if}

  {#if playback}
    {#if playerExpanded}
      <section
        class="player-dock player-sheet"
        class:dragging={sheetDragging}
        style:transform={`translateY(${sheetDrag}px)`}
        aria-label={$t("audiobooks.nowPlaying")}
      >
        <div
          class="player-sheet-drag"
          onpointerdown={onSheetPointerDown}
          onpointermove={onSheetPointerMove}
          onpointerup={onSheetPointerEnd}
          onpointercancel={onSheetPointerEnd}
          aria-hidden="true"
        >
          <span class="sheet-grab-pill"></span>
        </div>
        <header class="player-sheet-top">
          <button
            class="sheet-minimize"
            type="button"
            onclick={minimizePlayer}
            aria-label={$t("audiobooks.minimizePlayer")}
            title={$t("audiobooks.minimizePlayer")}
          >
            <Icon name="chevron" size={17} />
            <span>{$t("audiobooks.minimizePlayer")}</span>
          </button>
          <span class="sheet-top-actions">
            <span class="sheet-kicker">{$t("audiobooks.nowPlaying")}</span>
            <button
              class="sheet-hide"
              type="button"
              onclick={hidePlayer}
              aria-label={$t("audiobooks.hidePlayer")}
              title={$t("audiobooks.hidePlayer")}
            >
              <Icon name="close" size={16} />
            </button>
          </span>
        </header>
      <div
        class="player-cover"
        role="img"
        aria-label={playback.title}
        onpointerdown={onSheetPointerDown}
        onpointermove={onSheetPointerMove}
        onpointerup={onSheetPointerEnd}
        onpointercancel={onSheetPointerEnd}
      >
        {#if coverSrc(playerCover)}
          <img src={coverSrc(playerCover)} alt="" />
        {:else}
          <span><Icon name="headphones" size={24} /></span>
        {/if}
      </div>
      <div class="player-main">
        <div class="player-head">
          <div class="player-titles">
            <p class="eyebrow">{$t("audiobooks.nowPlaying")}</p>
            <strong>{playback.title}</strong>
            <span>{playback.chapterIndex + 1} · {playback.chapterTitle}</span>
          </div>
          <div class="player-actions">
            {#if playerItem}
              <button
                class="voice-marks-button icon-only"
                class:favorite-active={playerItem.favorite}
                type="button"
                onclick={() => void toggleFavorite(playerItem)}
                disabled={favoriteBusy === playerItem.id}
                aria-pressed={playerItem.favorite}
                aria-label={playerItem.favorite
                  ? $t("audiobooks.removeFavorite")
                  : $t("audiobooks.addFavorite")}
                title={playerItem.favorite
                  ? $t("audiobooks.removeFavorite")
                  : $t("audiobooks.addFavorite")}
              >
                <Icon name="star" size={15} />
              </button>
              <button
                class="voice-marks-button icon-only"
                type="button"
                onclick={() => openShare(playerItem)}
                aria-label={$t("audiobooks.share")}
                title={$t("audiobooks.share")}
              >
                <Icon name="share" size={15} />
              </button>
            {/if}
            {#if backgroundPlayback}
              <button
                class="voice-marks-button icon-only"
                class:favorite-active={keepScreenOn}
                type="button"
                onclick={toggleKeepScreenOn}
                aria-pressed={keepScreenOn}
                aria-label={$t("audiobooks.keepScreenOn")}
                title={$t("audiobooks.keepScreenOn")}
              >
                <Icon name="sun" size={15} />
              </button>
            {/if}
            <button
              class="voice-marks-button"
              class:live={voiceListening}
              type="button"
              onclick={openVoiceMarks}
              aria-label={$t("audiobooks.voiceMarksTitle")}
              title={$t("audiobooks.voiceMarksTitle")}
            >
              <Icon name="mic" size={15} />
              <span>{$t("audiobooks.voiceMarkersButton")}</span>
              {#if voiceMarkersCount > 0}
                <span class="voice-count">{voiceMarkersCount}</span>
              {/if}
            </button>
            {#if voiceListening}
              <button
                class="voice-live-button"
                type="button"
                onclick={() => void toggleVoiceListening()}
                disabled={voiceToggling}
                title={$t("audiobooks.voiceStop")}
                aria-label={$t("audiobooks.voiceStop")}
              >
                <span class="live-dot" aria-hidden="true"></span>
                {$t("audiobooks.voiceListening")}
                <Icon name="stop" size={11} />
              </button>
            {/if}
            <label class="speed">
              <span>{$t("audiobooks.speed")}</span>
              <select
                value={speed}
                onchange={(event) => changeSpeed(Number(event.currentTarget.value))}
              >
                {#each [0.75, 1, 1.25, 1.5, 1.75, 2] as value (value)}
                  <option value={value}>{value}×</option>
                {/each}
              </select>
            </label>
            <label class="speed sleep">
              <span>{$t("audiobooks.sleepTimer")}</span>
              <select
                value={sleepMinutes}
                onchange={(event) => startSleepTimer(Number(event.currentTarget.value))}
              >
                <option value={0}>{$t("audiobooks.sleepOff")}</option>
                <option value={15}>{$t("audiobooks.sleep15")}</option>
                <option value={30}>{$t("audiobooks.sleep30")}</option>
                <option value={60}>{$t("audiobooks.sleep60")}</option>
              </select>
              {#if sleepRemaining > 0}<small>{formatSleep(sleepRemaining)}</small>{/if}
            </label>
            <Button variant="subtle" size="small" onclick={() => void openCurrentText()}>
              {$t("audiobooks.openText")}
            </Button>
            <Button variant="subtle" size="small" onclick={stopPlayback}>
              {$t("audiobooks.stop")}
            </Button>
          </div>
        </div>

        <div class="player-controls">
          <button class="skip" onclick={() => skip(-30)} aria-label={$t("audiobooks.back30")}>
            <span aria-hidden="true">−30</span>
          </button>
          <button
            class="player-toggle"
            onclick={togglePlayer}
            aria-label={playback.playing ? $t("audiobooks.pause") : $t("audiobooks.play")}
          >
            <Icon name={playback.playing ? "pause" : "play"} size={20} />
          </button>
          <button class="skip" onclick={() => skip(30)} aria-label={$t("audiobooks.forward30")}>
            <span aria-hidden="true">+30</span>
          </button>
          <span class="time">{formatDuration(playback.positionMs)}</span>
          <div class="waveform-block">
            {#if waveform}
              <div
                class="waveform"
                role="img"
                aria-label={$t("audiobooks.waveformLabel")}
              >
                <svg
                  class="wave-svg"
                  viewBox="0 0 100 100"
                  preserveAspectRatio="none"
                  aria-hidden="true"
                >
                  <defs>
                    <clipPath id="audiobook-wave-clip">
                      <rect x="0" y="0" width={waveformPercent} height="100" />
                    </clipPath>
                  </defs>
                  <g>{@render waveBars("wave-bar")}</g>
                  <g clip-path="url(#audiobook-wave-clip)">
                    {@render waveBars("wave-bar played")}
                  </g>
                </svg>
                {#if voiceMarkersCount > 0 && playback.durationMs > 0}
                  <div class="voice-ticks" aria-hidden="true">
                    {#each voiceMarkers as marker (marker.id)}
                      <span
                        style={`left:${Math.min(100, Math.max(0, (marker.positionMs / playback.durationMs) * 100))}%`}
                      ></span>
                    {/each}
                  </div>
                {/if}
              </div>
            {:else if waveformLoading}
              <div class="waveform-skeleton" aria-hidden="true"></div>
            {:else}
              <div class="waveform-empty" aria-hidden="true"></div>
            {/if}
            <input
              class="range"
              type="range"
              min="0"
              max={playback.durationMs}
              value={seekValue}
              oninput={(event) => {
                scrubbing = true;
                seekValue = Number(event.currentTarget.value);
              }}
              onchange={(event) => {
                scrubbing = false;
                void onSeek(Number(event.currentTarget.value));
              }}
              aria-label={$t("audiobooks.position")}
            />
          </div>
          <span class="time">{formatDuration(playback.durationMs)}</span>
        </div>

        {#if karaokePrepared.length > 0}
          <div class="karaoke-panel">
            <div class="karaoke-head">
              <span class="karaoke-kicker">
                <Icon name="book" size={13} />
                {$t("audiobooks.karaokeTitle")}
              </span>
              {#if karaokePrecisionKey}
                <span
                  class="karaoke-precision"
                  title={$t(
                    karaoke?.precision === "estimated"
                      ? "audiobooks.karaokeEstimatedHelp"
                      : karaoke?.precision === "timed"
                        ? "audiobooks.karaokeTimedHelp"
                        : "audiobooks.karaokeHelp",
                  )}
                >
                  {$t(karaokePrecisionKey)}
                </span>
              {/if}
              <button
                class="karaoke-open"
                type="button"
                onclick={() => (karaokeOpen = true)}
              >
                <Icon name="expand" size={13} />
                {$t("audiobooks.karaokeOpen")}
              </button>
            </div>
            <div class="karaoke-lines">
              {#if karaokePrevious}
                <p class="karaoke-line prev">{karaokePrevious.text}</p>
              {/if}
              {#if karaokeLine}
                <p class="karaoke-line now">
                  {#each karaokeLine.segments as segment, index (index)}
                    {#if segment.wordIndex === null}
                      {segment.text}
                    {:else}
                      <span
                        class="karaoke-word"
                        class:spoken={segment.wordIndex < karaokeWordIndex}
                        class:active={segment.wordIndex === karaokeWordIndex}
                      >{segment.text}</span>
                    {/if}
                  {/each}
                </p>
              {/if}
              {#if karaokeNext}
                <p class="karaoke-line next">{karaokeNext.text}</p>
              {/if}
            </div>
          </div>
        {:else if karaokeLoading}
          <div
            class="karaoke-panel karaoke-skeleton"
            role="status"
            aria-label={$t("audiobooks.karaokeLoading")}
          >
            <span aria-hidden="true"></span>
            <span aria-hidden="true"></span>
          </div>
        {/if}

        {#if chapters.length > 0}
          <details class="chapter-drawer">
            <summary>
              {$t("audiobooks.chaptersToggle", { count: chapters.length })}
            </summary>
            <div class="chapter-list" aria-label={$t("audiobooks.chapters")}>
              {#each chapters as chapter (chapter.index)}
                <button
                  class="chapter"
                  class:current={chapter.index === playback.chapterIndex}
                  onclick={() => onSeek(chapter.startMs)}
                >
                  <span>{chapter.index + 1}</span>
                  <strong>{chapter.title}</strong>
                  <small>{formatDuration(chapter.startMs)}</small>
                </button>
              {/each}
            </div>
          </details>
        {:else}
          <p class="hint">{$t("audiobooks.noChapters")}</p>
        {/if}
      </div>
      </section>
    {:else if !$audiobookPlayerHidden}
      <section class="player-mini" aria-label={$t("audiobooks.nowPlaying")}>
        <button
          class="mini-open"
          type="button"
          onclick={openPlayer}
          aria-label={$t("audiobooks.expandPlayer")}
          title={$t("audiobooks.expandPlayer")}
        >
          <span class="mini-cover">
            {#if coverSrc(playerCover)}
              <img src={coverSrc(playerCover)} alt="" />
            {:else}
              <Icon name="headphones" size={22} />
            {/if}
          </span>
          <span class="mini-copy">
            <strong>{playback.title}</strong>
            <small>{playback.chapterIndex + 1} · {playback.chapterTitle}</small>
          </span>
        </button>
        <button
          class="mini-toggle"
          type="button"
          onclick={togglePlayer}
          aria-label={playback.playing ? $t("audiobooks.pause") : $t("audiobooks.play")}
        >
          <Icon name={playback.playing ? "pause" : "play"} size={18} />
        </button>
        <button
          class="mini-expand"
          type="button"
          onclick={openPlayer}
          aria-label={$t("audiobooks.expandPlayer")}
          title={$t("audiobooks.expandPlayer")}
        >
          <Icon name="chevron" size={16} />
        </button>
        <button
          class="mini-hide"
          type="button"
          onclick={hidePlayer}
          aria-label={$t("audiobooks.hidePlayer")}
          title={$t("audiobooks.hidePlayer")}
        >
          <Icon name="close" size={15} />
        </button>
        <span class="mini-progress" style:width={`${waveformPercent}%`}></span>
      </section>
    {/if}
  {/if}

  <section class="tools-section">
    <div class="tools-head">
      <div>
        <h2>{$t("audiobooks.toolsTitle")}</h2>
        <p>{$t("audiobooks.toolsHelp")}</p>
      </div>
      <span class="tools-status">
        <span class="status-dot" aria-hidden="true"></span>
        {$t("audiobooks.onDevice")}
      </span>
    </div>

    <div class="tools-grid">
      <article class="tool-card">
        <div class="tool-top">
          <span class="tool-icon tone-primary"><Icon name="waveform" size={21} /></span>
          <span class="tool-badge">Whisper Base</span>
        </div>
        <h3>{$t("audiobooks.toolTranscribe")}</h3>
        <p>{$t("audiobooks.toolTranscribeHelp")}</p>
        <button
          class="tool-link"
          type="button"
          onclick={() => openImport("transcribe")}
          disabled={!transcriptionSupported || busy !== null}
        >
          {$t("audiobooks.toolTranscribeAction")} <span aria-hidden="true">→</span>
        </button>
      </article>

      <article class="tool-card">
        <div class="tool-top">
          <span class="tool-icon tone-tertiary"><Icon name="chapters" size={21} /></span>
          <span class="tool-badge">{$t("audiobooks.chapters")}</span>
        </div>
        <h3>{$t("audiobooks.toolChapters")}</h3>
        <p>{$t("audiobooks.toolChaptersHelp")}</p>
        <button class="tool-link" type="button" onclick={openChaptersTool}>
          {$t("audiobooks.toolChaptersAction")} <span aria-hidden="true">→</span>
        </button>
      </article>

      <article class="tool-card">
        <div class="tool-top">
          <span class="tool-icon tone-secondary"><Icon name="mic" size={21} /></span>
          <span class="tool-badge">
            {voiceListening ? $t("audiobooks.voiceListening") : voiceModelLabel}
          </span>
        </div>
        <h3>{$t("audiobooks.toolVoiceMarks")}</h3>
        <p>{$t("audiobooks.toolVoiceMarksHelp")}</p>
        <button
          class="tool-link"
          type="button"
          onclick={openVoiceMarks}
          disabled={!voiceSupported || busy !== null}
        >
          {$t("audiobooks.toolVoiceMarksAction")} <span aria-hidden="true">→</span>
        </button>
      </article>

      <article class="tool-card">
        <div class="tool-top">
          <span class="tool-icon tone-primary"><Icon name="sliders" size={21} /></span>
          <span class="tool-badge">LUFS + EQ</span>
        </div>
        <h3>{$t("audiobooks.toolDsp")}</h3>
        <p>{$t("audiobooks.toolDspHelp")}</p>
        <button class="tool-link" type="button" onclick={() => openTreatment()}>
          {$t("audiobooks.toolDspAction")} <span aria-hidden="true">→</span>
        </button>
      </article>
    </div>
  </section>

  <section class="weave-panel">
    <div class="weave-head">
      <span class="weave-icon"><Icon name="diary" size={20} /></span>
      <div>
        <h2>{$t("audiobooks.weavePanel")}</h2>
        <p>{$t("audiobooks.weaveHelp")}</p>
      </div>
      <span class="weave-count">
        {$t("audiobooks.weaveSummary", { count: selectedExtracts.length })}
      </span>
    </div>

    <div class="weave-form">
      <label class="grow">
        <span>{$t("audiobooks.miniBookTitle")}</span>
        <input
          bind:value={weaveTitle}
          maxlength="120"
          placeholder={$t("audiobooks.placeholderTitle")}
        />
      </label>
      <label class="grow">
        <span>{$t("audiobooks.filterExtracts")}</span>
        <input bind:value={extractFilter} placeholder={$t("audiobooks.placeholderFilter")} />
      </label>
      <label class="grow">
        <span>{$t("audiobooks.voice")}</span>
        <select bind:value={voiceId}>
          {#each installedVoices as voice (voice.id)}
            <option value={voice.id}>{voiceLabel(voice)}</option>
          {/each}
        </select>
      </label>
      <Button
        variant="primary"
        onclick={weave}
        loading={busy === "weave"}
        disabled={
          busy !== null ||
          !platformSupported ||
          !ffmpegReady ||
          selectedExtracts.length === 0 ||
          installedVoices.length === 0
        }
      >
        {$t("audiobooks.weaveCount", { count: selectedExtracts.length })}
      </Button>
    </div>

    {#if extracts.length === 0}
      <p class="empty">{$t("audiobooks.noExtracts")}</p>
    {:else if filteredExtracts.length === 0}
      <p class="empty">{$t("audiobooks.searchEmpty")}</p>
    {:else}
      <div class="extracts">
        {#each filteredExtracts as extract (extract.id)}
          <label class="extract">
            <input
              type="checkbox"
              checked={selectedExtracts.includes(extract.id)}
              onchange={() => toggleExtract(extract.id)}
            />
            <div>
              <span class="extract-book">
                {booksById.get(extract.bookId)?.title ?? $t("audiobooks.book")}
              </span>
              <span class="extract-text">{excerpt(extract.text)}</span>
              {#if extract.note}
                <span class="note">{extract.note}</span>
              {/if}
            </div>
            <button
              class="remove-extract"
              type="button"
              onclick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                void removeExtract(extract);
              }}
            >
              {$t("common.delete")}
            </button>
          </label>
        {/each}
      </div>
    {/if}
  </section>

  <footer class="stats-line">
    <span>
      {$t("audiobooks.stats", {
        extracts: extracts.length,
        audiobooks: audiobooks.length,
        books: books.length,
      })}
    </span>
    {#if books.length > 0}
      <span>
        {$t("audiobooks.librarySize", {
          size: formatBytes(books.reduce((sum, book) => sum + book.sizeBytes, 0)),
        })}
      </span>
    {/if}
  </footer>
</section>

<AudiobookGeneratorModal
  open={generateOpen}
  {books}
  {voices}
  {sections}
  busy={busy}
  error={error}
  initialBookId={selectedBookId}
  initialVoiceId={voiceId}
  onClose={() => {
    generateOpen = false;
    error = null;
  }}
  onBookChange={onBookChange}
  onGenerate={generate}
  onManageVoices={() => {
    generateOpen = false;
    onNavigate?.("voice");
  }}
/>

<AudiobookImportModal
  open={importOpen}
  busy={busy === "import"}
  {capabilities}
  initialKind={importInitialKind}
  whisperModel={whisperBase}
  whisperTiny={whisperTiny}
  whisperProgress={whisperDownloadPercent}
  whisperTinyProgress={whisperTinyDownloadPercent}
  aiStatus={aiState}
  error={error}
  onClose={() => {
    importOpen = false;
    error = null;
  }}
  onChoose={(kind, detection, modelId) =>
    void importAudiobook(kind, detection, modelId)}
  onDownloadWhisper={() => void downloadWhisper("base")}
  onDownloadWhisperModel={(modelId) => void downloadWhisper(modelId)}
/>

<AudiobookChaptersModal
  open={chaptersOpen}
  audiobooks={audiobooks.filter((item) => item.status === "done")}
  selectedId={modalChapterId}
  chapters={modalChapters}
  loading={modalChaptersLoading}
  busy={busy === "reanalyze" || convertBusy}
  onClose={() => (chaptersOpen = false)}
  onSelect={(id) => void loadModalChapters(id)}
  onPlayChapter={(chapter) => void playChapter(chapter)}
  onEdit={(item) => void openChapterEditor(item.id)}
  onReanalyze={(item, mode) => void reanalyzeChapters(item.id, mode)}
  onTranscribe={(item) => openConvert(item)}
/>

<AudiobookChapterEditor
  open={chapterEditorOpen}
  title={chapterEditorItem?.bookTitle ?? ""}
  durationMs={chapterEditorItem?.durationMs ?? 0}
  chapters={chapterEditorChapters}
  playbackPositionMs={chapterEditorPlayhead}
  busy={savingChapters}
  error={chapterEditorError}
  onClose={() => (chapterEditorOpen = false)}
  onSave={(chapters) => void saveChapters(chapters)}
/>

<AudiobookTreatmentModal
  open={treatmentOpen}
  audiobooks={audiobooks.filter((item) => item.status === "done")}
  initialId={treatmentInitialId}
  busy={treating}
  {error}
  onClose={() => {
    treatmentOpen = false;
    error = null;
  }}
  onApply={(id, treatment) => void applyTreatment(id, treatment)}
/>

<AudiobookKaraokeModal
  open={karaokeOpen}
  title={karaoke?.title ?? playback?.chapterTitle ?? ""}
  precision={karaoke?.precision ?? "estimated"}
  lines={karaokePrepared}
  activeIndex={karaokeLineIndex}
  activeWord={karaokeWordIndex}
  startMs={karaoke?.startMs ?? 0}
  onClose={() => (karaokeOpen = false)}
  onSeek={(position) => void onSeek(position)}
  onOpenText={() => {
    karaokeOpen = false;
    void openCurrentText();
  }}
/>

<AudiobookShareModal
  open={shareOpen}
  audiobook={shareItem}
  busy={sharing}
  error={shareError}
  result={shareResult}
  onClose={() => {
    shareOpen = false;
    shareError = null;
  }}
  onCreate={shareAudiobook}
/>

<AudiobookVoiceMarksModal
  open={voiceMarksOpen}
  audiobook={playerItem}
  chapters={chapters}
  markers={voiceMarkers}
  status={voiceStatus}
  phrase={voicePhrase}
  whisperDownloading={voiceDownloadProgress}
  busy={voiceToggling || voiceMarkersLoading}
  canListen={Boolean(playback) && voiceSupported}
  externalError={error}
  onClose={() => (voiceMarksOpen = false)}
  onSavePhrase={saveMarkerPhrase}
  onToggleListening={toggleVoiceListening}
  onDownloadWhisper={downloadWhisper}
  onJump={jumpToVoiceMarker}
  onDelete={removeVoiceMarker}
/>

<AudiobookTranscribeModal
  open={convertItem !== null}
  audiobook={convertItem}
  whisperModel={whisperBase}
  whisperTiny={whisperTiny}
  whisperProgress={whisperDownloadPercent}
  whisperTinyProgress={whisperTinyDownloadPercent}
  busy={convertBusy}
  onClose={() => {
    if (convertBusy) return;
    convertItem = null;
  }}
  onConfirm={(modelId) => void confirmConvert(modelId)}
  onDownloadModel={(modelId) => void downloadWhisper(modelId)}
/>

<AudiobookAlbumEditorModal
  open={albumEditorOpen}
  album={albumEditing}
  busy={albumEditorBusy}
  error={albumEditorError}
  onClose={() => {
    if (albumEditorBusy) return;
    albumEditorOpen = false;
    albumEditing = null;
    albumEditorError = null;
  }}
  onSave={saveAlbum}
/>

<AudiobookAlbumModal
  open={albumDetail !== null}
  album={albumDetail}
  items={albumDetail ? albumItems(albumDetail) : []}
  busy={albumDetail ? albumBusy === albumDetail.id : false}
  onClose={() => (albumDetailId = null)}
  onPlay={() => {
    if (albumDetail) void playAlbum(albumDetail);
  }}
  onPlayItem={(item) => {
    if (albumDetail) void playAlbumItem(albumDetail, item);
  }}
  onEdit={() => {
    if (albumDetail) openAlbumEditor(albumDetail);
  }}
  onAdd={() => (albumAddOpen = true)}
  onRemove={(item) => {
    if (albumDetail) void removeAlbumItem(albumDetail, item);
  }}
  onMove={(item, direction) => {
    if (albumDetail) void moveAlbumItem(albumDetail, item, direction);
  }}
/>

<AudiobookAlbumAddModal
  open={albumAddOpen && albumDetail !== null}
  album={albumDetail}
  audiobooks={audiobooks}
  busy={albumDetail ? albumBusy === albumDetail.id : false}
  onClose={() => (albumAddOpen = false)}
  onAdd={(ids) => void addAlbumItems(ids)}
/>

<AudiobookAlbumPickerModal
  open={albumPickerFor !== null}
  albums={albums}
  audiobook={albumPickerFor}
  busy={albumPickerBusy}
  error={albumPickerError}
  onClose={() => {
    if (albumPickerBusy) return;
    albumPickerFor = null;
    albumPickerError = null;
  }}
  onSave={(ids) => void saveAlbumPicker(ids)}
/>

<style>
  .audiobooks {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .page-head {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem 1.5rem;
  }

  .title-block {
    max-width: 64ch;
  }

  .engine-eyebrow {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .pulse-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent-2);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 14%, transparent);
    animation: pulse 2.2s ease-in-out infinite;
  }

  .head-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }

  .format-chip {
    display: inline-flex;
    align-items: center;
    min-height: 20px;
    padding: 0.06rem 0.4rem;
    border-radius: 5px;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .notice,
  .ok {
    margin: 0;
    padding: 0.7rem 0.9rem;
    border-radius: 10px;
    font-size: 0.84rem;
  }

  .notice {
    border: 1px solid color-mix(in srgb, var(--danger) 55%, var(--border));
    background: color-mix(in srgb, var(--danger) 9%, transparent);
  }

  .notice.soft {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--muted);
  }

  .ok {
    border: 1px solid color-mix(in srgb, var(--ok) 55%, var(--border));
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }

  .active-task {
    position: relative;
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    overflow: hidden;
    padding: 14px 16px;
    border: 1px solid var(--accent-line);
    border-radius: var(--radius-lg);
    background:
      radial-gradient(
        300px 140px at 100% 0%,
        color-mix(in srgb, var(--accent) 12%, transparent),
        transparent 70%
      ),
      color-mix(in srgb, var(--surface-2) 70%, var(--surface));
    box-shadow: var(--shadow-sm);
  }

  .active-task.error {
    border-color: color-mix(in srgb, var(--danger) 50%, var(--border));
    background: color-mix(in srgb, var(--danger) 5%, var(--surface));
  }

  .task-cover {
    width: 64px;
    height: 64px;
    overflow: hidden;
    border-radius: 10px;
    background: var(--surface-3);
    box-shadow: var(--shadow-xs);
  }

  .task-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .task-cover span {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
  }

  .task-main {
    display: flex;
    flex-direction: column;
    gap: 9px;
    min-width: 0;
  }

  .task-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    min-width: 0;
  }

  .task-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 23px;
    padding: 0.1rem 0.5rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    color: var(--accent);
    font-size: 0.66rem;
    font-weight: 700;
  }

  .task-chip.error {
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    color: var(--danger);
  }

  .task-chip.paused {
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    color: var(--warn);
  }

  .task-pip {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    animation: pulse 1.4s ease-in-out infinite;
  }

  .task-pip.paused {
    animation: none;
  }

  .task-head strong {
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 620;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-voice {
    color: var(--muted);
    font-size: 0.7rem;
  }

  .task-progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .task-progress-head,
  .task-progress-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    color: var(--muted);
    font-size: 0.66rem;
  }

  .task-progress-head strong {
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }

  .task-track {
    width: 100%;
    height: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .task-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--gold), var(--accent-2));
    box-shadow: 0 0 10px -2px color-mix(in srgb, var(--gold) 60%, transparent);
    transition: width 300ms ease;
  }

  .task-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }

  .task-cancelling {
    color: var(--muted);
    font-size: 0.68rem;
  }

  .task-restore {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 36px;
    padding: 0.35rem 0.75rem;
    border: 1px solid var(--accent-line);
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--text);
    font: inherit;
    font-size: 0.72rem;
    text-align: left;
  }

  .task-restore span:nth-child(2) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-restore strong {
    margin-left: auto;
    color: var(--accent);
    font-size: 0.65rem;
    white-space: nowrap;
  }

  .queue-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .queue-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .queue-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 38px;
    height: 38px;
    border: 1px solid var(--accent-line);
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .queue-heading {
    flex: 1;
    min-width: 160px;
  }

  .queue-heading h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.02rem;
    font-weight: 620;
    letter-spacing: -0.012em;
  }

  .queue-heading p {
    margin: 1px 0 0;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.4;
  }

  .queue-count {
    flex: 0 0 auto;
    padding: 0.12rem 0.5rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.64rem;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }

  .queue-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 320px;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  .queue-list li {
    display: grid;
    grid-template-columns: 22px 34px minmax(0, 1fr) auto 30px;
    align-items: center;
    gap: 9px;
    min-height: 46px;
    padding: 5px 9px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: var(--surface-2);
  }

  .queue-list li:hover {
    border-color: var(--border);
  }

  .queue-index {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
    font-size: 0.62rem;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }

  .queue-cover {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    overflow: hidden;
    border-radius: 7px;
    background: var(--surface-3);
    color: var(--muted);
  }

  .queue-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .queue-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .queue-copy strong {
    overflow: hidden;
    font-size: 0.79rem;
    font-weight: 640;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .queue-copy small {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.66rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .queue-next {
    flex: 0 0 auto;
    padding: 0.1rem 0.42rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .queue-move,
  .queue-remove {
    display: grid;
    place-items: center;
    width: 28px;
    min-width: 28px;
    height: 28px;
    min-height: 28px;
    padding: 0;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.85rem;
    font-weight: 700;
  }

  .queue-move:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .queue-remove:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    color: var(--danger);
  }

  .catalog-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .view-switch {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--accent-line);
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 7%, var(--surface-2));
  }

  .view-switch button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0.3rem 0.72rem;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 650;
    white-space: nowrap;
  }

  .view-switch button:hover {
    color: var(--text);
  }

  .view-switch button.active {
    background: var(--accent);
    color: var(--accent-contrast);
    box-shadow: var(--shadow-xs);
  }

  .view-switch button span {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    border-radius: 999px;
    background: color-mix(in srgb, currentColor 14%, transparent);
    font-size: 0.6rem;
    font-variant-numeric: tabular-nums;
  }

  .new-album {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 13px;
    border: 1px solid var(--accent-line);
    border-radius: 11px;
    background: color-mix(in srgb, var(--accent) 9%, var(--surface));
    color: var(--accent);
    font: inherit;
    font-size: 0.74rem;
    font-weight: 650;
  }

  .new-album:hover {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .album-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
    align-items: stretch;
  }

  .filter-tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    max-width: 100%;
    padding: 4px;
    overflow-x: auto;
    border: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    border-radius: 14px;
    background: var(--surface-2);
    scrollbar-width: none;
  }

  .filter-tabs::-webkit-scrollbar {
    display: none;
  }

  .filter-tabs button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
    min-height: 32px;
    padding: 0.3rem 0.68rem;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .filter-tabs button:hover {
    color: var(--text);
    background: color-mix(in srgb, var(--surface) 55%, transparent);
  }

  .filter-tabs button.active {
    border-color: var(--border);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-xs);
  }

  .filter-tabs button span {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    border-radius: 999px;
    background: color-mix(in srgb, currentColor 10%, transparent);
    font-size: 0.6rem;
    font-variant-numeric: tabular-nums;
  }

  .toolbar-tools {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .search-field,
  .sort-field {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 7px;
    min-height: 40px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .search-field:focus-within {
    border-color: var(--accent);
    background: var(--surface);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .search-field input {
    width: min(240px, 40vw);
    min-height: 36px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    outline: none;
  }

  .search-field input::-webkit-search-cancel-button {
    display: none;
  }

  .clear-button {
    display: grid;
    place-items: center;
    width: 24px;
    min-width: 24px;
    height: 24px;
    min-height: 24px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
  }

  .clear-button:hover {
    color: var(--text);
  }

  .sort-field select {
    min-height: 36px;
    padding: 0 2px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
    outline: none;
  }

  .loading-state {
    display: grid;
    place-items: center;
    min-height: 220px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface-2) 45%, transparent);
  }

  .audiobook-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(370px, 1fr));
    gap: 14px;
    align-items: stretch;
  }

  .quick-add {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 190px;
    padding: 22px;
    border: 2px dashed var(--border-warm);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface-2) 56%, transparent);
    color: var(--text);
    text-align: center;
    transition:
      border-color 160ms ease,
      background-color 160ms ease,
      transform 160ms ease;
  }

  .quick-add:hover:not(:disabled) {
    transform: translateY(-2px);
    border-color: var(--accent);
    background: var(--surface-2);
  }

  .quick-add:disabled {
    cursor: default;
    opacity: 0.58;
  }

  .quick-add-icon {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    margin-bottom: 5px;
    border-radius: 50%;
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-sm);
  }

  .quick-add strong {
    font-family: var(--font-display);
    font-size: 1.12rem;
    font-weight: 620;
    letter-spacing: -0.012em;
  }

  .quick-add p {
    max-width: 34ch;
    margin: 0;
    color: var(--muted);
    font-size: 0.74rem;
    line-height: 1.45;
  }

  .quick-add-link {
    margin-top: 4px;
    color: var(--accent);
    font-size: 0.72rem;
    font-weight: 650;
  }

  .player-dock {
    position: sticky;
    bottom: 1rem;
    z-index: 30;
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr);
    align-items: center;
    gap: 1rem;
    padding: 0.85rem 1rem;
    border: 1px solid var(--accent-line);
    border-radius: 18px;
    background: color-mix(in srgb, var(--surface) 98%, transparent);
    box-shadow: var(--shadow-lg);
  }

  .player-cover {
    width: 76px;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: 0 8px 18px -12px var(--shadow-color);
  }

  .player-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .player-cover span {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
  }

  .player-main {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .player-head {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem 1rem;
  }

  .player-titles {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .player-titles .eyebrow {
    color: var(--accent);
  }

  .player-titles strong {
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 650;
    line-height: 1.25;
  }

  .player-titles span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: 0.73rem;
  }

  .player-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
  }

  .voice-marks-button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 34px;
    padding: 0 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
    transition:
      border-color 150ms ease,
      background-color 150ms ease;
  }

  .voice-marks-button:hover {
    border-color: var(--accent-line);
    background: var(--surface-3);
  }

  .voice-marks-button.live {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .voice-marks-button.icon-only {
    justify-content: center;
    width: 34px;
    min-width: 34px;
    padding: 0;
  }

  .voice-marks-button.favorite-active {
    border-color: color-mix(in srgb, var(--gold) 45%, var(--border));
    background: color-mix(in srgb, var(--gold) 10%, var(--surface));
    color: #d4921f;
  }

  .voice-marks-button.favorite-active :global(svg) {
    fill: currentColor;
  }

  .voice-count {
    display: grid;
    place-items: center;
    min-width: 17px;
    height: 17px;
    padding: 0 3px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--surface);
    font-size: 0.56rem;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
  }

  .voice-live-button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 34px;
    padding: 0 0.55rem;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: 0.66rem;
    font-weight: 700;
  }

  .live-dot {
    display: inline-block;
    flex: 0 0 auto;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: voice-pulse 1.6s ease-out infinite;
  }

  .voice-ticks {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .voice-ticks span {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    transform: translateX(-1px);
    background: color-mix(in srgb, var(--accent-3, var(--accent)) 80%, #fff);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 55%, transparent);
    opacity: 0.9;
  }

  @keyframes voice-pulse {
    0% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 45%, transparent);
    }
    70% {
      box-shadow: 0 0 0 6px transparent;
    }
    100% {
      box-shadow: 0 0 0 0 transparent;
    }
  }

  .speed {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .speed select {
    min-height: 34px;
    padding: 0.25rem 0.4rem;
    border-radius: 8px;
    background: var(--surface-2);
    font-size: 0.76rem;
  }

  .speed.sleep small {
    color: var(--accent);
    font-size: 0.6rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .skip {
    display: grid;
    place-items: center;
    width: 34px;
    min-width: 34px;
    height: 34px;
    min-height: 34px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.58rem;
    font-weight: 700;
  }

  .skip:hover {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .player-controls {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .player-toggle {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 44px;
    min-width: 44px;
    height: 44px;
    min-height: 44px;
    padding: 0;
    border: 1px solid color-mix(in srgb, var(--gold-deep) 68%, #1a0f0b);
    border-radius: 50%;
    background: linear-gradient(135deg, var(--gold), var(--gold-deep));
    color: #fff;
    box-shadow:
      0 10px 24px -12px color-mix(in srgb, var(--gold-deep) 70%, transparent),
      0 0 18px -5px color-mix(in srgb, var(--gold) 45%, transparent);
  }

  .time {
    flex: 0 0 auto;
    min-width: 2.6rem;
    color: var(--muted);
    font-size: 0.68rem;
    font-variant-numeric: tabular-nums;
  }

  .waveform-block {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .waveform {
    position: relative;
    width: 100%;
    height: 34px;
    overflow: hidden;
    border-radius: 6px;
    background: color-mix(in srgb, var(--surface-3) 60%, transparent);
  }

  .wave-svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  .wave-bar {
    fill: color-mix(in srgb, var(--muted) 40%, transparent);
  }

  .wave-bar.played {
    fill: var(--accent);
  }

  .waveform-skeleton,
  .waveform-empty {
    width: 100%;
    height: 34px;
    border-radius: 6px;
  }

  .waveform-skeleton {
    background: linear-gradient(
      90deg,
      var(--surface-3),
      var(--surface-2),
      var(--surface-3)
    );
    background-size: 200% 100%;
    animation: wave-shimmer 1.4s linear infinite;
  }

  .waveform-empty {
    background: var(--surface-3);
    opacity: 0.5;
  }

  .waveform-block .range {
    flex: none;
    width: 100%;
  }

  .range {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }

  .karaoke-panel {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 9px 11px 10px;
    border: 1px solid color-mix(in srgb, var(--border) 62%, transparent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface-2) 72%, transparent);
  }

  .karaoke-head {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .karaoke-kicker {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--accent);
    font-size: 0.61rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .karaoke-precision {
    padding: 1px 6px;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .karaoke-open {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    padding: 0.22rem 0.48rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.64rem;
    font-weight: 650;
    white-space: nowrap;
  }

  .karaoke-open:hover {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .karaoke-lines {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .karaoke-line {
    margin: 0;
    overflow-wrap: anywhere;
    line-height: 1.45;
  }

  .karaoke-line.prev,
  .karaoke-line.next {
    overflow: hidden;
    color: color-mix(in srgb, var(--muted) 84%, transparent);
    font-size: 0.7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .karaoke-line.next {
    opacity: 0.74;
  }

  .karaoke-line.now {
    align-self: flex-start;
    max-width: 100%;
    margin-left: -7px;
    padding: 2px 7px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.97rem;
  }

  .karaoke-word {
    border-radius: 3px;
    transition:
      background-color 110ms ease,
      color 110ms ease,
      box-shadow 110ms ease;
  }

  .karaoke-word.spoken {
    color: color-mix(in srgb, var(--text) 70%, var(--muted));
  }

  .karaoke-word.active {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
  }

  .karaoke-skeleton {
    gap: 7px;
    padding: 12px;
  }

  .karaoke-skeleton span {
    display: block;
    height: 9px;
    border-radius: 4px;
    background: linear-gradient(
      90deg,
      var(--surface-3),
      var(--surface-2),
      var(--surface-3)
    );
    background-size: 200% 100%;
    animation: wave-shimmer 1.4s linear infinite;
  }

  .karaoke-skeleton span:first-child {
    width: 38%;
  }

  .karaoke-skeleton span:last-child {
    width: 86%;
  }

  .chapter-drawer {
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    padding-top: 0.45rem;
  }

  .chapter-drawer summary {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.72rem;
    cursor: pointer;
    list-style: none;
  }

  .chapter-drawer summary::-webkit-details-marker {
    display: none;
  }

  .chapter-drawer summary:hover {
    color: var(--text);
  }

  .chapter-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    max-height: 220px;
    margin-top: 0.45rem;
    overflow-y: auto;
  }

  .chapter {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.5rem;
    min-height: 38px;
    padding: 0.3rem 0.5rem;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .chapter:hover {
    background: var(--surface-2);
  }

  .chapter.current {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .chapter span {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.66rem;
  }

  .chapter.current span {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .chapter strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.78rem;
    font-weight: 500;
  }

  .chapter small {
    color: var(--muted);
    font-size: 0.66rem;
  }

  .tools-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 6px;
  }

  .tools-head {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 8px 16px;
  }

  .tools-head h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.35rem;
    font-weight: 620;
    letter-spacing: -0.018em;
  }

  .tools-head p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .tools-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 14%, transparent);
  }

  .tools-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .tool-card {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface-2);
    transition:
      background-color 160ms ease,
      border-color 160ms ease,
      transform 160ms ease;
  }

  .tool-card:hover {
    transform: translateY(-1px);
    border-color: color-mix(in srgb, var(--accent) 22%, var(--border));
    background: var(--surface-3);
  }

  .tool-card.muted:hover {
    transform: none;
    border-color: var(--border);
    background: var(--surface-2);
  }

  .tool-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .tool-icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 11px;
  }

  .tool-icon.tone-primary {
    background: color-mix(in srgb, var(--accent) 11%, transparent);
    color: var(--accent);
  }

  .tool-icon.tone-tertiary {
    background: color-mix(in srgb, var(--accent-3) 12%, transparent);
    color: var(--accent-3);
  }

  .tool-icon.tone-secondary {
    background: color-mix(in srgb, var(--secondary, #545f73) 12%, transparent);
    color: var(--secondary, #545f73);
  }

  .tool-badge {
    color: var(--muted);
    font-size: 0.6rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .tool-card h3 {
    margin: 0;
    font-size: 0.92rem;
    font-weight: 650;
    line-height: 1.3;
  }

  .tool-card p {
    margin: 0;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .tool-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    min-height: 30px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 650;
  }

  .tool-link:hover:not(:disabled) {
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .tool-link:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .tool-soon {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 600;
  }

  .weave-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .weave-head {
    display: flex;
    align-items: flex-start;
    gap: 11px;
  }

  .weave-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 38px;
    height: 38px;
    border: 1px solid var(--accent-line);
    border-radius: 11px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .weave-head > div {
    min-width: 0;
    flex: 1;
  }

  .weave-head h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.16rem;
    font-weight: 620;
  }

  .weave-head p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.45;
  }

  .weave-count {
    flex: 0 0 auto;
    padding: 0.12rem 0.5rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.64rem;
    font-weight: 650;
  }

  .weave-form {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 9px;
    min-width: 0;
    max-width: 100%;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    min-width: 0;
    color: var(--muted);
    font-size: 0.72rem;
  }

  label.grow {
    flex: 1;
    min-width: 190px;
  }

  label input,
  label select {
    width: 100%;
    max-width: 100%;
    min-width: 0;
    min-height: 40px;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    text-overflow: ellipsis;
  }

  .extracts {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 360px;
    overflow-y: auto;
  }

  label.extract {
    flex-direction: row;
    align-items: flex-start;
    gap: 9px;
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
  }

  label.extract:hover {
    border-color: var(--accent-line);
  }

  label.extract > div {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  label.extract input {
    margin-top: 3px;
    width: 17px;
    height: 17px;
  }

  .extract-book {
    overflow-wrap: anywhere;
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .extract-text {
    overflow-wrap: anywhere;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.82rem;
    line-height: 1.45;
  }

  .note {
    overflow-wrap: anywhere;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .remove-extract {
    flex: 0 0 auto;
    min-height: 30px;
    padding: 0.22rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--surface);
    color: var(--muted);
    font: inherit;
    font-size: 0.68rem;
  }

  .remove-extract:hover {
    border-color: color-mix(in srgb, var(--danger) 45%, var(--border));
    color: var(--danger);
  }

  .empty,
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .stats-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 16px;
    padding-top: 4px;
    color: var(--muted);
    font-size: 0.68rem;
  }

  @keyframes wave-shimmer {
    from {
      background-position: 200% 0;
    }
    to {
      background-position: -200% 0;
    }
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.55;
      transform: scale(0.82);
    }
  }

  @media (max-width: 1100px) {
    .tools-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .audiobook-grid {
      grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
    }
  }

  @media (max-width: 900px) {
    .player-dock {
      bottom: calc(78px + var(--safe-bottom));
      grid-template-columns: minmax(0, 1fr);
      gap: 0.5rem;
      max-height: min(44vh, 340px);
      padding: 0.7rem 0.75rem;
      overflow-y: auto;
      border-radius: 16px;
    }

    .player-cover {
      display: none;
    }

    .player-head {
      flex-direction: column;
      gap: 0.35rem;
    }

    .player-titles strong,
    .player-titles span {
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
    }

    .player-actions {
      width: 100%;
      flex-wrap: nowrap;
      justify-content: flex-start;
      overflow-x: auto;
      padding-bottom: 2px;
      scrollbar-width: none;
    }

    .player-actions::-webkit-scrollbar {
      display: none;
    }

    .player-actions > :global(*) {
      flex: 0 0 auto;
    }

    .player-dock .karaoke-lines,
    .player-dock .chapter-drawer {
      display: none;
    }

    .player-dock .karaoke-panel {
      padding: 7px 9px;
    }

    .active-task {
      grid-template-columns: 56px minmax(0, 1fr);
    }

    .task-actions {
      grid-column: 1 / -1;
      justify-content: flex-end;
    }

    .task-cover {
      width: 56px;
      height: 56px;
    }
  }

  @media (max-width: 700px) {
    .audiobook-grid,
    .album-grid {
      grid-template-columns: 1fr;
    }

    .view-switch,
    .new-album {
      width: 100%;
    }

    .view-switch button,
    .new-album {
      flex: 1;
      justify-content: center;
    }

    .page-head {
      align-items: flex-start;
    }

    .head-actions {
      width: 100%;
      flex-direction: column;
    }

    .head-actions :global(.ui-button) {
      width: 100%;
      justify-content: center;
    }

    .catalog-toolbar {
      align-items: stretch;
      flex-direction: column;
    }

    .queue-head {
      align-items: flex-start;
    }

    .queue-count {
      margin-left: 48px;
    }

    .queue-list li {
      grid-template-columns: 30px minmax(0, 1fr) 30px;
    }

    .queue-index,
    .queue-next {
      display: none;
    }

    .filter-tabs {
      width: 100%;
    }

    .toolbar-tools {
      width: 100%;
    }

    .search-field {
      flex: 1;
    }

    .search-field input {
      width: 100%;
    }

    .sort-field {
      flex: 0 0 auto;
    }

    .tools-grid {
      grid-template-columns: 1fr;
    }

    .weave-head {
      flex-wrap: wrap;
    }

    .weave-count {
      margin-left: 49px;
    }

    .weave-form {
      flex-direction: column;
      align-items: stretch;
    }

    label.grow {
      min-width: 0;
    }

    .weave-form :global(.ui-button) {
      width: 100%;
    }

    .time {
      display: none;
    }

    .karaoke-line.prev,
    .karaoke-line.next {
      display: none;
    }

    .karaoke-precision {
      display: none;
    }

    .karaoke-line.now {
      font-size: 0.92rem;
    }

    .active-task {
      grid-template-columns: 48px minmax(0, 1fr);
      gap: 10px;
      padding: 12px;
    }

    .task-cover {
      width: 48px;
      height: 48px;
    }

    .task-head strong {
      font-size: 0.92rem;
    }

    .stats-line {
      flex-direction: column;
      align-items: flex-start;
    }
  }

  @media (max-width: 420px) {
    .filter-tabs button {
      padding: 0.28rem 0.55rem;
      font-size: 0.68rem;
    }

    .filter-tabs button span {
      display: none;
    }

    .weave-count {
      margin-left: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .pulse-dot,
    .task-pip,
    .live-dot,
    .waveform-skeleton,
    .karaoke-skeleton span {
      animation: none;
    }

    .active-task,
    .player-dock,
    .quick-add,
    .tool-card,
    .karaoke-word {
      transition: none;
    }
  }

  /* ── Reproductor a pantalla completa ─────────────────────────────── */
  .player-dock.player-sheet {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0;
    width: 100%;
    height: 100vh;
    height: 100dvh;
    max-height: none;
    margin: 0;
    padding: 0;
    overflow: hidden;
    border: 0;
    border-radius: 0;
    background:
      radial-gradient(
        680px 420px at 50% -8%,
        color-mix(in srgb, var(--accent) 12%, transparent),
        transparent 70%
      ),
      radial-gradient(
        520px 380px at 110% 30%,
        color-mix(in srgb, var(--gold) 10%, transparent),
        transparent 72%
      ),
      var(--bg);
    box-shadow: none;
    transition: transform 190ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .player-dock.player-sheet.dragging {
    transition: none;
  }

  .player-sheet-drag {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 100%;
    height: 34px;
    padding-top: var(--safe-top);
    touch-action: none;
    cursor: grab;
  }

  .sheet-grab-pill {
    width: 46px;
    height: 5px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--muted) 35%, transparent);
  }

  .player-sheet-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 0 0 auto;
    width: 100%;
    max-width: 760px;
    padding: 0 18px 2px;
  }

  .sheet-minimize {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 34px;
    padding: 0 0.6rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 84%, transparent);
    color: var(--muted);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 650;
  }

  .sheet-minimize:hover {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .sheet-minimize :global(svg) {
    transform: rotate(90deg);
  }

  .sheet-top-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sheet-hide {
    display: grid;
    place-items: center;
    width: 34px;
    min-width: 34px;
    height: 34px;
    min-height: 34px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: color-mix(in srgb, var(--surface) 84%, transparent);
    color: var(--muted);
  }

  .sheet-hide:hover {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .sheet-kicker {
    color: var(--accent);
    font-size: 0.65rem;
    font-weight: 750;
    letter-spacing: 0.13em;
    text-transform: uppercase;
  }

  .player-sheet .player-cover {
    display: block;
    flex: 0 0 auto;
    width: min(56vw, 240px);
    aspect-ratio: 2 / 3;
    margin: 8px auto 14px;
    border-radius: 18px;
    box-shadow:
      0 28px 48px -30px color-mix(in srgb, #1a0f0b 55%, transparent),
      0 0 0 1px color-mix(in srgb, var(--accent) 12%, transparent);
    touch-action: none;
    cursor: grab;
  }

  .player-sheet .player-cover img {
    object-fit: cover;
  }

  .player-sheet .player-main {
    flex: 1 1 auto;
    width: 100%;
    max-width: 760px;
    min-height: 0;
    padding: 0 18px calc(24px + var(--safe-bottom));
    gap: 14px;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
    scrollbar-width: thin;
  }

  .player-sheet .player-head {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }

  .player-sheet .player-titles {
    align-items: center;
    text-align: center;
  }

  .player-sheet .player-titles .eyebrow {
    display: none;
  }

  .player-sheet .player-titles strong {
    font-size: clamp(1.25rem, 4.4vw, 1.7rem);
    line-height: 1.2;
  }

  .player-sheet .player-titles span {
    padding: 0 0.4rem;
    font-size: 0.78rem;
    white-space: normal;
    text-align: center;
  }

  .player-sheet .player-actions {
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
    max-width: 100%;
    overflow: visible;
  }

  .player-sheet .player-actions > :global(*) {
    flex: 0 0 auto;
  }

  .player-sheet .speed {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .player-sheet .speed select {
    width: auto;
    min-height: 34px;
  }

  .player-sheet .player-controls {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas:
      "start wave end"
      "back play forward";
    align-items: center;
    gap: 10px 12px;
    justify-content: stretch;
  }

  .player-sheet .player-controls > :nth-child(1) {
    grid-area: back;
    justify-self: end;
  }

  .player-sheet .player-controls > :nth-child(2) {
    grid-area: play;
    justify-self: center;
  }

  .player-sheet .player-controls > :nth-child(3) {
    grid-area: forward;
    justify-self: start;
  }

  .player-sheet .player-controls > :nth-child(4) {
    grid-area: start;
    justify-self: start;
  }

  .player-sheet .player-controls > :nth-child(5) {
    grid-area: wave;
  }

  .player-sheet .player-controls > :nth-child(6) {
    grid-area: end;
    justify-self: end;
  }

  .player-sheet .player-toggle {
    width: 62px;
    min-width: 62px;
    height: 62px;
    min-height: 62px;
    border-radius: 50%;
  }

  .player-sheet .player-toggle :global(svg) {
    width: 28px;
    height: 28px;
  }

  .player-sheet .skip {
    width: 48px;
    min-width: 48px;
    height: 48px;
    min-height: 48px;
    font-size: 0.65rem;
  }

  .player-sheet .waveform {
    height: 44px;
    border-radius: 9px;
  }

  .player-sheet .waveform-skeleton,
  .player-sheet .waveform-empty {
    height: 44px;
  }

  .player-sheet .waveform-empty {
    display: none;
  }

  .player-sheet .time {
    display: inline;
    min-width: 3rem;
    font-size: 0.72rem;
  }

  .player-sheet .karaoke-line.prev,
  .player-sheet .karaoke-line.next {
    display: block;
  }

  .player-sheet .karaoke-precision {
    display: inline-flex;
  }

  .player-sheet .karaoke-panel {
    padding: 12px 14px;
    border-radius: 14px;
    background: color-mix(in srgb, var(--surface) 84%, transparent);
  }

  .player-sheet .karaoke-line {
    font-size: 0.9rem;
  }

  .player-sheet .karaoke-line.now {
    font-size: 1.02rem;
  }

  .player-sheet .karaoke-lines {
    display: flex;
  }

  .player-sheet .chapter-drawer {
    display: block;
  }

  .player-sheet .chapter-list {
    max-height: none;
  }

  @media (max-width: 900px) {
    .player-sheet .player-cover {
      display: block;
      width: min(62vw, 230px);
      margin-top: 4px;
      margin-bottom: 12px;
    }

    .player-sheet .player-main {
      padding: 0 15px calc(20px + var(--safe-bottom));
      overflow-x: hidden;
    }

    .player-sheet .player-actions {
      justify-content: flex-start;
      flex-wrap: wrap;
      max-width: 100%;
      overflow: visible;
      padding-bottom: 0;
    }

    .player-sheet .karaoke-lines {
      display: flex;
    }

    .player-sheet .chapter-drawer {
      display: block;
    }
  }

  /* ── Reproductor minimizado ──────────────────────────────────────── */
  .player-mini {
    position: sticky;
    bottom: calc(78px + var(--safe-bottom));
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 8px 10px;
    overflow: hidden;
    border: 1px solid var(--accent-line);
    border-radius: 16px;
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    box-shadow: var(--shadow-lg);
    backdrop-filter: blur(12px);
  }

  .mini-open {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1 1 auto;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    font: inherit;
  }

  .mini-cover {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 44px;
    height: 58px;
    overflow: hidden;
    border-radius: 7px;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    box-shadow: 0 5px 12px -8px var(--shadow-color);
  }

  .mini-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .mini-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .mini-copy strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: 0.88rem;
    font-weight: 650;
  }

  .mini-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: 0.66rem;
  }

  .mini-toggle,
  .mini-expand,
  .mini-hide {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 38px;
    min-width: 38px;
    height: 38px;
    min-height: 38px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
  }

  .mini-toggle {
    border-color: color-mix(in srgb, var(--gold-deep) 68%, #1a0f0b);
    background: linear-gradient(135deg, var(--gold), var(--gold-deep));
    color: #fff;
  }

  .mini-expand :global(svg) {
    transform: rotate(-90deg);
  }

  .mini-hide {
    width: 34px;
    min-width: 34px;
    height: 34px;
    min-height: 34px;
    border-color: transparent;
    background: transparent;
    color: var(--muted);
  }

  .mini-hide:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .mini-progress {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    border-radius: 0 999px 999px 0;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 160ms ease;
  }

  @media (min-width: 901px) {
    .player-mini {
      bottom: 1rem;
    }
  }
</style>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import * as pdfjsLib from "pdfjs-dist";
  import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import AnnotationPanel from "../lib/components/AnnotationPanel.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { t, translate } from "../lib/i18n";
  import {
    addMarker,
    buildAutoIndex,
    errorMessage,
    getMirrorStatus,
    getOcrPage,
    getPeers,
    joinMirror,
    leaveMirror,
    listAnnotations,
    ocrPdf,
    readerDeleteAnnotation,
    readerPdfBytes,
    readerUpdateAnnotation,
    readerUpdateProgress,
    searchPdf,
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
    ttsStop,
    ttsVoices as ttsVoiceList,
    practiceCancel,
    practiceDownloadModel,
    practiceHistory,
    practiceModels as practiceModelList,
    practiceStart,
    practiceStop,
  } from "../lib/api";
  import type {
    Annotation,
    AnnotationAnchor,
    AnnotationKind,
    AutoIndexEntry,
    CharacterInfo,
    Extract,
    MirrorPositionEvent,
    MirrorStatus,
    MirrorTtsEvent,
    ModelDownloadProgress,
    OcrProgress,
    PageTextLayer,
    PdfSearchHit,
    PeerInfo,
    PracticeHistoryEntry,
    PracticeResult,
    Reaction,
    ReactionKind,
    ReaderSession,
    ReaderSettings,
    SyncHandoffEvent,
    TextWord,
    TtsPhraseEvent,
    TtsPreferences,
    TtsSessionEvent,
    TtsStatus,
    VoiceDescriptor,
    WhisperModelInfo,
  } from "../lib/types";

  pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

  let {
    session,
    settings,
    onprogress,
    onzoom,
  }: {
    session: ReaderSession;
    settings: ReaderSettings;
    onprogress?: (page: number, percent: number) => void;
    onzoom?: (zoom: number) => void;
  } = $props();

  interface MarkRect {
    left: number;
    top: number;
    width: number;
    height: number;
    fontSize: number;
  }

  interface PageRender {
    canvas: HTMLCanvasElement;
    words: HTMLDivElement;
    marks: HTMLDivElement;
    layer: PageTextLayer | null;
    scale: number;
  }

  interface SelectionInfo {
    page: number;
    start: number;
    end: number;
    text: string;
    x: number;
    y: number;
  }

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

  // El modo paginado muestra dos hojas enfrentadas en cuanto hay ancho
  // suficiente; el umbral es menor que en EPUB porque cada hoja PDF ya
  // conserva su propia proporción. En ventanas muy estrechas vuelve a una.
  const SPREAD_MIN_WIDTH = 700;
  const SPREAD_GAP = 18;

  let scroller = $state<HTMLElement | null>(null);
  let documentProxy: PDFDocumentProxy | null = null;
  let loading = $state(true);
  let error = $state<string | null>(null);
  let pageCount = $state(0);
  let currentPage = $state(0);
  let zoom = $state(1);
  let mode = $state<"scroll" | "paged">("scroll");
  let spreadEnabled = $state(false);
  let generation = 0;
  let initialized = $state(false);
  let loadingTask: pdfjsLib.PDFDocumentLoadingTask | null = null;

  const pageNodes = new Map<number, HTMLElement>();
  const renders = new Map<number, PageRender>();
  const rendering = new Set<number>();
  const renderTokens = new Map<number, symbol>();

  let annotations = $state<Annotation[]>([]);
  let extracts = $state<Extract[]>([]);
  let reactions = $state<Reaction[]>([]);
  let selection = $state<SelectionInfo | null>(null);
  let activeAnnotation = $state<Annotation | null>(null);
  let annotationNote = $state("");
  let annotationColor = $state(colors[0]);
  let extractNotice = $state<string | null>(null);
  let annotationsOpen = $state(false);
  let annotationsTab = $state<"marker" | "note" | "extract" | "reaction">("marker");
  let annotationsRefresh = $state(0);
  let voiceRecordToken = $state(0);
  let panelAnchor = $state<AnnotationAnchor | null>(null);
  let handoff = $state<SyncHandoffEvent | null>(null);
  let mirror = $state<MirrorStatus | null>(null);
  let mirrorPeers = $state<PeerInfo[]>([]);
  let mirrorOpen = $state(false);
  let mirrorBusy = $state(false);

  let annotationSections = $derived(
    Array.from({ length: pageCount }, (_, index) => ({
      index,
      title: `Página ${index + 1}`,
    })),
  );
  let annotationAnchor = $derived.by<AnnotationAnchor>(() => {
    if (panelAnchor) return panelAnchor;
    if (selection) {
      return {
        section: selection.page,
        offsetStart: selection.start,
        offsetEnd: selection.end,
        text: selection.text,
      };
    }
    return {
      section: currentPage,
      offsetStart: 0,
      offsetEnd: 0,
      text: "",
    };
  });

  let tocOpen = $state(false);
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchResults = $state<PdfSearchHit[]>([]);
  let searchBusy = $state(false);
  let searchError = $state<string | null>(null);
  let indexEntries = $state<AutoIndexEntry[]>([]);
  let buildingIndex = $state(false);

  let ocrStatus = $state<"pending" | "running" | "done" | "failed" | "skipped">("skipped");
  let ocrDone = $state(0);
  let ocrTotal = $state(0);
  let ocrError = $state<string | null>(null);

  let voices = $state<VoiceDescriptor[]>([]);
  let preferredVoiceId = $state("");
  let voiceError = $state<string | null>(null);
  let downloadProgress = $state<number | null>(null);
  let ttsStatus = $state<TtsStatus | null>(null);
  let ttsRate = $state(1);
  let ttsPitch = $state(1);
  let ttsPreferencesState = $state<TtsPreferences | null>(null);
  let ttsRange = $state<{ page: number; start: number; end: number } | null>(null);
  let excerptTts = $state(false);
  let characterPanelOpen = $state(false);
  let characters = $state<CharacterInfo[]>([]);
  let characterVoices = $state<Record<string, string>>({});
  let practiceOpen = $state(false);
  let practiceRecording = $state(false);
  let practiceResult = $state<PracticeResult | null>(null);
  let practiceError = $state<string | null>(null);
  let practiceHistoryEntries = $state<PracticeHistoryEntry[]>([]);
  let whisperModels = $state<WhisperModelInfo[]>([]);
  let practiceModelId = $state("tiny");
  let practiceDownload = $state<number | null>(null);
  let practiceSeconds = $state(0);
  let autoAdvance = $state(true);
  let downloadingVoiceId = $state<string | null>(null);
  let practiceTimer: ReturnType<typeof setInterval> | undefined;
  let ttsSettingsTimer: ReturnType<typeof setTimeout> | undefined;
  let unlistenTtsStart: UnlistenFn | undefined;
  let unlistenTtsEnd: UnlistenFn | undefined;
  let unlistenTtsState: UnlistenFn | undefined;
  let unlistenPracticeDownload: UnlistenFn | undefined;
  let disposed = false;

  let ttsState = $derived(ttsStatus?.state ?? "idle");
  let ttsActive = $derived(ttsState === "playing" || ttsState === "paused");
  let installedVoices = $derived(voices.filter((voice) => voice.installed));

  let progressTimer: ReturnType<typeof setTimeout> | undefined;
  let unlistenOcr: UnlistenFn | undefined;
  let unlistenDownload: UnlistenFn | undefined;
  let unlistenHandoff: UnlistenFn | undefined;
  let unlistenMirrorPosition: UnlistenFn | undefined;
  let unlistenMirrorTts: UnlistenFn | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let resizeTimer: ReturnType<typeof setTimeout> | undefined;
  let lastSurfaceWidth = 0;
  let lastSurfaceHeight = 0;
  let scrollFrame = 0;

  let pageNumbers = $derived(Array.from({ length: pageCount }, (_, index) => index));
  let spreadPages = $derived(pagedPagesFor(currentPage));
  let pdf = $derived(session.pdf);
  let language = $derived.by(() => {
    const language = (session.document.language ?? "").toLowerCase();
    if (language.startsWith("es")) return "es";
    if (language.startsWith("en")) return "en";
    return "es";
  });
  let installedVoice = $derived(
    voices.find((voice) => voice.installed && voice.id === preferredVoiceId) ??
      voices.find((voice) => voice.installed && voice.language === language) ??
      null,
  );
  let downloadVoice = $derived(
    voices.find((voice) => voice.language === language && !voice.installed) ??
      voices.find((voice) => voice.language === language) ??
      voices.find((voice) => !voice.installed) ??
      voices[0] ??
      null,
  );

  onMount(() => {
    disposed = false;
    void initialise();
    void initSyncEvents();
    void refreshMirror();

    const onKeydown = (event: KeyboardEvent) => {
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
        searchOpen ||
        annotationsOpen ||
        mirrorOpen ||
        characterPanelOpen ||
        practiceOpen ||
        activeAnnotation !== null;
      if (panelOpen) {
        if (event.key === "Escape") closePanels();
        return;
      }

      if (event.key === "ArrowRight" || event.key === "PageDown") {
        event.preventDefault();
        void stepPage(1);
      } else if (event.key === "ArrowLeft" || event.key === "PageUp") {
        event.preventDefault();
        void stepPage(-1);
      } else if (event.key === "Escape") {
        closePanels();
      }
    };

    window.addEventListener("keydown", onKeydown);

    const onResize = () => {
      if (!initialized) return;
      const width = scroller?.clientWidth ?? 0;
      const height = scroller?.clientHeight ?? 0;
      if (
        Math.abs(width - lastSurfaceWidth) < 3 &&
        Math.abs(height - lastSurfaceHeight) < 3
      ) {
        return;
      }
      lastSurfaceWidth = width;
      lastSurfaceHeight = height;
      const spreadChanged = updateSpread();
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => resetPages(), spreadChanged ? 0 : 180);
    };

    window.addEventListener("resize", onResize);
    if (scroller && typeof ResizeObserver !== "undefined") {
      lastSurfaceWidth = scroller.clientWidth;
      lastSurfaceHeight = scroller.clientHeight;
      resizeObserver = new ResizeObserver(onResize);
      resizeObserver.observe(scroller);
    }

    return () => {
      disposed = true;
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("resize", onResize);
      clearTimeout(resizeTimer);
      clearTimeout(progressTimer);
      cancelAnimationFrame(scrollFrame);
      unlistenOcr?.();
      unlistenDownload?.();
      unlistenHandoff?.();
      unlistenMirrorPosition?.();
      unlistenMirrorTts?.();
      unlistenTtsStart?.();
      unlistenTtsEnd?.();
      unlistenTtsState?.();
      unlistenPracticeDownload?.();
      if (practiceRecording) void practiceCancel();
      if (mirror?.hosting) void stopMirror().catch(() => {});
      else if (mirror?.active) void leaveMirror().catch(() => {});
      clearInterval(practiceTimer);
      clearTimeout(ttsSettingsTimer);
      resizeObserver?.disconnect();
      void stopTts();
      void loadingTask?.destroy();
      if (pageCount > 0) {
        void readerUpdateProgress(
          session.bookId,
          currentPage,
          0,
          (currentPage + 1) / pageCount,
        ).catch(() => {});
      }
    };
  });

  $effect(() => {
    if (!initialized) return;
    if (settings.zoom !== zoom) {
      zoom = settings.zoom;
      resetPages();
    }
    if (settings.mode !== mode) {
      mode = settings.mode;
      updateSpread();
      resetPages();
      void tick().then(() => {
        if (mode === "paged") {
          if (scroller) {
            scroller.scrollTop = 0;
            scroller.scrollLeft = 0;
          }
          renderPagedPages(currentPage);
        }
      });
    }
  });

  $effect(() => {
    if (!practiceOpen && practiceRecording) {
      void cancelPractice();
    }
  });

  async function initialise() {
    pageCount = session.pdf?.pageCount ?? 0;
    currentPage = session.locator.section;
    zoom = settings.zoom;
    mode = settings.mode;
    ocrStatus = session.pdf?.ocrStatus ?? "skipped";
    ocrDone = session.pdf?.ocrPagesDone ?? 0;
    ocrTotal = session.pdf?.ocrPagesTotal ?? 0;

    try {
      await loadDocument();
      updateSpread();
      try {
        await loadBookDecorations(session.bookId);
      } catch (cause) {
        // Un fallo al cargar anotaciones no debe impedir ver el PDF.
        console.error("No se pudieron cargar las anotaciones del PDF", cause);
      }
      indexEntries = session.document.sections.map((section) => ({
        title: section.title ?? `Página ${section.locator.section + 1}`,
        page: section.locator.section,
        level: 0,
      }));
      voices = await ttsVoiceList();
      await loadTtsPreferences();
      await setupTtsEvents();
      preferredVoiceId =
        preferredVoiceId ||
        (voices.find(
          (voice) => voice.installed && voice.language === language,
        )?.id ??
          "");

      unlistenOcr = await listen<OcrProgress>("ocr-progress", (event) => {
        if (event.payload.bookId !== session.bookId) return;
        ocrStatus = event.payload.status;
        ocrDone = event.payload.pagesDone;
        ocrTotal = event.payload.pagesTotal;
        ocrError = event.payload.error;
        if (event.payload.status === "done") {
          resetPages();
          void refreshIndex();
        }
      });

      unlistenDownload = await listen<{
        voiceId: string;
        downloaded: number;
        total: number;
        done: boolean;
        error: string | null;
      }>("tts-download-progress", (event) => {
        if (!downloadVoice || event.payload.voiceId !== downloadVoice.id) return;
        downloadProgress =
          event.payload.total > 0
            ? event.payload.downloaded / event.payload.total
            : null;
        if (event.payload.error) voiceError = event.payload.error;
        if (event.payload.done) {
          downloadProgress = null;
          void ttsVoiceList().then((next) => {
            voices = next;
            if (!preferredVoiceId) {
              preferredVoiceId =
                next.find((voice) => voice.installed && voice.language === language)?.id ?? "";
            }
          });
        }
      });

      initialized = true;
      await tick();
      // Recalcular con la superficie ya maquetada: el ancho inicial puede
      // llegar después de loadDocument y decidir mal entre una o dos hojas.
      updateSpread();
      await tick();

      if (settings.mode === "paged") {
        renderPagedPages(currentPage);
      } else if (currentPage > 0) {
        goToPage(currentPage);
      }

      if (pdf?.scanned && ocrStatus !== "done" && ocrStatus !== "running") {
        void startOcr();
      } else if (indexEntries.length === 0) {
        void refreshIndex();
      }
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      loading = false;
    }
  }

  async function loadDocument() {
    const options = {
      cMapUrl: "/pdfjs/cmaps/",
      cMapPacked: true,
      standardFontDataUrl: "/pdfjs/standard_fonts/",
      isEvalSupported: false,
    };

    try {
      const url = convertFileSrc(pdf?.sourcePath ?? "");
      loadingTask = pdfjsLib.getDocument({ ...options, url });
      documentProxy = await loadingTask.promise;
    } catch {
      const bytes = await readerPdfBytes(session.bookId);
      loadingTask = pdfjsLib.getDocument({
        ...options,
        data: new Uint8Array(bytes),
      });
      documentProxy = await loadingTask.promise;
    }

    pageCount = documentProxy.numPages;
    currentPage = Math.min(currentPage, Math.max(0, pageCount - 1));
  }

  function pdfPage(node: HTMLElement, page: number) {
    pageNodes.set(page, node);
    node.dataset.page = String(page);

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) void renderPage(page);
        }
      },
      { root: scroller, rootMargin: "700px 0px", threshold: 0.01 },
    );
    observer.observe(node);

    return {
      destroy() {
        observer.disconnect();
        pageNodes.delete(page);
        renders.delete(page);
      },
    };
  }

  function updateSpread() {
    const next =
      mode === "paged" &&
      pageCount > 1 &&
      (scroller?.clientWidth ?? 0) >= SPREAD_MIN_WIDTH;
    if (next === spreadEnabled) return false;
    spreadEnabled = next;
    return true;
  }

  /**
   * Hojas visibles en modo paginado. La portada va sola y después se
   * emparejan como un libro: índices 1-2, 3-4, 5-6…
   */
  function pagedPagesFor(page: number): number[] {
    if (mode !== "paged" || !spreadEnabled || pageCount <= 1) {
      return [Math.max(0, Math.min(page, Math.max(0, pageCount - 1)))];
    }
    if (page <= 0) return [0];
    const start = page % 2 === 0 ? page - 1 : page;
    if (start >= pageCount) return [pageCount - 1];
    return start + 1 < pageCount ? [start, start + 1] : [start];
  }

  function renderPagedPages(page: number) {
    for (const target of pagedPagesFor(page)) void renderPage(target);
  }

  async function renderPage(page: number) {
    if (!documentProxy || page < 0 || page >= pageCount) return;
    if (rendering.has(page)) return;

    const container = pageNodes.get(page);
    if (!container) return;

    const existing = renders.get(page);
    if (existing && existing.canvas.dataset.generation === String(generation)) return;

    const mine = generation;
    const token = Symbol(`page-${page}`);
    renderTokens.set(page, token);
    rendering.add(page);

    try {
      const pdfPage = await documentProxy.getPage(page + 1);
      if (mine !== generation || renderTokens.get(page) !== token) return;

      const base = pdfPage.getViewport({ scale: 1 });
      const surfaceStyles = scroller ? getComputedStyle(scroller) : null;
      const paddingX = surfaceStyles
        ? (parseFloat(surfaceStyles.paddingLeft) || 0) +
          (parseFloat(surfaceStyles.paddingRight) || 0)
        : 0;
      const paddingY = surfaceStyles
        ? (parseFloat(surfaceStyles.paddingTop) || 0) +
          (parseFloat(surfaceStyles.paddingBottom) || 0)
        : 0;
      // En una sola página se ajusta al ancho útil: ajustar también a la
      // altura dejaba páginas diminutas con enormes márgenes laterales.
      const availableWidth = Math.max(
        200,
        (scroller?.clientWidth ?? 800) - paddingX - 2,
      );
      const availableHeight = Math.max(
        200,
        (scroller?.clientHeight ?? 800) - paddingY - 2,
      );
      const spreadMode = spreadEnabled && mode === "paged";
      const facing = spreadMode && pagedPagesFor(page).length > 1;
      const pageWidth = facing
        ? Math.max(180, (availableWidth - SPREAD_GAP) / 2)
        : availableWidth;
      const fitWidth = pageWidth / base.width;
      // En hoja doble se ven las dos páginas completas: se limita también
      // por altura para no obligar a desplazarse en pantallas grandes.
      const fit = spreadMode
        ? Math.min(fitWidth, availableHeight / base.height)
        : fitWidth;
      const scale = Math.max(0.05, fit * zoom);
      const viewport = pdfPage.getViewport({ scale });
      const ratio = Math.min(window.devicePixelRatio || 1, 2.5);
      // El canvas tiene más píxeles que su tamaño CSS. Hay que renderizar
      // con el viewport ampliado; si no, el contenido queda en una esquina
      // diminuta y el resto de la hoja aparece en blanco.
      const renderViewport =
        ratio === 1 ? viewport : pdfPage.getViewport({ scale: scale * ratio });

      container.replaceChildren();
      container.style.width = `${Math.round(viewport.width)}px`;
      container.style.height = `${Math.round(viewport.height)}px`;
      container.style.aspectRatio = `${viewport.width} / ${viewport.height}`;

      const canvas = document.createElement("canvas");
      canvas.className = "page-canvas";
      canvas.dataset.generation = String(generation);
      canvas.width = Math.max(1, Math.round(renderViewport.width));
      canvas.height = Math.max(1, Math.round(renderViewport.height));
      canvas.style.width = `${Math.round(viewport.width)}px`;
      canvas.style.height = `${Math.round(viewport.height)}px`;

      const context = canvas.getContext("2d", { alpha: false });
      if (!context) throw new Error("no se pudo crear el contexto del canvas");

      container.appendChild(canvas);
      await pdfPage.render({ canvas, viewport: renderViewport }).promise;
      if (mine !== generation || renderTokens.get(page) !== token) {
        canvas.remove();
        return;
      }

      const words = document.createElement("div");
      words.className = "word-layer";
      const marks = document.createElement("div");
      marks.className = "mark-layer";
      container.append(words, marks);

      const layer = await loadLayer(pdfPage, page);
      if (mine !== generation || renderTokens.get(page) !== token) {
        canvas.remove();
        words.remove();
        marks.remove();
        return;
      }

      renderWords(words, layer, viewport.scale);
      const render: PageRender = { canvas, words, marks, layer, scale: viewport.scale };
      renders.set(page, render);
      renderMarks(page, render);
    } catch (cause) {
      console.error("No se pudo renderizar la página", page, cause);
    } finally {
      rendering.delete(page);
    }
  }

  async function loadLayer(page: PDFPageProxy, index: number): Promise<PageTextLayer> {
    try {
      const layer = await getOcrPage(session.bookId, index);
      if (layer && layer.words.length > 0) return layer;
    } catch (cause) {
      console.warn("Capa de texto del backend no disponible", cause);
    }

    return layerFromPdfJs(page, index);
  }

  async function layerFromPdfJs(page: PDFPageProxy, index: number): Promise<PageTextLayer> {
    const base = page.getViewport({ scale: 1 });
    const content = await page.getTextContent();
    const words: TextWord[] = [];

    for (const item of content.items) {
      if (!("str" in item)) continue;
      const text = item.str.replace(/\s+/g, " ").trim();
      if (!text) continue;

      const transform = pdfjsLib.Util.transform(base.transform, item.transform);
      const height = Math.hypot(transform[2], transform[3]) || item.height || 10;
      const width = item.width || 0;
      const x = transform[4];
      const y = transform[5] - height;
      const parts = text.split(" ").filter(Boolean);

      if (parts.length <= 1) {
        words.push({
          text,
          x,
          y,
          width: Math.max(width, 1),
          height: Math.max(height, 1),
          fontSize: height,
          confidence: null,
        });
        continue;
      }

      const total = parts.reduce((count, part) => count + part.length, 0);
      let cursor = x;
      for (const part of parts) {
        const partWidth = width * (part.length / total);
        words.push({
          text: part,
          x: cursor,
          y,
          width: Math.max(partWidth, 1),
          height: Math.max(height, 1),
          fontSize: height,
          confidence: null,
        });
        cursor += partWidth;
      }
    }

    return {
      bookId: session.bookId,
      page: index,
      width: base.width,
      height: base.height,
      source: "native",
      words,
    };
  }

  function renderWords(container: HTMLDivElement, layer: PageTextLayer, scale: number) {
    const fragment = document.createDocumentFragment();

    layer.words.forEach((word, index) => {
      const span = document.createElement("span");
      span.className = "word";
      span.dataset.word = String(index);
      span.textContent = word.text;
      span.style.left = `${word.x * scale}px`;
      span.style.top = `${word.y * scale}px`;
      span.style.width = `${Math.max(word.width * scale, 1)}px`;
      span.style.height = `${Math.max(word.height * scale, 1)}px`;
      span.style.fontSize = `${Math.max(word.height * scale * 0.92, 1)}px`;
      fragment.appendChild(span);
    });

    container.appendChild(fragment);
  }

  function renderMarks(page: number, render: PageRender) {
    render.marks.replaceChildren();
    if (!render.layer) return;

    const textMarkSlots = new Map<string, number>();
    const slotFor = (anchorLeft: number, rect: MarkRect) => {
      const key = `${Math.round(anchorLeft)}:${Math.round(rect.top + rect.height / 2)}`;
      const index = textMarkSlots.get(key) ?? 0;
      textMarkSlots.set(key, index + 1);
      return index * 24;
    };

    for (const annotation of annotations) {
      if (annotation.chapterIndex !== page) continue;
      const rects = markRects(render.layer, annotation.offsetStart ?? 0, annotation.offsetEnd ?? 0);
      for (const rect of rects) {
        const button = document.createElement("button");
        button.className = `mark kind-${annotation.kind}`;
        // El subrayado se ciñe al alto real del texto para no quedar
        // separado de la línea cuando la caja de OCR es más alta.
        const tighten = annotation.kind === "underline";
        const height = tighten
          ? Math.min(rect.height, rect.fontSize * 1.1)
          : rect.height;
        const top = tighten ? rect.top + (rect.height - height) / 2 : rect.top;
        button.style.left = `${rect.left * render.scale}px`;
        button.style.top = `${top * render.scale}px`;
        button.style.width = `${rect.width * render.scale}px`;
        button.style.height = `${height * render.scale}px`;
        if (tighten) {
          button.style.fontSize = `${height * render.scale}px`;
        }
        button.style.setProperty("--mark-color", annotation.color ?? colors[0]);
        button.title = annotation.note ?? annotation.text ?? "Anotación";
        button.addEventListener("click", (event) => {
          event.stopPropagation();
          openAnnotation(annotation);
        });
        render.marks.appendChild(button);
      }
    }

    for (const extract of extracts) {
      if (
        extract.chapterIndex !== page ||
        extract.offsetStart == null ||
        extract.offsetEnd == null
      ) {
        continue;
      }
      const rects = markRects(render.layer, extract.offsetStart, extract.offsetEnd);
      const first = rects.at(0);
      if (!first) continue;
      const lineLeft =
        lineStartLeft(render.layer, extract.offsetStart) ?? first.left;

      const button = document.createElement("button");
      button.className = "text-mark kind-extract";
      button.style.left = `${lineLeft * render.scale - 13 - slotFor(lineLeft, first)}px`;
      button.style.top = `${(first.top + first.height / 2) * render.scale}px`;
      button.textContent = "❝";
      button.title = extract.note ?? extract.text ?? "Cita";
      button.setAttribute("aria-label", button.title);
      button.addEventListener("click", (event) => {
        event.stopPropagation();
        openAnnotations("extract");
      });
      render.marks.appendChild(button);
    }

    for (const reaction of reactions) {
      if (
        reaction.chapterIndex !== page ||
        reaction.offsetStart == null ||
        reaction.offsetEnd == null
      ) {
        continue;
      }
      const rects = markRects(render.layer, reaction.offsetStart, reaction.offsetEnd);
      const first = rects.at(0);
      if (!first) continue;
      const lineLeft =
        lineStartLeft(render.layer, reaction.offsetStart) ?? first.left;

      const meta = quickReactions.find((item) => item.kind === reaction.kind);
      const button = document.createElement("button");
      button.className = "text-mark kind-reaction";
      button.style.left = `${lineLeft * render.scale - 13 - slotFor(lineLeft, first)}px`;
      button.style.top = `${(first.top + first.height / 2) * render.scale}px`;
      button.textContent = meta?.emoji ?? "💬";
      button.title = meta?.label ?? "Reacción";
      button.setAttribute("aria-label", button.title);
      button.addEventListener("click", (event) => {
        event.stopPropagation();
        openAnnotations("reaction");
      });
      render.marks.appendChild(button);
    }

    if (ttsRange?.page === page) {
      const rects = markRects(render.layer, ttsRange.start, ttsRange.end);
      for (const rect of rects) {
        const highlight = document.createElement("div");
        highlight.className = "tts-highlight";
        highlight.style.left = `${rect.left * render.scale}px`;
        highlight.style.top = `${rect.top * render.scale}px`;
        highlight.style.width = `${rect.width * render.scale}px`;
        highlight.style.height = `${rect.height * render.scale}px`;
        render.marks.appendChild(highlight);
      }
    }
  }

  function lineStartLeft(layer: PageTextLayer, start: number): number | null {
    if (layer.words.length === 0) return null;
    const from = Math.max(0, Math.min(start, layer.words.length - 1));
    const line = layer.words[from];
    const tolerance = Math.max(line.height, 4) * 0.6;
    let index = from;
    while (index > 0) {
      const previous = layer.words[index - 1];
      if (Math.abs(previous.y - line.y) > tolerance) break;
      index -= 1;
    }
    return layer.words[index].x;
  }

  function markRects(layer: PageTextLayer, start: number, end: number): MarkRect[] {
    const from = Math.max(0, Math.min(start, layer.words.length - 1));
    const to = Math.max(from, Math.min(end, layer.words.length - 1));
    if (layer.words.length === 0) return [];

    const groups: TextWord[][] = [];
    for (let index = from; index <= to; index += 1) {
      const word = layer.words[index];
      const line = groups.at(-1);
      const tolerance = Math.max(word.height, 4) * 0.6;
      if (line && Math.abs(word.y - line[0].y) <= tolerance) {
        line.push(word);
      } else {
        groups.push([word]);
      }
    }

    return groups.map((line) => {
      const left = Math.min(...line.map((word) => word.x));
      const top = Math.min(...line.map((word) => word.y));
      const right = Math.max(...line.map((word) => word.x + word.width));
      const bottom = Math.max(...line.map((word) => word.y + word.height));
      const fontSize = Math.max(
        ...line.map((word) => word.fontSize || word.height),
      );
      return { left, top, width: right - left, height: bottom - top, fontSize };
    });
  }

  function resetPages() {
    generation += 1;
    renders.clear();
    rendering.clear();
    renderTokens.clear();
    for (const node of pageNodes.values()) node.replaceChildren();
    void tick().then(() => {
      if (mode === "paged") {
        renderPagedPages(currentPage);
      } else {
        const surface = scroller?.getBoundingClientRect();
        const top = (surface?.top ?? 0) - 900;
        const bottom = (surface?.bottom ?? window.innerHeight) + 900;
        for (const [page, node] of pageNodes) {
          const rect = node.getBoundingClientRect();
          if (rect.bottom >= top && rect.top <= bottom) void renderPage(page);
        }
      }
    });
  }

  function onScroll() {
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      if (mode !== "scroll" || !scroller) return;

      const anchor = scroller.getBoundingClientRect().top + 120;
      let best = currentPage;
      let distance = Infinity;
      for (const [page, node] of pageNodes) {
        const rect = node.getBoundingClientRect();
        const delta = Math.abs(rect.top - anchor);
        if (rect.bottom > anchor && delta < distance) {
          distance = delta;
          best = page;
        }
      }

      if (best !== currentPage) {
        currentPage = best;
        notifyProgress();
      }
    });
  }

  function notifyProgress() {
    const fraction = pageCount ? (currentPage + 1) / pageCount : 0;
    onprogress?.(currentPage, fraction);
    clearTimeout(progressTimer);
    progressTimer = setTimeout(() => {
      void readerUpdateProgress(session.bookId, currentPage, 0, fraction).catch(() => {});
    }, 600);
  }

  async function stepPage(direction: -1 | 1) {
    // Avanzar dos índices recorre la siguiente pareja de hojas, no la
    // compañera que ya está a la vista.
    const step = mode === "paged" && spreadEnabled ? 2 : 1;
    const next = Math.min(
      pageCount - 1,
      Math.max(0, currentPage + direction * step),
    );
    if (next === currentPage) return;
    goToPage(next);
  }

  function goToPage(page: number) {
    const target = Math.min(pageCount - 1, Math.max(0, page));
    currentPage = target;
    tocOpen = false;
    notifyProgress();

    if (mode === "paged") {
      void tick().then(() => {
        if (scroller) {
          scroller.scrollTop = 0;
          scroller.scrollLeft = 0;
        }
        renderPagedPages(target);
      });
      return;
    }

    const node = pageNodes.get(target);
    node?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function onMouseUp() {
    window.setTimeout(captureSelection, 0);
  }

  function captureSelection() {
    const current = window.getSelection();
    if (!current || current.rangeCount === 0 || current.isCollapsed) {
      selection = null;
      return;
    }

    const range = current.getRangeAt(0);
    const startWord = wordElement(range.startContainer);
    const endWord = wordElement(range.endContainer);
    if (!startWord || !endWord) {
      selection = null;
      return;
    }

    const startPage = Number(
      startWord.closest<HTMLElement>(".pdf-page-holder")?.dataset.page,
    );
    const endPage = Number(
      endWord.closest<HTMLElement>(".pdf-page-holder")?.dataset.page,
    );
    if (!Number.isFinite(startPage) || startPage !== endPage) {
      selection = null;
      return;
    }

    let start = Number(startWord.dataset.word);
    let end = Number(endWord.dataset.word);
    if (start > end) [start, end] = [end, start];

    // Los span de palabras son elementos independientes: `Selection.toString`
    // los concatena sin espacios («lasmareasMarinaSoler»). Reconstruimos el
    // pasaje desde la capa de texto para que citas, notas y modo ensayo
    // conserven las palabras separadas.
    const layer = renders.get(startPage)?.layer;
    const text =
      layer && layer.words.length > 0
        ? layer.words
            .slice(start, end + 1)
            .map((word) => word.text)
            .join(" ")
            .slice(0, 2000)
        : current.toString().replace(/\s+/g, " ").trim().slice(0, 2000);
    if (!text || !Number.isFinite(start) || !Number.isFinite(end)) {
      selection = null;
      return;
    }

    const rect = range.getBoundingClientRect();
    panelAnchor = null;
    selection = {
      page: startPage,
      start,
      end,
      text,
      x: rect.left + rect.width / 2,
      y: rect.top,
    };
  }

  function wordElement(node: Node | null): HTMLElement | null {
    if (!node) return null;
    const element = node.nodeType === Node.ELEMENT_NODE ? (node as HTMLElement) : node.parentElement;
    return element?.closest<HTMLElement>(".word") ?? null;
  }

  async function markSelection(kind: AnnotationKind, color: string) {
    const anchor =
      panelAnchor ??
      (selection
        ? {
            section: selection.page,
            offsetStart: selection.start,
            offsetEnd: selection.end,
            text: selection.text,
          }
        : null);
    if (!anchor || !anchor.text.trim()) return;

    // Conserva el pasaje cuando la marca se crea desde el panel para que las
    // demás acciones (extraer, reaccionar, nota, voz) sigan disponibles.
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
      refreshMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function addPageBookmark() {
    if (!session) return;
    try {
      const annotation = await addMarker({
        bookId: session.bookId,
        chapterIndex: currentPage,
        offsetStart: 0,
        offsetEnd: 0,
        kind: "bookmark",
        color: "#ffd166",
        text: `Página ${currentPage + 1}`,
        note: null,
      });
      annotations = [...annotations, annotation];
      annotationsRefresh += 1;
      refreshMarks();
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
    if (await copyText(citation)) {
      extractNotice = "Cita copiada.";
      window.setTimeout(() => (extractNotice = null), 2400);
    } else {
      extractNotice = "No se pudo copiar la cita.";
      window.setTimeout(() => (extractNotice = null), 2400);
    }
  }

  function refreshMarks() {
    for (const [page, render] of renders) renderMarks(page, render);
  }

  function captureAnchor(): AnnotationAnchor {
    if (selection) {
      return {
        section: selection.page,
        offsetStart: selection.start,
        offsetEnd: selection.end,
        text: selection.text,
      };
    }
    return {
      section: currentPage,
      offsetStart: 0,
      offsetEnd: 0,
      text: "",
    };
  }

  async function runSearch() {
    const query = searchQuery.trim();
    if (query.length < 2) {
      searchResults = [];
      return;
    }
    searchBusy = true;
    searchError = null;
    try {
      searchResults = await searchPdf(session.bookId, query, 60);
      if (searchResults.length === 0) {
        searchError = translate("reader.searchNoResults");
      }
    } catch (cause) {
      searchError = errorMessage(cause);
    } finally {
      searchBusy = false;
    }
  }

  function openSearchHit(hit: PdfSearchHit) {
    goToPage(hit.page);
    searchOpen = false;
  }

  function openAnnotations(
    tab: "marker" | "note" | "extract" | "reaction",
  ) {
    panelAnchor = captureAnchor();
    annotationsTab = tab;
    annotationsOpen = true;
    tocOpen = false;
    searchOpen = false;
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

  function closePanels() {
    closeAnnotations();
    activeAnnotation = null;
    tocOpen = false;
    searchOpen = false;
    mirrorOpen = false;
    characterPanelOpen = false;
    practiceOpen = false;
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
    await loadBookDecorations(session.bookId);
    refreshMarks();
  }

  async function jumpToAnnotation(section: number, offset: number) {
    goToPage(section);
    await tick();
    const node = pageNodes
      .get(section)
      ?.querySelector<HTMLElement>(`[data-word="${Math.max(0, offset)}"]`);
    node?.scrollIntoView({ behavior: "smooth", block: "center" });
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
      annotations = annotations.map((item) => (item.id === updated.id ? updated : item));
      activeAnnotation = updated;
      annotationsRefresh += 1;
      refreshMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function deleteAnnotation() {
    if (!activeAnnotation) return;
    try {
      await readerDeleteAnnotation(activeAnnotation.id);
      annotations = annotations.filter((item) => item.id !== activeAnnotation?.id);
      activeAnnotation = null;
      annotationsRefresh += 1;
      refreshMarks();
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function refreshIndex() {
    if (indexEntries.length > 0) return;
    try {
      const index = await buildAutoIndex(session.bookId);
      indexEntries = index.entries;
    } catch {
      // el índice es opcional; no bloquea la lectura
    }
  }

  async function generateIndex() {
    buildingIndex = true;
    error = null;
    try {
      const index = await buildAutoIndex(session.bookId);
      indexEntries = index.entries;
      if (indexEntries.length === 0) {
        error = "No se encontraron encabezados claros en este PDF.";
      }
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      buildingIndex = false;
    }
  }

  async function startOcr() {
    try {
      const state = await ocrPdf(session.bookId);
      ocrStatus = state.status;
      ocrDone = state.pagesDone;
      ocrTotal = state.pagesTotal;
    } catch (cause) {
      ocrStatus = "failed";
      ocrError = errorMessage(cause);
    }
  }

  async function downloadVoicePack() {
    if (!downloadVoice) return;
    await downloadSpecificVoice(downloadVoice);
  }

  async function downloadSpecificVoice(voice: VoiceDescriptor) {
    downloadingVoiceId = voice.id;
    voiceError = null;
    try {
      await ttsDownloadVoice(voice.id);
      voices = await ttsVoiceList();
      preferredVoiceId = voice.id;
      await saveTtsPreferences();
    } catch (cause) {
      voiceError = errorMessage(cause);
    } finally {
      downloadingVoiceId = null;
    }
  }

  async function loadTtsPreferences() {
    try {
      const preferences = await ttsPreferences(session.bookId);
      ttsPreferencesState = preferences;
      ttsRate = preferences.rate;
      ttsPitch = preferences.pitch;
      if (
        preferences.voiceId &&
        voices.some(
          (voice) => voice.installed && voice.id === preferences.voiceId,
        )
      ) {
        preferredVoiceId = preferences.voiceId;
      }
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function setupTtsEvents() {
    unlistenTtsStart = await listen<TtsPhraseEvent>(
      "tts::phrase_start",
      (event) => {
        if (event.payload.bookId !== session.bookId) return;
        if (event.payload.section !== currentPage) goToPage(event.payload.section);
        ttsRange = {
          page: event.payload.section,
          start: event.payload.offsetStart,
          end: Math.max(
            event.payload.offsetStart,
            event.payload.offsetEnd - 1,
          ),
        };
        refreshMarks();
      },
    );
    if (disposed) {
      unlistenTtsStart?.();
      return;
    }

    unlistenTtsEnd = await listen<TtsPhraseEvent>("tts::phrase_end", (event) => {
      if (event.payload.bookId !== session.bookId) return;
      ttsRange = null;
      refreshMarks();
    });
    if (disposed) {
      unlistenTtsEnd?.();
      return;
    }

    unlistenTtsState = await listen<TtsSessionEvent>("tts::state", (event) => {
      if (event.payload.bookId !== session.bookId) return;
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
      if (event.payload.error) voiceError = event.payload.error;
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
        ttsRange = null;
        refreshMarks();
      }
      if (event.payload.state === "finished") {
        excerptTts = false;
        ttsRange = null;
        refreshMarks();
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

  async function initSyncEvents() {
    unlistenHandoff = await listen<SyncHandoffEvent>("sync::handoff", (event) => {
      if (event.payload.handoff.bookId === session.bookId) {
        handoff = event.payload;
      }
    });
    unlistenMirrorPosition = await listen<MirrorPositionEvent>(
      "mirror::position",
      async (event) => {
        if (event.payload.book_id !== session.bookId) return;
        await jumpToAnnotation(event.payload.chapter, event.payload.offset);
      },
    );
    unlistenMirrorTts = await listen<MirrorTtsEvent>(
      "mirror::tts_state",
      async (event) => {
        if (event.payload.book_id !== session.bookId) return;
        await applyMirrorTts(event.payload);
      },
    );
  }

  async function refreshMirror() {
    try {
      mirror = await getMirrorStatus();
    } catch {
      // opcional
    }
  }

  async function openMirrorPanel() {
    mirrorOpen = !mirrorOpen;
    if (mirrorOpen) {
      tocOpen = false;
      searchOpen = false;
      annotationsOpen = false;
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
    if (mirror?.hosting) return;
    try {
      if (event.state === "playing") {
        if (ttsState !== "playing") {
          excerptTts = event.end != null;
          ttsStatus = await ttsStart(session.bookId, {
            section: event.section,
            offset: event.offset,
            end: event.end ?? undefined,
          });
        }
      } else if (event.state === "paused") {
        excerptTts = event.end != null;
        if (ttsState === "playing") ttsStatus = await ttsPause();
      } else if (ttsState === "playing" || ttsState === "paused") {
        excerptTts = false;
        ttsStatus = await ttsStop();
        ttsRange = null;
        refreshMarks();
      }
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function resumeHandoff() {
    if (!handoff) return;
    const target = handoff.handoff;
    handoff = null;
    await jumpToAnnotation(target.chapter, target.offset);
  }

  async function startTts() {
    if (!installedVoice) {
      voiceError = "Descarga una voz para escuchar el libro.";
      return;
    }

    voiceError = null;
    excerptTts = false;
    try {
      ttsStatus = await ttsStart(session.bookId, {
        section: currentPage,
        offset:
          ttsRange?.page === currentPage ? ttsRange.start : 0,
      });
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function listenSelection() {
    if (!installedVoice) {
      voiceError = "Descarga una voz para escuchar el libro.";
      return;
    }
    const anchor = annotationAnchor;
    // En PDF `offsetEnd` es la última palabra incluida; puede coincidir con
    // `offsetStart` cuando la selección es una sola palabra.
    if (anchor.offsetEnd < anchor.offsetStart || !anchor.text.trim()) return;

    voiceError = null;
    excerptTts = true;
    try {
      ttsStatus = await ttsStart(session.bookId, {
        section: anchor.section,
        offset: anchor.offsetStart,
        // En PDF el ancla guarda el índice de la última palabra incluida.
        end: anchor.offsetEnd + 1,
      });
    } catch (cause) {
      excerptTts = false;
      voiceError = errorMessage(cause);
    }
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
      voiceError = errorMessage(cause);
    }
  }

  async function stopTts() {
    excerptTts = false;
    try {
      ttsStatus = await ttsStop();
    } catch {
      ttsStatus = null;
    }
    ttsRange = null;
    refreshMarks();
  }

  function setTtsRate(value: number) {
    ttsRate = value;
    clearTimeout(ttsSettingsTimer);
    ttsSettingsTimer = setTimeout(() => {
      void ttsSetRate(value).catch((cause) => {
        voiceError = errorMessage(cause);
      });
      void saveTtsPreferences();
    }, 220);
  }

  function setTtsVoice(voiceId: string) {
    preferredVoiceId = voiceId;
    void saveTtsPreferences();
  }

  function setTtsPitch(value: number) {
    ttsPitch = value;
    void ttsSetPitch(value).catch((cause) => {
      voiceError = errorMessage(cause);
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
    if (!ttsPreferencesState) return;
    try {
      ttsPreferencesState = await ttsSavePreferences(session.bookId, {
        voiceId: preferredVoiceId || null,
        rate: ttsRate,
        pitch: ttsPitch,
        pauseScale: ttsPreferencesState.pauseScale,
        expressive: ttsPreferencesState.expressive,
        multiVoice: ttsPreferencesState.multiVoice,
      });
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function advanceTts(section: number) {
    const next = section + 1;
    if (next >= pageCount) return;
    goToPage(next);
    await tick();
    try {
      ttsStatus = await ttsStart(session.bookId, { section: next, offset: 0 });
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function openCharacterPanel() {
    characterPanelOpen = true;
    tocOpen = false;
    searchOpen = false;
    annotationsOpen = false;
    mirrorOpen = false;
    practiceOpen = false;
    activeAnnotation = null;
    try {
      const [detected, assigned] = await Promise.all([
        ttsCharacters(session.bookId, currentPage),
        ttsCharacterVoices(session.bookId),
      ]);
      characters = detected.filter((item) => item.name !== "Narrador");
      characterVoices = Object.fromEntries(
        assigned.map((voice) => [voice.character, voice.voiceId]),
      );
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function assignCharacterVoice(character: string, voiceId: string) {
    if (!voiceId) return;
    try {
      await ttsAssignCharacterVoice(session.bookId, character, voiceId);
      characterVoices = { ...characterVoices, [character]: voiceId };
    } catch (cause) {
      voiceError = errorMessage(cause);
    }
  }

  async function openPracticePanel() {
    practiceOpen = true;
    tocOpen = false;
    searchOpen = false;
    annotationsOpen = false;
    mirrorOpen = false;
    characterPanelOpen = false;
    activeAnnotation = null;
    practiceError = null;
    practiceResult = null;
    await loadWhisperModels();
    await loadPracticeHistory();
  }

  async function loadPracticeHistory() {
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
    if (!selection) {
      practiceError = "Selecciona un pasaje en la página para practicar.";
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
        selection.page,
        selection.start,
        selection.end,
        selection.text,
        language,
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

  function visibleMarks(page: number) {
    return annotations.filter((annotation) => annotation.chapterIndex === page);
  }

  function pageLabel(page: number) {
    const pages = pagedPagesFor(page);
    if (pages.length > 1) {
      return translate("reader.pagesOf", {
        from: pages[0] + 1,
        to: pages[pages.length - 1] + 1,
        total: pageCount,
      });
    }
    return translate("reader.pageOf", { page: page + 1, total: pageCount });
  }
</script>

<div class="pdf-reader">
  <div class="pdf-toolbar">
    <div class="pdf-actions">
      <button
        class="ghost small"
        onclick={() => {
          tocOpen = !tocOpen;
          if (tocOpen) {
            searchOpen = false;
            annotationsOpen = false;
            mirrorOpen = false;
            characterPanelOpen = false;
            practiceOpen = false;
            activeAnnotation = null;
          }
        }}
      >
        {$t("reader.contents")}
      </button>
      <button
        class="ghost small"
        class:active={searchOpen}
        onclick={() => {
          searchOpen = !searchOpen;
          if (searchOpen) {
            tocOpen = false;
            annotationsOpen = false;
            mirrorOpen = false;
            characterPanelOpen = false;
            practiceOpen = false;
            activeAnnotation = null;
          }
        }}
      >
        <Icon name="search" size={15} /> {$t("common.search")}
      </button>
      <button class="ghost small" onclick={() => openAnnotations("marker")}>
        {$t("reader.annotations")}
      </button>
      <button class="ghost small" onclick={openMirrorPanel}>{$t("reader.mirror")}</button>
      <button class="ghost small" onclick={toggleTts} disabled={!installedVoice}>
        {ttsState === "playing"
          ? $t("reader.pause")
          : ttsState === "paused"
            ? $t("reader.resume")
            : $t("reader.listen")}
      </button>
      {#if ttsActive}
        <button class="ghost small" onclick={stopTts}>{$t("reader.stop")}</button>
      {/if}
      {#if excerptTts}
        <span class="tts-excerpt">{$t("reader.excerptLabel")}</span>
      {/if}
      <label class="tts-rate-pdf" title={$t("reader.readingSpeed")}>
        <span>{ttsRate.toFixed(1)}×</span>
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
      <label class="tts-rate-pdf" title={$t("reader.voiceTone")}>
        <span>{ttsPitch.toFixed(2)}</span>
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
      {#if installedVoices.length > 0}
        <select
          class="voice-select"
          bind:value={preferredVoiceId}
          onchange={() => setTtsVoice(preferredVoiceId)}
          aria-label={$t("reader.readingVoice")}
        >
          {#each installedVoices as voice (voice.id)}
            <option value={voice.id}>
              {voice.language.toUpperCase()} · {voice.name}{voice.cloned ? ` · ${$t("reader.cloned")}` : ""}
            </option>
          {/each}
        </select>
      {/if}
      <button class="ghost small" onclick={openCharacterPanel}>{$t("reader.voices")}</button>
      <button class="ghost small" onclick={openPracticePanel}>{$t("reader.practice")}</button>
      <label class="tts-auto">
        <input type="checkbox" bind:checked={autoAdvance} />
        {$t("reader.autoAdvance")}
      </label>
      <button
        class="ghost small"
        onclick={() => onzoom?.(Math.max(0.6, Number((zoom - 0.1).toFixed(1))))}
      >
        −
      </button>
      <span class="zoom">{Math.round(zoom * 100)}%</span>
      <button
        class="ghost small"
        onclick={() => onzoom?.(Math.min(2.5, Number((zoom + 0.1).toFixed(1))))}
      >
        +
      </button>
      {#if installedVoices.find((voice) => voice.id === preferredVoiceId)?.cloned}
        <span class="voice-watermark">{$t("pdf.clonedWatermark")}</span>
      {/if}
    </div>
  </div>

  {#if handoff}
    <div class="handoff" role="status">
      <span>
        {$t("pdf.handoffAt", {
          peer: handoff.peerName,
          page: handoff.handoff.chapter + 1,
        })}
      </span>
      <button class="primary small" onclick={resumeHandoff}>{$t("common.resume")}</button>
      <button class="ghost small" onclick={() => (handoff = null)}>{$t("common.discard")}</button>
    </div>
  {/if}

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {/if}

  {#if voiceError}
    <p class="notice" role="alert">{voiceError}</p>
  {/if}

  {#if pdf?.scanned && ocrStatus !== "done" && ocrStatus !== "skipped"}
    <div class="ocr-banner">
      <div>
        <strong>{$t("pdf.scanned")}</strong>
        <span>
          {#if ocrStatus === "running"}
            {$t("pdf.ocrRunning", { done: ocrDone, total: ocrTotal })}
          {:else if ocrStatus === "failed"}
            {$t("pdf.ocrFailed")}
          {:else}
            {$t("pdf.noText")}
          {/if}
        </span>
        {#if ocrError}<span class="ocr-error">{ocrError}</span>{/if}
      </div>
      <div class="ocr-progress">
        <span class="progress-track">
          <span
            class="progress-fill"
            style:width={`${ocrTotal ? (ocrDone / ocrTotal) * 100 : 0}%`}
          ></span>
        </span>
        <button class="primary small" onclick={startOcr} disabled={ocrStatus === "running"}>
          {ocrStatus === "running" ? $t("pdf.recognizing") : $t("pdf.retryOcr")}
        </button>
      </div>
    </div>
  {/if}

  {#if !installedVoice && downloadVoice}
    <div class="voice-banner">
      <div>
        <strong>{downloadVoice.language === "es" ? $t("pdf.spanishVoice") : $t("pdf.englishVoice")}</strong>
        <span>{$t("pdf.offlineVoice")}</span>
        {#if voiceError}<span class="ocr-error">{voiceError}</span>{/if}
      </div>
      <button
        class="primary small"
        onclick={downloadVoicePack}
        disabled={downloadProgress !== null}
      >
        {#if downloadProgress !== null}
          {Math.round(downloadProgress * 100)}%
        {:else}
          {$t("pdf.downloadVoice")}
        {/if}
      </button>
    </div>
  {/if}

  {#if loading}
    <p class="loading">{$t("pdf.opening")}</p>
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="pdf-surface"
    class:paged={mode === "paged"}
    class:dark={settings.theme === "dark"}
    bind:this={scroller}
    onscroll={onScroll}
    onmouseup={onMouseUp}
    role="region"
    aria-label={$t("pdf.document")}
  >
    {#if mode === "paged"}
      {#key spreadPages.join("-")}
        <div class="pdf-spread" style:gap={`${SPREAD_GAP}px`}>
          {#each spreadPages as page (page)}
            <div class="pdf-page-holder paged" use:pdfPage={page}></div>
          {/each}
        </div>
      {/key}
    {:else}
      {#each pageNumbers as page (page)}
        <div class="pdf-page-holder" use:pdfPage={page}></div>
      {/each}
    {/if}
  </div>

  {#if extractNotice}
    <p class="extract-notice" role="status">
      {extractNotice}
      <button class="ghost small" onclick={() => (extractNotice = null)}>{$t("common.close")}</button>
    </p>
  {/if}

  <AnnotationPanel
    bookId={session.bookId}
    bookTitle={session.document.title}
    format="pdf"
    sectionCount={pageCount}
    sections={annotationSections}
    currentSection={currentPage}
    {annotations}
    anchor={annotationAnchor}
    open={annotationsOpen}
    initialTab={annotationsTab}
    refreshToken={annotationsRefresh}
    recordToken={voiceRecordToken}
    onClose={closeAnnotations}
    onJump={jumpToAnnotation}
    onRefreshMarkers={refreshMarkers}
    onAddMarker={addPageBookmark}
    onMarkSelection={markSelection}
    onCopyQuote={copyQuote}
    onListenSelection={listenSelection}
    onError={(message) => (error = message)}
  />

  {#if mirrorOpen}
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
          {mirror?.hosting ? $t("reader.stopMirror") : $t("pdf.sharePdf")}
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

  {#if tocOpen}
    <aside class="panel toc">
      <header>
        <h2>{$t("reader.contents")}</h2>
        <button class="ghost small" onclick={() => (tocOpen = false)}>{$t("common.close")}</button>
      </header>
      {#if indexEntries.length > 0}
        <ol>
          {#each indexEntries as entry, index (index)}
            <li style:padding-left={`${Math.min(entry.level, 3) * 12}px`}>
              <button class:active={Math.abs(entry.page - currentPage) <= 1} onclick={() => goToPage(entry.page)}>
                {entry.title}
              </button>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="hint">{$t("pdf.indexHelp")}</p>
        <button class="primary small" onclick={generateIndex} disabled={buildingIndex}>
          {buildingIndex ? $t("pdf.generating") : $t("pdf.generateIndex")}
        </button>
      {/if}
    </aside>
  {/if}

  {#if searchOpen}
    <aside class="panel toc pdf-search">
      <header>
        <h2>{$t("reader.searchBook")}</h2>
        <button class="ghost small" onclick={() => (searchOpen = false)}>{$t("common.close")}</button>
      </header>
      <div class="pdf-search-box">
        <input
          type="search"
          bind:value={searchQuery}
          placeholder={$t("reader.searchPlaceholder")}
          oninput={() => {
            if (searchResults.length > 0) searchResults = [];
            searchError = null;
          }}
          onkeydown={(event) => {
            if (event.key === "Enter") void runSearch();
          }}
        />
        <button class="primary small" onclick={runSearch} disabled={searchBusy}>
          {searchBusy ? $t("library.searching") : $t("common.search")}
        </button>
      </div>
      {#if searchError}
        <p class="hint" role="status">{searchError}</p>
      {/if}
      {#if searchResults.length > 0}
        <ol class="pdf-search-results">
          {#each searchResults as hit, index (`${hit.page}-${index}`)}
            <li>
              <button onclick={() => openSearchHit(hit)}>
                <strong>{$t("reader.pageOf", { page: hit.page + 1, total: pageCount })}</strong>
                <span>{hit.snippet}</span>
              </button>
            </li>
          {/each}
        </ol>
      {/if}
    </aside>
  {/if}

  {#if activeAnnotation}
    <aside class="panel annotation-editor">
      <header>
        <h2>{$t("reader.annotation")}</h2>
        <button class="ghost small" onclick={() => (activeAnnotation = null)}>{$t("common.close")}</button>
      </header>
      {#if activeAnnotation.text}
        <blockquote>{activeAnnotation.text}</blockquote>
      {/if}
      <textarea bind:value={annotationNote} rows="3" placeholder={$t("reader.writeNote")}></textarea>
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
      <div class="editor-actions">
        <button class="primary small" onclick={saveAnnotation}>{$t("reader.saveNote")}</button>
        <button class="danger small" onclick={deleteAnnotation}>{$t("common.delete")}</button>
      </div>
    </aside>
  {/if}

  {#if characterPanelOpen}
    <aside class="panel characters">
      <header>
        <h2>{$t("reader.characterVoices")}</h2>
        <button class="ghost small" onclick={() => (characterPanelOpen = false)}>
          {$t("common.close")}
        </button>
      </header>
      <p class="hint">{$t("reader.narratorHelp")}</p>
      {#if characters.length === 0}
        <p class="empty">{$t("reader.noDialoguesPage")}</p>
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

      {#if voices.some((voice) => !voice.installed)}
        <div class="voice-downloads">
          <span class="hint">{$t("reader.downloadVoices")}</span>
          {#each voices.filter((voice) => !voice.installed) as voice (voice.id)}
            <button
              class="ghost small"
              onclick={() => downloadSpecificVoice(voice)}
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

  {#if practiceOpen}
    <aside class="panel practice">
      <header>
        <h2>{$t("reader.practice")}</h2>
        <button class="ghost small" onclick={() => (practiceOpen = false)}>
          {$t("common.close")}
        </button>
      </header>
      <p class="hint">{$t("pdf.practiceHelp")}</p>
      {#if selection}
        <blockquote>{selection.text}</blockquote>
      {:else}
        <p class="empty">{$t("reader.noPassage")}</p>
      {/if}

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
            Detener y comparar ({practiceClock(practiceSeconds)})
          </button>
          <button class="ghost small" onclick={cancelPractice}>{$t("common.cancel")}</button>
        {:else}
          <button class="primary small" onclick={startPractice}>
            Grabar mi lectura
          </button>
        {/if}
      </div>

      {#if practiceError}
        <p class="notice" role="alert">{practiceError}</p>
      {/if}

      {#if practiceResult}
        <div class="practice-result">
          <p>
            <strong>{Math.round(practiceResult.accuracy * 100)}%</strong>
            {$t("reader.accuracy", {
              seconds: (practiceResult.durationMs / 1000).toFixed(1),
              model: practiceResult.model,
            })}
          </p>
          <p class="transcript">{practiceResult.transcript || $t("reader.noTranscript")}</p>
          <div class="diff">
            {#each practiceResult.diff as word, index (index)}
              <span class="diff-word {word.kind}">
                {#if word.kind === "extra"}
                  +{word.actual}
                {:else if word.kind === "substitution"}
                  {word.expected} → {word.actual}
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
          <span class="hint">{practiceHistoryEntries.length}/50</span>
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

  <nav class="pdf-pager" aria-label={$t("reader.pageNavigation")}>
    <button
      class="pager-step"
      onclick={() => stepPage(-1)}
      disabled={currentPage <= 0}
      aria-label={$t("reader.previousPage")}
      title={$t("reader.previousPage")}
    >
      <span class="step-icon" aria-hidden="true">‹</span>
      <span class="step-label">{$t("reader.previousShort")}</span>
    </button>

    <div class="pager-status">
      <strong>{pageLabel(currentPage)}</strong>
      <span class="pager-track" aria-hidden="true">
        <span
          style:width={`${pageCount ? (((spreadPages.at(-1) ?? currentPage) + 1) / pageCount) * 100 : 0}%`}
        ></span>
      </span>
    </div>

    <button
      class="pager-step primary"
      onclick={() => stepPage(1)}
      disabled={currentPage >= pageCount - 1}
      aria-label={$t("reader.nextPage")}
      title={$t("reader.nextPage")}
    >
      <span class="step-label">{$t("reader.nextShort")}</span>
      <span class="step-icon" aria-hidden="true">›</span>
    </button>
  </nav>

  {#if visibleMarks(currentPage).length > 0}
    <footer class="pdf-footer">
      {$t("pdf.pageAnnotations", { count: visibleMarks(currentPage).length })}
    </footer>
  {/if}
</div>

<style>
  .pdf-reader {
    position: relative;
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 0.55rem;
    min-height: 0;
    padding: 0.55rem 0.75rem 0;
    overflow: hidden;
  }

  .pdf-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .pdf-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.82rem;
    color: var(--muted);
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface) 82%, transparent);
  }

  .pdf-actions button {
    min-height: 34px;
    border-radius: 9px;
    background: transparent;
    font-size: 0.76rem;
  }

  .pdf-actions button:hover {
    background: var(--surface-2);
  }

  .pdf-pager {
    position: relative;
    left: auto;
    bottom: auto;
    z-index: 15;
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    width: calc(100% + 1.5rem);
    max-width: none;
    margin: 0 -0.75rem;
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

  .pager-step:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .pager-step .step-icon {
    font-size: 1.25rem;
    line-height: 0.8;
  }

  .pager-status {
    min-width: 10rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.16rem;
    text-align: center;
  }

  .pager-status strong {
    font-size: 0.7rem;
  }

  .pager-track {
    width: 8rem;
    height: 4px;
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

  @media (max-width: 560px) {
    .pager-step .step-label {
      display: none;
    }

    .pager-step {
      min-width: 44px;
      padding: 0.5rem 0.7rem;
    }
  }

  .zoom {
    min-width: 3rem;
    text-align: center;
  }

  .voice-select {
    font: inherit;
    font-size: 0.76rem;
    padding: 0.32rem 0.45rem;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    max-width: 12rem;
  }

  .tts-excerpt {
    padding: 0.18rem 0.5rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    font-size: 0.66rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tts-rate-pdf {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.74rem;
    color: var(--muted);
  }

  .tts-rate-pdf input {
    width: 4.5rem;
    accent-color: var(--accent);
  }

  .tts-auto {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.72rem;
    color: var(--muted);
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

  .voice-watermark {
    margin: 0.35rem 0 0;
    padding: 0.3rem 0.55rem;
    border-radius: 8px;
    border: 1px dashed var(--border);
    color: var(--muted);
    font-size: 0.72rem;
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

  .notice,
  .ocr-banner,
  .voice-banner {
    margin: 0;
    padding: 0.7rem 0.9rem;
    border-radius: 10px;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 10%, transparent);
    font-size: 0.84rem;
  }

  .ocr-banner,
  .voice-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    border-color: var(--border);
    background: var(--surface);
  }

  .ocr-banner div,
  .voice-banner div {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .ocr-progress {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-direction: row !important;
  }

  .progress-track {
    width: 8rem;
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
  }

  .ocr-error {
    color: var(--warn);
    font-size: 0.78rem;
  }

  .loading {
    margin: 2rem auto;
    color: var(--muted);
  }

  .pdf-surface {
    position: relative;
    flex: 1 1 auto;
    height: auto;
    min-height: 0;
    overflow-y: auto;
    overflow-x: auto;
    border: 1px solid var(--reader-border, var(--border));
    border-radius: 18px;
    background:
      radial-gradient(
        720px 320px at 50% 0%,
        color-mix(in srgb, var(--accent) 6%, transparent),
        transparent 72%
      ),
      var(--surface-2);
    padding: clamp(1rem, 2.5vw, 1.6rem);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1.35rem;
    scroll-behavior: smooth;
  }

  .pdf-surface.paged {
    overflow: auto;
    justify-content: flex-start;
  }

  .pdf-page-holder {
    position: relative;
    flex: 0 0 auto;
    width: min(100%, 820px);
    margin-inline: auto;
    aspect-ratio: 0.707;
    background: #fff;
    box-shadow:
      0 26px 60px -30px color-mix(in srgb, #000 55%, transparent),
      0 2px 0 color-mix(in srgb, #000 8%, transparent);
    border-radius: 4px;
    overflow: hidden;
  }

  .pdf-page-holder.paged {
    margin-block: auto;
  }

  .pdf-spread {
    display: flex;
    flex: 0 0 auto;
    align-items: flex-start;
    justify-content: center;
    /* Si el zoom hace que las hojas superen el ancho, el contenedor crece
       con ellas y el desplazamiento horizontal sigue siendo accesible. */
    width: max-content;
    min-width: 100%;
    max-width: none;
    margin-inline: auto;
    margin-block: auto;
  }

  /* Dentro de la hoja doble el margen automático lo aporta el contenedor. */
  .pdf-spread .pdf-page-holder.paged {
    margin-block: 0;
  }

  .pdf-page-holder :global(.page-canvas) {
    display: block;
  }

  .pdf-surface.dark .pdf-page-holder :global(.page-canvas) {
    filter: invert(0.92) hue-rotate(180deg);
  }

  .pdf-surface.dark .pdf-page-holder :global(.mark) {
    mix-blend-mode: screen;
    background: color-mix(in srgb, var(--mark-color) 32%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--mark-color) 75%, transparent);
  }

  .pdf-page-holder :global(.word-layer) {
    position: absolute;
    inset: 0;
    overflow: hidden;
    line-height: 1;
    user-select: text;
  }

  .pdf-page-holder :global(.word) {
    position: absolute;
    color: transparent;
    background: transparent;
    white-space: pre;
    transform-origin: 0 0;
    cursor: text;
  }

  .pdf-page-holder :global(.word::selection) {
    background: rgba(66, 133, 244, 0.35);
    color: transparent;
  }

  .pdf-page-holder :global(.mark-layer) {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .pdf-page-holder :global(.mark) {
    position: absolute;
    /* El botón global de la app fija min-height: 40px; sin este ajuste las
       marcas se alargan por debajo de la línea de texto. */
    min-height: 0;
    min-width: 0;
    border: 0;
    padding: 0;
    border-radius: 2px;
    pointer-events: auto;
    cursor: pointer;
    background: color-mix(in srgb, var(--mark-color) 38%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--mark-color) 55%, transparent);
    mix-blend-mode: multiply;
  }

  .pdf-page-holder :global(.mark.kind-underline) {
    background: transparent;
    box-shadow: none;
    border-radius: 0;
  }

  .pdf-page-holder :global(.mark.kind-underline)::after {
    content: "";
    position: absolute;
    right: 0;
    bottom: 20%;
    left: 0;
    height: max(1px, 0.09em);
    border-radius: 999px;
    background: var(--mark-color);
  }

  .pdf-page-holder :global(.mark.kind-strike) {
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

  .pdf-page-holder :global(.text-mark) {
    position: absolute;
    z-index: 2;
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
  }

  .pdf-page-holder :global(.text-mark.kind-extract) {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
    font-family: Georgia, serif;
    font-size: 0.95rem;
  }

  .pdf-page-holder :global(.text-mark.kind-reaction) {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    background: color-mix(in srgb, var(--surface) 88%, var(--accent));
  }

  .pdf-page-holder :global(.text-mark:hover) {
    transform: translate(-50%, -50%) scale(1.1);
    box-shadow: var(--shadow-sm);
  }

  .pdf-page-holder :global(.tts-highlight) {
    position: absolute;
    background: color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 3px;
    pointer-events: none;
    mix-blend-mode: multiply;
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

  .toc ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
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

  .pdf-search-box {
    display: flex;
    gap: 0.4rem;
  }

  .pdf-search-box input {
    flex: 1;
    min-width: 0;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
  }

  .pdf-search-results {
    gap: 0.3rem !important;
    max-height: 50vh;
    overflow-y: auto;
  }

  .pdf-search-results li button {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .pdf-search-results li strong {
    color: var(--accent);
    font-size: 0.7rem;
  }

  .pdf-search-results li span {
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.4;
  }

  .hint {
    margin: 0 0 0.75rem;
    color: var(--muted);
    font-size: 0.82rem;
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

  .pdf-footer {
    color: var(--muted);
    font-size: 0.76rem;
    text-align: right;
  }

  @media (max-width: 760px) {
    .pdf-reader {
      padding: 0.35rem 0.35rem 0;
    }

    .pdf-surface {
      padding: 0.5rem;
    }

    .pdf-pager {
      width: calc(100% + 0.7rem);
      margin: 0 -0.35rem;
      padding-inline: 0.35rem;
    }

    .panel {
      top: auto;
      right: 0.75rem;
      bottom: 0.75rem;
      left: 0.75rem;
      width: auto;
      max-height: 60vh;
    }

    .ocr-banner,
    .voice-banner {
      flex-direction: column;
      align-items: stretch;
    }
  }
</style>

// Backend simulado para `pnpm dev` en un navegador normal. Reemplaza a Tauri
// únicamente cuando la aplicación no se está ejecutando dentro del WebView:
// la biblioteca y el lector reciben respuestas con la misma forma que el
// backend real, pero los datos viven en memoria y desaparecen al recargar.

import type {
  Achievement,
  AiConfig,
  AiMessage,
  AiReply,
  AiStatus,
  Annotation,
  AnnotationEntry,
  AnnotationFilter,
  AppInfo,
  AppPreferences,
  AppearanceState,
  AudioTreatment,
  AudioWaveform,
  AudiobookAlbumInfo,
  AudiobookCapabilities,
  AudiobookChapterInfo,
  AudiobookGenerationView,
  AudiobookInfo,
  AudiobookQueueItem,
  AudiobookScope,
  AudiobookTextLocator,
  AuditEntry,
  BookSummary,
  Category,
  CategorySuggestion,
  ChapterContent,
  ChapterEditInput,
  ChapterKaraoke,
  CharacterInfo,
  CharacterVoice,
  ClassificationConfig,
  DeviceInfo,
  DiaryDay,
  DiaryEntry,
  DiaryExport,
  DiaryMonth,
  DiaryTotals,
  Extract,
  KaraokeLine,
  LibraryOverview,
  Locator,
  MirrorStatus,
  NarrationSettings,
  Note,
  NotesExportResult,
  PageTextLayer,
  PdfConversionPreview,
  PairedDeviceInfo,
  PairingTicketView,
  PeerInfo,
  PlaybackSnapshot,
  PomodoroConfig,
  PomodoroState,
  PomodoroStatus,
  PracticeHistoryEntry,
  PracticeResult,
  Reaction,
  ReactionBucket,
  ReaderSession,
  ReaderSettings,
  ReaderSettingsInput,
  SemanticHit,
  SessionKind,
  SessionStatus,
  ShareView,
  StorageUsage,
  SyncDeviceInfo,
  SyncDiagnostics,
  SyncStatus,
  SynthesisOutput,
  TrashedBookSummary,
  TtsPreferences,
  TtsPhraseEvent,
  TtsSessionEvent,
  TtsStatus,
  VoiceCapabilities,
  VoiceDescriptor,
  VoiceDownloadProgress,
  VoiceMarkerStatus,
  VoiceModelInfo,
  VoiceModelProgress,
  VoiceNote,
  VoicePreview,
  VoiceProfile,
  VoiceSample,
  WeaveResult,
  WhisperModelInfo,
} from "../types";
import {
  chaptersFor,
  coverDataUri,
  demoAccount,
  demoAnnotationSeeds,
  demoBookSeeds,
  demoBookSummary,
  demoCategories,
  demoExtractSeeds,
  demoImportedSeeds,
  demoNoteSeeds,
  demoTrashSeeds,
  findBookSeed,
  registerBookSeed,
  type DemoBookSeed,
} from "./data";
import { buildDemoPdf, bytesToBase64 } from "./pdf";

type Args = Record<string, any>;

const OWNER = demoAccount.id;

const clone = <T>(value: T): T =>
  value === undefined ? value : JSON.parse(JSON.stringify(value));

const isoNow = () => new Date().toISOString();
const daysAgo = (days: number) =>
  new Date(Date.now() - days * 86_400_000).toISOString();

function localDateIso(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function dateDaysAgo(days: number): Date {
  const date = new Date();
  date.setDate(date.getDate() - days);
  return date;
}

function cleanOptional(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function chapterHtml(seed: DemoBookSeed, index: number): ChapterContent {
  const chapters = chaptersFor(seed);
  const entry = chapters[index];
  const plain = entry.paragraphs.map((paragraph) => paragraph.replace(/\n/g, ""));
  const text = `${entry.title}\n${plain.join("\n")}`;
  const html =
    `<h2>${escapeHtml(entry.title)}</h2>\n` +
    entry.paragraphs
      .map(
        (paragraph) =>
          `<p>${escapeHtml(paragraph).replace(/\n/g, "<br>")}</p>`,
      )
      .join("\n");
  return {
    bookId: seed.id,
    index,
    title: entry.title,
    html,
    charCount: text.length,
    hasPrevious: index > 0,
    hasNext: index < chapters.length - 1,
  };
}

function chapterText(seed: DemoBookSeed, index: number): string {
  const entry = chaptersFor(seed)[index];
  if (!entry) return "";
  return `${entry.title}\n${entry.paragraphs
    .map((paragraph) => paragraph.replace(/\n/g, ""))
    .join("\n")}`;
}

const defaultSettings = (bookId: string): ReaderSettings => ({
  bookId,
  fontSize: 19,
  lineHeight: 1.7,
  margin: 48,
  fontFamily: "serif",
  fontFace: "newsreader",
  columnWidth: 840,
  textAlign: "left",
  paragraphIndent: 0,
  hyphenate: false,
  theme: "inherit",
  texture: "inherit",
  mode: "paged",
  zoom: 1,
  updatedAt: isoNow(),
  ownerAccountId: OWNER,
  originShareId: null,
});

const demoVoices: VoiceDescriptor[] = [
  {
    id: "es_ES-davefx-medium",
    name: "DaveFX",
    language: "es",
    gender: "male",
    engine: "piper",
    installed: true,
    sizeBytes: 63_200_000,
    cloned: false,
    enhanced: false,
    quality: null,
    latency: null,
    requiresNetwork: false,
    locale: null,
  },
  {
    id: "es_ES-sharvard-medium",
    name: "Sharvard",
    language: "es",
    gender: "female",
    engine: "piper",
    installed: true,
    sizeBytes: 63_100_000,
    cloned: false,
    enhanced: false,
    quality: null,
    latency: null,
    requiresNetwork: false,
    locale: null,
  },
  {
    id: "en_US-lessac-medium",
    name: "Lessac",
    language: "en",
    gender: "female",
    engine: "piper",
    installed: false,
    sizeBytes: 63_000_000,
    cloned: false,
    enhanced: false,
    quality: null,
    latency: null,
    requiresNetwork: false,
    locale: null,
  },
];

const VOICE_CONSENT_TEXT = "Clono solo voces propias o con permiso";

const demoVoiceModelSpecs: Omit<VoiceModelInfo, "installed">[] = [
  {
    id: "encoder",
    file: "encoder.onnx",
    label: "Codificador NeuCodec",
    kind: "encoder",
    sizeBytes: 276_970_840,
    license: "Apache-2.0",
    languages: ["es", "en"],
  },
  {
    id: "encoder-data",
    file: "encoder.onnx.data",
    label: "Pesos del codificador",
    kind: "encoder",
    sizeBytes: 241_565_696,
    license: "Apache-2.0",
    languages: ["es", "en"],
  },
  {
    id: "decoder",
    file: "decoder.onnx",
    label: "Decodificador NeuCodec int8",
    kind: "decoder",
    sizeBytes: 312_292_102,
    license: "Apache-2.0",
    languages: ["es", "en"],
  },
  {
    id: "backbone-es",
    file: "neutts-nano-es-q4.gguf",
    label: "NeuTTS Nano español (Q4)",
    kind: "backbone",
    sizeBytes: 209_572_288,
    license: "NeuTTS Open License 1.0 (gratis por debajo de 5 M$ anuales)",
    languages: ["es"],
  },
  {
    id: "backbone-en",
    file: "neutts-air-en-q4.gguf",
    label: "NeuTTS Air inglés (Q4)",
    kind: "backbone",
    sizeBytes: 526_237_056,
    license: "Apache-2.0",
    languages: ["en"],
  },
];

let demoInstalledModels = new Set<string>();
let demoVoiceProfiles: VoiceProfile[] = [];
let demoRecording: { startedAt: number; deviceName: string | null } | null = null;
const demoAudioDurations = new Map<string, number>();
const demoAudioCache = new Map<string, string>();

function demoVoiceId(): string {
  return `voice-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function demoDelay(milliseconds: number) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function registerDemoAudio(path: string, durationMs: number) {
  demoAudioDurations.set(path, durationMs);
  demoAudioCache.delete(path);
}

function demoVoiceSamplePath(id: string, durationMs: number): string {
  const path = `demo/voice/${id}.wav`;
  registerDemoAudio(path, durationMs);
  return path;
}

function demoAudioDataUrl(path: string): string {
  const cached = demoAudioCache.get(path);
  if (cached) return cached;
  const durationMs = demoAudioDurations.get(path) ?? 4_200;
  const url = `data:audio/wav;base64,${wavBase64(durationMs, 410)}`;
  demoAudioCache.set(path, url);
  return url;
}

function resetVoiceStudio() {
  demoInstalledModels = new Set([
    "encoder",
    "encoder-data",
    "decoder",
    "backbone-es",
  ]);
  demoVoices.forEach((voice, index) => {
    voice.installed = index < 2;
  });
  demoRecording = null;
  demoAudioDurations.clear();
  demoAudioCache.clear();
  demoVoiceProfiles = [
    {
      id: "profile-elena",
      name: "Mi voz",
      engine: "neutts-nano",
      language: "es",
      inputFormat: "text",
      samplePath: demoVoiceSamplePath("profile-elena", 8_400),
      referenceDurationMs: 8_400,
      createdAt: daysAgo(9),
    },
    {
      id: "profile-marta",
      name: "Marta en inglés",
      engine: "neutts-air",
      language: "en",
      inputFormat: "phonemes",
      samplePath: demoVoiceSamplePath("profile-marta", 12_700),
      referenceDurationMs: 12_700,
      createdAt: daysAgo(3),
    },
  ];
}

function voiceModelList(): VoiceModelInfo[] {
  return demoVoiceModelSpecs.map((model) => ({
    ...model,
    languages: [...model.languages],
    installed: demoInstalledModels.has(model.id),
  }));
}

function voiceProfileById(id: string): VoiceProfile | undefined {
  const clean = id.startsWith("clone:") ? id.slice("clone:".length) : id;
  return demoVoiceProfiles.find((profile) => profile.id === clean);
}

function voiceProfileDescriptor(profile: VoiceProfile): VoiceDescriptor {
  return {
    id: `clone:${profile.id}`,
    name: profile.name,
    language: profile.language,
    gender: null,
    engine: profile.engine,
    installed: true,
    sizeBytes: 0,
    cloned: true,
    enhanced: false,
    quality: null,
    latency: null,
    requiresNetwork: false,
    locale: null,
  };
}

function voiceDescriptorList(): VoiceDescriptor[] {
  return [
    ...demoVoices.map(clone),
    ...demoVoiceProfiles.map(voiceProfileDescriptor).map(clone),
  ];
}

function voiceEngineForLanguage(language: string): string {
  return language === "en" ? "neutts-air" : "neutts-nano";
}

function voiceInputFormatForLanguage(language: string): string {
  return language === "en" ? "phonemes" : "text";
}

async function simulateModelDownload(modelIds: string[]) {
  for (const modelId of modelIds) {
    if (demoInstalledModels.has(modelId)) continue;
    const model = demoVoiceModelSpecs.find((item) => item.id === modelId);
    if (!model) {
      throw { code: "not_found", message: `el modelo ${modelId} no existe` };
    }
    emit("voice-model-progress", {
      modelId,
      downloaded: 0,
      total: 100,
      done: false,
      error: null,
    } satisfies VoiceModelProgress);
    for (const percent of [18, 47, 79]) {
      await demoDelay(90);
      emit("voice-model-progress", {
        modelId,
        downloaded: percent,
        total: 100,
        done: false,
        error: null,
      } satisfies VoiceModelProgress);
    }
    demoInstalledModels.add(modelId);
    emit("voice-model-progress", {
      modelId,
      downloaded: 100,
      total: 100,
      done: true,
      error: null,
    } satisfies VoiceModelProgress);
  }
}

async function simulateVoiceDownload(voice: VoiceDescriptor) {
  if (voice.installed) return;
  emit("tts-download-progress", {
    voiceId: voice.id,
    downloaded: 0,
    total: 100,
    done: false,
    error: null,
  } satisfies VoiceDownloadProgress);
  for (const percent of [32, 68]) {
    await demoDelay(110);
    emit("tts-download-progress", {
      voiceId: voice.id,
      downloaded: percent,
      total: 100,
      done: false,
      error: null,
    } satisfies VoiceDownloadProgress);
  }
  voice.installed = true;
  emit("tts-download-progress", {
    voiceId: voice.id,
    downloaded: 100,
    total: 100,
    done: true,
    error: null,
  } satisfies VoiceDownloadProgress);
}

const bookCharacters: Record<string, CharacterInfo[]> = {
  "book-mareas": [
    { name: "Elena", lines: 46, sample: "Tenía que saber la verdad." },
    { name: "El farero", lines: 31, sample: "La marea no espera a nadie." },
    { name: "La madre", lines: 14, sample: "Todas las historias son verdad." },
    { name: "El abuelo", lines: 6, sample: "El mar nunca olvida." },
  ],
  "book-dias-de-sal": [
    { name: "Inés", lines: 38, sample: "¿Por qué nadie dice el nombre de mi tía?" },
    { name: "Damián", lines: 24, sample: "No era para nosotros." },
    { name: "Ramón", lines: 27, sample: "Este año el mar no quiere nada con nosotros." },
    { name: "La madre", lines: 9, sample: "El silencio también es una forma de hablar." },
  ],
  "book-cartas-faro": [
    { name: "La narradora", lines: 52, sample: "El faro solo funciona si alguien le cuenta lo que ve." },
    { name: "Marta", lines: 18, sample: "Desde allí todo parecía posible." },
  ],
  "book-mecanica": [
    { name: "Ilan", lines: 43, sample: "No hacía falta saber la hora para recordar." },
    { name: "El maestro", lines: 19, sample: "Ya falta menos." },
    { name: "La niña", lines: 7, sample: "Quería darle la hora exacta." },
  ],
  "book-jardin": [
    { name: "Marta", lines: 41, sample: "Las casas también crían raíces." },
    { name: "La abuela", lines: 26, sample: "El tiempo de las siestas es el único que no se pierde." },
  ],
  "book-ultima-biblioteca": [
    { name: "Lía", lines: 44, sample: "No se trata de terminar. Se trata de no dejar que se pierdan." },
    { name: "El supervisor", lines: 21, sample: "Los libros que importan siempre encuentran un lector." },
  ],
};

const bookAiNotes: Record<
  string,
  { summary: string; terms: string; characters: string }
> = {
  "book-mareas": {
    summary:
      "El capítulo sigue a Elena en su regreso al pueblo y la enfrenta a la puerta que solo aparece con la marea baja. La escena combina el descubrimiento físico del archivo de su madre con la presión silenciosa de un pueblo que lleva décadas callando. El tono es de misterio íntimo: lo sobrenatural nunca se separa del todo de la memoria familiar.",
    terms:
      "1. **La marea**: ciclo natural que también marca el ritmo de los secretos familiares.\n2. **La puerta de hierro**: umbral entre lo conocido y lo que el pueblo decidió enterrar.\n3. **La llave fría**: la responsabilidad de abrir lo que otros cerraron.\n4. **El faro**: vigilancia, pero también complicidad; su luz no solo guía, también cuenta.\n5. **Los faroles con nombres**: imágenes de las personas que el mar devolvió a la memoria.",
    characters:
      "**Elena**: protagonista; vuelve para reconstruir la historia de su madre.\n**El farero**: guardián del secreto y, a la vez, testigo que empieza a hablar.\n**La madre**: ausente presente; su cuaderno funciona como una segunda voz narrativa.\n**El abuelo**: mencionado como origen de la advertencia familiar sobre el mar.",
  },
  "book-cartas-faro": {
    summary:
      "La carta convierte el faro en un lugar de espera y de escritura. La voz narradora revisa el pasado sin dramatizarlo: los objetos concretos —la escalera, la placa del muelle, la silla vacía— sostienen una emoción que nunca se nombra del todo.",
    terms:
      "1. **El faro**: vigilancia y soledad; también la continuidad de una promesa.\n2. **Las cartas no enviadas**: palabras que buscan destino sin atreverse a pedirlo.\n3. **La silla vacía**: la ausencia de Marta convertida en espacio doméstico.\n4. **La luz que gira**: el tiempo que pasa aunque nadie lo mire.",
    characters:
      "**La narradora**: escribe cartas a Marta desde el faro; su voz mezcla recuerdo y presente.\n**Marta**: destinataria ausente; aparece por lo que la narradora recuerda y calla.",
  },
  "book-mecanica": {
    summary:
      "El capítulo presenta a Ilan y el taller heredado como un espacio donde el tiempo se ha vuelto materia. La máquina de latón no viaja al pasado: abre una rendija en la memoria de los objetos. El relato mantiene la duda entre el milagro y el mecanismo.",
    terms:
      "1. **Los relojes parados**: el tiempo detenido como duelo y como resistencia.\n2. **La máquina de latón**: tecnología imposible que conserva ecos, no imágenes.\n3. **La llave diminuta**: acceso a una memoria que no pertenece a quien la encuentra.\n4. **La nieve en Varena**: un tiempo atmosférico que llega tarde, como los recuerdos.",
    characters:
      "**Ilan**: relojero; hereda el taller y la obsesión del maestro.\n**El maestro**: ausente; sus notas y la máquina organizan el misterio.\n**La niña del reloj**: devuelve a Ilan al presente y a lo cotidiano.",
  },
  "book-jardin": {
    summary:
      "La narradora hereda la casa de su abuela y, con ella, un cuaderno de recetas, cuentas y frases sueltas. El capítulo trabaja con la lentitud de los objetos domésticos: el jardín, la parra, la aguja. La melancolía no se impone; se descubre en los márgenes.",
    terms:
      "1. **Las buganvillas**: belleza que crece sin pedir permiso y sobrevive a quienes la plantaron.\n2. **El cuaderno**: memoria fragmentaria, más verdadera que un relato ordenado.\n3. **La silla de más**: esperanza y costumbre de dejar sitio a quien no está.\n4. **Las siestas**: el tiempo que no se mide en productividad.",
    characters:
      "**Marta**: narradora y nieta; vuelve a la casa y decide quedarse.\n**La abuela**: presencia central a través del jardín, el cuaderno y los objetos.\n**La madre**: mencionada en las cartas, forma parte del linaje de ausencias.",
  },
  "book-dias-de-sal": {
    summary:
      "El capítulo retrata un verano de escasez en un pueblo pesquero. La mirada de Inés convierte el silencio adulto en materia narrativa: las redes vacías, las conversaciones interrumpidas y la partida de la madre. El conflicto no se explica, se acumula en gestos y en objetos.",
    terms:
      "1. **La sal**: trabajo, herida y memoria del mar.\n2. **Las redes vacías**: crisis económica y vacío afectivo.\n3. **El muelle**: frontera entre los que se quedan y los que se van.\n4. **La botella con el mensaje**: irrupción de una historia ajena en la vida cotidiana.",
    characters:
      "**Inés**: narradora adolescente; observa y pregunta lo que los adultos callan.\n**Damián**: hermano mayor; elige marcharse.\n**Ramón**: padre pescador; su silencio es una forma de protección fallida.\n**La madre**: su ausencia organiza la vida familiar.",
  },
  "book-ultima-biblioteca": {
    summary:
      "Lía trabaja en un archivo de libros que nunca llegaron a escribirse. La llegada de un volumen inclasificable pone en cuestión el sistema que ella misma aplica. El capítulo mezcla la burocracia del archivo con una imaginación desbordante.",
    terms:
      "1. **El archivo de libros imposibles**: espacio donde lo no escrito sigue teniendo consecuencias.\n2. **Los libros que esperan**: metáfora de las historias que sobreviven a su abandono.\n3. **La torre sin planos**: institución invisible dentro de la ciudad.\n4. **El departamento de Incendios**: ironía sobre la conservación y la destrucción.",
    characters:
      "**Lía**: bibliotecaria; su curiosidad la lleva a completar lo incompleto.\n**El supervisor**: guardián de las reglas del archivo.\n**El libro sin autor**: presencia que desestabiliza el orden de la biblioteca.",
  },
};

const genericAiNotes = {
  summary:
    "El fragmento desarrolla una escena de observación y memoria. La voz narrativa avanza con cuidado, dejando que los objetos y los gestos digan más que las explicaciones. El conflicto se mantiene en un segundo plano, como una tensión que el texto prefiere no resolver todavía.",
  terms:
    "1. **La memoria**: eje que conecta los objetos y las decisiones de los personajes.\n2. **El silencio**: espacio donde se acumula lo que no se dice.\n3. **El paisaje**: no es decorado, sino una forma de presión sobre los personajes.\n4. **El viaje o el regreso**: movimiento que permite mirar de nuevo lo conocido.",
  characters:
    "El fragmento se centra en una voz narradora que observa y recuerda. Los demás personajes aparecen a través de sus gestos y de lo que otros dicen de ellos; no hay antagonistas claros, sino presencias que condicionan la escena.",
};

interface DemoState {
  books: BookSummary[];
  categories: Category[];
  trash: TrashedBookSummary[];
  suggestions: CategorySuggestion[];
  settings: Map<string, ReaderSettings>;
  progress: Map<string, { section: number; offset: number; percent: number }>;
  annotations: Annotation[];
  notes: Note[];
  extracts: Extract[];
  reactions: Reaction[];
  voiceNotes: VoiceNote[];
  diary: DiaryEntry[];
  voiceCharacters: Map<string, string>;
  ttsPreferences: Map<string, TtsPreferences>;
  practiceHistory: PracticeHistoryEntry[];
  practice: {
    bookId: string;
    section: number;
    offsetStart: number;
    offsetEnd: number;
    expected: string;
    startedAt: number;
  } | null;
  imported: number;
}

interface TtsRuntime {
  status: TtsStatus;
  text: string;
  words: string[];
  unit: "utf16" | "word";
  cursor: number;
  end: number | null;
  timer?: ReturnType<typeof setTimeout>;
}

interface DemoPomodoroRuntime {
  state: PomodoroState;
  startedAt: number | null;
  endsAt: number | null;
  round: number;
  completedFocus: number;
  config: PomodoroConfig;
  timer?: ReturnType<typeof setTimeout>;
  generation: number;
}

interface DemoSession {
  bookId: string;
  kind: SessionKind;
  startedAt: number;
}

let state: DemoState;
let mirror: MirrorStatus;
let tts: TtsRuntime;
let pomodoro: DemoPomodoroRuntime;
let demoSessions = new Map<string, DemoSession>();
let achievementProgressStore = new Map<string, number>();
let achievementUnlockedStore = new Map<string, string>();
let audiobooks: AudiobookInfo[] = [];
let demoAlbums: AudiobookAlbumInfo[] = [];
let generation: AudiobookGenerationView = { active: null, queue: [] };
let generationPercent = 0;
let generationTimer: ReturnType<typeof setTimeout> | undefined;
let audiobookPlayback: PlaybackSnapshot | null = null;
let audiobookPlaybackBase = 0;
let audiobookPlaybackStartedAt = 0;
let audiobookSpeed = 1;
let voiceMarkerPhrase = "Kora, marca esto";
let voiceMarkerListening = false;
let voiceMarkerTimer: ReturnType<typeof setTimeout> | undefined;
let shareSeq = 1;
let demoPeers: PeerInfo[] = [];
let demoAccountDevices: DeviceInfo[] = [];
let demoSyncStatus: SyncStatus = {
  enabled: false,
  running: false,
  peerCount: 0,
  listenPort: 0,
  trustedPeers: 0,
  pendingChanges: 0,
  lastSync: null,
  pairing: false,
  sharing: false,
  mirrorActive: false,
  mirrorHosting: false,
  syncing: false,
  autoSync: true,
  onlinePeers: 0,
  lastError: null,
};
let demoSyncSelections = new Map<
  string,
  { categoryIds: string[]; syncAll: boolean }
>();
let demoAudit: AuditEntry[] = [];
let demoAuditSeq = 1;
let demoUsage: StorageUsage = {
  total: 0,
  database: 0,
  library: 0,
  audiobooks: 0,
  voices: 0,
  models: 0,
  trash: 0,
  logs: 0,
  exports: 0,
  temporary: 0,
};
let demoAiConfig: AiConfig = {
  provider: "auto",
  ollamaUrl: "http://127.0.0.1:11434",
  llamaUrl: "http://127.0.0.1:8080",
  model: "llama3.2:3b",
  temperature: 0.55,
  maxTokens: 900,
};
let demoClassificationConfig: ClassificationConfig = {
  enabled: true,
  classifyOnImport: true,
  useAi: true,
  allowNewCategories: true,
  embeddingThreshold: 0.72,
  aiThreshold: 0.58,
  maxSuggestions: 5,
};
const pdfCache = new Map<string, Uint8Array>();
const coverOverrides = new Map<string, string>();
const chapterOverrides = new Map<string, ChapterEditInput[]>();
const callbacks = new Map<number, (message: unknown) => void>();
const listeners = new Map<string, Set<number>>();
let callbackSeq = 1;
let fullscreen = false;

function emit(event: string, payload: unknown) {
  const ids = listeners.get(event);
  if (!ids) return;
  for (const id of [...ids]) {
    const callback = callbacks.get(id);
    if (callback) callback({ event, id, payload });
  }
}

function demoAuditEntry(
  action: string,
  resourceType: string | null,
  resourceId: string | null,
  metadata: Record<string, unknown> = {},
  createdAt = isoNow(),
): AuditEntry {
  return {
    id: `audit-${String(demoAuditSeq++).padStart(4, "0")}`,
    actorAccountId: OWNER,
    action,
    resourceType,
    resourceId,
    metadata: JSON.stringify(metadata),
    createdAt,
  };
}

function seedDemoPeers(): PeerInfo[] {
  return [
    {
      id: "peer-marta",
      name: "Portátil de Marta",
      platform: "linux",
      address: "192.168.1.42",
      addresses: ["192.168.1.42"],
      port: 43_777,
      lastSeen: daysAgo(0),
      trusted: true,
      accountHint: "@marta",
      purpose: "library",
      sasPrefix: "7F2A",
      fingerprint: "8c:41:2e:9a:74:0b:55:de",
      fullname: "Marta Ríos",
      protocolVersion: 2,
      ticket: null,
    },
    {
      id: "peer-salon",
      name: "Kora del salón",
      platform: "android",
      address: "192.168.1.18",
      addresses: ["192.168.1.18"],
      port: 43_777,
      lastSeen: daysAgo(1),
      trusted: true,
      accountHint: "@casa",
      purpose: "mirror",
      sasPrefix: "C19B",
      fingerprint: "2a:0f:63:bd:11:84:7e:30",
      fullname: "Kora del salón",
      protocolVersion: 2,
      ticket: null,
    },
  ];
}

function seedDemoDevices(peers: PeerInfo[]): DeviceInfo[] {
  const local: DeviceInfo = {
    id: "device-elena",
    accountId: OWNER,
    name: "Kora de Elena",
    platform: "linux",
    publicKey: demoAccount.publicKey,
    certificateFingerprint: "7d:91:3c:4e:5f:02:ab:66",
    trusted: true,
    sasVerifiedAt: null,
    lastAddress: "127.0.0.1",
    lastPort: null,
    peerAccountId: null,
    peerUsername: null,
    peerAccountPublicKey: null,
    accountSignature: null,
    peerAccountSignature: null,
    revokedAt: null,
    lastSeenAt: isoNow(),
    isLocal: true,
    createdAt: daysAgo(64),
  };
  const remote = peers.map(
    (peer, index): DeviceInfo => ({
      id: peer.id,
      accountId: OWNER,
      name: peer.name,
      platform: index === 0 ? "linux" : "android",
      publicKey: null,
      certificateFingerprint: peer.fingerprint,
      trusted: peer.trusted,
      sasVerifiedAt: peer.lastSeen,
      lastAddress: peer.address,
      lastPort: peer.port,
      peerAccountId: null,
      peerUsername: peer.accountHint,
      peerAccountPublicKey: null,
      accountSignature: null,
      peerAccountSignature: null,
      revokedAt: peer.trusted ? null : isoNow(),
      lastSeenAt: peer.lastSeen,
      isLocal: false,
      createdAt: daysAgo(32 - index * 6),
    }),
  );
  return [local, ...remote];
}

function syncDevice(device: DeviceInfo): SyncDeviceInfo {
  return {
    id: device.id,
    name: device.name,
    platform: device.platform,
    isLocal: device.isLocal,
    trusted: device.trusted,
    lastSeenAt: device.lastSeenAt,
  };
}

function syncDeviceById(deviceId: string | null): DeviceInfo | undefined {
  if (!deviceId) {
    return demoAccountDevices.find((device) => device.isLocal);
  }
  return demoAccountDevices.find((device) => device.id === deviceId);
}

function revokeDemoPeer(deviceId: string): void {
  const device = demoAccountDevices.find((item) => item.id === deviceId);
  if (!device) {
    throw { code: "not_found", message: "dispositivo no encontrado" };
  }
  if (device.isLocal) {
    throw {
      code: "invalid_input",
      message: "no se puede revocar el dispositivo local",
    };
  }
  device.trusted = false;
  device.revokedAt = isoNow();
  demoPeers = demoPeers.filter((peer) => peer.id !== deviceId);
  demoAccountDevices = demoAccountDevices.map((item) =>
    item.id === deviceId ? device : item,
  );
  emitSyncStatus();
}

function emitSyncStatus() {
  demoSyncStatus.trustedPeers = demoPeers.filter((peer) => peer.trusted).length;
  demoSyncStatus.peerCount = demoSyncStatus.running ? demoPeers.length : 0;
  demoSyncStatus.mirrorActive = mirror.active;
  demoSyncStatus.mirrorHosting = mirror.hosting;
  emit("sync::status", clone(demoSyncStatus));
  return clone(demoSyncStatus);
}

function startDemoSync(): SyncStatus {
  demoSyncStatus.running = true;
  demoSyncStatus.enabled = true;
  demoSyncStatus.listenPort = 43_777;
  demoSyncStatus.peerCount = demoPeers.length;
  return emitSyncStatus();
}

function stopDemoSync(): SyncStatus {
  demoSyncStatus.running = false;
  demoSyncStatus.enabled = false;
  demoSyncStatus.peerCount = 0;
  demoSyncStatus.pairing = false;
  demoSyncStatus.sharing = false;
  demoSyncStatus.mirrorActive = false;
  demoSyncStatus.mirrorHosting = false;
  return emitSyncStatus();
}

function startDemoShare(): SyncStatus {
  startDemoSync();
  demoSyncStatus.sharing = true;
  return emitSyncStatus();
}

function usageTotal(usage: StorageUsage): number {
  return (
    usage.database +
    usage.library +
    usage.audiobooks +
    usage.voices +
    usage.models +
    usage.trash +
    usage.logs +
    usage.exports +
    usage.temporary
  );
}

function resetSyncAndSettingsDemo() {
  demoPeers = seedDemoPeers();
  demoAccountDevices = seedDemoDevices(demoPeers);
  demoSyncSelections = new Map(
    demoPeers.map((peer) => [peer.id, { categoryIds: [], syncAll: true }]),
  );
  demoSyncStatus = {
    enabled: false,
    running: false,
    peerCount: 0,
    listenPort: 0,
    trustedPeers: demoPeers.length,
    pendingChanges: 7,
    lastSync: daysAgo(1),
    pairing: false,
    sharing: false,
    mirrorActive: false,
    mirrorHosting: false,
    syncing: false,
    autoSync: true,
    onlinePeers: demoPeers.length,
    lastError: null,
  };
  demoAuditSeq = 1;
  demoAudit = [
    demoAuditEntry(
      "device.pair",
      null,
      "peer-marta",
      { deviceName: "Portátil de Marta" },
      daysAgo(4),
    ),
    demoAuditEntry(
      "book.import",
      "book",
      state.books[1]?.id ?? "book-mareas",
      { format: "epub" },
      daysAgo(3),
    ),
    demoAuditEntry(
      "share.accepted",
      "book",
      state.books[2]?.id ?? "book-jardin",
      { peer: "Kora del salón" },
      daysAgo(1),
    ),
  ];
  demoUsage = {
    total: 0,
    database: 2_400_000,
    library: 96_800_000,
    audiobooks: 41_200_000,
    voices: 12_600_000,
    models: 27_800_000,
    trash: 640_000,
    logs: 410_000,
    exports: 400_000,
    temporary: 200_000,
  };
  demoUsage.total = usageTotal(demoUsage);
  demoAiConfig = {
    provider: "auto",
    ollamaUrl: "http://127.0.0.1:11434",
    llamaUrl: "http://127.0.0.1:8080",
    model: "llama3.2:3b",
    temperature: 0.55,
    maxTokens: 900,
  };
  demoClassificationConfig = {
    enabled: true,
    classifyOnImport: true,
    useAi: true,
    allowNewCategories: true,
    embeddingThreshold: 0.72,
    aiThreshold: 0.58,
    maxSuggestions: 5,
  };
}

function makeSuggestion(
  bookId: string,
  categoryId: string | null,
  score: number,
  source: CategorySuggestion["source"],
  reason: string,
  proposedName: string | null = null,
): CategorySuggestion {
  const book = state.books.find((item) => item.id === bookId);
  const category = state.categories.find((item) => item.id === categoryId);
  return {
    id: `suggestion-${bookId}-${categoryId ?? proposedName ?? "nueva"}`,
    bookId,
    bookTitle: book?.title ?? "Libro",
    author: book?.author ?? null,
    format: book?.format ?? "epub",
    audioOnly: false,
    coverPath: book?.coverPath ?? null,
    categoryId,
    categoryName: category?.name ?? null,
    categoryColor: category?.color ?? null,
    proposedName,
    score,
    source,
    reason,
    status: "pending",
    createdAt: daysAgo(1),
  };
}

function buildInitialState(): DemoState {
  const books = demoBookSeeds.map(demoBookSummary);
  const categories = demoCategories.map((category) => ({ ...category }));
  const settings = new Map<string, ReaderSettings>();
  const progress = new Map<
    string,
    { section: number; offset: number; percent: number }
  >();
  for (const seed of demoBookSeeds) {
    const defaults = defaultSettings(seed.id);
    settings.set(seed.id, {
      ...defaults,
      ...seed.settings,
      bookId: seed.id,
      updatedAt: daysAgo(3),
      mode: seed.format === "pdf" ? "scroll" : (seed.settings?.mode ?? "paged"),
    });
    progress.set(seed.id, {
      section: seed.locator.section,
      offset: seed.locator.offset,
      percent: seed.progress,
    });
  }

  const annotations: Annotation[] = [];
  for (const marker of demoAnnotationSeeds) {
    const seed = findBookSeed(marker.bookId);
    if (!seed) continue;
    const text = chapterText(seed, marker.chapterIndex);
    const start = text.indexOf(marker.quote);
    if (start < 0) continue;
    annotations.push({
      id: marker.id,
      bookId: marker.bookId,
      kind: marker.kind,
      locator: JSON.stringify({
        section: marker.chapterIndex,
        offset: start,
      }),
      chapterIndex: marker.chapterIndex,
      offsetStart: start,
      offsetEnd: start + marker.quote.length,
      text: marker.quote,
      note: marker.note,
      color: marker.color,
      createdAt: daysAgo(2),
      updatedAt: daysAgo(2),
      deleted: false,
      ownerAccountId: OWNER,
      originShareId: null,
    });
  }

  const notes: Note[] = demoNoteSeeds.map((seed, index) => ({
    id: seed.id,
    bookId: seed.bookId,
    title: seed.title,
    body: seed.body,
    contentJson: JSON.stringify({
      ops: [{ insert: `${seed.body}\n` }],
    }),
    chapterIndex: seed.chapterIndex,
    offsetStart: null,
    offsetEnd: null,
    text: seed.text,
    audioHash: null,
    audioExt: null,
    createdAt: daysAgo(index + 2),
    updatedAt: daysAgo(index + 2),
    deleted: false,
    ownerAccountId: OWNER,
    originShareId: null,
  }));

  const extracts: Extract[] = [];
  for (const seed of demoExtractSeeds) {
    const bookSeed = findBookSeed(seed.bookId);
    if (!bookSeed) continue;
    const text = chapterText(bookSeed, seed.chapterIndex);
    const start = text.indexOf(seed.quote);
    if (start < 0) continue;
    extracts.push({
      id: seed.id,
      bookId: seed.bookId,
      locator: JSON.stringify({
        section: seed.chapterIndex,
        offset: start,
      }),
      text: seed.quote,
      note: seed.note,
      chapterIndex: seed.chapterIndex,
      offsetStart: start,
      offsetEnd: start + seed.quote.length,
      citation: `${bookSeed.title} · ${bookSeed.author}`,
      createdAt: daysAgo(4),
      updatedAt: daysAgo(4),
      deleted: false,
      ownerAccountId: OWNER,
      originShareId: null,
    });
  }

  const reactions: Reaction[] = [
    {
      id: "reaction-demo-1",
      bookId: "book-mareas",
      chapterIndex: 2,
      offsetStart: 0,
      offsetEnd: 0,
      kind: "emotion",
      text: "La despedida de la madre.",
      createdAt: daysAgo(3),
      updatedAt: daysAgo(3),
      deleted: false,
      ownerAccountId: OWNER,
      originShareId: null,
    },
    {
      id: "reaction-demo-2",
      bookId: "book-jardin",
      chapterIndex: 1,
      offsetStart: 0,
      offsetEnd: 0,
      kind: "like",
      text: "La lista de la abuela.",
      createdAt: daysAgo(5),
      updatedAt: daysAgo(5),
      deleted: false,
      ownerAccountId: OWNER,
      originShareId: null,
    },
  ];

  const diary: DiaryEntry[] = [
    {
      id: "diary-demo-1",
      bookId: "book-mareas",
      entryDate: localDateIso(dateDaysAgo(1)),
      body:
        "Sesión de lectura al volver del trabajo. La escena de la puerta me dejó pensando en las historias familiares que se cuentan a medias.",
      mood: "pensativa",
      kind: "reading",
      sessionStart: daysAgo(1),
      sessionEnd: daysAgo(1),
      durationMinutes: 42,
      pagesRead: 18,
      extractsCreated: 1,
      notesCreated: 0,
      pomodoroRound: null,
      createdAt: daysAgo(1),
      updatedAt: daysAgo(1),
      ownerAccountId: OWNER,
      originShareId: null,
    },
    {
      id: "diary-demo-2",
      bookId: "book-jardin",
      entryDate: localDateIso(dateDaysAgo(3)),
      body:
        "Leí el capítulo del cuaderno con el café de la tarde. Subrayé la frase de las siestas.",
      mood: "tranquila",
      kind: "reading",
      sessionStart: daysAgo(3),
      sessionEnd: daysAgo(3),
      durationMinutes: 28,
      pagesRead: 11,
      extractsCreated: 0,
      notesCreated: 1,
      pomodoroRound: null,
      createdAt: daysAgo(3),
      updatedAt: daysAgo(3),
      ownerAccountId: OWNER,
      originShareId: null,
    },
    {
      id: "diary-demo-3",
      bookId: null,
      entryDate: localDateIso(dateDaysAgo(7)),
      body:
        "Pomodoro de 25 minutos con «La mecánica del silencio». Me costó concentrarme, pero terminé el capítulo.",
      mood: null,
      kind: "pomodoro",
      sessionStart: daysAgo(7),
      sessionEnd: daysAgo(7),
      durationMinutes: 25,
      pagesRead: 0,
      extractsCreated: 0,
      notesCreated: 0,
      pomodoroRound: 1,
      createdAt: daysAgo(7),
      updatedAt: daysAgo(7),
      ownerAccountId: OWNER,
      originShareId: null,
    },
  ];

  const trashSeeds = [demoTrashSeeds[0]];
  const trash = trashSeeds.map((seed) => ({
    ...demoBookSummary(seed),
    deletedAt: daysAgo(2),
  }));

  const base: DemoState = {
    books,
    categories,
    trash,
    suggestions: [],
    settings,
    progress,
    annotations,
    notes,
    extracts,
    reactions,
    voiceNotes: [],
    diary,
    voiceCharacters: new Map(),
    ttsPreferences: new Map(),
    practiceHistory: [],
    practice: null,
    imported: 0,
  };

  state = base;
  base.voiceNotes = [
    {
      id: "voice-marker-demo-1",
      bookId: "book-jardin",
      chapterIndex: 1,
      offsetStart: 0,
      offsetEnd: 0,
      text: "El cuaderno de las siestas",
      durationMs: 4_200,
      audiobookId: "audiobook-jardin",
      positionMs: 4_170_000,
      quote: "«El tiempo de las siestas es el único que no se pierde».",
      transcript: "Kora, marca esto",
      source: "voice_marker",
      createdAt: daysAgo(2),
      updatedAt: daysAgo(2),
      deleted: false,
      ownerAccountId: OWNER,
      originShareId: null,
      audioPath: null,
    },
    {
      id: "voice-marker-demo-2",
      bookId: "book-mareas",
      chapterIndex: 0,
      offsetStart: 0,
      offsetEnd: 0,
      text: "La casa junto al mar",
      durationMs: 3_600,
      audiobookId: "audiobook-mareas",
      positionMs: 912_000,
      quote: "«El mar nunca olvida a quienes lo escuchan».",
      transcript: "Kora, marca esto",
      source: "voice_marker",
      createdAt: daysAgo(6),
      updatedAt: daysAgo(6),
      deleted: false,
      ownerAccountId: OWNER,
      originShareId: null,
      audioPath: null,
    },
  ];
  base.suggestions = [
    makeSuggestion(
      "book-ultima-biblioteca",
      "cat-ciencia",
      0.93,
      "embedding",
      "Los temas de archivo, futuros posibles y libros imposibles encajan con Ciencia ficción.",
    ),
    makeSuggestion(
      "book-sombras-pdf",
      "cat-historia",
      0.86,
      "metadata",
      "El título y la editorial sugieren un ensayo histórico sobre la memoria.",
    ),
  ];
  return base;
}

function updateBook(bookId: string, patch: Partial<BookSummary>): BookSummary {
  const current = state.books.find((book) => book.id === bookId);
  if (!current) throw { code: "not_found", message: "Libro no encontrado" };
  Object.assign(current, patch);
  return clone(current);
}

function pdfFor(seed: DemoBookSeed): Uint8Array {
  const cached = pdfCache.get(seed.id);
  if (cached) return cached;
  const bytes = buildDemoPdf(
    seed.title,
    seed.author,
    seed.pdfPages ?? [["Página en blanco"]],
  );
  pdfCache.set(seed.id, bytes);
  return bytes;
}

function pdfDataUrl(bookId: string): string {
  const seed = findBookSeed(bookId);
  if (!seed) return "";
  return `data:application/pdf;base64,${bytesToBase64(pdfFor(seed))}`;
}

function ttsStatus(): TtsStatus {
  return clone(tts.status);
}

function ttsPreferences(bookId: string): TtsPreferences {
  const current = state.ttsPreferences.get(bookId);
  if (current) return clone(current);
  return {
    bookId,
    voiceId: "es_ES-davefx-medium",
    rate: 1,
    pitch: 1,
    pauseScale: 1,
    expressive: true,
    multiVoice: false,
    updatedAt: daysAgo(4),
    ownerAccountId: OWNER,
    originShareId: null,
  };
}

function emitTtsState(stateName: TtsStatus["state"], error: string | null = null) {
  tts.status.state = stateName;
  tts.status.error = error;
  const payload: TtsSessionEvent = {
    bookId: tts.status.bookId ?? "",
    section: tts.status.section ?? 0,
    state: stateName,
    error,
    excerpt: tts.status.excerpt,
  };
  emit("tts::state", payload);
}

function clearPhraseTimer() {
  if (tts.timer) {
    clearTimeout(tts.timer);
    tts.timer = undefined;
  }
}

function schedulePhrase() {
  clearPhraseTimer();
  if (tts.status.state !== "playing") return;

  if (tts.unit === "word") {
    const limit = tts.end ?? tts.words.length;
    if (tts.cursor >= limit) {
      emitTtsState("finished");
      return;
    }
    const start = tts.cursor;
    const end = Math.min(limit, start + 12);
    const phrase: TtsPhraseEvent = {
      bookId: tts.status.bookId ?? "",
      section: tts.status.section ?? 0,
      offsetStart: start,
      offsetEnd: end,
      chunkIndex: 0,
      unit: "word",
      character: null,
      voiceId: "es_ES-davefx-medium",
      text: tts.words.slice(start, end).join(" "),
    };
    emit("tts::phrase_start", phrase);
    tts.timer = setTimeout(() => {
      if (tts.status.state !== "playing") return;
      emit("tts::phrase_end", phrase);
      tts.cursor = end;
      schedulePhrase();
    }, Math.max(900, (end - start) * 150));
    return;
  }

  const limit = tts.end ?? tts.text.length;
  if (tts.cursor >= limit) {
    emitTtsState("finished");
    return;
  }

  let start = tts.cursor;
  while (start < limit && /\s/.test(tts.text[start] ?? "")) start += 1;
  if (start >= limit) {
    emitTtsState("finished");
    return;
  }

  let end = Math.min(limit, start + 92);
  const nextStop = tts.text.slice(end, Math.min(limit, end + 38)).search(/[.!?;:]/);
  if (nextStop >= 0) {
    end = Math.min(limit, end + nextStop + 1);
  } else {
    const space = tts.text.indexOf(" ", end);
    if (space > 0 && space < limit) end = space;
  }
  if (end <= start) end = Math.min(limit, start + 1);

  const phrase: TtsPhraseEvent = {
    bookId: tts.status.bookId ?? "",
    section: tts.status.section ?? 0,
    offsetStart: start,
    offsetEnd: end,
    chunkIndex: 0,
    unit: "utf16",
    character: null,
    voiceId: "es_ES-davefx-medium",
    text: tts.text.slice(start, end),
  };
  emit("tts::phrase_start", phrase);
  tts.timer = setTimeout(() => {
    if (tts.status.state !== "playing") return;
    emit("tts::phrase_end", phrase);
    tts.cursor = end;
    schedulePhrase();
  }, Math.max(900, Math.round((end - start) * 46)));
}

function startTts(bookId: string, fromOffset: { section: number; offset: number; end?: number }) {
  const seed = findBookSeed(bookId);
  if (!seed) {
    throw { code: "not_found", message: "Libro no encontrado" };
  }
  let section = 0;
  if (seed.format === "pdf") {
    section = Math.min(
      Math.max(0, fromOffset.section),
      Math.max(0, (seed.pdfPages?.length ?? 1) - 1),
    );
    tts.words = (seed.pdfPages?.[section] ?? [])
      .join(" ")
      .split(/\s+/)
      .filter(Boolean);
    tts.text = tts.words.join(" ");
    tts.unit = "word";
    tts.cursor = Math.min(Math.max(0, fromOffset.offset), tts.words.length);
    tts.end =
      fromOffset.end != null
        ? Math.min(tts.words.length, fromOffset.end)
        : null;
  } else {
    const chapters = chaptersFor(seed);
    section = Math.min(
      Math.max(0, fromOffset.section),
      Math.max(0, chapters.length - 1),
    );
    const text = chapterText(seed, section);
    tts.text = text;
    tts.words = [];
    tts.unit = "utf16";
    tts.cursor = Math.min(
      Math.max(0, fromOffset.offset),
      Math.max(0, text.length - 1),
    );
    tts.end = fromOffset.end != null ? Math.min(text.length, fromOffset.end) : null;
  }
  tts.status = {
    bookId,
    section,
    state: "playing",
    rate: tts.status.rate,
    pitch: tts.status.pitch,
    chunkIndex: 0,
    phraseStart: null,
    phraseEnd: null,
    error: null,
    excerpt: tts.end != null,
    media: null,
  };
  emit("tts::state", {
    bookId,
    section,
    state: "playing",
    error: null,
    excerpt: tts.status.excerpt,
  } satisfies TtsSessionEvent);
  schedulePhrase();
  return ttsStatus();
}

function pauseTts() {
  if (tts.status.state === "playing") {
    clearPhraseTimer();
    emitTtsState("paused");
  }
  return ttsStatus();
}

function resumeTts() {
  if (tts.status.state === "paused") {
    tts.status.state = "playing";
    emitTtsState("playing");
    schedulePhrase();
  }
  return ttsStatus();
}

function stopTts() {
  clearPhraseTimer();
  const previousBook = tts.status.bookId;
  const previousSection = tts.status.section;
  tts.status = {
    bookId: previousBook,
    section: previousSection,
    state: "stopped",
    rate: tts.status.rate,
    pitch: tts.status.pitch,
    chunkIndex: null,
    phraseStart: null,
    phraseEnd: null,
    error: null,
    excerpt: false,
    media: null,
  };
  if (previousBook) {
    emit("tts::state", {
      bookId: previousBook,
      section: previousSection ?? 0,
      state: "stopped",
      error: null,
      excerpt: false,
    } satisfies TtsSessionEvent);
  }
  tts.text = "";
  tts.words = [];
  tts.unit = "utf16";
  tts.cursor = 0;
  tts.end = null;
  return ttsStatus();
}

function annotationEntries(bookId: string, kind: AnnotationFilter): AnnotationEntry[] {
  const entries: AnnotationEntry[] = [];
  if (kind === "all" || kind === "marker") {
    for (const item of state.annotations) {
      if (item.bookId === bookId && !item.deleted) {
        entries.push({ type: "marker", data: clone(item) });
      }
    }
  }
  if (kind === "all" || kind === "note") {
    for (const item of state.notes) {
      if (item.bookId === bookId && !item.deleted) {
        entries.push({ type: "note", data: clone(item) });
      }
    }
  }
  if (kind === "all" || kind === "voice_note") {
    for (const item of state.voiceNotes) {
      if (item.bookId === bookId && !item.deleted) {
        entries.push({ type: "voice_note", data: clone(item) });
      }
    }
  }
  if (kind === "all" || kind === "extract") {
    for (const item of state.extracts) {
      if (item.bookId === bookId && !item.deleted) {
        entries.push({ type: "extract", data: clone(item) });
      }
    }
  }
  if (kind === "all" || kind === "reaction") {
    for (const item of state.reactions) {
      if (item.bookId === bookId && !item.deleted) {
        entries.push({ type: "reaction", data: clone(item) });
      }
    }
  }
  return entries;
}

function annotationEntryTimestamp(entry: AnnotationEntry): number {
  return new Date(entry.data.createdAt).getTime();
}

function reactionBuckets(bookId: string): ReactionBucket[] {
  const seed = findBookSeed(bookId);
  const sections = seed
    ? seed.format === "pdf"
      ? (seed.pdfPages?.length ?? 0)
      : chaptersFor(seed).length
    : 0;
  return Array.from({ length: sections }, (_, chapterIndex) => {
    const items = state.reactions.filter(
      (entry) => entry.bookId === bookId && entry.chapterIndex === chapterIndex,
    );
    return {
      chapterIndex,
      total: items.length,
      like: items.filter((entry) => entry.kind === "like").length,
      surprise: items.filter((entry) => entry.kind === "surprise").length,
      tension: items.filter((entry) => entry.kind === "tension").length,
      boredom: items.filter((entry) => entry.kind === "boredom").length,
      emotion: items.filter((entry) => entry.kind === "emotion").length,
    };
  });
}

// --- Audiolibros ------------------------------------------------------------

function bookFor(bookId: string): BookSummary | undefined {
  return state.books.find((book) => book.id === bookId);
}

function audiobookById(id: string): AudiobookInfo | undefined {
  return audiobooks.find((item) => item.id === id);
}

function onDemandAudiobookForBook(bookId: string): AudiobookInfo | undefined {
  return audiobooks.find(
    (item) => item.delivery === "on_demand" && item.bookId === bookId,
  );
}

function chapterTitlesFor(item: AudiobookInfo): string[] {
  const seed = findBookSeed(item.bookId);
  if (!seed) {
    return Array.from({ length: Math.max(1, item.chapterCount) }, (_, index) =>
      `Capítulo ${index + 1}`,
    );
  }
  if (seed.format === "pdf") {
    return (seed.pdfPages ?? []).map((_, index) => `Página ${index + 1}`);
  }
  return chaptersFor(seed).map((entry) => entry.title);
}

function chapterWeightsFor(item: AudiobookInfo): number[] {
  const seed = findBookSeed(item.bookId);
  if (!seed) return chapterTitlesFor(item).map(() => 1);
  if (seed.format === "pdf") {
    return (seed.pdfPages ?? []).map((lines) =>
      Math.max(1, lines.join(" ").length),
    );
  }
  return chaptersFor(seed).map((entry) =>
    Math.max(1, entry.paragraphs.join(" ").length),
  );
}

function buildAudiobookChapters(item: AudiobookInfo): AudiobookChapterInfo[] {
  if (item.status !== "done" || item.durationMs <= 0) return [];
  const override = chapterOverrides.get(item.id);
  if (override && override.length > 0) {
    const seed = findBookSeed(item.bookId);
    const seedChapters = seed ? chaptersFor(seed) : [];
    return override.map((chapter, index) => {
      const section = seedChapters.length
        ? Math.min(index, seedChapters.length - 1)
        : index;
      return {
        index,
        title: chapter.title,
        startMs: chapter.startMs,
        endMs: chapter.endMs,
        section,
        offsetStart: 0,
        offsetEnd: seedChapters.length
          ? chapterText(seed!, section).length
          : 0,
        karaoke: item.origin === "generated" && !item.audioOnly,
      };
    });
  }
  const titles = chapterTitlesFor(item);
  const weights = chapterWeightsFor(item);
  const count = Math.min(
    Math.max(1, item.chapterCount || titles.length),
    Math.max(1, titles.length),
  );
  const totalWeight = weights.slice(0, count).reduce((sum, weight) => sum + weight, 0) || 1;
  let elapsed = 0;
  return Array.from({ length: count }, (_, index) => {
    const startMs = Math.round((elapsed / totalWeight) * item.durationMs);
    elapsed += weights[index] ?? 1;
    const endMs =
      index === count - 1
        ? item.durationMs
        : Math.round((elapsed / totalWeight) * item.durationMs);
    const seed = findBookSeed(item.bookId);
    const sectionText = seed ? chapterText(seed, index) : "";
    return {
      index,
      title: titles[index] ?? `Capítulo ${index + 1}`,
      startMs,
      endMs: Math.max(startMs + 1000, endMs),
      section: index,
      offsetStart: 0,
      offsetEnd: sectionText.length,
      karaoke: item.origin === "generated" && !item.audioOnly,
    };
  });
}

function chapterAt(
  item: AudiobookInfo,
  positionMs: number,
): { chapter: AudiobookChapterInfo; index: number } {
  const chapters = buildAudiobookChapters(item);
  const clamped = Math.max(0, Math.min(positionMs, item.durationMs));
  const index = Math.max(
    0,
    chapters.findIndex((chapter) => clamped < chapter.endMs),
  );
  const safeIndex =
    index < 0 || chapters[index] === undefined ? Math.max(0, chapters.length - 1) : index;
  return {
    chapter:
      chapters[safeIndex] ?? {
        index: 0,
        title: item.bookTitle,
        startMs: 0,
        endMs: item.durationMs,
        section: 0,
        offsetStart: 0,
        offsetEnd: 0,
        karaoke: false,
      },
    index: safeIndex,
  };
}

function offsetForPosition(item: AudiobookInfo, positionMs: number): number {
  const seed = findBookSeed(item.bookId);
  if (!seed) return 0;
  const { chapter } = chapterAt(item, positionMs);
  const text = chapterText(seed, chapter.section);
  const fraction =
    chapter.endMs > chapter.startMs
      ? Math.max(
          0,
          Math.min(1, (positionMs - chapter.startMs) / (chapter.endMs - chapter.startMs)),
        )
      : 0;
  return Math.round(fraction * text.length);
}

function snapshotFor(
  item: AudiobookInfo,
  positionMs: number,
  playing: boolean,
): PlaybackSnapshot {
  const { chapter, index } = chapterAt(item, positionMs);
  const offsetStart = offsetForPosition(item, positionMs);
  return {
    audiobookId: item.id,
    title: item.bookTitle,
    positionMs,
    durationMs: item.durationMs,
    chapterIndex: index,
    chapterTitle: chapter.title,
    section: chapter.section,
    offsetStart,
    offsetEnd: offsetStart + 1,
    playing,
    coverPath: item.coverPath,
  };
}

function refreshAudiobookPlayback(): PlaybackSnapshot | null {
  if (!audiobookPlayback) return null;
  const item = audiobookById(audiobookPlayback.audiobookId);
  if (!item) {
    audiobookPlayback = null;
    return null;
  }
  if (audiobookPlayback.playing) {
    const elapsed = Math.max(0, Date.now() - audiobookPlaybackStartedAt);
    const position = Math.min(
      item.durationMs,
      audiobookPlaybackBase + elapsed * audiobookSpeed,
    );
    audiobookPlayback.positionMs = position;
    if (position >= item.durationMs) {
      audiobookPlayback.playing = false;
      audiobookPlaybackBase = position;
    }
  }
  const { chapter, index } = chapterAt(item, audiobookPlayback.positionMs);
  audiobookPlayback.chapterIndex = index;
  audiobookPlayback.chapterTitle = chapter.title;
  audiobookPlayback.section = chapter.section;
  audiobookPlayback.offsetStart = offsetForPosition(item, audiobookPlayback.positionMs);
  audiobookPlayback.offsetEnd = audiobookPlayback.offsetStart + 1;
  audiobookPlayback.durationMs = item.durationMs;
  audiobookPlayback.coverPath = item.coverPath;
  item.positionMs = audiobookPlayback.positionMs;
  item.percent =
    item.durationMs > 0 ? audiobookPlayback.positionMs / item.durationMs : 0;
  item.progressChapterIndex = index;
  item.progressChapterTitle = chapter.title;
  item.progressSection = chapter.section;
  item.progressOffset = audiobookPlayback.offsetStart;
  if (audiobookPlayback.playing) item.lastListenedAt = isoNow();
  return clone(audiobookPlayback);
}

function initialAudiobooks(): AudiobookInfo[] {
  const mareas = bookFor("book-mareas");
  const jardin = bookFor("book-jardin");
  const cartas = bookFor("book-cartas-faro");
  const ultima = bookFor("book-ultima-biblioteca");
  const poemas = bookFor("book-poemas");
  const mecanica = bookFor("book-mecanica");
  const ciudad = bookFor("book-ciudad-pdf");
  const dias = bookFor("book-dias-de-sal");
  if (
    !mareas ||
    !jardin ||
    !cartas ||
    !ultima ||
    !poemas ||
    !mecanica ||
    !ciudad ||
    !dias
  ) {
    return [];
  }
  const diasSeed = findBookSeed(dias.id);

  const items: AudiobookInfo[] = [
    {
      id: "audiobook-mareas",
      bookId: mareas.id,
      bookTitle: mareas.title,
      author: mareas.author,
      voiceId: "es_ES-davefx-medium",
      voiceName: "DaveFX",
      status: "done",
      delivery: "file",
      origin: "generated",
      durationMs: 7_500_000,
      audioHash: "demo-audio-mareas",
      audioExt: "m4b",
      scope: { kind: "fullBook" },
      narration: { style: "calm", speed: 1, multiVoice: false },
      cast: [],
      chapterSource: "sections",
      treatment: null,
      chapterCount: 5,
      watermark: false,
      coverPath: mareas.coverPath,
      createdAt: daysAgo(11),
      positionMs: 3_000_000,
      lastListenedAt: daysAgo(1),
      percent: 0.4,
      progressChapterIndex: 1,
      progressChapterTitle: "El cuarto de las mareas",
      progressSection: 1,
      progressOffset: 0,
      favorite: true,
      audioOnly: false,
      categoryIds: [...mareas.categoryIds],
    },
    {
      id: "audiobook-jardin",
      bookId: jardin.id,
      bookTitle: jardin.title,
      author: jardin.author,
      voiceId: "es_ES-sharvard-medium",
      voiceName: "Sharvard",
      status: "done",
      delivery: "file",
      origin: "generated",
      durationMs: 6_720_000,
      audioHash: "demo-audio-jardin",
      audioExt: "m4b",
      scope: { kind: "fullBook" },
      narration: { style: "calm", speed: 1, multiVoice: false },
      cast: [],
      chapterSource: "sections",
      treatment: { normalize: true, targetLufs: -16, eq: "clearVoice" },
      chapterCount: 3,
      watermark: false,
      coverPath: jardin.coverPath,
      createdAt: daysAgo(7),
      positionMs: 4_500_000,
      lastListenedAt: daysAgo(1),
      percent: 0.67,
      progressChapterIndex: 2,
      progressChapterTitle: "Un invierno sin prisa",
      progressSection: 2,
      progressOffset: 0,
      favorite: true,
      audioOnly: false,
      categoryIds: [...jardin.categoryIds],
    },
    {
      id: "audiobook-cartas",
      bookId: cartas.id,
      bookTitle: cartas.title,
      author: cartas.author,
      voiceId: null,
      voiceName: null,
      status: "done",
      delivery: "file",
      origin: "external",
      durationMs: 4_500_000,
      audioHash: "demo-audio-cartas",
      audioExt: "m4b",
      scope: { kind: "fullBook" },
      narration: null,
      cast: [],
      chapterSource: "embedded",
      treatment: null,
      chapterCount: 3,
      watermark: false,
      coverPath: cartas.coverPath,
      createdAt: daysAgo(23),
      positionMs: 810_000,
      lastListenedAt: daysAgo(9),
      percent: 0.18,
      progressChapterIndex: 0,
      progressChapterTitle: "Carta primera: la luz de octubre",
      progressSection: 0,
      progressOffset: 0,
      favorite: false,
      audioOnly: true,
      categoryIds: [...cartas.categoryIds],
    },
    {
      id: "audiobook-ultima",
      bookId: ultima.id,
      bookTitle: ultima.title,
      author: ultima.author,
      voiceId: "es_ES-davefx-medium",
      voiceName: "DaveFX",
      status: "done",
      delivery: "file",
      origin: "generated",
      durationMs: 9_180_000,
      audioHash: "demo-audio-ultima",
      audioExt: "m4b",
      scope: { kind: "fullBook" },
      narration: { style: "dramatic", speed: 1.05, multiVoice: true },
      cast: [
        {
          character: "Lía",
          voiceId: "es_ES-sharvard-medium",
          voiceName: "Sharvard",
          cloned: false,
        },
        {
          character: "El supervisor",
          voiceId: "es_ES-davefx-medium",
          voiceName: "DaveFX",
          cloned: false,
        },
      ],
      chapterSource: "ai",
      treatment: null,
      chapterCount: 3,
      watermark: false,
      coverPath: ultima.coverPath,
      createdAt: daysAgo(2),
      positionMs: 0,
      lastListenedAt: null,
      percent: 0,
      progressChapterIndex: 0,
      progressChapterTitle: "El archivo de los libros imposibles",
      progressSection: 0,
      progressOffset: 0,
      favorite: false,
      audioOnly: false,
      categoryIds: [...ultima.categoryIds],
    },
    {
      id: "audiobook-poemas",
      bookId: poemas.id,
      bookTitle: "Poemas de invierno · selección",
      author: poemas.author,
      voiceId: "es_ES-sharvard-medium",
      voiceName: "Sharvard",
      status: "done",
      delivery: "file",
      origin: "tejido",
      durationMs: 1_680_000,
      audioHash: "demo-audio-poemas",
      audioExt: "m4b",
      scope: { kind: "fullBook" },
      narration: null,
      cast: [],
      chapterSource: "manual",
      treatment: { normalize: true, targetLufs: -18, eq: "warm" },
      chapterCount: 3,
      watermark: false,
      coverPath: poemas.coverPath,
      createdAt: daysAgo(16),
      positionMs: 1_680_000,
      lastListenedAt: daysAgo(5),
      percent: 1,
      progressChapterIndex: 2,
      progressChapterTitle: "Deshielo",
      progressSection: 2,
      progressOffset: 0,
      favorite: true,
      audioOnly: false,
      categoryIds: [...poemas.categoryIds],
    },
    {
      id: "audiobook-mecanica",
      bookId: mecanica.id,
      bookTitle: mecanica.title,
      author: mecanica.author,
      voiceId: "es_ES-davefx-medium",
      voiceName: "DaveFX",
      status: "failed",
      delivery: "file",
      origin: "generated",
      durationMs: 0,
      audioHash: null,
      audioExt: null,
      scope: { kind: "fullBook" },
      narration: { style: "neutral", speed: 1, multiVoice: false },
      cast: [],
      chapterSource: "sections",
      treatment: null,
      chapterCount: 0,
      watermark: false,
      coverPath: mecanica.coverPath,
      createdAt: daysAgo(3),
      positionMs: 0,
      lastListenedAt: null,
      percent: 0,
      progressChapterIndex: 0,
      progressChapterTitle: null,
      progressSection: 0,
      progressOffset: 0,
      favorite: false,
      audioOnly: false,
      categoryIds: [...mecanica.categoryIds],
    },
    {
      id: "audiobook-ciudad",
      bookId: ciudad.id,
      bookTitle: ciudad.title,
      author: ciudad.author,
      voiceId: null,
      voiceName: null,
      status: "done",
      delivery: "file",
      origin: "external",
      durationMs: 5_460_000,
      audioHash: "demo-audio-ciudad",
      audioExt: "mp3",
      scope: { kind: "fullBook" },
      narration: null,
      cast: [],
      chapterSource: "embedded",
      treatment: null,
      chapterCount: 4,
      watermark: false,
      coverPath: ciudad.coverPath,
      createdAt: daysAgo(31),
      positionMs: 0,
      lastListenedAt: null,
      percent: 0,
      progressChapterIndex: 0,
      progressChapterTitle: "La ciudad y sus fantasmas",
      progressSection: 0,
      progressOffset: 0,
      favorite: false,
      audioOnly: true,
      categoryIds: [...ciudad.categoryIds],
    },
    {
      id: "audiobook-dias-sal",
      bookId: dias.id,
      bookTitle: dias.title,
      author: dias.author,
      voiceId: null,
      voiceName: null,
      status: "done",
      delivery: "on_demand",
      origin: "generated",
      durationMs: 0,
      audioHash: null,
      audioExt: null,
      scope: { kind: "fullBook" },
      narration: null,
      cast: [],
      chapterSource: null,
      treatment: null,
      chapterCount: diasSeed ? chaptersFor(diasSeed).length : 0,
      watermark: false,
      coverPath: dias.coverPath,
      createdAt: daysAgo(4),
      positionMs: 0,
      lastListenedAt: null,
      percent: 0,
      progressChapterIndex: 0,
      progressChapterTitle: null,
      progressSection: 0,
      progressOffset: 0,
      favorite: false,
      audioOnly: false,
      categoryIds: [...dias.categoryIds],
    },
  ];
  return items;
}

function initialPlayback(items: AudiobookInfo[]): PlaybackSnapshot | null {
  const item = items.find((entry) => entry.id === "audiobook-jardin");
  if (!item) return null;
  return snapshotFor(item, item.positionMs, false);
}

function wavBase64(durationMs = 700, frequency = 440): string {
  const sampleRate = 16_000;
  const samples = Math.max(1, Math.round((durationMs / 1000) * sampleRate));
  const bytes = new Uint8Array(44 + samples * 2);
  const view = new DataView(bytes.buffer);
  const writeAscii = (offset: number, text: string) => {
    for (let index = 0; index < text.length; index += 1) {
      view.setUint8(offset + index, text.charCodeAt(index));
    }
  };
  writeAscii(0, "RIFF");
  view.setUint32(4, 36 + samples * 2, true);
  writeAscii(8, "WAVE");
  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeAscii(36, "data");
  view.setUint32(40, samples * 2, true);
  for (let index = 0; index < samples; index += 1) {
    const t = index / sampleRate;
    const envelope = Math.min(1, t * 8, (durationMs / 1000 - t) * 8);
    const sample =
      Math.sin(2 * Math.PI * frequency * t) * 0.18 * Math.max(0, envelope);
    view.setInt16(44 + index * 2, Math.round(sample * 32767), true);
  }
  return bytesToBase64(bytes);
}

function shareQr(seed: number): string {
  const size = 21;
  let stateValue = seed;
  const cells: string[] = [];
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      stateValue = (stateValue * 1103515245 + 12345) & 0x7fffffff;
      const on = (stateValue >> 8) % 3 === 0;
      if (on) cells.push(`<rect x="${x}" y="${y}" width="1" height="1"/>`);
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges"><rect width="${size}" height="${size}" fill="#fff"/><g fill="#111">${cells.join("")}</g></svg>`;
}

function generateAudiobookFromQueue(
  item: AudiobookQueueItem,
): AudiobookInfo {
  const book = bookFor(item.bookId);
  const seed = findBookSeed(item.bookId);
  const chapters = seed
    ? seed.format === "pdf"
      ? (seed.pdfPages?.length ?? 1)
      : chaptersFor(seed).length
    : 3;
  const duration = Math.max(1_800_000, chapters * 1_320_000);
  const pending = pendingAudiobookFromQueue(item);
  const firstTitle = chapterTitlesFor(pending)[0] ?? null;
  return {
    id: item.id,
    bookId: item.bookId,
    bookTitle: item.bookTitle,
    author: item.author,
    voiceId: item.voiceId,
    voiceName: item.voiceName,
    status: "done",
    delivery: "file",
    origin: item.origin,
    durationMs: duration,
    audioHash: `demo-audio-${item.id}`,
    audioExt: "m4b",
    scope: item.scope,
    narration: item.narration,
    cast: item.cast.enabled
      ? item.cast.assignments.map((assignment) => ({
          character: assignment.character,
          voiceId: assignment.voiceId,
          voiceName:
            demoVoices.find((voice) => voice.id === assignment.voiceId)?.name ??
            "Voz Kora",
          cloned: false,
        }))
      : [],
    chapterSource: "ai",
    treatment: null,
    chapterCount: chapters,
    watermark: false,
    coverPath: book?.coverPath ?? null,
    createdAt: isoNow(),
    positionMs: 0,
    lastListenedAt: null,
    percent: 0,
    progressChapterIndex: 0,
    progressChapterTitle: firstTitle,
    progressSection: 0,
    progressOffset: 0,
    favorite: false,
    audioOnly: false,
    categoryIds: book?.categoryIds ?? [],
  };
}

function emitGenerationProgress(
  phase: string,
  percent: number,
  message: string,
  chapter: number,
  totalChapters: number,
  audiobookId: string | null,
) {
  emit("audiobook::progress", {
    audiobookId,
    phase,
    chapter,
    totalChapters,
    percent,
    message,
  });
}

function scheduleGeneration() {
  if (generationTimer) {
    clearTimeout(generationTimer);
    generationTimer = undefined;
  }
  if (!generation.active || generation.active.paused) return;
  generationTimer = setTimeout(() => {
    if (!generation.active || generation.active.paused) return;
    const active = generation.active;
    generationPercent = Math.min(100, generationPercent + 12);
    const totalChapters = 5;
    emitGenerationProgress(
      "synthesizing",
      generationPercent,
      "Sintetizando voz y capítulos…",
      Math.min(totalChapters, Math.max(1, Math.round((generationPercent / 100) * totalChapters))),
      totalChapters,
      active.audiobookId,
    );
    if (generationPercent >= 100) {
      const item = generation.queue.shift() ?? null;
      // El elemento activo se crea al encolarse; la cola solo contiene los
      // siguientes trabajos. Se resuelve el activo desde sus datos guardados.
      const activeData = queuedActiveData;
      if (activeData) {
        const created = generateAudiobookFromQueue(activeData);
        audiobooks = [created, ...audiobooks.filter((entry) => entry.id !== created.id)];
        active.audiobookId = created.id;
        emitGenerationProgress(
          "done",
          100,
          "Audiolibro listo.",
          created.chapterCount,
          created.chapterCount,
          created.id,
        );
      }
      queuedActiveData = null;
      generationPercent = 0;
      if (item) {
        generation.active = {
          title: item.bookTitle,
          voiceName: item.voiceName,
          audiobookId: item.id,
          paused: false,
          startedAt: isoNow(),
        };
        queuedActiveData = item;
        emit("audiobook::queue", clone(generation));
        emitGenerationProgress(
          "synthesizing",
          0,
          "Preparando la síntesis…",
          0,
          Math.max(1, 1),
          item.id,
        );
        scheduleGeneration();
      } else {
        generation.active = null;
        emit("audiobook::queue", clone(generation));
      }
    } else {
      scheduleGeneration();
    }
  }, 850);
}

let queuedActiveData: AudiobookQueueItem | null = null;

function queueView(): AudiobookGenerationView {
  return clone(generation);
}

function queueItemFromArgs(args: Args, origin: AudiobookInfo["origin"]): AudiobookQueueItem {
  const book = bookFor(String(args.bookId ?? ""));
  const voice = demoVoices.find((item) => item.id === args.voiceId);
  return {
    id: `audiobook-${Date.now()}`,
    bookId: String(args.bookId ?? ""),
    bookTitle: book?.title ?? "Audiolibro",
    author: book?.author ?? null,
    coverPath: book?.coverPath ?? null,
    scope: (args.scope as AudiobookScope) ?? { kind: "fullBook" },
    voiceId: String(args.voiceId ?? "es_ES-davefx-medium"),
    voiceName: voice?.name ?? "DaveFX",
    narration: (args.narration as NarrationSettings | null) ?? null,
    cast: (args.cast as AudiobookQueueItem["cast"]) ?? {
      enabled: false,
      assignments: [],
    },
    origin,
    position: generation.queue.length,
    createdAt: isoNow(),
  };
}

function pendingAudiobookFromQueue(item: AudiobookQueueItem): AudiobookInfo {
  const book = bookFor(item.bookId);
  return {
    id: item.id,
    bookId: item.bookId,
    bookTitle: item.bookTitle,
    author: item.author,
    voiceId: item.voiceId,
    voiceName: item.voiceName,
    status: "pending",
    delivery: "file",
    origin: item.origin,
    durationMs: 0,
    audioHash: null,
    audioExt: null,
    scope: item.scope,
    narration: item.narration,
    cast: [],
    chapterSource: null,
    treatment: null,
    chapterCount: 0,
    watermark: false,
    coverPath: item.coverPath ?? book?.coverPath ?? null,
    createdAt: item.createdAt,
    positionMs: 0,
    lastListenedAt: null,
    percent: 0,
    progressChapterIndex: 0,
    progressChapterTitle: null,
    progressSection: 0,
    progressOffset: 0,
    favorite: false,
    audioOnly: false,
    categoryIds: book?.categoryIds ?? [],
  };
}

function enqueueJob(item: AudiobookQueueItem): AudiobookGenerationView {
  if (!audiobookById(item.id)) {
    audiobooks = [pendingAudiobookFromQueue(item), ...audiobooks];
  }
  if (!generation.active) {
    generation.active = {
      title: item.bookTitle,
      voiceName: item.voiceName,
      audiobookId: item.id,
      paused: false,
      startedAt: isoNow(),
    };
    queuedActiveData = item;
    generationPercent = 0;
    emitGenerationProgress(
      "synthesizing",
      4,
      "Preparando la síntesis…",
      1,
      5,
      item.id,
    );
    scheduleGeneration();
  } else {
    generation.queue = [...generation.queue, item];
  }
  emit("audiobook::queue", clone(generation));
  return queueView();
}

function playAudiobookSnapshot(id: string): PlaybackSnapshot {
  const item = audiobookById(id);
  if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
  if (item.status !== "done") {
    throw { code: "not_ready", message: "El audiolibro todavía no está listo." };
  }
  const atEnd = item.durationMs > 0 && item.positionMs >= item.durationMs - 1000;
  audiobookPlaybackBase = atEnd ? 0 : item.positionMs;
  audiobookPlaybackStartedAt = Date.now();
  audiobookPlayback = snapshotFor(item, audiobookPlaybackBase, true);
  item.lastListenedAt = isoNow();
  item.positionMs = audiobookPlaybackBase;
  item.percent = item.durationMs > 0 ? item.positionMs / item.durationMs : 0;
  return clone(audiobookPlayback);
}

function pauseAudiobookSnapshot(): PlaybackSnapshot | null {
  if (!audiobookPlayback) return null;
  refreshAudiobookPlayback();
  audiobookPlayback.playing = false;
  audiobookPlaybackBase = audiobookPlayback.positionMs;
  return clone(audiobookPlayback);
}

function seekAudiobookSnapshot(positionMs: number): PlaybackSnapshot | null {
  if (!audiobookPlayback) return null;
  const item = audiobookById(audiobookPlayback.audiobookId);
  if (!item) return null;
  const position = Math.max(0, Math.min(item.durationMs, positionMs));
  audiobookPlayback = snapshotFor(item, position, audiobookPlayback.playing);
  audiobookPlaybackBase = position;
  audiobookPlaybackStartedAt = Date.now();
  item.positionMs = position;
  item.percent = item.durationMs > 0 ? position / item.durationMs : 0;
  return clone(audiobookPlayback);
}

function setAudiobookSpeedSnapshot(nextSpeed: number): PlaybackSnapshot | null {
  if (!audiobookPlayback) return null;
  refreshAudiobookPlayback();
  audiobookPlaybackBase = audiobookPlayback.positionMs;
  audiobookPlaybackStartedAt = Date.now();
  audiobookSpeed = Math.max(0.5, Math.min(3, nextSpeed));
  return clone(audiobookPlayback);
}

function karaokeFor(id: string, chapterIndex: number): ChapterKaraoke {
  const item = audiobookById(id);
  if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
  if (item.audioOnly) {
    throw { code: "no_text", message: "Este audiolibro no tiene texto sincronizado." };
  }
  const seed = findBookSeed(item.bookId);
  const chapters = buildAudiobookChapters(item);
  const chapter = chapters[chapterIndex] ?? chapters[0];
  if (!seed || !chapter) {
    throw { code: "not_found", message: "Capítulo no encontrado" };
  }
  const text = chapterText(seed, chapter.section);
  const rawLines = text
    .split(/\n+/)
    .flatMap((paragraph) =>
      paragraph
        .split(/(?<=[.!?…])\s+/)
        .map((line) => line.trim())
        .filter(Boolean),
    );
  const lines: KaraokeLine[] = [];
  const totalChars = rawLines.reduce((sum, line) => sum + line.length, 0) || 1;
  const span = Math.max(1, chapter.endMs - chapter.startMs);
  let cursor = 0;
  for (const line of rawLines) {
    if (cursor >= span) break;
    const duration = Math.max(
      1200,
      Math.round((line.length / totalChars) * span),
    );
    const end = Math.min(span, Math.max(cursor + 1, cursor + duration));
    lines.push({
      startMs: cursor,
      endMs: end,
      text: line,
    });
    cursor = end;
  }
  if (lines.length > 0) lines[lines.length - 1].endMs = span;
  return {
    chapterIndex,
    title: chapter.title,
    startMs: chapter.startMs,
    endMs: chapter.endMs,
    precision: item.chapterSource === "ai" ? "aligned" : "timed",
    lines,
  };
}

function waveformFor(id: string, buckets: number): AudioWaveform {
  const item = audiobookById(id);
  if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
  const count = Math.max(24, Math.min(1200, Math.round(buckets)));
  let seed = 0;
  for (const char of id) seed = (seed * 31 + char.charCodeAt(0)) & 0x7fffffff;
  const values = Array.from({ length: count }, (_, index) => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    const noise = (seed >> 9) / 0x7fffffff;
    const shape =
      0.38 +
      0.28 * Math.sin(index / 7.5) +
      0.18 * Math.sin(index / 2.7) +
      0.12 * noise;
    return Math.max(0.04, Math.min(1, shape));
  });
  return { buckets: values, durationMs: item.durationMs };
}

function voiceMarkerStatus(): VoiceMarkerStatus {
  return {
    supported: true,
    listening: voiceMarkerListening,
    modelId: "base",
    modelReady: true,
    phrase: voiceMarkerPhrase,
  };
}

function importAudiobookFile(path: string, transcribe: boolean): AudiobookInfo {
  const base = path.split(/[\\/]/).pop() ?? "Audiolibro";
  const title = base.replace(/\.[^.]+$/, "").replace(/[-_]+/g, " ");
  const extension = base.split(".").pop()?.toLowerCase() ?? "m4b";
  const id = `book-audio-${Date.now()}`;
  const seed: DemoBookSeed = {
    id,
    title,
    author: "Autor desconocido",
    format: "epub",
    description: "Libro creado al importar un audiolibro en el modo demostración.",
    publisher: "Importación de audio",
    language: "es",
    sizeBytes: 1_400_000,
    addedAt: isoNow(),
    lastReadAt: null,
    progress: 0,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: [],
    palette: ["#263b4a", "#6f8f9c", "#f4ead8"],
    motif: "waves",
    chapters: transcribe
      ? [
          {
            title: "Primera parte",
            paragraphs: [
              `La transcripción de «${title}» comienza con una voz que aún no sabe si está recordando o imaginando.`,
              "Las primeras frases avanzan despacio, como quien prueba el terreno antes de confiar en él.",
              "Hay nombres propios que se repiten y una pregunta que queda flotando: ¿de quién es la historia que estamos escuchando?",
              "El audio original conserva ruidos de fondo, pequeñas respiraciones y una pausa larga antes de la primera revelación.",
            ],
          },
          {
            title: "Segunda parte",
            paragraphs: [
              "A mitad de la grabación, el relato cambia de tono. Lo que parecía una conversación se convierte en una confesión.",
              "La voz describe un lugar concreto: una habitación, una ventana, el ruido de un tren que pasa cada cierto tiempo.",
              "Los capítulos detectados por Whisper no siempre coinciden con las pausas del audio; este es uno de esos casos.",
            ],
          },
          {
            title: "Tercera parte",
            paragraphs: [
              "El final llega sin estridencias. La voz se detiene, respira y deja que el silencio forme parte de la grabación.",
              "La transcripción termina con una frase que podría ser el comienzo de otra historia.",
            ],
          },
        ]
      : undefined,
  };
  registerBookSeed(seed);
  const book: BookSummary = {
    id,
    title,
    author: seed.author,
    format: "epub",
    coverPath: coverDataUri(seed),
    sizeBytes: seed.sizeBytes,
    addedAt: seed.addedAt,
    language: "es",
    publisher: "Importación de audio",
    description: seed.description,
    lastReadAt: null,
    progress: 0,
    origin: transcribe ? "transcribed" : "imported",
    favorite: false,
    readOnly: false,
    receivedShareId: null,
    categoryIds: [],
  };
  state.books.push(book);
  state.settings.set(id, {
    ...defaultSettings(id),
    mode: "paged",
  });
  state.progress.set(id, { section: 0, offset: 0, percent: 0 });
  const item: AudiobookInfo = {
    id: `audiobook-${Date.now()}`,
    bookId: book.id,
    bookTitle: title,
    author: null,
    voiceId: null,
    voiceName: null,
    status: "done",
    delivery: "file",
    origin: "external",
    durationMs: 4_860_000,
    audioHash: `demo-audio-${Date.now()}`,
    audioExt: extension,
    scope: { kind: "fullBook" },
    narration: null,
    cast: [],
    chapterSource: transcribe ? "ai" : "embedded",
    treatment: null,
    chapterCount: 3,
    watermark: false,
    coverPath: book.coverPath,
    createdAt: isoNow(),
    positionMs: 0,
    lastListenedAt: null,
    percent: 0,
    progressChapterIndex: 0,
    progressChapterTitle: transcribe ? "Primera parte" : null,
    progressSection: 0,
    progressOffset: 0,
    favorite: false,
    audioOnly: !transcribe,
    categoryIds: [],
  };
  audiobooks.unshift(item);
  return clone(item);
}



function addMarker(args: Args): Annotation {
  const entry: Annotation = {
    id: `marker-${Date.now()}`,
    bookId: args.bookId,
    kind: args.kind ?? "highlight",
    locator: JSON.stringify({
      section: args.chapterIndex,
      offset: args.offsetStart,
    }),
    chapterIndex: args.chapterIndex ?? null,
    offsetStart: args.offsetStart ?? null,
    offsetEnd: args.offsetEnd ?? null,
    text: args.text ?? null,
    note: args.note ?? null,
    color: args.color ?? "#ffd166",
    createdAt: isoNow(),
    updatedAt: isoNow(),
    deleted: false,
    ownerAccountId: OWNER,
    originShareId: null,
  };
  state.annotations.push(entry);
  return clone(entry);
}

function addNote(args: Args): Note {
  const entry: Note = {
    id: `note-${Date.now()}`,
    bookId: args.bookId ?? null,
    title: args.title ?? null,
    body: args.plainText ?? "",
    contentJson: args.contentJson ?? null,
    chapterIndex: args.chapterIndex ?? null,
    offsetStart: args.offsetStart ?? null,
    offsetEnd: args.offsetEnd ?? null,
    text: args.text ?? null,
    audioHash: null,
    audioExt: null,
    createdAt: isoNow(),
    updatedAt: isoNow(),
    deleted: false,
    ownerAccountId: OWNER,
    originShareId: null,
  };
  state.notes.push(entry);
  return clone(entry);
}

function addExtract(args: Args): Extract {
  const entry: Extract = {
    id: `extract-${Date.now()}`,
    bookId: args.bookId,
    locator: JSON.stringify({
      section: args.chapterIndex,
      offset: args.offsetStart,
    }),
    text: args.text ?? "",
    note: args.note ?? null,
    chapterIndex: args.chapterIndex ?? null,
    offsetStart: args.offsetStart ?? null,
    offsetEnd: args.offsetEnd ?? null,
    citation: args.citation ?? null,
    createdAt: isoNow(),
    updatedAt: isoNow(),
    deleted: false,
    ownerAccountId: OWNER,
    originShareId: null,
  };
  state.extracts.push(entry);
  return clone(entry);
}

function addReaction(args: Args): Reaction {
  const entry: Reaction = {
    id: `reaction-${Date.now()}`,
    bookId: args.bookId,
    chapterIndex: args.chapterIndex ?? 0,
    offsetStart: args.offsetStart ?? 0,
    offsetEnd: args.offsetEnd ?? 0,
    kind: args.kind ?? "like",
    text: args.text ?? null,
    createdAt: isoNow(),
    updatedAt: isoNow(),
    deleted: false,
    ownerAccountId: OWNER,
    originShareId: null,
  };
  state.reactions.push(entry);
  return clone(entry);
}

function addVoiceNote(args: Args): VoiceNote {
  const entry: VoiceNote = {
    id: `voice-note-${Date.now()}`,
    bookId: args.bookId,
    chapterIndex: args.chapterIndex ?? 0,
    offsetStart: args.offsetStart ?? 0,
    offsetEnd: args.offsetEnd ?? 0,
    text: args.text ?? "Nota de voz",
    durationMs: args.durationMs ?? 18_400,
    audiobookId: null,
    positionMs: 0,
    quote: args.quote ?? null,
    transcript: args.transcript ?? "Nota de voz de demostración.",
    source: "reader",
    createdAt: isoNow(),
    updatedAt: isoNow(),
    deleted: false,
    ownerAccountId: OWNER,
    originShareId: null,
    audioPath: null,
  };
  state.voiceNotes.push(entry);
  return clone(entry);
}

function continueBook(): LibraryOverview {
  const pending = state.books
    .filter((book) => book.progress > 0.001 && book.progress < 0.995)
    .sort((left, right) => {
      const leftTime = left.lastReadAt
        ? new Date(left.lastReadAt).getTime()
        : new Date(left.addedAt).getTime();
      const rightTime = right.lastReadAt
        ? new Date(right.lastReadAt).getTime()
        : new Date(right.addedAt).getTime();
      return rightTime - leftTime;
    });
  const book = pending[0];
  return {
    current: book
      ? {
          id: book.id,
          title: book.title,
          format: book.format,
          coverPath: book.coverPath,
          progress: book.progress,
          addedAt: book.addedAt,
          lastReadAt: book.lastReadAt,
        }
      : null,
    inProgress: pending.length,
  };
}

function semanticStatus() {
  return {
    supported: true,
    available: true,
    model: "multilingual-e5-small",
    dimensions: 384,
    indexedBooks: state.books.length,
    totalBooks: state.books.length,
    chunks: state.books.length * 36,
    indexing: false,
    lastError: null,
  };
}

function normalize(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase();
}

function searchSemantic(query: string, limit: number): SemanticHit[] {
  const needle = normalize(query.trim());
  if (needle.length < 2) return [];
  const hits: SemanticHit[] = [];
  for (const seed of demoBookSeeds) {
    if (seed.format === "pdf") {
      for (let page = 0; page < (seed.pdfPages?.length ?? 0); page += 1) {
        const lines = seed.pdfPages?.[page] ?? [];
        const text = lines.join(" ");
        const index = normalize(text).indexOf(needle);
        if (index < 0) continue;
        const start = Math.max(0, index - 90);
        hits.push({
          bookId: seed.id,
          bookTitle: seed.title,
          author: seed.author,
          section: page,
          sectionTitle: `Página ${page + 1}`,
          offset: 0,
          snippet: text.slice(start, start + 220),
          score: 0.82 - page * 0.01,
        });
        break;
      }
      continue;
    }
    const chapters = chaptersFor(seed);
    for (let section = 0; section < chapters.length; section += 1) {
      const entry = chapters[section];
      const text = `${entry.title}\n${entry.paragraphs.join("\n")}`;
      const index = normalize(text).indexOf(needle);
      if (index < 0) continue;
      const start = Math.max(0, index - 90);
      hits.push({
        bookId: seed.id,
        bookTitle: seed.title,
        author: seed.author,
        section,
        sectionTitle: entry.title,
        offset: 0,
        snippet: text.slice(start, start + 220).replace(/\n+/g, " "),
        score: Math.max(0.55, 0.93 - section * 0.03),
      });
      break;
    }
  }
  return hits
    .sort((left, right) => right.score - left.score)
    .slice(0, Math.max(1, limit));
}

function findChapterSeed(bookId: string): DemoBookSeed | undefined {
  return findBookSeed(bookId);
}

function aiReply(messages: AiMessage[]): AiReply {
  const last = [...messages].reverse().find((message) => message.role === "user");
  const prompt = last?.content ?? "";
  const context = messages.map((message) => message.content).join("\n");
  const bookTitle = context.match(/Libro:\s*(.+)/)?.[1]?.trim() ?? "";
  const chapterTitle = context.match(/Capítulo:\s*(.+)/)?.[1]?.trim() ?? "";
  const seed = demoBookSeeds.find((book) => book.title === bookTitle);
  const notes = (seed && bookAiNotes[seed.id]) || genericAiNotes;
  let content = "";

  if (prompt.includes("Consulta lexicográfica")) {
    const term = prompt.match(/«([^»]+)»/)?.[1] ?? "la palabra";
    content = [
      `**Definición contextual.** En este pasaje, «${term}» se usa para nombrar una experiencia que el texto prefiere no cerrar del todo: algo entre la percepción y el recuerdo.`,
      `**Etimología.** Del latín *focus*, «hogar» o «lugar donde se concentra el calor»; con el tiempo pasó a designar el punto donde converge la atención.`,
      "**Sinónimos.** Centro, núcleo, señal, referencia, llamada.",
      "**Uso en este pasaje.** El término no funciona como una descripción neutra: marca el lugar donde el personaje decide detenerse y mirar de nuevo.",
    ].join("\n\n");
  } else if (prompt.includes("Resume la escena")) {
    content = notes.summary;
  } else if (prompt.includes("Extrae entre 4 y 6 términos")) {
    content = notes.terms;
  } else if (prompt.includes("Enumera los personajes")) {
    content = notes.characters;
  } else if (/final|termina|acaba|después/i.test(prompt)) {
    content =
      "No puedo adelantar acontecimientos que quedan fuera del fragmento que tengo delante. Puedo ayudarte a leer lo que ya está en este capítulo, o a formular una hipótesis sin darla por confirmada.";
  } else if (/quién|personaje/i.test(prompt)) {
    content = notes.characters;
  } else if (/simbol|símbolo|significa|tema|interpret/i.test(prompt)) {
    content = notes.terms;
  } else {
    const context = chapterTitle
      ? `Estamos en «${chapterTitle}»${bookTitle ? `, de «${bookTitle}»` : ""}. `
      : "";
    content = `${context}${notes.summary} Si quieres, puedo detenerme en un personaje, en un símbolo o en la relación entre este pasaje y el resto del capítulo.`;
  }

  return {
    content,
    provider: "ollama",
    model: "llama3.2:3b",
    elapsedMs: 380,
  };
}

function practiceStop(modelId: string): PracticeResult {
  const active = state.practice;
  if (!active) {
    throw { code: "practice", message: "No hay ninguna grabación en curso." };
  }
  state.practice = null;
  const expectedWords = active.expected.trim().split(/\s+/).filter(Boolean);
  const words = expectedWords.slice(0, 80);
  const transcriptWords = words.map((word, index) =>
    index === 4 ? "silencio" : index === 13 ? "regresar" : word,
  );
  if (transcriptWords.length > 7) transcriptWords.splice(7, 1);
  const diff = words.map((word, index) => {
    const actual = transcriptWords[index] ?? null;
    if (actual === null) {
      return {
        expected: word,
        actual: null,
        kind: "missing" as const,
        expectedIndex: index,
        actualIndex: null,
      };
    }
    if (actual === word) {
      return {
        expected: word,
        actual,
        kind: "match" as const,
        expectedIndex: index,
        actualIndex: index,
      };
    }
    return {
      expected: word,
      actual,
      kind: "substitution" as const,
      expectedIndex: index,
      actualIndex: index,
    };
  });
  const matches = diff.filter((word) => word.kind === "match").length;
  const durationMs = Math.max(8_000, Date.now() - active.startedAt);
  const result: PracticeResult = {
    attemptId: `practice-${Date.now()}`,
    transcript: transcriptWords.join(" "),
    accuracy: words.length ? matches / words.length : 0,
    diff,
    durationMs,
    sampleRate: 16_000,
    model: modelId,
    recordingPath: "/demo/practice/lectura.wav",
  };
  state.practiceHistory.unshift({
    id: result.attemptId,
    section: active.section,
    offsetStart: active.offsetStart,
    offsetEnd: active.offsetEnd,
    expected: active.expected,
    transcript: result.transcript,
    accuracy: result.accuracy,
    diff: result.diff,
    createdAt: isoNow(),
  });
  return clone(result);
}

function diaryMonth(year: number, month: number): DiaryMonth {
  const prefix = `${year}-${String(month).padStart(2, "0")}-`;
  const entries = state.diary
    .filter((entry) => entry.entryDate.startsWith(prefix))
    .sort(
      (left, right) =>
        left.entryDate.localeCompare(right.entryDate) ||
        left.createdAt.localeCompare(right.createdAt),
    );
  const daysInMonth = new Date(year, month, 0).getDate();
  const days: DiaryDay[] = Array.from({ length: daysInMonth }, (_, index) => ({
    date: `${prefix}${String(index + 1).padStart(2, "0")}`,
    minutes: 0,
    pages: 0,
    extracts: 0,
    notes: 0,
    entries: 0,
    level: 0,
  }));
  const totals: DiaryTotals = {
    minutes: 0,
    pages: 0,
    extracts: 0,
    notes: 0,
    entries: entries.length,
    activeDays: 0,
  };

  for (const entry of entries) {
    totals.minutes += Math.max(0, entry.durationMinutes);
    totals.pages += Math.max(0, entry.pagesRead);
    totals.extracts += Math.max(0, entry.extractsCreated);
    totals.notes += Math.max(0, entry.notesCreated);
    const day = days.find((item) => item.date === entry.entryDate);
    if (!day) continue;
    day.minutes += Math.max(0, entry.durationMinutes);
    day.pages += Math.max(0, entry.pagesRead);
    day.extracts += Math.max(0, entry.extractsCreated);
    day.notes += Math.max(0, entry.notesCreated);
    day.entries += 1;
  }

  for (const day of days) {
    day.level = heatLevel(day.minutes);
    if (day.entries > 0) totals.activeDays += 1;
  }

  return { year, month, days, entries, totals };
}

function heatLevel(minutes: number): number {
  if (minutes <= 0) return 0;
  if (minutes <= 29) return 1;
  if (minutes <= 59) return 2;
  if (minutes <= 119) return 3;
  return 4;
}

interface DemoAchievementDefinition {
  key: string;
  title: string;
  description: string;
  target: number;
}

// Mismo catálogo que `features/achievements.rs`, para que la vista de Diario
// muestre los logros reales durante la demostración.
const demoAchievementDefinitions: DemoAchievementDefinition[] = [
  { key: "first_book", title: "Primer libro", description: "Añade tu primer libro a la biblioteca.", target: 1 },
  { key: "library_10", title: "Estantería en marcha", description: "Reúne 10 libros en tu biblioteca.", target: 10 },
  { key: "library_50", title: "Bibliotecario", description: "Reúne 50 libros en tu biblioteca.", target: 50 },
  { key: "first_extract", title: "Primera chispa", description: "Guarda tu primer extracto.", target: 1 },
  { key: "extracts_50", title: "Coleccionista", description: "Guarda 50 extractos.", target: 50 },
  { key: "extracts_200", title: "Archivo personal", description: "Guarda 200 extractos.", target: 200 },
  { key: "first_note", title: "Nota al margen", description: "Escribe tu primera nota.", target: 1 },
  { key: "notes_50", title: "Cuaderno vivo", description: "Escribe 50 notas.", target: 50 },
  { key: "first_diary", title: "Primer día", description: "Registra tu primera sesión en el diario.", target: 1 },
  { key: "diary_30", title: "Constancia", description: "Registra 30 sesiones en el diario.", target: 30 },
  { key: "minutes_60", title: "Una hora", description: "Acumula 60 minutos de lectura o escucha.", target: 60 },
  { key: "minutes_600", title: "Diez horas", description: "Acumula 600 minutos de lectura o escucha.", target: 600 },
  { key: "minutes_3000", title: "Cincuenta horas", description: "Acumula 3000 minutos de lectura o escucha.", target: 3000 },
  { key: "pages_100", title: "Cien páginas", description: "Lee 100 páginas.", target: 100 },
  { key: "pages_1000", title: "Mil páginas", description: "Lee 1000 páginas.", target: 1000 },
  { key: "first_pomodoro", title: "Enfoque", description: "Completa tu primer Pomodoro.", target: 1 },
  { key: "pomodoros_25", title: "Máquina de foco", description: "Completa 25 Pomodoros.", target: 25 },
  { key: "streak_7", title: "Siete días", description: "Lee o escucha 7 días consecutivos.", target: 7 },
  { key: "streak_30", title: "Un mes de hábito", description: "Lee o escucha 30 días consecutivos.", target: 30 },
  { key: "first_audiobook", title: "Voz propia", description: "Genera tu primer audiolibro.", target: 1 },
  { key: "first_share", title: "Compartir es vivir", description: "Crea tu primer recurso compartido.", target: 1 },
  { key: "first_voice_note", title: "Voz al margen", description: "Graba tu primera nota de voz.", target: 1 },
  { key: "night_reader", title: "Lector Nocturno", description: "Lee o escucha en cinco sesiones nocturnas.", target: 5 },
  { key: "bimodal_master", title: "Bimodal Master", description: "Lee y escucha el mismo libro.", target: 1 },
  { key: "focus_sentinel", title: "Centinela del Enfoque", description: "Completa diez Pomodoros.", target: 10 },
  { key: "book_devourer", title: "Devorador de Libros", description: "Termina cinco libros.", target: 5 },
];

interface DemoAchievementMetrics {
  books: number;
  extracts: number;
  notes: number;
  diaryEntries: number;
  readingMinutes: number;
  pages: number;
  pomodoros: number;
  streak: number;
  audiobooks: number;
  shares: number;
  voiceNotes: number;
  nightSessions: number;
  bimodalBooks: number;
  finishedBooks: number;
}

function demoAchievementMetrics(): DemoAchievementMetrics {
  const sessionEntries = state.diary.filter(
    (entry) => entry.kind === "reading" || entry.kind === "listening",
  );
  const dates = [...new Set(sessionEntries.map((entry) => entry.entryDate))].sort();
  let streak = 0;
  let current = 0;
  let previous: Date | null = null;
  for (const value of dates) {
    const date = new Date(`${value}T12:00:00`);
    current =
      previous && Math.round((date.getTime() - previous.getTime()) / 86_400_000) === 1
        ? current + 1
        : 1;
    streak = Math.max(streak, current);
    previous = date;
  }

  const kindsByBook = new Map<string, Set<string>>();
  for (const entry of sessionEntries) {
    if (!entry.bookId) continue;
    const kinds = kindsByBook.get(entry.bookId) ?? new Set<string>();
    kinds.add(entry.kind);
    kindsByBook.set(entry.bookId, kinds);
  }

  return {
    books: state.books.length,
    extracts: state.extracts.length,
    notes: state.notes.length,
    diaryEntries: state.diary.length,
    readingMinutes: sessionEntries.reduce(
      (sum, entry) => sum + Math.max(0, entry.durationMinutes),
      0,
    ),
    pages: state.diary.reduce(
      (sum, entry) => sum + Math.max(0, entry.pagesRead),
      0,
    ),
    pomodoros: state.diary.filter((entry) => entry.kind === "pomodoro").length,
    streak,
    audiobooks: audiobooks.length,
    shares: 0,
    voiceNotes: state.voiceNotes.length,
    nightSessions: sessionEntries.filter((entry) => {
      if (!entry.sessionStart) return false;
      const hour = new Date(entry.sessionStart).getUTCHours();
      return hour >= 22 || hour < 6;
    }).length,
    bimodalBooks: [...kindsByBook.values()].filter((kinds) => kinds.size === 2).length,
    finishedBooks: state.books.filter((book) => book.progress >= 0.995).length,
  };
}

function evaluateDemoAchievements(emitUnlocked = false): Achievement[] {
  const metrics = demoAchievementMetrics();
  const result: Achievement[] = [];
  const unlocked: Achievement[] = [];

  for (const definition of demoAchievementDefinitions) {
    const value =
      definition.key === "first_book" || definition.key.startsWith("library_")
        ? metrics.books
        : definition.key === "first_extract" || definition.key.startsWith("extracts_")
          ? metrics.extracts
          : definition.key === "first_note" || definition.key.startsWith("notes_")
            ? metrics.notes
            : definition.key === "first_diary" || definition.key === "diary_30"
              ? metrics.diaryEntries
              : definition.key.startsWith("minutes_")
                ? metrics.readingMinutes
                : definition.key.startsWith("pages_")
                  ? metrics.pages
                  : definition.key === "first_pomodoro" ||
                      definition.key === "pomodoros_25" ||
                      definition.key === "focus_sentinel"
                    ? metrics.pomodoros
                    : definition.key.startsWith("streak_")
                      ? metrics.streak
                      : definition.key === "first_audiobook"
                        ? metrics.audiobooks
                        : definition.key === "first_share"
                          ? metrics.shares
                          : definition.key === "first_voice_note"
                            ? metrics.voiceNotes
                            : definition.key === "night_reader"
                              ? metrics.nightSessions
                              : definition.key === "bimodal_master"
                                ? metrics.bimodalBooks
                                : definition.key === "book_devourer"
                                  ? metrics.finishedBooks
                                  : 0;
    const progress = Math.max(
      achievementProgressStore.get(definition.key) ?? 0,
      Math.min(1, Math.max(0, value / definition.target)),
    );
    achievementProgressStore.set(definition.key, progress);
    let unlockedAt = achievementUnlockedStore.get(definition.key) ?? null;
    if (!unlockedAt && progress >= 1) {
      unlockedAt = isoNow();
      achievementUnlockedStore.set(definition.key, unlockedAt);
      unlocked.push({
        id: `achievement-${definition.key}`,
        key: definition.key,
        title: definition.title,
        description: definition.description,
        progress,
        unlockedAt,
        createdAt: daysAgo(30),
        updatedAt: unlockedAt,
        ownerAccountId: OWNER,
        originShareId: null,
      });
    }
    result.push({
      id: `achievement-${definition.key}`,
      key: definition.key,
      title: definition.title,
      description: definition.description,
      progress,
      unlockedAt,
      createdAt: daysAgo(30),
      updatedAt: unlockedAt ?? daysAgo(30),
      ownerAccountId: OWNER,
      originShareId: null,
    });
  }

  if (emitUnlocked) {
    for (const achievement of unlocked) {
      emit("achievement::unlocked", achievement);
    }
  }
  return result;
}

function defaultPomodoroConfig(): PomodoroConfig {
  return {
    focusMinutes: 25,
    breakMinutes: 5,
    longBreakMinutes: 15,
    roundsBeforeLongBreak: 4,
    autoStartBreaks: true,
    autoStartFocus: false,
  };
}

function validatePomodoroConfig(config: PomodoroConfig) {
  const validNumber = (value: number) => Number.isFinite(value) && Number.isInteger(value);
  if (!validNumber(config.focusMinutes) || config.focusMinutes < 1 || config.focusMinutes > 180) {
    throw { code: "invalid_input", message: "el foco debe durar entre 1 y 180 minutos" };
  }
  if (!validNumber(config.breakMinutes) || config.breakMinutes < 1 || config.breakMinutes > 60) {
    throw { code: "invalid_input", message: "el descanso debe durar entre 1 y 60 minutos" };
  }
  if (
    !validNumber(config.longBreakMinutes) ||
    config.longBreakMinutes < 1 ||
    config.longBreakMinutes > 120
  ) {
    throw {
      code: "invalid_input",
      message: "el descanso largo debe durar entre 1 y 120 minutos",
    };
  }
  if (
    !validNumber(config.roundsBeforeLongBreak) ||
    config.roundsBeforeLongBreak < 2 ||
    config.roundsBeforeLongBreak > 12
  ) {
    throw {
      code: "invalid_input",
      message: "las rondas antes del descanso largo deben estar entre 2 y 12",
    };
  }
}

function pomodoroStatus(): PomodoroStatus {
  return {
    state: pomodoro.state,
    endsAt: pomodoro.endsAt === null ? null : new Date(pomodoro.endsAt).toISOString(),
    remainingSeconds:
      pomodoro.endsAt === null
        ? 0
        : Math.max(0, Math.floor((pomodoro.endsAt - Date.now()) / 1000)),
    round: pomodoro.round,
    completedFocus: pomodoro.completedFocus,
    config: { ...pomodoro.config },
  };
}

function emitPomodoroStatus() {
  emit("pomodoro::state", pomodoroStatus());
}

function clearPomodoroTimer() {
  if (pomodoro.timer) {
    clearTimeout(pomodoro.timer);
    pomodoro.timer = undefined;
  }
}

function schedulePomodoro() {
  clearPomodoroTimer();
  if (pomodoro.state === "idle" || pomodoro.endsAt === null) return;
  const generation = pomodoro.generation;
  const delay = Math.max(0, pomodoro.endsAt - Date.now());
  pomodoro.timer = setTimeout(() => {
    if (generation !== pomodoro.generation) return;
    if (pomodoro.state === "focus") {
      completePomodoroFocus();
    } else if (pomodoro.state === "break" || pomodoro.state === "long_break") {
      completePomodoroBreak();
    }
  }, delay);
}

function completePomodoroFocus() {
  if (pomodoro.state !== "focus") return;
  const config = pomodoro.config;
  const round = Math.max(1, pomodoro.round);
  const startedAt = pomodoro.startedAt ?? Date.now() - config.focusMinutes * 60_000;
  const endedAt = startedAt + config.focusMinutes * 60_000;
  pomodoro.completedFocus += 1;
  pomodoro.startedAt = null;
  pomodoro.endsAt = null;

  if (config.autoStartBreaks) {
    const long = pomodoro.completedFocus % config.roundsBeforeLongBreak === 0;
    const minutes = long ? config.longBreakMinutes : config.breakMinutes;
    const now = Date.now();
    pomodoro.state = long ? "long_break" : "break";
    pomodoro.startedAt = now;
    pomodoro.endsAt = now + minutes * 60_000;
  } else {
    pomodoro.state = "idle";
  }
  pomodoro.round += 1;

  state.diary.push({
    id: `diary-pomodoro-${Date.now()}-${pomodoro.completedFocus}`,
    bookId: null,
    entryDate: localDateIso(new Date(endedAt)),
    body: `Pomodoro completado · ${config.focusMinutes} min\nRonda ${round}`,
    mood: null,
    kind: "pomodoro",
    sessionStart: new Date(startedAt).toISOString(),
    sessionEnd: new Date(endedAt).toISOString(),
    durationMinutes: config.focusMinutes,
    pagesRead: 0,
    extractsCreated: 0,
    notesCreated: 0,
    pomodoroRound: round,
    createdAt: isoNow(),
    updatedAt: isoNow(),
    ownerAccountId: OWNER,
    originShareId: null,
  });
  emit("diary::updated", { source: "pomodoro" });
  evaluateDemoAchievements(true);
  emitPomodoroStatus();
  schedulePomodoro();
}

function completePomodoroBreak() {
  if (pomodoro.state !== "break" && pomodoro.state !== "long_break") return;
  pomodoro.startedAt = null;
  pomodoro.endsAt = null;
  if (pomodoro.config.autoStartFocus) {
    const now = Date.now();
    pomodoro.state = "focus";
    pomodoro.startedAt = now;
    pomodoro.endsAt = now + pomodoro.config.focusMinutes * 60_000;
  } else {
    pomodoro.state = "idle";
  }
  emitPomodoroStatus();
  schedulePomodoro();
}

function startDemoPomodoro(config: PomodoroConfig | null): PomodoroStatus {
  const resolved = config ? { ...config } : { ...pomodoro.config };
  validatePomodoroConfig(resolved);
  pomodoro.generation += 1;
  clearPomodoroTimer();
  pomodoro.config = resolved;
  pomodoro.state = "focus";
  pomodoro.round = Math.max(1, pomodoro.round);
  const now = Date.now();
  pomodoro.startedAt = now;
  pomodoro.endsAt = now + resolved.focusMinutes * 60_000;
  emitPomodoroStatus();
  schedulePomodoro();
  return pomodoroStatus();
}

function stopDemoPomodoro(): PomodoroStatus {
  pomodoro.generation += 1;
  clearPomodoroTimer();
  pomodoro.state = "idle";
  pomodoro.startedAt = null;
  pomodoro.endsAt = null;
  emitPomodoroStatus();
  return pomodoroStatus();
}

function importBook(source: string): BookSummary {
  const base = source.split(/[\\/]/).pop() ?? "libro.epub";
  const title = base
    .replace(/\.(epub|pdf)$/i, "")
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
  const known = demoImportedSeeds[0];
  if (known && /isla/i.test(source)) {
    const existing = state.books.find((book) => book.id === known.id);
    if (existing) return clone(existing);
    const book = demoBookSummary(known);
    state.books.unshift(book);
    state.settings.set(known.id, {
      ...defaultSettings(known.id),
      ...known.settings,
    });
    state.progress.set(known.id, { section: 0, offset: 0, percent: 0 });
    return clone(book);
  }
  state.imported += 1;
  const seed: DemoBookSeed = {
    id: `book-import-${Date.now()}`,
    title: title || `Libro importado ${state.imported}`,
    author: "Autor desconocido",
    format: /\.pdf$/i.test(base) ? "pdf" : "epub",
    description: "Libro importado en el modo demostración.",
    publisher: "Importación local",
    language: "es",
    sizeBytes: 1_200_000 + state.imported * 13_000,
    addedAt: isoNow(),
    lastReadAt: null,
    progress: 0,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: [],
    palette: ["#243447", "#5c7a89", "#f4ead9"],
    motif: "dots",
  };
  const book = demoBookSummary(seed);
  registerBookSeed(seed);
  state.books.unshift(book);
  state.settings.set(seed.id, {
    ...defaultSettings(seed.id),
    mode: seed.format === "pdf" ? "scroll" : "paged",
  });
  state.progress.set(seed.id, { section: 0, offset: 0, percent: 0 });
  return clone(book);
}

async function invoke(command: string, args: Args = {}): Promise<unknown> {
  const latency =
    command === "reader_open"
      ? 220
      : command === "reader_get_chapter"
        ? 120
        : command === "library_list"
          ? 90
          : command.startsWith("plugin:")
            ? 0
            : 40;
  if (latency > 0) await new Promise((resolve) => setTimeout(resolve, latency));

  switch (command) {
    // --- Aplicación, apariencia y cuenta ---------------------------------
    case "app_info":
      return {
        name: "Kora",
        version: "0.1.0-demo",
        identifier: "dev.kora.reader.demo",
        platform: "web",
        arch: "browser",
        storage: {
          root: "/demo/kora",
          db: "/demo/kora/db/app.sqlite",
          assets: "/demo/kora/assets",
          exports: "/demo/kora/exports",
          trash: "/demo/kora/trash",
          logs: "/demo/kora/logs",
        },
      } satisfies AppInfo;

    case "storage_layout":
      return {
        root: "/demo/kora",
        db: "/demo/kora/db/app.sqlite",
        assets: "/demo/kora/assets",
        exports: "/demo/kora/exports",
        trash: "/demo/kora/trash",
        logs: "/demo/kora/logs",
      };

    case "storage_usage":
      return clone(demoUsage);

    case "backup_library": {
      const destination = String(args.destination ?? "kora-copia.zip");
      demoUsage.exports = Math.max(demoUsage.exports, 1_200_000);
      demoUsage.total = usageTotal(demoUsage);
      return {
        path: destination,
        bytes: demoUsage.total,
        entries: 48,
      };
    }

    case "restore_library":
      return { requiresRestart: true };

    case "purge_storage": {
      const scope = String(args.scope ?? "");
      if (scope === "trash") {
        demoUsage.trash = 0;
        state.trash = [];
      } else if (scope === "temporary") {
        demoUsage.temporary = 0;
      } else {
        throw {
          code: "invalid_input",
          message: "solo se puede vaciar la papelera o los temporales",
        };
      }
      demoUsage.total = usageTotal(demoUsage);
      return clone(demoUsage);
    }

    case "get_appearance":
      return clone(appearanceState());

    case "save_appearance": {
      const current = appearanceState();
      current.preferences.themeId = args.themeId ?? current.preferences.themeId;
      current.preferences.texture = args.texture ?? current.preferences.texture;
      current.preferences.locale = args.locale ?? current.preferences.locale;
      if (args.onboardingDone !== null && args.onboardingDone !== undefined) {
        current.preferences.onboardingDone = Boolean(args.onboardingDone);
      }
      if (args.libraryLayout) {
        current.preferences.libraryLayout = args.libraryLayout;
      }
      current.preferences.updatedAt = isoNow();
      appearance = current;
      return clone(current);
    }

    case "save_custom_theme": {
      const current = appearanceState();
      const existing = current.themes.find((theme) => theme.id === args.id);
      if (existing) {
        existing.name = args.name;
        existing.background = args.background;
        existing.foreground = args.foreground;
        existing.accent = args.accent;
        existing.updatedAt = isoNow();
        appearance = current;
        return clone(existing);
      }
      const theme = {
        id: args.id ?? `theme-${Date.now()}`,
        name: args.name,
        background: args.background,
        foreground: args.foreground,
        accent: args.accent,
        createdAt: isoNow(),
        updatedAt: isoNow(),
        ownerAccountId: OWNER,
      };
      current.themes.push(theme);
      appearance = current;
      return clone(theme);
    }

    case "delete_custom_theme": {
      const current = appearanceState();
      const themeId = String(args.themeId ?? "");
      current.themes = current.themes.filter((theme) => theme.id !== themeId);
      if (current.preferences.themeId === themeId) {
        current.preferences.themeId = "light";
      }
      appearance = current;
      return clone(current);
    }

    case "get_account":
      return clone(demoAccount);
    case "account_status":
      return {
        signedIn: true,
        accountId: demoAccount.id,
        displayName: demoAccount.displayName,
        deviceCount: demoAccountDevices.length,
      };
    case "list_devices":
      return clone(demoAccountDevices);
    case "revoke_device":
      return revokeDemoPeer(String(args.deviceId ?? ""));
    case "update_display_name":
      demoAccount.displayName = args.displayName;
      return clone(demoAccount);
    case "update_account_avatar":
      demoAccount.avatarPath = args.image ?? null;
      return clone(demoAccount);
    case "remove_account_avatar":
      demoAccount.avatarPath = null;
      return clone(demoAccount);
    case "features_status":
      return {
        ocr: false,
        embeddings: true,
        journal: true,
        achievements: true,
      };

    // --- Biblioteca -------------------------------------------------------
    case "library_list":
      return state.books.map(clone);

    case "library_overview":
      return continueBook();

    case "library_audiobook_book_ids":
      return [
        ...new Set(
          audiobooks
            .filter((item) => bookFor(item.bookId))
            .map((item) => item.bookId),
        ),
      ];

    case "export_book":
      return String(args.destination ?? "/demo/export/libro.epub");

    case "audiobook_album_list":
      return demoAlbums.map(clone);

    case "audiobook_album_create": {
      const title = String(args.title ?? "").trim();
      if (!title) throw { code: "invalid_input", message: "El álbum necesita un nombre" };
      const album: AudiobookAlbumInfo = {
        id: `album-${Date.now()}`,
        title,
        description: cleanOptional(args.description),
        sortOrder: demoAlbums.length,
        createdAt: isoNow(),
        updatedAt: isoNow(),
        audiobookIds: [],
      };
      demoAlbums.push(album);
      return clone(album);
    }

    case "audiobook_album_update": {
      const album = demoAlbums.find((item) => item.id === args.albumId);
      if (!album) throw { code: "not_found", message: "Álbum no encontrado" };
      const title = String(args.title ?? "").trim();
      if (!title) throw { code: "invalid_input", message: "El álbum necesita un nombre" };
      album.title = title;
      album.description = cleanOptional(args.description);
      album.updatedAt = isoNow();
      return clone(album);
    }

    case "audiobook_album_delete":
      demoAlbums = demoAlbums.filter((item) => item.id !== args.albumId);
      return null;

    case "audiobook_album_add": {
      const album = demoAlbums.find((item) => item.id === args.albumId);
      if (!album) throw { code: "not_found", message: "Álbum no encontrado" };
      const ids = Array.isArray(args.audiobookIds)
        ? (args.audiobookIds as string[])
        : [];
      for (const id of ids) {
        if (
          audiobooks.some((item) => item.id === id) &&
          !album.audiobookIds.includes(id)
        ) {
          album.audiobookIds.push(id);
        }
      }
      album.updatedAt = isoNow();
      return clone(album);
    }

    case "audiobook_album_remove": {
      const album = demoAlbums.find((item) => item.id === args.albumId);
      if (!album) throw { code: "not_found", message: "Álbum no encontrado" };
      album.audiobookIds = album.audiobookIds.filter(
        (id) => id !== args.audiobookId,
      );
      album.updatedAt = isoNow();
      return clone(album);
    }

    case "audiobook_album_reorder": {
      const album = demoAlbums.find((item) => item.id === args.albumId);
      if (!album) throw { code: "not_found", message: "Álbum no encontrado" };
      const ids = Array.isArray(args.audiobookIds)
        ? (args.audiobookIds as string[])
        : [];
      const valid = ids.filter((id) => album.audiobookIds.includes(id));
      const rest = album.audiobookIds.filter((id) => !valid.includes(id));
      album.audiobookIds = [...valid, ...rest];
      album.updatedAt = isoNow();
      return clone(album);
    }

    case "category_list":
      return state.categories.map(clone);

    case "category_create": {
      const category: Category = {
        id: `category-${Date.now()}`,
        name: args.name,
        color: args.color ?? "#c2410c",
        sortOrder: state.categories.length,
        createdAt: isoNow(),
        updatedAt: isoNow(),
        ownerAccountId: OWNER,
        originShareId: null,
      };
      state.categories.push(category);
      return clone(category);
    }

    case "category_update": {
      const category = state.categories.find(
        (item) => item.id === args.categoryId,
      );
      if (!category) throw { code: "not_found", message: "Categoría no encontrada" };
      category.name = args.name;
      category.color = args.color ?? null;
      category.updatedAt = isoNow();
      return clone(category);
    }

    case "category_delete": {
      state.categories = state.categories.filter(
        (item) => item.id !== args.categoryId,
      );
      for (const book of state.books) {
        book.categoryIds = book.categoryIds.filter(
          (id) => id !== args.categoryId,
        );
      }
      return null;
    }

    case "library_assign_category": {
      const book = state.books.find((item) => item.id === args.bookId);
      if (!book) throw { code: "not_found", message: "Libro no encontrado" };
      if (!book.categoryIds.includes(args.categoryId)) {
        book.categoryIds.push(args.categoryId);
      }
      return null;
    }

    case "library_unassign_category": {
      const book = state.books.find((item) => item.id === args.bookId);
      if (book) {
        book.categoryIds = book.categoryIds.filter(
          (id) => id !== args.categoryId,
        );
      }
      return null;
    }

    case "library_set_favorite": {
      return updateBook(args.bookId, { favorite: Boolean(args.favorite) });
    }

    case "library_update": {
      const patch: Partial<BookSummary> = {};
      if (typeof args.title === "string") patch.title = args.title;
      if ("author" in args) patch.author = args.author;
      if ("description" in args) patch.description = args.description;
      if ("language" in args) patch.language = args.language;
      if ("publisher" in args) patch.publisher = args.publisher;
      return updateBook(args.bookId, patch);
    }

    case "library_ensure_cover": {
      const book = state.books.find((item) => item.id === args.bookId);
      if (!book) throw { code: "not_found", message: "Libro no encontrado" };
      return clone(book);
    }

    case "library_replace_cover": {
      const seed = findBookSeed(args.bookId);
      const book = state.books.find((item) => item.id === args.bookId);
      if (seed && book) {
        const alternative = coverDataUri({
          ...seed,
          palette: ["#3a2a22", "#9c6b4a", "#f7ead6"],
          motif: "arch",
        });
        coverOverrides.set(seed.id, alternative);
        book.coverPath = alternative;
      }
      return updateBook(args.bookId, {});
    }

    case "library_delete": {
      const book = state.books.find((item) => item.id === args.bookId);
      if (!book) throw { code: "not_found", message: "Libro no encontrado" };
      state.books = state.books.filter((item) => item.id !== args.bookId);
      state.trash = [
        { ...book, deletedAt: isoNow() },
        ...state.trash.filter((item) => item.id !== args.bookId),
      ];
      return null;
    }

    case "library_trash":
      return state.trash.map(clone);

    case "library_restore": {
      const trashed = state.trash.find((item) => item.id === args.bookId);
      if (!trashed) throw { code: "not_found", message: "Libro no encontrado" };
      state.trash = state.trash.filter((item) => item.id !== args.bookId);
      const { deletedAt: _deletedAt, ...book } = trashed;
      state.books = [book, ...state.books];
      return clone(book);
    }

    case "library_purge_book":
      state.trash = state.trash.filter((item) => item.id !== args.bookId);
      return null;

    case "library_empty_trash": {
      const count = state.trash.length;
      state.trash = [];
      return count;
    }

    case "library_import":
      return importBook(String(args.source ?? ""));

    case "category_suggestion_list_pending":
      return state.suggestions.map(clone);

    case "library_classify": {
      const book = state.books.find((item) => item.id === args.bookId);
      if (!book) throw { code: "not_found", message: "Libro no encontrado" };
      const categoryId = book.categoryIds[0] ?? null;
      const created = makeSuggestion(
        book.id,
        categoryId,
        categoryId ? 0.81 : 0.72,
        "ai",
        categoryId
          ? "La clasificación temática del texto coincide con una categoría existente."
          : "El texto no encaja del todo en las categorías actuales.",
        categoryId ? null : "Narrativa contemporánea",
      );
      if (!state.suggestions.some((item) => item.bookId === book.id)) {
        state.suggestions.push(created);
      }
      return state.suggestions
        .filter((item) => item.bookId === book.id)
        .map(clone);
    }

    case "category_suggestion_accept": {
      const suggestion = state.suggestions.find(
        (item) => item.id === args.suggestionId,
      );
      if (!suggestion) return null;
      let categoryId = suggestion.categoryId;
      if (!categoryId && suggestion.proposedName) {
        const category: Category = {
          id: `category-${Date.now()}`,
          name: suggestion.proposedName,
          color: "#0f766e",
          sortOrder: state.categories.length,
          createdAt: isoNow(),
          updatedAt: isoNow(),
          ownerAccountId: OWNER,
          originShareId: null,
        };
        state.categories.push(category);
        categoryId = category.id;
      }
      if (categoryId) {
        const book = state.books.find((item) => item.id === suggestion.bookId);
        if (book && !book.categoryIds.includes(categoryId)) {
          book.categoryIds.push(categoryId);
        }
      }
      state.suggestions = state.suggestions.filter(
        (item) => item.id !== args.suggestionId,
      );
      return null;
    }

    case "category_suggestion_dismiss":
      state.suggestions = state.suggestions.filter(
        (item) => item.id !== args.suggestionId,
      );
      return null;

    case "category_suggestion_accept_all": {
      const remaining = state.suggestions.filter(
        (item) => item.bookId === args.bookId,
      );
      for (const suggestion of remaining) {
        if (suggestion.categoryId) {
          const book = state.books.find((item) => item.id === suggestion.bookId);
          if (book && !book.categoryIds.includes(suggestion.categoryId)) {
            book.categoryIds.push(suggestion.categoryId);
          }
        }
      }
      state.suggestions = state.suggestions.filter(
        (item) => item.bookId !== args.bookId,
      );
      return remaining.length;
    }

    case "category_seed_apply":
      return state.categories.map(clone);

    case "classification_get_config":
      return clone(demoClassificationConfig);

    case "classification_save_config": {
      const config = args.config as Partial<ClassificationConfig> | undefined;
      if (config) {
        demoClassificationConfig = {
          ...demoClassificationConfig,
          ...config,
          embeddingThreshold: Math.min(
            1,
            Math.max(
              0,
              Number(
                config.embeddingThreshold ??
                  demoClassificationConfig.embeddingThreshold,
              ),
            ),
          ),
          aiThreshold: Math.min(
            1,
            Math.max(
              0,
              Number(config.aiThreshold ?? demoClassificationConfig.aiThreshold),
            ),
          ),
          maxSuggestions: Math.min(
            8,
            Math.max(
              1,
              Math.round(
                Number(
                  config.maxSuggestions ??
                    demoClassificationConfig.maxSuggestions,
                ),
              ),
            ),
          ),
        };
      }
      return clone(demoClassificationConfig);
    }

    case "classification_status":
      return {
        enabled: demoClassificationConfig.enabled,
        seeded: true,
        aiAvailable: true,
        embeddingsReady: true,
        pendingSuggestions: state.suggestions.length,
        classifiedBooks: state.books.length,
        totalBooks: state.books.length,
      };

    case "library_classify_pending":
      return null;

    case "semantic_status":
      return semanticStatus();

    case "prepare_semantic_model":
      return semanticStatus();

    case "reindex_library":
      return semanticStatus();

    case "search_semantic":
      return searchSemantic(String(args.query ?? ""), Number(args.limit ?? 10));

    case "pdf_conversion_preview": {
      const seed = findBookSeed(args.bookId);
      if (!seed || seed.format !== "pdf") {
        throw { code: "not_found", message: "PDF no encontrado" };
      }
      return {
        bookId: seed.id,
        title: seed.title,
        author: seed.author,
        language: seed.language,
        pages: seed.pdfPages?.length ?? 0,
        scanned: false,
        ocrStatus: "skipped",
        ocrPagesDone: 0,
        ocrPagesTotal: seed.pdfPages?.length ?? 0,
        hasNativeOutline: false,
        estimatedChapters: seed.pdfPages?.length ?? 0,
        extraction: "none",
        sample: seed.pdfPages?.[0]?.slice(0, 6).join(" ") ?? "",
        annotations: 0,
        extracts: 0,
        notes: 0,
        voiceNotes: 0,
        reactions: 0,
        audiobooks: 0,
        canConvert: false,
        warnings: [
          "La conversión PDF→EPUB está desactivada en el modo demostración.",
        ],
      } satisfies PdfConversionPreview;
    }

    case "pdf_conversion_status":
      return null;

    case "convert_pdf_to_epub":
      throw {
        code: "demo",
        message: "La conversión PDF→EPUB necesita el backend de Tauri.",
      };

    // --- Audiolibros ------------------------------------------------------
    case "list_audiobooks":
      return audiobooks.map(clone);

    case "send_book_to_audiobooks": {
      const bookId = String(args.bookId ?? "");
      const book = bookFor(bookId);
      if (!book) throw { code: "not_found", message: "Libro no encontrado" };
      const existing = onDemandAudiobookForBook(bookId);
      if (existing) return clone(existing);
      const seed = findBookSeed(bookId);
      const chapterCount = seed
        ? seed.format === "pdf"
          ? (seed.pdfPages?.length ?? 1)
          : chaptersFor(seed).length
        : 0;
      const item: AudiobookInfo = {
        id: `audiobook-stream-${Date.now()}`,
        bookId,
        bookTitle: book.title,
        author: book.author,
        voiceId: null,
        voiceName: null,
        status: "done",
        delivery: "on_demand",
        origin: "generated",
        durationMs: 0,
        audioHash: null,
        audioExt: null,
        scope: { kind: "fullBook" },
        narration: null,
        cast: [],
        chapterSource: null,
        treatment: null,
        chapterCount,
        watermark: false,
        coverPath: book.coverPath,
        createdAt: isoNow(),
        positionMs: 0,
        lastListenedAt: null,
        percent: 0,
        progressChapterIndex: 0,
        progressChapterTitle: null,
        progressSection: 0,
        progressOffset: 0,
        favorite: false,
        audioOnly: false,
        categoryIds: [...book.categoryIds],
      };
      audiobooks = [item, ...audiobooks];
      return clone(item);
    }


    case "audiobook_capabilities":
      return {
        platformSupported: true,
        generationSupported: true,
        nativeMux: false,
        audioImportSupported: true,
        nativeAudioImport: false,
        transcriptionSupported: true,
        playbackSupported: true,
        backgroundPlayback: false,
        ffmpeg: true,
        whisperBase: true,
      } satisfies AudiobookCapabilities;

    case "audiobook_queue_status":
      return queueView();

    case "get_audiobook_progress":
      return refreshAudiobookPlayback();

    case "play_audiobook":
      return playAudiobookSnapshot(String(args.audiobookId ?? ""));

    case "pause_audiobook":
      return pauseAudiobookSnapshot();

    case "seek_audiobook":
      return seekAudiobookSnapshot(Number(args.positionMs ?? 0));

    case "set_audiobook_speed":
      return setAudiobookSpeedSnapshot(Number(args.speed ?? 1));

    case "set_audiobook_keep_screen_on":
    case "set_audiobook_sleep_timer":
      return null;

    case "stop_audiobook":
      audiobookPlayback = null;
      audiobookPlaybackBase = 0;
      return null;

    case "audiobook_chapters":
      return buildAudiobookChapters(
        audiobookById(String(args.audiobookId ?? "")) ?? {
          durationMs: 0,
          chapterCount: 0,
        } as AudiobookInfo,
      );

    case "audiobook_karaoke":
      return karaokeFor(
        String(args.audiobookId ?? ""),
        Number(args.chapterIndex ?? 0),
      );

    case "audiobook_waveform":
      return waveformFor(
        String(args.audiobookId ?? ""),
        Number(args.buckets ?? 600),
      );

    case "audiobook_locator": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      const position = Number(args.positionMs ?? 0);
      const seed = findBookSeed(item.bookId);
      const { chapter, index } = chapterAt(item, position);
      const offsetStart = offsetForPosition(item, position);
      const text = seed ? chapterText(seed, chapter.section) : "";
      return {
        chapterIndex: index,
        section: chapter.section,
        offsetStart,
        offsetEnd: Math.min(text.length, offsetStart + 1),
        quote: text
          .slice(Math.max(0, offsetStart - 40), Math.min(text.length, offsetStart + 120))
          .trim(),
      } satisfies AudiobookTextLocator;
    }

    case "audiobook_set_favorite": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      item.favorite = Boolean(args.favorite);
      return clone(item);
    }

    case "audiobook_voice_markers":
      return state.voiceNotes
        .filter(
          (entry) =>
            entry.audiobookId === args.audiobookId && entry.source === "voice_marker",
        )
        .sort((left, right) => left.positionMs - right.positionMs)
        .map(clone);

    case "voice_marker_status":
      return voiceMarkerStatus();

    case "save_voice_marker_phrase":
      voiceMarkerPhrase = String(args.phrase ?? voiceMarkerPhrase).trim() || voiceMarkerPhrase;
      return voiceMarkerStatus();

    case "start_voice_marker_listening": {
      voiceMarkerListening = true;
      if (voiceMarkerTimer) clearTimeout(voiceMarkerTimer);
      const audiobookId = String(args.audiobookId ?? "");
      voiceMarkerTimer = setTimeout(() => {
        if (!voiceMarkerListening) return;
        const item = audiobookById(audiobookId);
        if (!item) return;
        const current = refreshAudiobookPlayback();
        const positionMs = Math.min(
          item.durationMs,
          Math.max(0, (current?.positionMs ?? item.positionMs) + 4_000),
        );
        const marker: VoiceNote = {
          id: `voice-marker-${Date.now()}`,
          bookId: item.bookId,
          chapterIndex: chapterAt(item, positionMs).index,
          offsetStart: offsetForPosition(item, positionMs),
          offsetEnd: offsetForPosition(item, positionMs) + 1,
          text: voiceMarkerPhrase,
          durationMs: 4_000,
          audiobookId: item.id,
          positionMs,
          quote: null,
          transcript: voiceMarkerPhrase,
          source: "voice_marker",
          createdAt: isoNow(),
          updatedAt: isoNow(),
          deleted: false,
          ownerAccountId: OWNER,
          originShareId: null,
          audioPath: null,
        };
        state.voiceNotes.push(marker);
        emit("audiobook::voice_marker", clone(marker));
        voiceMarkerListening = false;
      }, 4_500);
      return voiceMarkerStatus();
    }

    case "stop_voice_marker_listening":
      voiceMarkerListening = false;
      if (voiceMarkerTimer) {
        clearTimeout(voiceMarkerTimer);
        voiceMarkerTimer = undefined;
      }
      return voiceMarkerStatus();

    case "list_all_extracts":
      return state.extracts.map(clone);

    case "delete_audiobook": {
      const id = String(args.audiobookId ?? "");
      audiobooks = audiobooks.filter((item) => item.id !== id);
      demoAlbums = demoAlbums.map((album) => ({
        ...album,
        audiobookIds: album.audiobookIds.filter((entry) => entry !== id),
      }));
      if (audiobookPlayback?.audiobookId === id) audiobookPlayback = null;
      return null;
    }

    case "export_audiobook":
      return String(args.destination ?? "/demo/export/audiolibro.m4b");

    case "apply_audiobook_treatment": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      item.treatment = args.treatment as AudioTreatment;
      return clone(item);
    }

    case "enqueue_audiobook": {
      const scope = (args.scope as AudiobookScope) ?? { kind: "fullBook" };
      const item = queueItemFromArgs(
        { ...args, scope },
        "generated",
      );
      return enqueueJob(item);
    }

    case "generate_audiobook": {
      const item = queueItemFromArgs(args, "generated");
      const view = enqueueJob(item);
      return audiobooks.find((entry) => entry.id === view.active?.audiobookId) ?? null;
    }

    case "remove_queued_audiobook": {
      const id = String(args.id ?? "");
      generation.queue = generation.queue.filter((item) => item.id !== id);
      emit("audiobook::queue", clone(generation));
      return queueView();
    }

    case "move_queued_audiobook": {
      const id = String(args.id ?? "");
      const direction = Number(args.direction ?? 1);
      const index = generation.queue.findIndex((item) => item.id === id);
      if (index >= 0) {
        const target = Math.max(
          0,
          Math.min(generation.queue.length - 1, index + direction),
        );
        const next = [...generation.queue];
        const [moved] = next.splice(index, 1);
        next.splice(target, 0, moved);
        generation.queue = next;
      }
      emit("audiobook::queue", clone(generation));
      return queueView();
    }

    case "clear_audiobook_queue":
      generation.queue = [];
      emit("audiobook::queue", clone(generation));
      return queueView();

    case "pause_audiobook_generation":
      if (generation.active) generation.active.paused = true;
      if (generationTimer) {
        clearTimeout(generationTimer);
        generationTimer = undefined;
      }
      emit("audiobook::queue", clone(generation));
      return queueView();

    case "resume_audiobook_generation":
      if (generation.active) {
        generation.active.paused = false;
        scheduleGeneration();
      }
      emit("audiobook::queue", clone(generation));
      return queueView();

    case "cancel_audiobook_generation":
      if (generationTimer) {
        clearTimeout(generationTimer);
        generationTimer = undefined;
      }
      if (generation.active) {
        const pending = audiobookById(generation.active.audiobookId ?? "");
        if (pending && pending.status !== "done") pending.status = "cancelled";
        emitGenerationProgress(
          "cancelled",
          generationPercent,
          "Generación cancelada.",
          0,
          0,
          generation.active.audiobookId,
        );
      }
      generation.active = null;
      queuedActiveData = null;
      generationPercent = 0;
      emit("audiobook::queue", clone(generation));
      return queueView();

    case "retry_audiobook": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      item.status = "pending";
      const queued: AudiobookQueueItem = {
        id: item.id,
        bookId: item.bookId,
        bookTitle: item.bookTitle,
        author: item.author,
        coverPath: item.coverPath,
        scope: item.scope ?? { kind: "fullBook" },
        voiceId: item.voiceId ?? "es_ES-davefx-medium",
        voiceName: item.voiceName,
        narration: item.narration,
        cast: { enabled: false, assignments: [] },
        origin: "generated",
        position: 0,
        createdAt: isoNow(),
      };
      if (!generation.active) {
        generation.active = {
          title: item.bookTitle,
          voiceName: item.voiceName,
          audiobookId: item.id,
          paused: false,
          startedAt: isoNow(),
        };
        queuedActiveData = queued;
        generationPercent = 0;
        scheduleGeneration();
      } else {
        generation.queue = [...generation.queue, queued];
      }
      emit("audiobook::queue", clone(generation));
      return clone(item);
    }

    case "import_external_audiobook":
      return importAudiobookFile(String(args.path ?? ""), true);

    case "import_audio_audiobook":
      return importAudiobookFile(String(args.path ?? ""), false);

    case "reanalyze_audiobook_chapters": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      chapterOverrides.delete(item.id);
      item.chapterSource = "ai";
      item.chapterCount = Math.max(3, item.chapterCount || 3);
      item.durationMs = Math.max(item.durationMs, 3_600_000);
      return clone(item);
    }

    case "transcribe_audiobook_later": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      chapterOverrides.delete(item.id);
      item.audioOnly = false;
      item.chapterSource = "ai";
      return clone(item);
    }

    case "update_audiobook_chapters": {
      const item = audiobookById(String(args.audiobookId ?? ""));
      if (!item) throw { code: "not_found", message: "Audiolibro no encontrado" };
      const chapters = (args.chapters as ChapterEditInput[]) ?? [];
      chapterOverrides.set(item.id, chapters.map((chapter) => ({ ...chapter })));
      item.chapterCount = Math.max(1, chapters.length);
      item.chapterSource = "manual";
      item.durationMs = Math.max(
        item.durationMs,
        chapters[chapters.length - 1]?.endMs ?? item.durationMs,
      );
      return clone(item);
    }

    case "create_audiobook_share": {
      const id = `share-${shareSeq++}`;
      const permission = (args.permission as "read" | "copy") ?? "copy";
      return {
        share: {
          id,
          ownerAccountId: OWNER,
          resourceType: "audiobook",
          resourceId: String(args.audiobookId ?? ""),
          scopeType: "book",
          permission,
          expiresAt: (args.expiresAt as string | null) ?? null,
          maxUses: (args.maxUses as number | null) ?? null,
          uses: 0,
          createdAt: isoNow(),
          updatedAt: isoNow(),
          revokedAt: null,
        },
        key: `KORA-DEMO-${String(shareSeq).padStart(2, "0")}7F2A`,
        uri: `kora://share/${id}`,
        qrSvg: shareQr(shareSeq * 37 + 11),
      } satisfies ShareView;
    }

    case "start_share":
      return {
        enabled: true,
        running: true,
        peerCount: 2,
        listenPort: 43_777,
        trustedPeers: 2,
        pendingChanges: 0,
        lastSync: isoNow(),
        pairing: false,
        sharing: true,
        mirrorActive: mirror.active,
        mirrorHosting: mirror.hosting,
        syncing: false,
        autoSync: demoSyncStatus.autoSync,
        onlinePeers: 2,
        lastError: null,
      };

    case "stop_share":
      return {
        enabled: true,
        running: true,
        peerCount: 2,
        listenPort: 43_777,
        trustedPeers: 2,
        pendingChanges: 0,
        lastSync: isoNow(),
        pairing: false,
        sharing: false,
        mirrorActive: mirror.active,
        mirrorHosting: mirror.hosting,
        syncing: false,
        autoSync: demoSyncStatus.autoSync,
        onlinePeers: 2,
        lastError: null,
      };

    case "weave_extracts": {
      const ids = (args.extractIds as string[]) ?? [];
      const selected = state.extracts.filter((entry) => ids.includes(entry.id));
      if (selected.length === 0) {
        throw { code: "empty", message: "Selecciona al menos un extracto." };
      }
      const title = String(args.title ?? "Tejido");
      const bookId = `book-weave-${Date.now()}`;
      const seed: DemoBookSeed = {
        id: bookId,
        title,
        author: "Kora · Tejido",
        format: "epub",
        description: "Mini libro creado a partir de extractos de la biblioteca.",
        publisher: "Tejido local",
        language: "es",
        sizeBytes: 420_000,
        addedAt: isoNow(),
        lastReadAt: null,
        progress: 0,
        locator: { section: 0, offset: 0 },
        favorite: false,
        readOnly: false,
        categoryIds: [],
        palette: ["#3e2a35", "#8c5b68", "#f5ead8"],
        motif: "leaves",
        chapters: [
          {
            title,
            paragraphs: selected.map(
              (entry) =>
                `${entry.text}${entry.note ? `\n\n${entry.note}` : ""}`,
            ),
          },
        ],
      };
      registerBookSeed(seed);
      const book: BookSummary = {
        id: bookId,
        title,
        author: seed.author,
        format: "epub",
        coverPath: coverDataUri(seed),
        sizeBytes: seed.sizeBytes,
        addedAt: seed.addedAt,
        language: "es",
        publisher: seed.publisher,
        description: seed.description,
        lastReadAt: null,
        progress: 0,
        origin: "tejido",
        favorite: false,
        readOnly: false,
        receivedShareId: null,
        categoryIds: [],
      };
      state.books.unshift(book);
      state.settings.set(bookId, defaultSettings(bookId));
      state.progress.set(bookId, { section: 0, offset: 0, percent: 0 });
      const audiobook: AudiobookInfo = {
        id: `audiobook-weave-${Date.now()}`,
        bookId: book.id,
        bookTitle: title,
        author: book.author,
        voiceId: String(args.voiceId ?? "es_ES-davefx-medium"),
        voiceName:
          demoVoices.find((voice) => voice.id === args.voiceId)?.name ?? "DaveFX",
        status: "done",
        delivery: "file",
        origin: "tejido",
        durationMs: Math.max(60_000, selected.length * 55_000),
        audioHash: `demo-audio-weave-${Date.now()}`,
        audioExt: "m4b",
        scope: { kind: "fullBook" },
        narration: null,
        cast: [],
        chapterSource: "manual",
        treatment: null,
        chapterCount: 1,
        watermark: false,
        coverPath: book.coverPath,
        createdAt: isoNow(),
        positionMs: 0,
        lastListenedAt: null,
        percent: 0,
        progressChapterIndex: 0,
        progressChapterTitle: title,
        progressSection: 0,
        progressOffset: 0,
        favorite: false,
        audioOnly: false,
        categoryIds: [],
      };
      audiobooks.unshift(audiobook);
      return {
        book: { id: book.id, title: book.title },
        audiobook: { id: audiobook.id },
      } satisfies WeaveResult;
    }

    case "audiobook_characters": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      return seed ? (bookCharacters[seed.id] ?? []).map(clone) : [];
    }

    case "tts_synthesize": {
      const request = args.request as
        | { text?: string; voiceId?: string }
        | undefined;
      const voiceId = String(request?.voiceId ?? "");
      if (voiceId.startsWith("clone:")) {
        if (!voiceProfileById(voiceId)) {
          throw { code: "not_found", message: "el perfil de voz no existe" };
        }
      } else {
        const voice = demoVoices.find((item) => item.id === voiceId);
        if (!voice) {
          throw { code: "not_found", message: "la voz no existe" };
        }
        if (!voice.installed) {
          throw {
            code: "not_found",
            message: "la voz no está instalada",
          };
        }
      }
      const durationMs = Math.max(
        500,
        Math.min(2_500, (request?.text?.length ?? 30) * 42),
      );
      return {
        audioBase64: wavBase64(durationMs, 420),
        sampleRate: 16_000,
        durationMs,
      } satisfies SynthesisOutput;
    }

    case "voice_clone_preview": {
      const profile = voiceProfileById(String(args.voiceId ?? ""));
      if (!profile) {
        throw { code: "not_found", message: "el perfil de voz no existe" };
      }
      const text = String(args.text ?? "").trim();
      if (!text || text.length > 400) {
        throw {
          code: "invalid_input",
          message: "la vista previa debe tener entre 1 y 400 caracteres",
        };
      }
      const durationMs = Math.max(
        600,
        Math.min(2_800, String(args.text ?? "").length * 52),
      );
      return {
        audioBase64: wavBase64(durationMs, 380),
        sampleRate: 16_000,
        durationMs,
      } satisfies VoicePreview;
    }

    // --- Lector -----------------------------------------------------------
    case "reader_open": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      if (!seed) throw { code: "not_found", message: "Libro no encontrado" };
      const saved = state.progress.get(seed.id);
      const savedSettings = state.settings.get(seed.id) ?? defaultSettings(seed.id);
      const chapters = chaptersFor(seed);
      const sections =
        seed.format === "pdf"
          ? (seed.pdfPages ?? []).map((_, index) => ({
              id: `page-${index}`,
              title: `Página ${index + 1}`,
              locator: { section: index, offset: 0 },
            }))
          : chapters.map((entry, index) => ({
              id: `section-${index}`,
              title: entry.title,
              locator: { section: index, offset: 0 },
            }));
      const requested = (args.locator as Locator | null) ?? null;
      const section = Math.min(
        Math.max(0, requested?.section ?? saved?.section ?? seed.locator.section),
        Math.max(0, sections.length - 1),
      );
      const offset = requested?.offset ?? saved?.offset ?? seed.locator.offset;
      const settings: ReaderSettings = {
        ...savedSettings,
        bookId: seed.id,
        mode:
          seed.format === "pdf"
            ? "scroll"
            : (savedSettings.mode as ReaderSettings["mode"]),
      };
      return {
        bookId: seed.id,
        document: {
          id: seed.id,
          format: seed.format,
          title: seed.title,
          author: seed.author,
          language: seed.language,
          publisher: seed.publisher,
          description: seed.description,
          sections,
        },
        locator: { section, offset },
        coverPath: coverOverrides.get(seed.id) ?? coverDataUrl(seed),
        progress: saved?.percent ?? seed.progress,
        settings: clone(settings),
        pdf:
          seed.format === "pdf"
            ? {
                pageCount: seed.pdfPages?.length ?? 0,
                scanned: false,
                hasNativeOutline: false,
                sourcePath: `demo-pdf:${seed.id}`,
                ocrStatus: "skipped",
                ocrPagesDone: 0,
                ocrPagesTotal: seed.pdfPages?.length ?? 0,
              }
            : null,
      } satisfies ReaderSession;
    }

    case "reader_get_chapter": {
      const seed = findChapterSeed(String(args.bookId ?? ""));
      if (!seed) throw { code: "not_found", message: "Libro no encontrado" };
      const index = Number(args.index ?? 0);
      const chapters = chaptersFor(seed);
      if (index < 0 || index >= chapters.length) {
        throw { code: "not_found", message: "Capítulo no encontrado" };
      }
      return chapterHtml(seed, index);
    }

    case "reader_update_progress": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      if (!seed) return null;
      state.progress.set(seed.id, {
        section: Number(args.chapter ?? 0),
        offset: Number(args.offset ?? 0),
        percent: Math.min(1, Math.max(0, Number(args.percent ?? 0))),
      });
      const book = state.books.find((item) => item.id === seed.id);
      if (book) {
        book.progress = Math.min(1, Math.max(0, Number(args.percent ?? 0)));
        book.lastReadAt = isoNow();
      }
      return null;
    }

    case "reader_get_settings":
      return clone(
        state.settings.get(String(args.bookId ?? "")) ??
          defaultSettings(String(args.bookId ?? "")),
      );

    case "reader_get_defaults": {
      const stored = state.settings.get("");
      return clone(stored ? { ...stored, bookId: "" } : defaultSettings(""));
    }

    case "reader_save_settings": {
      const bookId = String(args.bookId ?? "");
      const previous = state.settings.get(bookId) ?? defaultSettings(bookId);
      const next: ReaderSettings = {
        ...previous,
        ...(args.settings as ReaderSettingsInput),
        bookId,
        updatedAt: isoNow(),
      };
      state.settings.set(bookId, next);
      return clone(next);
    }

    case "reader_apply_to_all": {
      const settings = args.settings as ReaderSettingsInput;
      const previousDefaults = state.settings.get("") ?? defaultSettings("");
      state.settings.set("", {
        ...previousDefaults,
        ...settings,
        bookId: "",
        updatedAt: isoNow(),
      });
      let count = 0;
      for (const seed of demoBookSeeds) {
        const current = state.settings.get(seed.id) ?? defaultSettings(seed.id);
        state.settings.set(seed.id, {
          ...current,
          ...settings,
          bookId: seed.id,
          updatedAt: isoNow(),
        });
        count += 1;
      }
      return count;
    }

    case "reader_pdf_bytes": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      if (!seed || seed.format !== "pdf") {
        throw { code: "not_found", message: "PDF no encontrado" };
      }
      const bytes = pdfFor(seed);
      return bytes.buffer.slice(
        bytes.byteOffset,
        bytes.byteOffset + bytes.byteLength,
      );
    }

    case "get_ocr_page": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      if (!seed || seed.format !== "pdf") {
        throw { code: "not_found", message: "PDF no encontrado" };
      }
      const page = Number(args.page ?? 0);
      return {
        bookId: seed.id,
        page,
        width: 612,
        height: 792,
        source: "empty",
        words: [],
      } satisfies PageTextLayer;
    }

    case "search_pdf": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      if (!seed || seed.format !== "pdf") return [];
      const needle = normalize(String(args.query ?? ""));
      if (!needle) return [];
      const hits = [];
      for (let page = 0; page < (seed.pdfPages?.length ?? 0); page += 1) {
        const text = (seed.pdfPages?.[page] ?? []).join(" ");
        const index = normalize(text).indexOf(needle);
        if (index >= 0) {
          hits.push({
            page,
            snippet: text.slice(Math.max(0, index - 60), index + 160),
          });
        }
      }
      return hits.slice(0, Number(args.limit ?? 60));
    }

    case "ocr_pdf": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      return {
        bookId: seed?.id ?? "",
        status: "skipped",
        pagesDone: 0,
        pagesTotal: seed?.pdfPages?.length ?? 0,
        language: "spa",
        error: null,
        updatedAt: isoNow(),
        ownerAccountId: OWNER,
        originShareId: null,
      };
    }

    case "build_auto_index": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      if (!seed || seed.format !== "pdf") {
        return { bookId: "", source: "heuristic", entries: [], generatedAt: isoNow() };
      }
      const entries = (seed.pdfPages ?? []).map((lines, index) => ({
        title:
          index === 0
            ? seed.title
            : lines[0]?.trim() || `Página ${index + 1}`,
        page: index,
        level: index === 0 ? 1 : 2,
      }));
      return {
        bookId: seed.id,
        source: "heuristic",
        entries,
        generatedAt: isoNow(),
      };
    }

    case "get_auto_index":
      return null;

    case "list_annotations":
      return annotationEntries(
        String(args.bookId ?? ""),
        (args.kind as AnnotationFilter) ?? "all",
      );

    case "reader_list_annotations":
      return state.annotations
        .filter((item) => item.bookId === args.bookId)
        .map(clone);

    case "reader_add_annotation":
      return addMarker(args);

    case "add_marker":
      return addMarker(args);

    case "reader_update_annotation":
    case "update_marker": {
      const entry = state.annotations.find(
        (item) => item.id === (args.annotationId ?? args.id),
      );
      if (!entry) throw { code: "not_found", message: "Anotación no encontrada" };
      if ("color" in args) entry.color = args.color ?? entry.color;
      if ("note" in args) entry.note = cleanOptional(args.note);
      entry.updatedAt = isoNow();
      return clone(entry);
    }

    case "reader_delete_annotation":
      state.annotations = state.annotations.filter(
        (item) => item.id !== args.annotationId,
      );
      return null;

    case "add_note":
      return addNote(args);

    case "update_note": {
      const entry = state.notes.find((item) => item.id === args.noteId);
      if (!entry) throw { code: "not_found", message: "Nota no encontrada" };
      if ("title" in args) entry.title = cleanOptional(args.title);
      if ("plainText" in args && typeof args.plainText === "string") {
        entry.body = args.plainText;
      }
      if ("contentJson" in args) entry.contentJson = args.contentJson ?? null;
      entry.updatedAt = isoNow();
      return clone(entry);
    }

    case "add_extract":
      return addExtract(args);

    case "update_extract": {
      const entry = state.extracts.find((item) => item.id === args.extractId);
      if (!entry) throw { code: "not_found", message: "Extracto no encontrado" };
      if ("note" in args) entry.note = cleanOptional(args.note);
      if ("citation" in args) entry.citation = cleanOptional(args.citation);
      entry.updatedAt = isoNow();
      return clone(entry);
    }

    case "add_reaction":
      return addReaction(args);

    case "update_reaction": {
      const entry = state.reactions.find((item) => item.id === args.reactionId);
      if (!entry) throw { code: "not_found", message: "Reacción no encontrada" };
      if (args.kind) entry.kind = args.kind;
      if ("text" in args) entry.text = cleanOptional(args.text);
      entry.updatedAt = isoNow();
      return clone(entry);
    }

    case "add_voice_note":
      return addVoiceNote(args);

    case "update_voice_note": {
      const entry = state.voiceNotes.find((item) => item.id === args.noteId);
      if (!entry) throw { code: "not_found", message: "Nota de voz no encontrada" };
      const text = String(args.text ?? "").trim();
      if (!text) {
        throw { code: "invalid_input", message: "la nota de voz no tiene texto" };
      }
      entry.text = text;
      if ("transcript" in args) entry.transcript = cleanOptional(args.transcript);
      if ("quote" in args) entry.quote = cleanOptional(args.quote);
      entry.updatedAt = isoNow();
      return clone(entry);
    }

    case "start_voice_note_recording":
    case "cancel_voice_note_recording":
      return null;

    case "delete_annotation": {
      const id = String(args.id ?? "");
      const kind = args.kind as AnnotationFilter;
      if (kind === "marker") state.annotations = state.annotations.filter((item) => item.id !== id);
      if (kind === "note") state.notes = state.notes.filter((item) => item.id !== id);
      if (kind === "extract") state.extracts = state.extracts.filter((item) => item.id !== id);
      if (kind === "reaction") state.reactions = state.reactions.filter((item) => item.id !== id);
      if (kind === "voice_note") state.voiceNotes = state.voiceNotes.filter((item) => item.id !== id);
      return null;
    }

    case "search_extracts": {
      const query = normalize(String(args.query ?? ""));
      const bookId = args.bookId ? String(args.bookId) : null;
      const limit = Number(args.limit ?? 100);
      return state.extracts
        .filter(
          (entry) =>
            (!bookId || entry.bookId === bookId) &&
            (!query || normalize(entry.text).includes(query)),
        )
        .slice(0, limit)
        .map(clone);
    }

    case "reaction_map":
      return reactionBuckets(String(args.bookId ?? ""));

    case "list_library_annotations": {
      const result = [];
      for (const book of state.books) {
        for (const entry of annotationEntries(book.id, "all")) {
          result.push({
            bookId: book.id,
            bookTitle: book.title,
            entry,
          });
        }
      }
      result.sort(
        (left, right) =>
          annotationEntryTimestamp(right.entry) -
          annotationEntryTimestamp(left.entry),
      );
      return result;
    }

    case "reader_list_extracts":
      return state.extracts
        .filter((entry) => entry.bookId === args.bookId)
        .map(clone);

    case "reader_delete_extract":
      state.extracts = state.extracts.filter(
        (entry) => entry.id !== args.extractId,
      );
      return null;

    // --- Sesiones, TTS y modo ensayo --------------------------------------
    case "session_start": {
      const bookId = String(args.bookId ?? "");
      const book = state.books.find((item) => item.id === bookId);
      if (!book) throw { code: "not_found", message: "Libro no encontrado" };
      const kind = args.kind as SessionKind;
      const existing = [...demoSessions.entries()].find(
        ([, session]) => session.bookId === bookId && session.kind === kind,
      );
      if (existing) {
        const [id, session] = existing;
        return {
          id,
          bookId,
          kind,
          startedAt: new Date(session.startedAt).toISOString(),
        } satisfies SessionStatus;
      }
      const id = `session-${Date.now()}`;
      const startedAt = Date.now();
      demoSessions.set(id, { bookId, kind, startedAt });
      return {
        id,
        bookId,
        kind,
        startedAt: new Date(startedAt).toISOString(),
      } satisfies SessionStatus;
    }

    case "session_end": {
      const sessionId = String(args.sessionId ?? "");
      const active = demoSessions.get(sessionId);
      if (!active) {
        throw { code: "not_found", message: "sesión no encontrada" };
      }
      demoSessions.delete(sessionId);
      const endedAt = Date.now();
      const seconds = Math.max(0, Math.floor((endedAt - active.startedAt) / 1000));
      if (seconds < 30) return null;
      const durationMinutes = Math.max(1, Math.ceil(seconds / 60));
      const startedIso = new Date(active.startedAt).toISOString();
      const book = state.books.find((item) => item.id === active.bookId);
      const title = book?.title ?? "Libro desconocido";
      const extractsCreated = state.extracts.filter(
        (entry) => entry.bookId === active.bookId && entry.createdAt >= startedIso,
      ).length;
      const notesCreated =
        state.notes.filter(
          (entry) => entry.bookId === active.bookId && entry.createdAt >= startedIso,
        ).length +
        state.voiceNotes.filter(
          (entry) => entry.bookId === active.bookId && entry.createdAt >= startedIso,
        ).length;
      const pagesRead = Math.max(0, Number(args.pagesRead ?? 0));
      const label = active.kind === "reading" ? "lectura" : "escucha";
      const entry: DiaryEntry = {
        id: `diary-session-${Date.now()}`,
        bookId: active.bookId,
        entryDate: localDateIso(new Date(endedAt)),
        body: `Sesión de ${label} · ${durationMinutes} min · ${pagesRead} páginas\n«${title}»`,
        mood: null,
        kind: active.kind,
        sessionStart: startedIso,
        sessionEnd: new Date(endedAt).toISOString(),
        durationMinutes,
        pagesRead,
        extractsCreated,
        notesCreated,
        pomodoroRound: null,
        createdAt: isoNow(),
        updatedAt: isoNow(),
        ownerAccountId: OWNER,
        originShareId: book?.receivedShareId ?? null,
      };
      state.diary.push(entry);
      emit("diary::updated", { source: "session" });
      evaluateDemoAchievements(true);
      return clone(entry);
    }

    case "tts_engines":
      return [
        {
          id: "piper",
          name: "Piper",
          offline: true,
          voiceCloning: false,
          streaming: true,
          available: true,
          supportsPause: true,
          prefetch: false,
        },
        {
          id: "neutts",
          name: "NeuTTS",
          offline: true,
          voiceCloning: true,
          streaming: false,
          available: true,
          supportsPause: true,
          prefetch: false,
        },
      ];

    // --- Estudio de Voz ---------------------------------------------------
    case "voice_capabilities":
      return {
        recording: true,
        cloning: true,
        modelDownload: true,
        voiceDownload: true,
      } satisfies VoiceCapabilities;

    case "voice_consent_text":
      return VOICE_CONSENT_TEXT;

    case "voice_input_devices":
      return ["Micrófono predeterminado", "Micrófono USB"];

    case "voice_models":
      return voiceModelList();

    case "voice_download_models": {
      const language = String(args.language ?? "");
      if (language !== "es" && language !== "en") {
        throw { code: "invalid_input", message: "el idioma debe ser es o en" };
      }
      const ids = demoVoiceModelSpecs
        .filter((model) => model.languages.includes(language))
        .map((model) => model.id);
      await simulateModelDownload(ids);
      return null;
    }

    case "voice_download_model": {
      const modelId = String(args.modelId ?? "");
      if (!demoVoiceModelSpecs.some((model) => model.id === modelId)) {
        throw { code: "not_found", message: `el modelo ${modelId} no existe` };
      }
      await simulateModelDownload([modelId]);
      return null;
    }

    case "delete_voice_model": {
      const modelId = String(args.modelId ?? "");
      if (!demoVoiceModelSpecs.some((model) => model.id === modelId)) {
        throw { code: "not_found", message: `el modelo ${modelId} no existe` };
      }
      demoInstalledModels.delete(modelId);
      return null;
    }

    case "voice_sample_start": {
      if (demoRecording) {
        throw { code: "conflict", message: "ya hay una grabación en curso" };
      }
      demoRecording = {
        startedAt: Date.now(),
        deviceName: args.deviceName ? String(args.deviceName) : null,
      };
      return null;
    }

    case "voice_sample_level": {
      if (!demoRecording) return 0;
      const elapsed = (Date.now() - demoRecording.startedAt) / 1000;
      return Math.min(0.92, 0.32 + Math.abs(Math.sin(elapsed * 3.1)) * 0.48);
    }

    case "voice_sample_stop": {
      if (!demoRecording) {
        throw { code: "invalid_input", message: "no hay una grabación activa" };
      }
      const startedAt = demoRecording.startedAt;
      demoRecording = null;
      const durationMs = Math.max(
        400,
        Math.min(30_000, Date.now() - startedAt),
      );
      const path = demoVoiceSamplePath(
        `recording-${Date.now()}`,
        durationMs,
      );
      return {
        path,
        durationMs,
        warnings: durationMs > 29_500 ? ["trimmed"] : [],
      } satisfies VoiceSample;
    }

    case "voice_sample_cancel": {
      demoRecording = null;
      return null;
    }

    case "voice_sample_discard": {
      const path = String(args.path ?? "");
      demoAudioDurations.delete(path);
      demoAudioCache.delete(path);
      return null;
    }

    case "create_voice_profile": {
      if (!args.consent) {
        throw {
          code: "invalid_input",
          message: "debes aceptar que solo clonas voces propias o con permiso",
        };
      }
      const name = String(args.name ?? "").trim();
      if (!name || name.length > 80) {
        throw {
          code: "invalid_input",
          message: "el nombre del perfil debe tener entre 1 y 80 caracteres",
        };
      }
      const language = String(args.language ?? "").trim().toLowerCase();
      if (language !== "es" && language !== "en") {
        throw {
          code: "unsupported",
          message: `la clonación de voz solo está disponible en español e inglés; se pidió ${language}`,
        };
      }
      const referenceText = String(args.referenceText ?? "").trim();
      if (!referenceText) {
        throw {
          code: "invalid_input",
          message: "escribe lo que dice el clip de referencia",
        };
      }
      if (referenceText.length > 2_000) {
        throw {
          code: "invalid_input",
          message: "la transcripción de referencia es demasiado larga",
        };
      }
      const audioPath = String(args.audioPath ?? "");
      if (!audioPath) {
        throw { code: "not_found", message: "no se encontró el audio" };
      }
      const durationMs = demoAudioDurations.get(audioPath) ?? 4_200;
      if (durationMs < 3_000 || durationMs > 30_000) {
        throw {
          code: "invalid_input",
          message: `el clip debe durar entre 3 y 30 segundos; dura ${(durationMs / 1000).toFixed(1)} s`,
        };
      }
      const id = demoVoiceId();
      const profile: VoiceProfile = {
        id,
        name,
        engine: voiceEngineForLanguage(language),
        language,
        inputFormat: voiceInputFormatForLanguage(language),
        samplePath: demoVoiceSamplePath(`profile-${id}`, durationMs),
        referenceDurationMs: durationMs,
        createdAt: isoNow(),
      };
      demoVoiceProfiles.unshift(profile);
      return clone(profile);
    }

    case "list_voice_profiles":
      return demoVoiceProfiles.map(clone);

    case "delete_voice_profile": {
      const profile = voiceProfileById(String(args.voiceId ?? ""));
      if (!profile) {
        throw { code: "not_found", message: "el perfil de voz no existe" };
      }
      demoVoiceProfiles = demoVoiceProfiles.filter(
        (item) => item.id !== profile.id,
      );
      if (profile.samplePath) {
        demoAudioDurations.delete(profile.samplePath);
        demoAudioCache.delete(profile.samplePath);
      }
      return null;
    }

    case "update_voice_profile": {
      const profile = voiceProfileById(String(args.voiceId ?? ""));
      if (!profile) {
        throw { code: "not_found", message: "el perfil de voz no existe" };
      }
      const name = String(args.name ?? "").trim();
      if (!name || name.length > 80) {
        throw {
          code: "invalid_input",
          message: "el nombre del perfil debe tener entre 1 y 80 caracteres",
        };
      }
      profile.name = name;
      return clone(profile);
    }

    case "voice_profile_sample": {
      const profile = voiceProfileById(String(args.voiceId ?? ""));
      if (!profile) {
        throw { code: "not_found", message: "el perfil de voz no existe" };
      }
      if (!profile.samplePath) {
        throw {
          code: "not_found",
          message: "el perfil no tiene una muestra de audio",
        };
      }
      return profile.samplePath;
    }

    case "replace_voice_sample": {
      const profile = voiceProfileById(String(args.voiceId ?? ""));
      if (!profile) {
        throw { code: "not_found", message: "el perfil de voz no existe" };
      }
      if (!args.consent) {
        throw {
          code: "invalid_input",
          message: "debes aceptar que solo clonas voces propias o con permiso",
        };
      }
      const referenceText = String(args.referenceText ?? "").trim();
      if (!referenceText || referenceText.length > 2_000) {
        throw {
          code: "invalid_input",
          message: "escribe lo que dice el nuevo clip de referencia",
        };
      }
      const audioPath = String(args.audioPath ?? "");
      if (!audioPath) {
        throw { code: "not_found", message: "no se encontró el audio" };
      }
      const durationMs = demoAudioDurations.get(audioPath) ?? 4_200;
      if (durationMs < 3_000 || durationMs > 30_000) {
        throw {
          code: "invalid_input",
          message: `el clip debe durar entre 3 y 30 segundos; dura ${(durationMs / 1000).toFixed(1)} s`,
        };
      }
      if (profile.samplePath) {
        demoAudioDurations.delete(profile.samplePath);
        demoAudioCache.delete(profile.samplePath);
      }
      profile.samplePath = demoVoiceSamplePath(
        `profile-${profile.id}-${Date.now()}`,
        durationMs,
      );
      profile.referenceDurationMs = durationMs;
      return clone(profile);
    }

    case "export_voice_profile": {
      const profile = voiceProfileById(String(args.voiceId ?? ""));
      if (!profile) {
        throw { code: "not_found", message: "el perfil de voz no existe" };
      }
      if (!String(args.destination ?? "").trim()) {
        throw { code: "cancelled", message: "exportación cancelada" };
      }
      return null;
    }

    case "import_voice_profile": {
      if (!args.consent) {
        throw {
          code: "invalid_input",
          message: "debes aceptar que solo clonas voces propias o con permiso",
        };
      }
      const source = String(args.source ?? "");
      if (!source.toLowerCase().endsWith(".koravoice")) {
        throw {
          code: "invalid_input",
          message: "el archivo no es un perfil .koravoice válido",
        };
      }
      const base = source
        .split(/[/\\]/)
        .pop()
        ?.replace(/\.koravoice$/i, "")
        .replace(/[-_]+/g, " ")
        .trim();
      const name = base || "Voz importada";
      const id = demoVoiceId();
      const profile: VoiceProfile = {
        id,
        name,
        engine: "neutts-nano",
        language: "es",
        inputFormat: "text",
        samplePath: demoVoiceSamplePath(`profile-${id}`, 7_350),
        referenceDurationMs: 7_350,
        createdAt: isoNow(),
      };
      demoVoiceProfiles.unshift(profile);
      return clone(profile);
    }

    case "tts_voices":
      return voiceDescriptorList();

    case "tts_download_voice": {
      const voice = demoVoices.find((item) => item.id === args.voiceId);
      if (!voice) {
        throw { code: "not_found", message: "la voz no existe" };
      }
      await simulateVoiceDownload(voice);
      return null;
    }

    case "tts_status":
      return ttsStatus();

    case "start_tts":
      return startTts(
        String(args.bookId ?? ""),
        args.fromOffset as { section: number; offset: number; end?: number },
      );

    case "pause_tts":
      return pauseTts();

    case "resume_tts":
      return resumeTts();

    case "stop_tts":
      return stopTts();

    case "set_rate": {
      tts.status.rate = Number(args.rate ?? 1);
      return ttsStatus();
    }

    case "set_pitch": {
      tts.status.pitch = Number(args.pitch ?? 1);
      return ttsStatus();
    }

    case "tts_preferences":
      return ttsPreferences(String(args.bookId ?? ""));

    case "save_tts_preferences": {
      const bookId = String(args.bookId ?? "");
      const current = ttsPreferences(bookId);
      state.ttsPreferences.set(bookId, {
        ...current,
        ...(args.preferences as Partial<TtsPreferences>),
        bookId,
        updatedAt: isoNow(),
      });
      return clone(state.ttsPreferences.get(bookId));
    }

    case "tts_characters": {
      const seed = findBookSeed(String(args.bookId ?? ""));
      return seed ? (bookCharacters[seed.id] ?? []).map(clone) : [];
    }

    case "tts_character_voices": {
      const bookId = String(args.bookId ?? "");
      return [...state.voiceCharacters.entries()]
        .filter(([key]) => key.startsWith(`${bookId}:`))
        .map(([key, voiceId]) => ({
          bookId,
          character: key.slice(bookId.length + 1),
          voiceId,
          updatedAt: isoNow(),
          ownerAccountId: OWNER,
          originShareId: null,
        })) satisfies CharacterVoice[];
    }

    case "assign_character_voice": {
      const bookId = String(args.bookId ?? "");
      const character = String(args.character ?? "");
      const voiceId = String(args.voiceId ?? "");
      state.voiceCharacters.set(`${bookId}:${character}`, voiceId);
      return {
        bookId,
        character,
        voiceId,
        updatedAt: isoNow(),
        ownerAccountId: OWNER,
        originShareId: null,
      } satisfies CharacterVoice;
    }

    case "practice_models":
      return [
        {
          id: "tiny",
          name: "Whisper Tiny",
          installed: true,
          sizeBytes: 75_200_000,
        },
        {
          id: "base",
          name: "Whisper Base",
          installed: true,
          sizeBytes: 142_000_000,
        },
      ] satisfies WhisperModelInfo[];

    case "practice_download_model": {
      emit("tts-practice-download-progress", {
        modelId: args.modelId,
        downloaded: 100,
        total: 100,
        done: true,
        error: null,
      });
      return null;
    }

    case "practice_history":
      return state.practiceHistory.map(clone);

    case "practice_start": {
      state.practice = {
        bookId: String(args.bookId ?? ""),
        section: Number(args.section ?? 0),
        offsetStart: Number(args.offsetStart ?? 0),
        offsetEnd: Number(args.offsetEnd ?? 0),
        expected: String(args.expected ?? ""),
        startedAt: Date.now(),
      };
      return null;
    }

    case "practice_stop":
      return practiceStop(String(args.modelId ?? "tiny"));

    case "practice_cancel":
      state.practice = null;
      return null;

    // --- IA local ---------------------------------------------------------
    case "ai_status":
      return {
        provider: "ollama",
        configuredProvider: "auto",
        available: true,
        baseUrl: "http://localhost:11434",
        model: "llama3.2:3b",
        models: ["llama3.2:3b", "qwen2.5:7b"],
        error: null,
      } satisfies AiStatus;

    case "ai_chat":
      return aiReply((args.messages as AiMessage[]) ?? []);

    case "ai_get_config":
      return clone(demoAiConfig);

    case "ai_save_config": {
      const config = args.config as Partial<AiConfig> | undefined;
      if (config) {
        demoAiConfig = {
          ...demoAiConfig,
          ...config,
          temperature: Math.min(
            2,
            Math.max(0, Number(config.temperature ?? demoAiConfig.temperature)),
          ),
          maxTokens: Math.min(
            8192,
            Math.max(64, Math.round(Number(config.maxTokens ?? demoAiConfig.maxTokens))),
          ),
        };
      }
      return clone(demoAiConfig);
    }

    // --- Espejo y sync ----------------------------------------------------
    case "get_mirror_status":
      return clone(mirror);

    case "get_peers":
    case "sync_peers":
      return clone(demoPeers);

    case "get_sync_status":
    case "sync_status":
      demoSyncStatus.mirrorActive = mirror.active;
      demoSyncStatus.mirrorHosting = mirror.hosting;
      return clone(demoSyncStatus);

    case "start_sync":
    case "sync_start":
      return startDemoSync();

    case "stop_sync":
      return stopDemoSync();

    case "start_share":
      return startDemoShare();

    case "stop_share":
      demoSyncStatus.sharing = false;
      emitSyncStatus();
      return clone(demoSyncStatus);

    case "start_device_pairing":
      return {
        code: "482913",
        qrSvg: shareQr(482),
        uri: "kora://pair?v=2&t=demo&c=482913&h=192.168.1.10&p=47821&f=demo",
        expiresAt: Math.floor(Date.now() / 1000) + 120,
        addresses: ["192.168.1.10"],
        port: 47_821,
        deviceName: "Kora de demo",
      } satisfies PairingTicketView;

    case "join_device_pairing": {
      const device = {
        id: "peer-demo-new",
        name: "Dispositivo de demo",
        platform: "android",
        fingerprint: "demo",
        accountHint: "@demo",
      } satisfies PairedDeviceInfo;
      if (!demoPeers.some((peer) => peer.id === device.id)) {
        demoPeers.push({
          id: device.id,
          name: device.name,
          platform: device.platform,
          address: "192.168.1.77",
          addresses: ["192.168.1.77"],
          port: 47_821,
          lastSeen: isoNow(),
          trusted: true,
          accountHint: device.accountHint,
          purpose: "sync",
          sasPrefix: "",
          fingerprint: device.fingerprint,
          fullname: device.name,
          protocolVersion: 2,
          ticket: null,
        });
        demoAccountDevices = seedDemoDevices(demoPeers);
      }
      emit("sync::paired", device);
      emitSyncStatus();
      return device;
    }

    case "cancel_device_pairing":
      demoSyncStatus.pairing = false;
      return emitSyncStatus();

    case "set_auto_sync":
      demoSyncStatus.autoSync = Boolean(args.enabled);
      return emitSyncStatus();

    case "sync_now_all": {
      if (!demoSyncStatus.running) startDemoSync();
      const outcomes = demoPeers
        .filter((peer) => peer.trusted)
        .map((peer) => ({
          peerId: peer.id,
          sent: 4,
          received: 3,
          durationMs: 420,
        }));
      demoSyncStatus.lastSync = isoNow();
      demoSyncStatus.pendingChanges = 0;
      emitSyncStatus();
      return outcomes;
    }

    case "get_sync_diagnostics":
      return {
        mdnsActive: true,
        listenPort: demoSyncStatus.listenPort || 47_821,
        localAddresses: ["192.168.1.10"],
        autoSync: demoSyncStatus.autoSync,
        trustedPeers: demoPeers.filter((peer) => peer.trusted).length,
        onlinePeers: demoPeers.length,
        lastError: null,
      } satisfies SyncDiagnostics;

    case "sync_now": {
      const peerId = String(args.peerId ?? "");
      const peer = demoPeers.find((item) => item.id === peerId && item.trusted);
      if (!peer) {
        throw { code: "not_found", message: "dispositivo de sync no encontrado" };
      }
      if (!demoSyncStatus.running) startDemoSync();
      const sent = 5 + (peerId.length % 4);
      const received = 3 + (peerId.length % 3);
      const durationMs = 380 + (peerId.length % 5) * 90;
      demoSyncStatus.lastSync = isoNow();
      demoSyncStatus.pendingChanges = 0;
      emitSyncStatus();
      emit("sync::completed", {
        peerId,
        sent,
        received,
        durationMs,
      });
      return { peerId, sent, received, durationMs };
    }

    case "revoke_peer":
      revokeDemoPeer(String(args.deviceId ?? ""));
      return null;

    case "list_sync_devices":
      return demoAccountDevices
        .filter((device) => device.isLocal || device.trusted)
        .map(syncDevice)
        .sort((left, right) => {
          if (left.isLocal !== right.isLocal) return left.isLocal ? -1 : 1;
          return left.name.localeCompare(right.name);
        });

    case "get_sync_collections": {
      const device = syncDeviceById(args.deviceId ? String(args.deviceId) : null);
      if (!device) {
        throw { code: "not_found", message: "dispositivo de sincronización" };
      }
      const selection = demoSyncSelections.get(device.id) ?? {
        categoryIds: [],
        syncAll: true,
      };
      return {
        deviceId: device.id,
        deviceName: device.name,
        syncAll: selection.syncAll,
        categories: state.categories.map((category) => ({
          category: clone(category),
          enabled:
            selection.syncAll || selection.categoryIds.includes(category.id),
        })),
      };
    }

    case "set_sync_categories": {
      const device = syncDeviceById(args.deviceId ? String(args.deviceId) : null);
      if (!device) {
        throw { code: "not_found", message: "dispositivo de sincronización" };
      }
      const syncAll = Boolean(args.syncAll);
      const categoryIds = syncAll
        ? []
        : ((args.categoryIds as string[]) ?? []).filter((categoryId) =>
            state.categories.some((category) => category.id === categoryId),
          );
      demoSyncSelections.set(device.id, { categoryIds, syncAll });
      return {
        deviceId: device.id,
        deviceName: device.name,
        syncAll,
        categories: state.categories.map((category) => ({
          category: clone(category),
          enabled: syncAll || categoryIds.includes(category.id),
        })),
      };
    }

    case "set_sync_collections": {
      const local = demoAccountDevices.find((device) => device.isLocal);
      if (!local) {
        throw { code: "not_found", message: "dispositivo local" };
      }
      const syncAll = Boolean(args.syncAll);
      const categoryIds = syncAll
        ? []
        : ((args.categoryIds as string[]) ?? []).filter((categoryId) =>
            state.categories.some((category) => category.id === categoryId),
          );
      demoSyncSelections.set(local.id, { categoryIds, syncAll });
      return {
        deviceId: local.id,
        deviceName: local.name,
        syncAll,
        categories: state.categories.map((category) => ({
          category: clone(category),
          enabled: syncAll || categoryIds.includes(category.id),
        })),
      };
    }

    case "list_audit_log": {
      const limit = Math.min(
        500,
        Math.max(1, Number(args.limit ?? 50)),
      );
      return clone(
        [...demoAudit]
          .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
          .slice(0, limit),
      );
    }

    case "export_audit_log": {
      const destination = String(args.destination ?? "kora-auditoria.json");
      return destination;
    }

    case "start_mirror": {
      mirror = {
        active: true,
        hosting: true,
        bookId: String(args.bookId ?? ""),
        hostDeviceId: null,
        followers: ["Portátil de Marta"],
      };
      emitSyncStatus();
      return clone(mirror);
    }

    case "stop_mirror":
    case "leave_mirror":
      mirror = {
        active: false,
        hosting: false,
        bookId: null,
        hostDeviceId: null,
        followers: [],
      };
      emitSyncStatus();
      return clone(mirror);

    case "join_mirror": {
      mirror = {
        active: true,
        hosting: false,
        bookId: String(args.bookId ?? ""),
        hostDeviceId: String(args.peerId ?? ""),
        followers: [],
      };
      emitSyncStatus();
      return clone(mirror);
    }

    // --- Diario y ajustes -------------------------------------------------
    case "get_diary": {
      const now = new Date();
      const year = Number(args.year ?? now.getFullYear());
      const month = Number(args.month ?? now.getMonth() + 1);
      if (!Number.isInteger(year) || year < 1970 || year > 2100) {
        throw { code: "invalid_input", message: "año fuera de rango" };
      }
      if (!Number.isInteger(month) || month < 1 || month > 12) {
        throw { code: "invalid_input", message: "mes fuera de rango" };
      }
      return diaryMonth(year, month);
    }

    case "diary_create": {
      const entryDate = String(args.entryDate ?? "");
      const parsedDate = new Date(`${entryDate}T12:00:00`);
      if (
        !/^\d{4}-\d{2}-\d{2}$/.test(entryDate) ||
        Number.isNaN(parsedDate.getTime()) ||
        localDateIso(parsedDate) !== entryDate
      ) {
        throw {
          code: "invalid_input",
          message: "la fecha del diario debe usar AAAA-MM-DD",
        };
      }
      const body = String(args.body ?? "").trim();
      if (!body) {
        throw {
          code: "invalid_input",
          message: "la entrada del diario no puede estar vacía",
        };
      }
      if (body.length > 20_000) {
        throw {
          code: "invalid_input",
          message: "la entrada del diario no puede superar 20000 caracteres",
        };
      }
      const mood = cleanOptional(args.mood);
      if (mood && mood.length > 40) {
        throw {
          code: "invalid_input",
          message: "el estado de ánimo no puede superar 40 caracteres",
        };
      }
      const entry: DiaryEntry = {
        id: `diary-${Date.now()}`,
        bookId: null,
        entryDate,
        body,
        mood,
        kind: "manual",
        sessionStart: null,
        sessionEnd: null,
        durationMinutes: 0,
        pagesRead: 0,
        extractsCreated: 0,
        notesCreated: 0,
        pomodoroRound: null,
        createdAt: isoNow(),
        updatedAt: isoNow(),
        ownerAccountId: OWNER,
        originShareId: null,
      };
      state.diary.push(entry);
      emit("diary::updated", { source: "manual" });
      return clone(entry);
    }

    case "diary_update": {
      const entry = state.diary.find((item) => item.id === args.entryId);
      if (!entry) {
        throw { code: "not_found", message: "entrada de diario no encontrada" };
      }
      if (entry.kind !== "manual") {
        throw {
          code: "invalid_input",
          message: "solo las notas manuales se pueden editar",
        };
      }
      const body = String(args.body ?? "").trim();
      if (!body) {
        throw {
          code: "invalid_input",
          message: "la entrada del diario no puede estar vacía",
        };
      }
      if (body.length > 20_000) {
        throw {
          code: "invalid_input",
          message: "la entrada del diario no puede superar 20000 caracteres",
        };
      }
      const mood = cleanOptional(args.mood);
      if (mood && mood.length > 40) {
        throw {
          code: "invalid_input",
          message: "el estado de ánimo no puede superar 40 caracteres",
        };
      }
      entry.body = body;
      entry.mood = mood;
      entry.updatedAt = isoNow();
      emit("diary::updated", { source: "manual" });
      return clone(entry);
    }

    case "diary_delete": {
      const index = state.diary.findIndex((item) => item.id === args.entryId);
      if (index < 0) {
        throw { code: "not_found", message: "entrada de diario no encontrada" };
      }
      state.diary.splice(index, 1);
      emit("diary::updated", { source: "manual" });
      return null;
    }

    case "get_achievements":
      return evaluateDemoAchievements(true);

    case "get_pomodoro":
      return pomodoroStatus();

    case "start_pomodoro":
      return startDemoPomodoro((args.config as PomodoroConfig | null) ?? null);

    case "stop_pomodoro":
      return stopDemoPomodoro();

    case "get_audiobook_progress":
      return null;

    case "export_notes": {
      return {
        path: String(args.destination ?? "/demo/export/notas.md"),
        books: 1,
        entries:
          state.notes.length + state.extracts.length + state.annotations.length,
      } satisfies NotesExportResult;
    }

    case "export_diary": {
      const year = Number(args.year ?? new Date().getFullYear());
      const month = args.month === null || args.month === undefined ? null : Number(args.month);
      const prefix = month === null ? `${year}-` : `${year}-${String(month).padStart(2, "0")}-`;
      const entries = state.diary.filter((entry) =>
        entry.entryDate.startsWith(prefix),
      ).length;
      const suffix = month === null ? `${year}` : `${year}-${String(month).padStart(2, "0")}`;
      return {
        epubPath: `/demo/export/diario-${suffix}.epub`,
        audiobookPath: args.includeAudiobook
          ? `/demo/export/diario-${suffix}.m4b`
          : null,
        entries,
      } satisfies DiaryExport;
    }

    // --- Diálogos y ventana ----------------------------------------------
    case "plugin:dialog|open": {
      const extensions: string[] =
        args.options?.filters?.[0]?.extensions ?? [];
      if (extensions.some((extension: string) => /zip/i.test(extension))) {
        return "demo/kora-copia-de-seguridad.zip";
      }
      if (extensions.some((extension: string) => /koravoice/i.test(extension))) {
        return "demo/Voz de ejemplo.koravoice";
      }
      if (extensions.some((extension: string) => /jpg|jpeg|png|webp/i.test(extension))) {
        return "demo/portada-elegida.jpg";
      }
      if (extensions.some((extension: string) => /m4b|m4a|mp3|aac|flac|ogg|opus|wav/i.test(extension))) {
        return extensions.some((extension: string) => /wav/i.test(extension))
          ? "demo/Conversaciones en el tren.wav"
          : "demo/Conversaciones en el tren.m4b";
      }
      if (extensions.some((extension: string) => /pdf/i.test(extension))) {
        return "demo/La ciudad y sus fantasmas.pdf";
      }
      return "demo/La isla de los ecos.epub";
    }

    case "plugin:dialog|save": {
      const defaultPath =
        typeof args.options?.defaultPath === "string"
          ? args.options.defaultPath
          : "export.bin";
      return defaultPath.startsWith("/")
        ? defaultPath
        : `/demo/export/${defaultPath}`;
    }

    case "plugin:window|set_fullscreen":
      fullscreen = Boolean(args.value);
      return null;

    case "plugin:window|is_fullscreen":
      return fullscreen;

    case "plugin:event|listen": {
      const event = String(args.event ?? "");
      const id = Number(args.handler ?? 0);
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event)?.add(id);
      return id;
    }

    case "plugin:event|unlisten": {
      const event = String(args.event ?? "");
      listeners.get(event)?.delete(Number(args.eventId ?? 0));
      return null;
    }

    case "plugin:event|emit":
      emit(String(args.event ?? ""), args.payload);
      return null;

    case "plugin:opener|open_url":
    case "plugin:shell|open":
      return null;

    default:
      if (command.endsWith("_status") || command.endsWith("_config")) {
        return null;
      }
      if (
        command.startsWith("list_") ||
        command.endsWith("_list") ||
        command.endsWith("_voices") ||
        command.endsWith("_models") ||
        command.endsWith("_peers") ||
        command.endsWith("_achievements") ||
        command.endsWith("_shares") ||
        command.endsWith("_devices")
      ) {
        return [];
      }
      return null;
  }
}

function coverDataUrl(seed: DemoBookSeed): string {
  return coverOverrides.get(seed.id) ?? coverDataUri(seed);
}

let appearance: AppearanceState;

function appearanceState(): AppearanceState {
  return appearance;
}

function installDemoBackend() {
  const anyWindow = window as unknown as {
    __TAURI_INTERNALS__?: unknown;
    __TAURI_EVENT_PLUGIN_INTERNALS__?: unknown;
    __KORA_DEMO__?: unknown;
  };
  if (anyWindow.__TAURI_INTERNALS__) return;

  buildInitialState();
  audiobooks = initialAudiobooks();
  demoAlbums = [
    {
      id: "album-mar",
      title: "Trilogía del mar",
      description: "Las historias de la costa, en orden de escucha.",
      sortOrder: 0,
      createdAt: daysAgo(9),
      updatedAt: daysAgo(2),
      audiobookIds: ["audiobook-mareas", "audiobook-cartas"],
    },
    {
      id: "album-taller",
      title: "Taller de escritura",
      description: "Lecturas breves para acompañar las sesiones de trabajo.",
      sortOrder: 1,
      createdAt: daysAgo(5),
      updatedAt: daysAgo(1),
      audiobookIds: ["audiobook-poemas", "audiobook-jardin"],
    },
  ];
  demoSessions = new Map();
  resetVoiceStudio();
  resetSyncAndSettingsDemo();
  achievementProgressStore = new Map();
  achievementUnlockedStore = new Map();
  const completedPomodoros = state.diary.filter(
    (entry) => entry.kind === "pomodoro",
  ).length;
  const initialConfig = defaultPomodoroConfig();
  pomodoro = {
    state: "idle",
    startedAt: null,
    endsAt: null,
    round: (completedPomodoros % initialConfig.roundsBeforeLongBreak) + 1,
    completedFocus: completedPomodoros,
    config: initialConfig,
    generation: 0,
  };
  evaluateDemoAchievements(false);
  audiobookPlayback = initialPlayback(audiobooks);
  audiobookPlaybackBase = audiobookPlayback?.positionMs ?? 0;
  audiobookPlaybackStartedAt = Date.now();
  audiobookSpeed = 1;
  generation = { active: null, queue: [] };
  generationPercent = 0;
  queuedActiveData = null;
  voiceMarkerPhrase = "Kora, marca esto";
  voiceMarkerListening = false;
  tts = {
    status: {
      bookId: null,
      section: null,
      state: "idle",
      rate: 1,
      pitch: 1,
      chunkIndex: null,
      phraseStart: null,
      phraseEnd: null,
      error: null,
      excerpt: false,
      media: null,
    },
    text: "",
    words: [],
    unit: "utf16",
    cursor: 0,
    end: null,
  };
  mirror = {
    active: false,
    hosting: false,
    bookId: null,
    hostDeviceId: null,
    followers: [],
  };
  appearance = {
    preferences: {
      ownerAccountId: OWNER,
      themeId: "light",
      texture: "paper",
      locale: "es",
      onboardingDone: true,
      libraryLayout: "grid",
      updatedAt: daysAgo(12),
    } satisfies AppPreferences,
    themes: [],
  };

  anyWindow.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (
      callback: (message: unknown) => void,
      once = false,
    ) => {
      const id = callbackSeq++;
      callbacks.set(id, (message) => {
        callback(message);
        if (once) {
          callbacks.delete(id);
          for (const set of listeners.values()) set.delete(id);
        }
      });
      return id;
    },
    unregisterCallback: (id: number) => {
      callbacks.delete(id);
      for (const set of listeners.values()) set.delete(id);
    },
    convertFileSrc: (path: string) => {
      if (path.startsWith("demo-pdf:")) {
        return pdfDataUrl(path.slice("demo-pdf:".length));
      }
      if (/\.(wav|mp3|m4a|m4b|flac|ogg|opus)$/i.test(path)) {
        return demoAudioDataUrl(path);
      }
      return path;
    },
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main" },
    },
  };
  anyWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => {
      listeners.get(event)?.delete(id);
    },
  };
  anyWindow.__KORA_DEMO__ = {
    installed: true,
    state,
    invoke,
    reset: () => {
      window.location.reload();
    },
  };
  console.info(
    "Kora: modo demostración activo. Los datos son simulados y se pierden al recargar.",
  );
}

export { installDemoBackend };

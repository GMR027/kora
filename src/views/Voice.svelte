<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import {
    createVoiceProfile,
    deleteVoiceModel,
    deleteVoiceProfile,
    exportVoiceProfile,
    importVoiceProfile,
    listVoiceProfiles,
    replaceVoiceSample,
    ttsDownloadVoice,
    ttsSynthesize,
    ttsVoices,
    voiceCapabilities,
    voiceClonePreview,
    voiceConsentText,
    voiceDownloadModel,
    voiceDownloadModels,
    voiceInputDevices,
    voiceModels,
    voiceProfileSample,
    voiceSampleCancel,
    voiceSampleDiscard,
    voiceSampleLevel,
    voiceSampleStart,
    voiceSampleStop,
    updateVoiceProfile,
  } from "../lib/api";
  import {
    formatBytes,
    formatDate,
    formatDuration,
    t,
    translate,
  } from "../lib/i18n";
  import { notifyError, notifySuccess } from "../lib/toast";
  import Button from "../lib/components/Button.svelte";
  import EmptyState from "../lib/components/EmptyState.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import type {
    SynthesisOutput,
    VoiceCapabilities,
    VoiceDescriptor,
    VoiceDownloadProgress,
    VoiceModelInfo,
    VoiceModelProgress,
    VoiceProfile,
    VoiceSample,
  } from "../lib/types";

  type VoiceTab = "system" | "cloned" | "models";
  type GenderFilter = "all" | "male" | "female" | "neutral";
  type DetailsTarget =
    | { kind: "voice"; voice: VoiceDescriptor }
    | { kind: "profile"; profile: VoiceProfile };

  const PREVIEW_RATE_KEY = "kora.voice.previewRate";
  const PREVIEW_TEXT_KEY = "kora.voice.previewText";
  const FEATURED_VOICE_KEY = "kora.voice.featuredVoice";
  const FEATURED_PROFILE_KEY = "kora.voice.featuredProfile";
  const LATENCY_HISTORY_KEY = "kora.voice.latencyHistory";
  const INPUT_DEVICE_KEY = "kora.voice.inputDevice";
  const LATENCY_HISTORY_LIMIT = 10;
  const previewRates = [1, 1.2, 1.5];

  let tab = $state<VoiceTab>("system");
  let voices = $state<VoiceDescriptor[]>([]);
  let models = $state<VoiceModelInfo[]>([]);
  let profiles = $state<VoiceProfile[]>([]);
  let consentText = $state("");
  let loading = $state(true);
  let busy = $state<string | null>(null);
  let progress = $state<Record<string, number>>({});
  let voiceProgress = $state<Record<string, number>>({});
  let capabilities = $state<VoiceCapabilities>({
    recording: true,
    cloning: true,
    modelDownload: true,
    voiceDownload: true,
  });

  let audioPath = $state("");
  let recording = $state(false);
  let recordingSeconds = $state(0);
  let recordedSample = $state<VoiceSample | null>(null);
  let importedPath = $state("");
  let importedDurationMs = $state<number | null>(null);
  let inputDevices = $state<string[]>([]);
  let selectedDevice = $state("");
  let inputLevel = $state(0);
  let sessionPeak = $state(0);
  let levelTimer: ReturnType<typeof setInterval> | null = null;
  let samplePlaying = $state(false);
  let samplePlayer: HTMLAudioElement | null = null;
  let recordingTimer: ReturnType<typeof setInterval> | null = null;
  let profileName = $state("");
  let profileLanguage = $state("es");
  let referenceText = $state("");
  let consent = $state(false);
  let importConsent = $state(false);
  let player: HTMLAudioElement | null = null;
  let unlisten: UnlistenFn | undefined;
  let unlistenVoice: UnlistenFn | undefined;
  let nameInput = $state<HTMLInputElement | null>(null);

  let selectedLanguage = $state("all");
  let genderFilter = $state<GenderFilter>("all");
  let onlyInstalled = $state(false);
  let searchQuery = $state("");
  let profileQuery = $state("");
  let playingId = $state<string | null>(null);
  let previewDuration = $state(0);
  let previewRate = $state(1);
  let previewText = $state("");
  let customPreviewOpen = $state(false);
  let featuredVoiceId = $state("");
  let featuredProfileId = $state("");
  let latencyHistory = $state<number[]>([]);
  let lastLatencyMs = $derived(
    latencyHistory.length > 0 ? latencyHistory[latencyHistory.length - 1] : null,
  );
  let averageLatencyMs = $derived(
    latencyHistory.length > 0
      ? Math.round(
          latencyHistory.reduce((sum, value) => sum + value, 0) /
            latencyHistory.length,
        )
      : null,
  );
  let details = $state<DetailsTarget | null>(null);
  let detailsDialog = $state<HTMLDialogElement | null>(null);
  let detailsTrigger: HTMLElement | null = null;
  let renamingProfile = $state(false);
  let renameValue = $state("");
  let replacingSample = $state(false);
  let replacePath = $state("");
  let replaceDurationMs = $state<number | null>(null);
  let replaceReferenceText = $state("");
  let replaceConsent = $state(false);
  let replacePlaying = $state(false);
  let replacePlayer: HTMLAudioElement | null = null;
  let detailsSamplePlaying = $state(false);
  let detailsSampleId = $state<string | null>(null);
  let detailsSamplePlayer: HTMLAudioElement | null = null;
  let rememberedDurations = $state<Record<string, number>>({});

  const waveBars = Array.from({ length: 28 }, (_, index) => index);

  let systemVoices = $derived(voices.filter((voice) => !voice.cloned));
  let clonedVoices = $derived(voices.filter((voice) => voice.cloned));
  let languages = $derived(
    [...new Set(models.flatMap((model) => model.languages))].sort(),
  );
  let voiceLanguages = $derived(
    [...new Set(systemVoices.map((voice) => voice.language))].sort(),
  );
  let installedModels = $derived(
    models.filter((model) => model.installed).length,
  );
  let pendingModels = $derived(models.length - installedModels);
  let modelsReadyPercent = $derived(
    models.length > 0 ? Math.round((installedModels / models.length) * 100) : 0,
  );
  let normalizedQuery = $derived(searchQuery.trim().toLowerCase());
  let normalizedProfileQuery = $derived(profileQuery.trim().toLowerCase());
  let filteredSystemVoices = $derived(
    systemVoices.filter(
      (voice) =>
        (selectedLanguage === "all" ||
          voice.language === selectedLanguage) &&
        (!onlyInstalled || voice.installed) &&
        (genderFilter === "all" ||
          (voice.gender ?? "neutral") === genderFilter) &&
        (!normalizedQuery || voice.name.toLowerCase().includes(normalizedQuery)),
    ),
  );
  let featuredVoice = $derived(
    filteredSystemVoices.find((voice) => voice.id === featuredVoiceId) ??
      filteredSystemVoices.find((voice) => voice.installed) ??
      filteredSystemVoices[0] ??
      null,
  );
  let otherSystemVoices = $derived(
    filteredSystemVoices.filter((voice) => voice.id !== featuredVoice?.id),
  );
  let engineComponents = $derived(
    models.filter((model) => model.kind !== "backbone"),
  );
  let backboneModels = $derived(
    models.filter((model) => model.kind === "backbone"),
  );
  let filteredProfiles = $derived(
    profiles.filter(
      (profile) =>
        !normalizedProfileQuery ||
        profile.name.toLowerCase().includes(normalizedProfileQuery),
    ),
  );
  let featuredProfile = $derived(
    filteredProfiles.find((profile) => profile.id === featuredProfileId) ??
      [...filteredProfiles].sort(
        (a, b) =>
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
      )[0] ??
      null,
  );
  let otherProfiles = $derived(
    filteredProfiles.filter((profile) => profile.id !== featuredProfile?.id),
  );
  let sampleDurationMs = $derived(
    recordedSample?.durationMs ?? importedDurationMs,
  );
  let sampleTooShort = $derived(
    sampleDurationMs !== null && sampleDurationMs < 3_000,
  );
  let sampleTooLong = $derived(
    sampleDurationMs !== null && sampleDurationMs > 30_000,
  );
  let inputTooLow = $derived(
    recording && recordingSeconds >= 3 && sessionPeak < 0.03,
  );
  let replaceTooShort = $derived(
    replaceDurationMs !== null && replaceDurationMs < 3_000,
  );
  let replaceTooLong = $derived(
    replaceDurationMs !== null && replaceDurationMs > 30_000,
  );
  let replaceReady = $derived(
    replacePath !== "" &&
      replaceReferenceText.trim() !== "" &&
      replaceConsent &&
      !replaceTooShort &&
      !replaceTooLong,
  );
  let missingForProfile = $derived(missingModels(profileLanguage));
  let detailsFeatured = $derived(
    details === null
      ? false
      : details.kind === "voice"
        ? details.voice.id === featuredVoiceId
        : details.profile.id === featuredProfileId,
  );
  let detailsSampleActive = $derived(
    details !== null &&
      details.kind === "profile" &&
      detailsSamplePlaying &&
      detailsSampleId === details.profile.id,
  );

  onMount(() => {
    const storedRate = Number(localStorage.getItem(PREVIEW_RATE_KEY));
    if (previewRates.includes(storedRate)) previewRate = storedRate;
    previewText = (localStorage.getItem(PREVIEW_TEXT_KEY) ?? "").slice(0, 400);
    featuredVoiceId = localStorage.getItem(FEATURED_VOICE_KEY) ?? "";
    featuredProfileId = localStorage.getItem(FEATURED_PROFILE_KEY) ?? "";
    selectedDevice = localStorage.getItem(INPUT_DEVICE_KEY) ?? "";
    void loadInputDevices();
    try {
      const stored = JSON.parse(
        localStorage.getItem(LATENCY_HISTORY_KEY) ?? "[]",
      );
      if (Array.isArray(stored)) {
        latencyHistory = stored
          .filter((value) => typeof value === "number" && value > 0)
          .slice(-LATENCY_HISTORY_LIMIT);
      }
    } catch {
      latencyHistory = [];
    }
    void load();
    void initProgress();
    return () => {
      unlisten?.();
      unlistenVoice?.();
      player?.pause();
      stopSamplePlayback();
      stopDetailsSample();
      stopReplacePlayback();
      stopRecordingTimer();
      if (recording) void voiceSampleCancel();
      else if (recordedSample && busy !== "create")
        void voiceSampleDiscard(recordedSample.path);
    };
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && details) closeDetails();
  }

  function trapDetailsFocus(event: KeyboardEvent) {
    if (event.key !== "Tab" || !detailsDialog) return;
    const focusable = Array.from(
      detailsDialog.querySelectorAll<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
      ),
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function closeDetails() {
    details = null;
    renamingProfile = false;
    resetReplaceSample();
    stopDetailsSample();
    const trigger = detailsTrigger;
    detailsTrigger = null;
    if (trigger) trigger.focus();
  }

  function stopDetailsSample() {
    detailsSamplePlayer?.pause();
    detailsSamplePlayer = null;
    detailsSamplePlaying = false;
    detailsSampleId = null;
  }

  async function toggleDetailsSample(profile: VoiceProfile) {
    if (detailsSamplePlaying && detailsSampleId === profile.id) {
      stopDetailsSample();
      return;
    }
    stopDetailsSample();
    stopPreview();
    busy = `sample-${profile.id}`;
    try {
      const path = await voiceProfileSample(profile.id);
      const audio = new Audio(convertFileSrc(path));
      detailsSamplePlayer = audio;
      detailsSampleId = profile.id;
      audio.onended = () => {
        if (detailsSamplePlayer === audio) stopDetailsSample();
      };
      await audio.play();
      detailsSamplePlaying = true;
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function stopReplacePlayback() {
    replacePlayer?.pause();
    replacePlayer = null;
    replacePlaying = false;
  }

  function resetReplaceSample() {
    stopReplacePlayback();
    replacingSample = false;
    replacePath = "";
    replaceDurationMs = null;
    replaceReferenceText = "";
    replaceConsent = false;
  }

  function startReplaceSample() {
    if (!capabilities.cloning) return;
    resetReplaceSample();
    stopDetailsSample();
    renamingProfile = false;
    replacingSample = true;
  }

  function toggleDetailsSampleFromModal() {
    if (!details || details.kind !== "profile") return;
    void toggleDetailsSample(details.profile);
  }

  function toggleReplacePlayback() {
    if (!replacePath) return;
    if (replacePlaying) {
      stopReplacePlayback();
      return;
    }
    const audio = new Audio(convertFileSrc(replacePath));
    replacePlayer = audio;
    audio.onended = () => {
      if (replacePlayer === audio) {
        replacePlayer = null;
        replacePlaying = false;
      }
    };
    void audio
      .play()
      .then(() => {
        if (replacePlayer === audio) replacePlaying = true;
      })
      .catch((cause) => {
        stopReplacePlayback();
        notifyError(cause);
      });
  }

  function probeReplaceDuration(path: string) {
    const audio = new Audio(convertFileSrc(path));
    audio.onloadedmetadata = () => {
      if (replacePath !== path) return;
      replaceDurationMs = Number.isFinite(audio.duration)
        ? Math.round(audio.duration * 1000)
        : null;
    };
    audio.onerror = () => {
      if (replacePath === path) replaceDurationMs = null;
    };
  }

  async function chooseReplaceAudio() {
    if (busy === "replace") return;
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["wav", "mp3", "m4a", "flac", "ogg", "opus"],
        },
      ],
    });
    if (typeof selected !== "string") return;
    stopReplacePlayback();
    replacePath = selected;
    replaceDurationMs = null;
    probeReplaceDuration(selected);
  }

  async function saveReplaceSample() {
    if (!details || details.kind !== "profile" || !replaceReady) return;
    busy = "replace";
    try {
      const updated = await replaceVoiceSample(
        details.profile.id,
        replacePath,
        replaceReferenceText,
        replaceConsent,
      );
      profiles = profiles.map((profile) =>
        profile.id === updated.id ? updated : profile,
      );
      details = { kind: "profile", profile: updated };
      voices = await ttsVoices();
      resetReplaceSample();
      notifySuccess($t("voice.sampleReplaced", { name: updated.name }));
      focusDetails();
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function focusDetails() {
    void tick().then(() => {
      if (!details) return;
      detailsDialog?.querySelector<HTMLElement>(".details-close")?.focus();
    });
  }

  function formatLatency(milliseconds: number) {
    if (milliseconds < 1000) return `${Math.round(milliseconds)} ms`;
    return `${new Intl.NumberFormat(undefined, {
      maximumFractionDigits: 2,
    }).format(milliseconds / 1000)} s`;
  }

  function setPreviewRate(rate: number) {
    if (previewRate === rate) return;
    previewRate = rate;
    localStorage.setItem(PREVIEW_RATE_KEY, String(rate));
    if (player) {
      player.preservesPitch = true;
      player.playbackRate = rate;
    }
  }

  function recordLatency(milliseconds: number) {
    latencyHistory = [...latencyHistory, Math.round(milliseconds)].slice(
      -LATENCY_HISTORY_LIMIT,
    );
    localStorage.setItem(LATENCY_HISTORY_KEY, JSON.stringify(latencyHistory));
  }

  function persistPreviewText() {
    localStorage.setItem(PREVIEW_TEXT_KEY, previewText);
  }

  function resetPreviewText() {
    previewText = "";
    persistPreviewText();
  }

  function previewSentence(language: string, kind: "system" | "profile") {
    const custom = previewText.trim();
    if (custom) return custom;
    if (language === "en") {
      return kind === "profile"
        ? "Hello, this is a preview of my cloned voice."
        : "Hello, this is a preview of the selected voice.";
    }
    return kind === "profile"
      ? "Hola, esta es una prueba de mi voz clonada."
      : "Hola, esta es una prueba de la voz seleccionada.";
  }

  function openVoiceDetails(voice: VoiceDescriptor, trigger?: EventTarget | null) {
    detailsTrigger = trigger instanceof HTMLElement ? trigger : null;
    details = { kind: "voice", voice };
    focusDetails();
  }

  function openProfileDetails(
    profile: VoiceProfile,
    trigger?: EventTarget | null,
  ) {
    detailsTrigger = trigger instanceof HTMLElement ? trigger : null;
    details = { kind: "profile", profile };
    focusDetails();
  }

  function toggleFeaturedVoice(id: string) {
    featuredVoiceId = featuredVoiceId === id ? "" : id;
    if (featuredVoiceId) {
      localStorage.setItem(FEATURED_VOICE_KEY, featuredVoiceId);
    } else {
      localStorage.removeItem(FEATURED_VOICE_KEY);
    }
  }

  function toggleFeaturedProfile(id: string) {
    featuredProfileId = featuredProfileId === id ? "" : id;
    if (featuredProfileId) {
      localStorage.setItem(FEATURED_PROFILE_KEY, featuredProfileId);
    } else {
      localStorage.removeItem(FEATURED_PROFILE_KEY);
    }
  }

  function toggleFeaturedFromDetails() {
    if (!details) return;
    if (details.kind === "voice") toggleFeaturedVoice(details.voice.id);
    else toggleFeaturedProfile(details.profile.id);
  }

  function startRename() {
    if (!details || details.kind !== "profile") return;
    resetReplaceSample();
    stopDetailsSample();
    renamingProfile = true;
    renameValue = details.profile.name;
    void tick().then(() =>
      detailsDialog
        ?.querySelector<HTMLInputElement>(".rename-form input")
        ?.focus(),
    );
  }

  function cancelRename() {
    renamingProfile = false;
    renameValue = "";
  }

  async function saveRename() {
    if (!details || details.kind !== "profile") return;
    const name = renameValue.trim();
    if (!name) return;
    busy = "rename";
    try {
      const updated = await updateVoiceProfile(details.profile.id, name);
      profiles = profiles.map((profile) =>
        profile.id === updated.id ? updated : profile,
      );
      details = { kind: "profile", profile: updated };
      voices = await ttsVoices();
      renamingProfile = false;
      notifySuccess($t("voice.renamed", { name: updated.name }));
      focusDetails();
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function load() {
    loading = true;
    try {
      [voices, models, profiles, consentText, capabilities] =
        await Promise.all([
          ttsVoices(),
          voiceModels(),
          listVoiceProfiles(),
          voiceConsentText(),
          voiceCapabilities(),
        ]);
    } catch (cause) {
      notifyError(cause);
    } finally {
      loading = false;
    }
  }

  async function initProgress() {
    unlisten = await listen<VoiceModelProgress>("voice-model-progress", (event) => {
      const payload = event.payload;
      if (payload.done) {
        const next = { ...progress };
        delete next[payload.modelId];
        progress = next;
        if (payload.error) notifyError(new Error(payload.error));
        else void load();
      } else if (payload.total > 0) {
        progress = {
          ...progress,
          [payload.modelId]: (payload.downloaded / payload.total) * 100,
        };
      }
    });

    unlistenVoice = await listen<VoiceDownloadProgress>(
      "tts-download-progress",
      (event) => {
        const payload = event.payload;
        if (payload.done) {
          const next = { ...voiceProgress };
          delete next[payload.voiceId];
          voiceProgress = next;
          if (payload.error) notifyError(new Error(payload.error));
          else void load();
        } else if (payload.total > 0) {
          voiceProgress = {
            ...voiceProgress,
            [payload.voiceId]: (payload.downloaded / payload.total) * 100,
          };
        }
      },
    );
  }

  function languageLabel(language: string) {
    if (language === "es") return $t("voice.languageSpanish");
    if (language === "en") return $t("voice.languageEnglish");
    return language;
  }

  function genderLabel(gender: string) {
    if (gender === "male") return $t("voice.genderMale");
    if (gender === "female") return $t("voice.genderFemale");
    return $t("voice.genderNeutral");
  }

  function kindLabel(kind: string) {
    if (kind === "backbone") return $t("voice.kindBackbone");
    if (kind === "encoder") return $t("voice.kindEncoder");
    if (kind === "decoder") return $t("voice.kindDecoder");
    return kind;
  }

  function initials(name: string) {
    return name
      .trim()
      .split(/\s+/)
      .slice(0, 2)
      .map((part) => part[0]?.toUpperCase() ?? "")
      .join("");
  }

  function languageProgress(language: string) {
    const list = modelsFor(language);
    const total = list.reduce((sum, model) => sum + model.sizeBytes, 0);
    const installed = list
      .filter((model) => model.installed)
      .reduce((sum, model) => sum + model.sizeBytes, 0);
    return total > 0 ? Math.round((installed / total) * 100) : 0;
  }

  async function installVoice(voice: VoiceDescriptor) {
    if (!capabilities.voiceDownload) return;
    busy = `voice-${voice.id}`;
    voiceProgress = { ...voiceProgress, [voice.id]: 0 };
    try {
      await ttsDownloadVoice(voice.id);
      voices = await ttsVoices();
      notifySuccess($t("voice.installed"), voice.name);
    } catch (cause) {
      const next = { ...voiceProgress };
      delete next[voice.id];
      voiceProgress = next;
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function previewSystem(voice: VoiceDescriptor) {
    busy = `preview-${voice.id}`;
    try {
      const started = performance.now();
      const output = await ttsSynthesize({
        text: previewSentence(voice.language, "system"),
        voiceId: voice.id,
        rate: 1,
        pitch: 1,
      });
      recordLatency(performance.now() - started);
      playPreview(output, voice.id);
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function previewProfile(profile: VoiceProfile) {
    busy = `preview-${profile.id}`;
    try {
      const started = performance.now();
      const output = await voiceClonePreview(
        `clone:${profile.id}`,
        previewSentence(profile.language, "profile"),
      );
      recordLatency(performance.now() - started);
      playPreview({ ...output, audioBase64: output.audioBase64 }, profile.id);
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function playPreview(output: SynthesisOutput, id: string) {
    player?.pause();
    playingId = id;
    previewDuration = output.durationMs;
    rememberedDurations = { ...rememberedDurations, [id]: output.durationMs };
    player = new Audio(`data:audio/wav;base64,${output.audioBase64}`);
    player.preservesPitch = true;
    player.playbackRate = previewRate;
    player.onended = () => {
      if (playingId === id) {
        playingId = null;
        previewDuration = 0;
      }
    };
    void player.play().catch(() => {
      if (playingId === id) {
        playingId = null;
        previewDuration = 0;
      }
    });
  }

  function stopPreview() {
    player?.pause();
    player = null;
    playingId = null;
    previewDuration = 0;
  }

  function stopRecordingTimer() {
    if (recordingTimer) {
      clearInterval(recordingTimer);
      recordingTimer = null;
    }
    if (levelTimer) {
      clearInterval(levelTimer);
      levelTimer = null;
    }
    inputLevel = 0;
  }

  async function loadInputDevices() {
    try {
      const devices = await voiceInputDevices();
      inputDevices = devices;
      if (selectedDevice && !devices.includes(selectedDevice)) {
        selectedDevice = "";
        localStorage.removeItem(INPUT_DEVICE_KEY);
      }
    } catch {
      inputDevices = [];
    }
  }

  function setInputDevice(name: string) {
    selectedDevice = name;
    if (name) localStorage.setItem(INPUT_DEVICE_KEY, name);
    else localStorage.removeItem(INPUT_DEVICE_KEY);
  }

  async function startRecording() {
    if (!capabilities.recording || busy === "create") return;
    stopPreview();
    stopSamplePlayback();
    await discardRecordedSample();
    importedPath = "";
    importedDurationMs = null;
    sessionPeak = 0;
    inputLevel = 0;
    busy = "record";
    try {
      await voiceSampleStart(selectedDevice || undefined);
      recording = true;
      recordingSeconds = 0;
      recordingTimer = setInterval(() => {
        recordingSeconds += 1;
        if (recordingSeconds >= 30) void finishRecording();
      }, 1000);
      levelTimer = setInterval(async () => {
        try {
          const level = await voiceSampleLevel();
          inputLevel = level;
          sessionPeak = Math.max(sessionPeak, level);
        } catch {
          // Si el medidor falla, la grabación continúa.
        }
      }, 120);
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function finishRecording() {
    if (!recording) return;
    stopRecordingTimer();
    recording = false;
    busy = "record-stop";
    try {
      const sample = await voiceSampleStop();
      recordedSample = sample;
      audioPath = sample.path;
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function cancelRecording() {
    if (!recording) return;
    stopRecordingTimer();
    recording = false;
    busy = "record-cancel";
    try {
      await voiceSampleCancel();
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function discardRecordedSample() {
    const sample = recordedSample;
    recordedSample = null;
    stopSamplePlayback();
    if (!sample) return;
    if (audioPath === sample.path) audioPath = "";
    try {
      await voiceSampleDiscard(sample.path);
    } catch {
      // La muestra temporal se puede borrar en la siguiente limpieza.
    }
  }

  function stopSamplePlayback() {
    samplePlayer?.pause();
    samplePlayer = null;
    samplePlaying = false;
  }

  function toggleSamplePlayback() {
    const path = recordedSample?.path ?? importedPath;
    if (!path) return;
    if (samplePlaying) {
      stopSamplePlayback();
      return;
    }

    const audio = new Audio(convertFileSrc(path));
    samplePlayer = audio;
    audio.onended = () => {
      if (samplePlayer === audio) {
        samplePlayer = null;
        samplePlaying = false;
      }
    };
    void audio
      .play()
      .then(() => {
        if (samplePlayer === audio) samplePlaying = true;
      })
      .catch((cause) => {
        stopSamplePlayback();
        notifyError(cause);
      });
  }

  function selectTab(next: VoiceTab) {
    if (tab === next) return;
    if (recording) void cancelRecording();
    tab = next;
  }

  async function startClone() {
    if (!capabilities.cloning) return;
    tab = "cloned";
    await tick();
    nameInput?.focus();
  }

  async function importAudio() {
    if (!capabilities.cloning) return;
    tab = "cloned";
    await tick();
    await chooseAudio();
  }

  async function chooseAudio() {
    if (busy === "create") return;
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["wav", "mp3", "m4a", "flac", "ogg", "opus"],
        },
      ],
    });
    if (typeof selected === "string") {
      await discardRecordedSample();
      importedPath = selected;
      importedDurationMs = null;
      audioPath = selected;
      probeImportedDuration(selected);
    }
  }

  function probeImportedDuration(path: string) {
    const audio = new Audio(convertFileSrc(path));
    audio.onloadedmetadata = () => {
      if (importedPath !== path) return;
      importedDurationMs = Number.isFinite(audio.duration)
        ? Math.round(audio.duration * 1000)
        : null;
    };
    audio.onerror = () => {
      if (importedPath === path) importedDurationMs = null;
    };
  }

  function fileName(path: string) {
    return path.split(/[/\\]/).pop() ?? path;
  }

  function sampleWarningLabel(code: string) {
    if (code === "clipped") return $t("voice.sampleWarningClipped");
    if (code === "trimmed") return $t("voice.sampleWarningTrimmed");
    if (code === "lowLevel") return $t("voice.sampleWarningLowLevel");
    return code;
  }

  async function createProfile() {
    if (!capabilities.cloning) return;
    if (!consent) {
      notifyError(new Error($t("voice.consentRequired")));
      return;
    }
    if (!audioPath) {
      notifyError(new Error($t("voice.noAudio")));
      return;
    }
    busy = "create";
    const recordedPath = recordedSample?.path;
    try {
      const profile = await createVoiceProfile(
        audioPath,
        profileName,
        profileLanguage,
        referenceText,
        consent,
      );
      notifySuccess($t("voice.profileCreated", { name: profile.name }));
      if (recordedPath) {
        recordedSample = null;
        stopSamplePlayback();
        void voiceSampleDiscard(recordedPath).catch(() => {});
      }
      audioPath = "";
      importedPath = "";
      importedDurationMs = null;
      profileName = "";
      referenceText = "";
      consent = false;
      voices = await ttsVoices();
      profiles = await listVoiceProfiles();
      tab = "cloned";
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function exportProfile(profile: VoiceProfile) {
    const destination = await save({
      defaultPath: `${profile.name}.koravoice`,
      filters: [{ name: "Kora Voice", extensions: ["koravoice"] }],
    });
    if (!destination) return;
    busy = `export-${profile.id}`;
    try {
      await exportVoiceProfile(`clone:${profile.id}`, destination);
      notifySuccess($t("voice.export"), profile.name);
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function importProfile() {
    if (!importConsent) {
      notifyError(new Error($t("voice.consentRequired")));
      return;
    }
    const source = await open({
      multiple: false,
      filters: [{ name: "Kora Voice", extensions: ["koravoice"] }],
    });
    if (typeof source !== "string") return;
    busy = "import";
    try {
      const profile = await importVoiceProfile(source, true);
      notifySuccess($t("voice.profileImported", { name: profile.name }));
      voices = await ttsVoices();
      profiles = await listVoiceProfiles();
      tab = "cloned";
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function removeProfile(profile: VoiceProfile) {
    if (!window.confirm(`${translate("voice.delete")} «${profile.name}»?`)) return;
    busy = `delete-${profile.id}`;
    try {
      await deleteVoiceProfile(`clone:${profile.id}`);
      if (featuredProfileId === profile.id) {
        featuredProfileId = "";
        localStorage.removeItem(FEATURED_PROFILE_KEY);
      }
      voices = await ttsVoices();
      profiles = await listVoiceProfiles();
      notifySuccess($t("voice.profileDeleted"));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function downloadModels(language: string) {
    if (!capabilities.modelDownload) return;
    busy = `models-${language}`;
    try {
      await voiceDownloadModels(language);
      models = await voiceModels();
      notifySuccess($t("voice.modelsReady", { language: languageLabel(language) }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function downloadModel(model: VoiceModelInfo) {
    if (!capabilities.modelDownload) return;
    busy = `model-${model.id}`;
    try {
      await voiceDownloadModel(model.id);
      models = await voiceModels();
      notifySuccess($t("voice.modelDownloaded", { name: model.label }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function removeModel(model: VoiceModelInfo) {
    if (!capabilities.modelDownload) return;
    if (
      !window.confirm(
        translate("voice.modelDeleteConfirm", { name: model.label }),
      )
    ) {
      return;
    }
    busy = `model-${model.id}`;
    try {
      await deleteVoiceModel(model.id);
      models = await voiceModels();
      notifySuccess($t("voice.modelDeleted", { name: model.label }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  async function repairModel(model: VoiceModelInfo) {
    if (!capabilities.modelDownload) return;
    if (
      !window.confirm(
        translate("voice.modelRepairConfirm", { name: model.label }),
      )
    ) {
      return;
    }
    busy = `model-${model.id}`;
    try {
      await deleteVoiceModel(model.id);
      await voiceDownloadModel(model.id);
      models = await voiceModels();
      notifySuccess($t("voice.modelRepaired", { name: model.label }));
    } catch (cause) {
      notifyError(cause);
    } finally {
      busy = null;
    }
  }

  function modelsFor(language: string) {
    return models.filter((model) => model.languages.includes(language));
  }

  function missingModels(language: string) {
    return modelsFor(language).filter((model) => !model.installed).length;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#snippet waveStrip(id: string)}
  {@const isPlaying = playingId === id}
  {@const duration = isPlaying ? previewDuration : rememberedDurations[id]}
  <div class="wave-strip" class:playing={isPlaying}>
    <span class="wave-label">
      {#if isPlaying}
        <strong>{$t("voice.playing")}</strong>
        <span>· {formatDuration(previewDuration)}</span>
      {:else}
        <span>{$t("voice.waveIdle")}</span>
        {#if duration}
          <span>· {formatDuration(duration)}</span>
        {/if}
      {/if}
    </span>
    <div class="wave-bars" aria-hidden="true">
      {#each waveBars as bar (bar)}
        <span></span>
      {/each}
    </div>
    {#if isPlaying}
      <button
        class="wave-stop"
        type="button"
        aria-label={$t("voice.stopPreview")}
        title={$t("voice.stopPreview")}
        onclick={stopPreview}
      >
        <Icon name="stop" size={14} />
      </button>
    {/if}
  </div>
{/snippet}

{#snippet voiceCard(voice: VoiceDescriptor, featured = false)}
  <article class="voice-card" class:featured class:playing={playingId === voice.id}>
    <div class="voice-main">
      <span class="voice-avatar" class:installed={voice.installed}>
        <span class="initials">{initials(voice.name)}</span>
        <span class="voice-dot" class:on={voice.installed}></span>
      </span>
      <div class="voice-copy">
        <div class="voice-title-row">
          <h3>{voice.name}</h3>
          <span class="pill language">{languageLabel(voice.language)}</span>
          {#if voice.gender}
            <span class="pill">{genderLabel(voice.gender)}</span>
          {/if}
          {#if featured}
            <span class="pill featured-pill">{$t("voice.featured")}</span>
          {/if}
        </div>
        <p class="voice-desc">
          {$t("voice.systemVoiceDesc", {
            language: languageLabel(voice.language),
            engine: voice.engine,
          })}
        </p>
        <div class="voice-meta">
          <span>
            <Icon name="check" size={13} />
            {voice.installed ? $t("voice.installed") : $t("voice.notInstalled")}
          </span>
          <span>{formatBytes(voice.sizeBytes)}</span>
          <span>{voice.engine}</span>
        </div>
        {#if voiceProgress[voice.id] !== undefined}
          <div class="voice-download">
            <div
              class="voice-download-track"
              role="progressbar"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Math.round(voiceProgress[voice.id])}
              aria-label={$t("voice.downloadingVoice", { name: voice.name })}
            >
              <span style={`width: ${voiceProgress[voice.id]}%`}></span>
            </div>
            <small>{Math.round(voiceProgress[voice.id])}%</small>
          </div>
        {/if}
      </div>
    </div>
    <div class="voice-actions">
      {#if voice.installed}
        <Button
          variant="ghost"
          size="small"
          loading={busy === `preview-${voice.id}`}
          onclick={() =>
            playingId === voice.id ? stopPreview() : previewSystem(voice)}
        >
          <Icon name={playingId === voice.id ? "pause" : "play"} size={16} />
          {playingId === voice.id ? $t("voice.pause") : $t("voice.preview")}
        </Button>
      {:else if capabilities.voiceDownload}
        <Button
          variant="primary"
          size="small"
          loading={busy === `voice-${voice.id}`}
          onclick={() => installVoice(voice)}
        >
          <Icon name="download" size={16} />
          {$t("voice.download")}
        </Button>
      {:else}
        <Button
          variant="ghost"
          size="small"
          disabled
          title={$t("voice.downloadUnavailable")}
        >
          <Icon name="download" size={16} />
          {$t("voice.notAvailable")}
        </Button>
      {/if}
      <Button
        variant="ghost"
        size="small"
        onclick={(event) => openVoiceDetails(voice, event.currentTarget)}
      >
        <Icon name="expand" size={16} />
        {$t("voice.details")}
      </Button>
    </div>
    {@render waveStrip(voice.id)}
  </article>
{/snippet}

{#snippet profileCard(profile: VoiceProfile, featured = false)}
  <article class="voice-card profile-card" class:featured class:playing={playingId === profile.id}>
    <div class="voice-main">
      <span class="voice-avatar installed">
        <span class="initials">{initials(profile.name)}</span>
        <span class="voice-dot on"></span>
      </span>
      <div class="voice-copy">
        <div class="voice-title-row">
          <h3>{profile.name}</h3>
          <span class="pill language">{languageLabel(profile.language)}</span>
          <span class="pill featured-pill">{$t("voice.voiceClone")}</span>
        </div>
        <p class="voice-desc">{$t("voice.profileDesc")}</p>
        <div class="voice-meta">
          <span>{formatDate(profile.createdAt)}</span>
          <span>{formatDuration(profile.referenceDurationMs)}</span>
          <span>{profile.engine}</span>
        </div>
      </div>
    </div>
    <div class="voice-actions">
      <Button
        variant="ghost"
        size="small"
        loading={busy === `preview-${profile.id}`}
        onclick={() =>
          playingId === profile.id ? stopPreview() : previewProfile(profile)}
      >
        <Icon name={playingId === profile.id ? "pause" : "play"} size={16} />
        {playingId === profile.id ? $t("voice.pause") : $t("voice.preview")}
      </Button>
      <Button
        variant="ghost"
        size="small"
        onclick={(event) => openProfileDetails(profile, event.currentTarget)}
      >
        <Icon name="expand" size={16} />
        {$t("voice.details")}
      </Button>
      <div class="profile-secondary">
        <Button
          variant="ghost"
          size="small"
          loading={busy === `export-${profile.id}`}
          onclick={() => exportProfile(profile)}
        >
          <Icon name="download" size={16} />
          {$t("voice.export")}
        </Button>
        <Button
          variant="danger"
          size="small"
          loading={busy === `delete-${profile.id}`}
          onclick={() => removeProfile(profile)}
        >
          <Icon name="trash" size={16} />
          {$t("voice.delete")}
        </Button>
      </div>
      <details class="more-menu">
        <summary>
          <Icon name="more" size={16} />
          {$t("voice.moreActions")}
        </summary>
        <div class="more-menu-body">
          <Button
            variant="ghost"
            size="small"
            loading={busy === `export-${profile.id}`}
            onclick={() => exportProfile(profile)}
          >
            <Icon name="download" size={16} />
            {$t("voice.export")}
          </Button>
          <Button
            variant="danger"
            size="small"
            loading={busy === `delete-${profile.id}`}
            onclick={() => removeProfile(profile)}
          >
            <Icon name="trash" size={16} />
            {$t("voice.delete")}
          </Button>
        </div>
      </details>
    </div>
    {@render waveStrip(profile.id)}
  </article>
{/snippet}

<section class="voice-view">
  <header class="hero">
    <div class="hero-copy">
      <div class="hero-kicker">
        <span class="studio-badge">
          <Icon name="sparkles" size={13} />
          {$t("voice.studioBadge")}
        </span>
        <span class="engine-status">
          <span
            class="status-dot"
            class:on={models.length > 0 && pendingModels === 0}
          ></span>
          {#if models.length === 0}
            {$t("voice.engineNoModels")}
          {:else if pendingModels === 0}
            {$t("voice.engineActive")}
          {:else}
            {$t("voice.enginePending", { count: pendingModels })}
          {/if}
        </span>
      </div>
      <h1 class="page-title">{$t("voice.titleLong")}</h1>
      <p class="page-subtitle">{$t("voice.subtitleLong")}</p>
    </div>
    <div class="hero-metrics">
      <div class="metric">
        <span>{$t("voice.metricLatency")}</span>
        <strong>
          {#if lastLatencyMs === null}
            <span class="metric-empty">{$t("voice.metricNotTested")}</span>
          {:else}
            {formatLatency(lastLatencyMs)}
          {/if}
        </strong>
        {#if averageLatencyMs !== null && latencyHistory.length > 1}
          <em class="metric-average">
            {$t("voice.metricAverage", {
              value: formatLatency(averageLatencyMs),
              count: latencyHistory.length,
            })}
          </em>
        {/if}
      </div>
      <span class="metric-icon" aria-hidden="true">
        <Icon name="speed" size={22} />
      </span>
    </div>
  </header>

  <nav class="studio-tabs" aria-label={$t("voice.title")}>
    <button
      class:active={tab === "system"}
      aria-pressed={tab === "system"}
      onclick={() => selectTab("system")}
    >
      <Icon name="voice" size={17} />
      <span>{$t("voice.system")}</span>
      <span class="tab-count">{systemVoices.length}</span>
    </button>
    <button
      class:active={tab === "cloned"}
      aria-pressed={tab === "cloned"}
      onclick={() => selectTab("cloned")}
    >
      <Icon name="mic" size={17} />
      <span>{$t("voice.cloned")}</span>
      {#if profiles.length}
        <span class="tab-count">{profiles.length}</span>
      {/if}
    </button>
    <button
      class:active={tab === "models"}
      aria-pressed={tab === "models"}
      onclick={() => selectTab("models")}
    >
      <Icon name="sliders" size={17} />
      <span>{$t("voice.models")}</span>
      {#if pendingModels}
        <span class="tab-count attention">{pendingModels}</span>
      {:else if models.length}
        <span class="tab-count">{models.length}</span>
      {/if}
    </button>
  </nav>

  {#if !loading && tab !== "models"}
    <section class="preview-bar">
      <div class="preview-bar-head">
        <button
          type="button"
          class="preview-toggle"
          class:active={customPreviewOpen}
          aria-expanded={customPreviewOpen}
          onclick={() => (customPreviewOpen = !customPreviewOpen)}
        >
          <Icon name="edit" size={15} />
          {$t("voice.customPreview")}
          {#if previewText.trim()}
            <span class="preview-dot" aria-hidden="true"></span>
          {/if}
        </button>
        {#if !customPreviewOpen && previewText.trim()}
          <span class="preview-state" title={previewText}>
            {$t("voice.customPreviewActive")}
          </span>
        {/if}
      </div>
      {#if customPreviewOpen}
        <div class="preview-editor">
          <input
            bind:value={previewText}
            maxlength={400}
            placeholder={$t("voice.customPreviewPlaceholder")}
            aria-label={$t("voice.customPreview")}
            oninput={persistPreviewText}
          />
          <span class="preview-count">{previewText.length}/400</span>
          <button
            type="button"
            class="preview-reset"
            disabled={!previewText}
            onclick={resetPreviewText}
          >
            {$t("voice.clearPreview")}
          </button>
        </div>
        <p class="preview-hint">{$t("voice.customPreviewHint")}</p>
      {/if}
    </section>
  {/if}

  {#if loading}
    <Spinner label={$t("common.loading")} />
  {:else if tab === "system"}
    <div class="studio-toolbar">
      <div class="toolbar-actions">
        {#if capabilities.cloning}
          <Button variant="primary" onclick={startClone}>
            <Icon name="mic" size={17} />
            {$t("voice.cloneNew")}
          </Button>
          <Button variant="ghost" onclick={importAudio}>
            <Icon name="upload" size={17} />
            {$t("voice.importAudio")}
          </Button>
        {/if}
      </div>
      <div class="toolbar-filters">
        <label class="filter-search">
          <Icon name="search" size={16} />
          <input
            bind:value={searchQuery}
            placeholder={$t("voice.searchPlaceholder")}
            aria-label={$t("voice.searchPlaceholder")}
          />
          {#if searchQuery}
            <button
              type="button"
              class="search-clear"
              aria-label={$t("common.close")}
              onclick={() => (searchQuery = "")}
            >
              <Icon name="close" size={13} />
            </button>
          {/if}
        </label>
        <label class="filter-select">
          <Icon name="globe" size={17} />
          <select
            bind:value={selectedLanguage}
            aria-label={$t("voice.filterLanguage")}
          >
            <option value="all">{$t("voice.filterAllLanguages")}</option>
            {#each voiceLanguages as language (language)}
              <option value={language}>{languageLabel(language)}</option>
            {/each}
          </select>
        </label>
        <label class="filter-select">
          <Icon name="account" size={17} />
          <select
            bind:value={genderFilter}
            aria-label={$t("voice.filterGender")}
          >
            <option value="all">{$t("voice.filterAllGenders")}</option>
            <option value="male">{$t("voice.genderMale")}</option>
            <option value="female">{$t("voice.genderFemale")}</option>
            <option value="neutral">{$t("voice.genderNeutral")}</option>
          </select>
        </label>
        <div
          class="filter-segment"
          role="group"
          aria-label={$t("voice.filterStatus")}
        >
          <button
            class:active={!onlyInstalled}
            aria-pressed={!onlyInstalled}
            onclick={() => (onlyInstalled = false)}
          >
            {$t("voice.filterAll")}
          </button>
          <button
            class:active={onlyInstalled}
            aria-pressed={onlyInstalled}
            onclick={() => (onlyInstalled = true)}
          >
            {$t("voice.installed")}
          </button>
        </div>
        <div
          class="rate-group"
          role="group"
          aria-label={$t("voice.previewRate")}
        >
          <Icon name="speed" size={15} />
          {#each previewRates as rate (rate)}
            <button
              type="button"
              class:active={previewRate === rate}
              aria-pressed={previewRate === rate}
              onclick={() => setPreviewRate(rate)}
            >
              {rate.toFixed(1)}×
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="section-head">
      <div class="section-title">
        <h2>{$t("voice.catalogTitle")}</h2>
        <span class="pill count-pill">
          {$t("voice.catalogCount", { count: filteredSystemVoices.length })}
        </span>
      </div>
      <span class="section-note">{$t("voice.catalogNote")}</span>
    </div>

    <div class="studio-grid">
      <div class="catalog">
        {#if filteredSystemVoices.length === 0}
          <EmptyState
            title={normalizedQuery || genderFilter !== "all"
              ? $t("voice.searchEmpty")
              : $t("voice.noSystem")}
            description={normalizedQuery || genderFilter !== "all"
              ? $t("voice.searchEmptyHint")
              : $t("voice.catalogEmpty")}
            icon={normalizedQuery ? "search" : "voice"}
          />
        {:else}
          {#if featuredVoice}
            {@render voiceCard(featuredVoice, true)}
          {/if}
          {#each otherSystemVoices as voice (voice.id)}
            {@render voiceCard(voice)}
          {/each}
        {/if}
      </div>

      <aside class="lab">
        <section class="rail-card">
          <div class="rail-head">
            <div class="rail-title">
              <span class="rail-icon"><Icon name="sliders" size={18} /></span>
              <h2>{$t("voice.engineTitle")}</h2>
            </div>
            <span class="pill">{$t("voice.engineLocal")}</span>
          </div>
          <p class="rail-desc">{$t("voice.engineDesc")}</p>

          {#if backboneModels.length > 0}
            <span class="lumina-label">{$t("voice.kindBackbone")}</span>
            <div class="engine-list">
              {#each backboneModels as model (model.id)}
                <div class="engine-row" class:ready={model.installed}>
                  <span class="engine-state">
                    <Icon
                      name={model.installed ? "check" : "download"}
                      size={13}
                    />
                  </span>
                  <div>
                    <strong>{model.label}</strong>
                    <span>
                      {model.languages.map(languageLabel).join(" · ")} ·
                      {formatBytes(model.sizeBytes)}
                    </span>
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          {#if engineComponents.length > 0}
            <span class="lumina-label">{$t("voice.engineComponents")}</span>
            <div class="engine-list">
              {#each engineComponents as model (model.id)}
                <div class="engine-row" class:ready={model.installed}>
                  <span class="engine-state">
                    <Icon
                      name={model.installed ? "check" : "download"}
                      size={13}
                    />
                  </span>
                  <div>
                    <strong>{model.label}</strong>
                    <span>
                      {kindLabel(model.kind)} · {formatBytes(model.sizeBytes)}
                    </span>
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          <div class="rail-progress">
            <span class="lumina-label">{$t("voice.modelsProgress")}</span>
            {#each languages as language (language)}
              {@const percent = languageProgress(language)}
              <div class="progress-row">
                <div class="progress-label">
                  <span>{languageLabel(language)}</span>
                  <strong>{percent}%</strong>
                </div>
                <div class="progress-track">
                  <span style={`width: ${percent}%`}></span>
                </div>
              </div>
            {/each}
          </div>
        </section>

        <section class="privacy-card">
          <span class="privacy-icon"><Icon name="lock" size={18} /></span>
          <div>
            <h3>{$t("voice.privacyTitle")}</h3>
            <p>{$t("voice.privacyBody")}</p>
          </div>
        </section>
      </aside>
    </div>
  {:else if tab === "cloned"}
    <div class="studio-toolbar">
      <div class="toolbar-actions">
        <span class="toolbar-label">
          <Icon name="mic" size={16} />
          {$t("voice.cloned")}
        </span>
      </div>
      <div class="toolbar-filters">
        <label class="filter-search">
          <Icon name="search" size={16} />
          <input
            bind:value={profileQuery}
            placeholder={$t("voice.searchProfilesPlaceholder")}
            aria-label={$t("voice.searchProfilesPlaceholder")}
          />
          {#if profileQuery}
            <button
              type="button"
              class="search-clear"
              aria-label={$t("common.close")}
              onclick={() => (profileQuery = "")}
            >
              <Icon name="close" size={13} />
            </button>
          {/if}
        </label>
        <div
          class="rate-group"
          role="group"
          aria-label={$t("voice.previewRate")}
        >
          <Icon name="speed" size={15} />
          {#each previewRates as rate (rate)}
            <button
              type="button"
              class:active={previewRate === rate}
              aria-pressed={previewRate === rate}
              onclick={() => setPreviewRate(rate)}
            >
              {rate.toFixed(1)}×
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="studio-grid">
      <div class="cloned-main">
        {#if filteredProfiles.length > 0}
          <div class="section-head">
            <div class="section-title">
              <h2>{$t("voice.featuredProfile")}</h2>
            </div>
            <span class="section-note">
              {$t("voice.clonedCount", { count: profiles.length })}
            </span>
          </div>
          {#if featuredProfile}
            {@render profileCard(featuredProfile, true)}
          {/if}
        {/if}

        <section class="panel create-panel">
          <div class="panel-head">
            <h2>{$t("voice.create")}</h2>
            <span class="pill">
              {capabilities.cloning
                ? $t("voice.engineLocal")
                : $t("voice.notAvailable")}
            </span>
          </div>
          <p class="hint">
            {capabilities.cloning
              ? $t("voice.createHint")
              : $t("voice.cloningUnavailable")}
          </p>

          {#if capabilities.cloning}
            {#if missingForProfile > 0}
            <div class="models-warning" role="status">
              <span class="models-warning-icon" aria-hidden="true">
                <Icon name="download" size={17} />
              </span>
              <div>
                <strong>
                  {$t("voice.modelsMissingTitle", {
                    count: missingForProfile,
                    language: languageLabel(profileLanguage),
                  })}
                </strong>
                <p>{$t("voice.modelsMissingBody")}</p>
              </div>
              <div class="models-warning-actions">
                <Button
                  variant="primary"
                  size="small"
                  loading={busy === `models-${profileLanguage}`}
                  onclick={() => downloadModels(profileLanguage)}
                >
                  <Icon name="download" size={16} />
                  {$t("voice.download")}
                </Button>
                <Button
                  variant="ghost"
                  size="small"
                  onclick={() => (tab = "models")}
                >
                  {$t("voice.goToModels")}
                </Button>
              </div>
            </div>
          {/if}

          {#if inputDevices.length > 0}
            <label class="mic-select">
              <span>{$t("voice.inputDevice")}</span>
              <select
                value={selectedDevice}
                disabled={recording || busy === "create"}
                onchange={(event) => setInputDevice(event.currentTarget.value)}
              >
                <option value="">{$t("voice.inputDefault")}</option>
                {#each inputDevices as device (device)}
                  <option value={device}>{device}</option>
                {/each}
              </select>
            </label>
          {/if}

          <div
            class="audio-picker"
            class:has-file={Boolean(audioPath)}
            class:recording
          >
            <span class="audio-picker-icon" class:live={recording}>
              {#if recording}
                <span class="record-dot" aria-hidden="true"></span>
              {:else}
                <Icon name="mic" size={20} />
              {/if}
            </span>
            <div class="audio-picker-copy">
              {#if recording}
                <strong>{$t("voice.recordingNow")}</strong>
                <span>
                  {formatDuration(recordingSeconds * 1000)} ·
                  {$t("voice.recordingLimit")}
                </span>
              {:else if recordedSample}
                <strong>{$t("voice.recordedSample")}</strong>
                <span>
                  {formatDuration(recordedSample.durationMs)} ·
                  {$t("voice.recordedHint")}
                </span>
              {:else if importedPath}
                <strong>{$t("voice.importedSample")}</strong>
                <span title={audioPath}>
                  {#if importedDurationMs !== null}
                    {formatDuration(importedDurationMs)} ·
                  {/if}
                  {fileName(importedPath)}
                </span>
              {:else}
                <strong>{$t("voice.sampleTitle")}</strong>
                <span title={audioPath}>
                  {audioPath || $t("voice.sampleHint")}
                </span>
              {/if}
            </div>
            <div class="audio-picker-actions">
              {#if recording}
                <Button
                  variant="primary"
                  size="small"
                  loading={busy === "record-stop"}
                  onclick={finishRecording}
                >
                  <Icon name="stop" size={16} />
                  {$t("voice.stopRecording")}
                </Button>
                <Button
                  variant="ghost"
                  size="small"
                  loading={busy === "record-cancel"}
                  onclick={cancelRecording}
                >
                  <Icon name="close" size={16} />
                  {$t("common.cancel")}
                </Button>
              {:else if recordedSample}
                <Button
                  variant="ghost"
                  size="small"
                  onclick={toggleSamplePlayback}
                >
                  <Icon
                    name={samplePlaying ? "pause" : "play"}
                    size={16}
                  />
                  {samplePlaying
                    ? $t("voice.pause")
                    : $t("voice.listenSample")}
                </Button>
                {#if capabilities.recording}
                  <Button
                    variant="ghost"
                    size="small"
                    loading={busy === "record"}
                    disabled={busy === "create"}
                    onclick={startRecording}
                  >
                    <Icon name="mic" size={16} />
                    {$t("voice.recordAgain")}
                  </Button>
                {/if}
                <Button
                  variant="ghost"
                  size="small"
                  disabled={busy === "create"}
                  onclick={chooseAudio}
                >
                  <Icon name="folder" size={16} />
                  {$t("voice.chooseAudio")}
                </Button>
              {:else}
                {#if importedPath}
                  <Button
                    variant="ghost"
                    size="small"
                    onclick={toggleSamplePlayback}
                  >
                    <Icon
                      name={samplePlaying ? "pause" : "play"}
                      size={16}
                    />
                    {samplePlaying
                      ? $t("voice.pause")
                      : $t("voice.listenSample")}
                  </Button>
                {/if}
                {#if capabilities.recording}
                  <Button
                    variant="ghost"
                    size="small"
                    loading={busy === "record"}
                    disabled={busy === "create"}
                    onclick={startRecording}
                  >
                    <Icon name="mic" size={16} />
                    {$t("voice.record")}
                  </Button>
                {/if}
                <Button
                  variant="ghost"
                  size="small"
                  disabled={busy === "create"}
                  onclick={chooseAudio}
                >
                  <Icon name="folder" size={16} />
                  {$t("voice.chooseAudio")}
                </Button>
              {/if}
            </div>
          </div>

          {#if !capabilities.recording}
            <p class="hint">{$t("voice.recordingUnavailable")}</p>
          {/if}

          {#if recording}
            <div class="level-meter">
              <span>{$t("voice.inputLevel")}</span>
              <div
                class="level-track"
                role="meter"
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow={Math.round(inputLevel * 100)}
                aria-label={$t("voice.inputLevel")}
              >
                <span
                  class:hot={inputLevel >= 0.9}
                  style={`width: ${Math.round(inputLevel * 100)}%`}
                ></span>
              </div>
            </div>
          {/if}

          {#if inputTooLow}
            <p class="level-warning" role="status">{$t("voice.inputTooLow")}</p>
          {/if}

          {#if sampleTooShort}
            <p class="sample-error" role="alert">{$t("voice.sampleTooShort")}</p>
          {:else if sampleTooLong}
            <p class="sample-error" role="alert">{$t("voice.sampleTooLong")}</p>
          {/if}

          {#if recordedSample && recordedSample.warnings.length > 0}
            <ul class="sample-warnings">
              {#each recordedSample.warnings as code (code)}
                <li>{sampleWarningLabel(code)}</li>
              {/each}
            </ul>
          {/if}

          <div class="form-grid">
            <label class="field" for="voice-name">
              <span>{$t("common.name")}</span>
              <input
                id="voice-name"
                bind:this={nameInput}
                bind:value={profileName}
                maxlength="80"
                placeholder={$t("voice.profilePlaceholder")}
              />
            </label>
            <label class="field" for="voice-language">
              <span>{$t("common.language")}</span>
              <select id="voice-language" bind:value={profileLanguage}>
                <option value="es">{$t("voice.languageSpanish")}</option>
                <option value="en">{$t("voice.languageEnglish")}</option>
              </select>
            </label>
          </div>

          <label class="field" for="voice-reference">
            <span>{$t("voice.reference")}</span>
            <textarea
              id="voice-reference"
              bind:value={referenceText}
              rows="3"
              maxlength="2000"
            ></textarea>
          </label>

          <label class="check">
            <input type="checkbox" bind:checked={consent} />
            <span>{consentText || $t("voice.consent")}</span>
          </label>

          <Button
            variant="primary"
            loading={busy === "create"}
            disabled={!consent ||
              sampleTooShort ||
              sampleTooLong ||
              recording ||
              missingForProfile > 0}
            onclick={createProfile}
          >
            <Icon name="mic" size={16} />
            {$t("voice.create")}
          </Button>
          {/if}

          <div class="import-row">
            <div class="import-copy">
              <strong>{$t("voice.import")}</strong>
              <span>{$t("voice.importHint")}</span>
            </div>
            <label class="check">
              <input type="checkbox" bind:checked={importConsent} />
              <span>{consentText || $t("voice.consent")}</span>
            </label>
            <Button
              variant="ghost"
              loading={busy === "import"}
              disabled={!importConsent}
              onclick={importProfile}
            >
              <Icon name="folder" size={16} />
              {$t("voice.import")}
            </Button>
          </div>
        </section>

        {#if profiles.length === 0 ||
          filteredProfiles.length === 0 ||
          otherProfiles.length > 0}
          <section class="panel">
            <div class="panel-head">
              <h2>{$t("voice.cloned")}</h2>
              <span class="pill">{filteredProfiles.length}</span>
            </div>
            {#if profiles.length === 0}
              <EmptyState
                title={$t("voice.noProfiles")}
                description={capabilities.cloning
                  ? $t("voice.createHint")
                  : $t("voice.importHint")}
                icon="voice"
              />
            {:else if filteredProfiles.length === 0}
              <EmptyState
                title={$t("voice.searchEmpty")}
                description={$t("voice.searchEmptyHint")}
                icon="search"
              />
            {:else}
              <div class="profiles">
                {#each otherProfiles as profile (profile.id)}
                  {@render profileCard(profile)}
                {/each}
              </div>
            {/if}
            {#if clonedVoices.length > 0}
              <p class="hint">{$t("voice.cloned")}: {clonedVoices.length}</p>
            {/if}
          </section>
        {/if}
      </div>

      <aside class="lab">
        <section class="rail-card">
          <div class="rail-head">
            <div class="rail-title">
              <span class="rail-icon">
                <Icon name="sparkles" size={18} />
              </span>
              <h2>{$t("voice.stepsTitle")}</h2>
            </div>
          </div>
          <ol class="steps">
            <li>
              <span>1</span>
              <div>
                <strong>{$t("voice.step1Title")}</strong>
                <p>{$t("voice.step1Body")}</p>
              </div>
            </li>
            <li>
              <span>2</span>
              <div>
                <strong>{$t("voice.step2Title")}</strong>
                <p>{$t("voice.step2Body")}</p>
              </div>
            </li>
            <li>
              <span>3</span>
              <div>
                <strong>{$t("voice.step3Title")}</strong>
                <p>{$t("voice.step3Body")}</p>
              </div>
            </li>
          </ol>
        </section>

        <section class="privacy-card">
          <span class="privacy-icon"><Icon name="lock" size={18} /></span>
          <div>
            <h3>{$t("voice.privacyTitle")}</h3>
            <p>{$t("voice.privacyBody")}</p>
          </div>
        </section>
      </aside>
    </div>
  {:else if tab === "models"}
    {#if !capabilities.modelDownload}
      <div class="platform-note" role="status">
        <span class="platform-note-icon"><Icon name="lock" size={17} /></span>
        <p>{$t("voice.modelManagementUnavailable")}</p>
      </div>
    {/if}
    {#if models.length === 0}
      <EmptyState
        title={$t("voice.modelsEmptyTitle")}
        description={$t("voice.modelsEmptyDesc")}
        icon="sliders"
      />
    {:else}
      <div class="models-overview">
        <div class="overview-copy">
          <h2>{$t("voice.models")}</h2>
          <p>{$t("voice.modelsDesc")}</p>
        </div>
        <div class="overview-progress">
          <div class="progress-label">
            <span>{$t("voice.modelsProgress")}</span>
            <strong>{installedModels}/{models.length}</strong>
          </div>
          <div class="progress-track">
            <span style={`width: ${modelsReadyPercent}%`}></span>
          </div>
        </div>
      </div>

      <div class="model-grid">
        {#each languages as language (language)}
          {@const missing = missingModels(language)}
          {@const percent = languageProgress(language)}
          <article class="panel model-card">
            <div class="panel-head">
              <h2>{languageLabel(language)}</h2>
              <span class="pill" class:ok={missing === 0}>
                {missing === 0
                  ? $t("voice.installed")
                  : $t("voice.modelsPending", { count: missing })}
              </span>
            </div>

            <div class="progress-track">
              <span style={`width: ${percent}%`}></span>
            </div>

            {#each modelsFor(language) as model (model.id)}
              <div class="model-row">
                <div class="model-copy">
                  <strong>{model.label}</strong>
                  <span>
                    {kindLabel(model.kind)} · {formatBytes(model.sizeBytes)} ·
                    {model.license}
                  </span>
                </div>
                {#if progress[model.id] !== undefined}
                  <div class="model-progress">
                    <div class="progress-track">
                      <span style={`width: ${progress[model.id]}%`}></span>
                    </div>
                    <small>{Math.round(progress[model.id])}%</small>
                  </div>
                {:else}
                  <span class="pill" class:ok={model.installed}>
                    {model.installed
                      ? $t("voice.installed")
                      : $t("voice.notInstalled")}
                  </span>
                  {#if capabilities.modelDownload}
                    <div class="model-actions">
                    {#if model.installed}
                      <Button
                        variant="ghost"
                        size="icon"
                        ariaLabel={$t("voice.repairModel", {
                          name: model.label,
                        })}
                        title={$t("voice.repairModel", { name: model.label })}
                        loading={busy === `model-${model.id}`}
                        onclick={() => repairModel(model)}
                      >
                        <Icon name="sync" size={15} />
                      </Button>
                      <Button
                        variant="danger"
                        size="icon"
                        ariaLabel={$t("voice.deleteModel", {
                          name: model.label,
                        })}
                        title={$t("voice.deleteModel", { name: model.label })}
                        loading={busy === `model-${model.id}`}
                        onclick={() => removeModel(model)}
                      >
                        <Icon name="trash" size={15} />
                      </Button>
                    {:else}
                      <Button
                        variant="ghost"
                        size="icon"
                        ariaLabel={$t("voice.installModel", {
                          name: model.label,
                        })}
                        title={$t("voice.installModel", { name: model.label })}
                        loading={busy === `model-${model.id}`}
                        onclick={() => downloadModel(model)}
                      >
                        <Icon name="download" size={15} />
                      </Button>
                    {/if}
                    </div>
                  {/if}
                {/if}
              </div>
            {/each}

            {#if missing > 0 && capabilities.modelDownload}
              <Button
                variant="primary"
                loading={busy === `models-${language}`}
                onclick={() => downloadModels(language)}
              >
                <Icon name="download" size={16} />
                {$t("voice.download")}
              </Button>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  {/if}
</section>

{#if details}
  <div
    class="details-backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) closeDetails();
    }}
  >
    <dialog
      open
      bind:this={detailsDialog}
      class="details-modal"
      aria-modal="true"
      aria-labelledby="voice-details-title"
      onkeydown={trapDetailsFocus}
    >
      <header class="details-head">
        <div>
          <span class="lumina-label">
            {$t(
              details.kind === "voice"
                ? "voice.systemVoice"
                : "voice.voiceClone",
            )}
          </span>
          <h2 id="voice-details-title">
            {details.kind === "voice"
              ? details.voice.name
              : details.profile.name}
          </h2>
        </div>
        <button
          type="button"
          class="details-close"
          aria-label={$t("common.close")}
          onclick={closeDetails}
        >
          <Icon name="close" size={18} />
        </button>
      </header>
      {#if details.kind === "profile" && renamingProfile}
        <form
          class="rename-form"
          onsubmit={(event) => {
            event.preventDefault();
            void saveRename();
          }}
        >
          <input
            bind:value={renameValue}
            maxlength="80"
            aria-label={$t("voice.rename")}
            placeholder={$t("voice.profilePlaceholder")}
          />
          <Button
            type="submit"
            variant="primary"
            size="small"
            loading={busy === "rename"}
            disabled={!renameValue.trim()}
          >
            {$t("common.save")}
          </Button>
          <Button variant="ghost" size="small" onclick={cancelRename}>
            {$t("common.cancel")}
          </Button>
        </form>
      {/if}
      {#if details.kind === "profile" && replacingSample}
        <div class="replace-sample">
          <div class="audio-picker" class:has-file={Boolean(replacePath)}>
            <span class="audio-picker-icon">
              <Icon name="mic" size={18} />
            </span>
            <div class="audio-picker-copy">
              <strong>{$t("voice.replaceSample")}</strong>
              <span title={replacePath}>
                {#if replacePath}
                  {fileName(replacePath)}
                  {#if replaceDurationMs !== null}
                    · {formatDuration(replaceDurationMs)}
                  {/if}
                {:else}
                  {$t("voice.replaceSampleHint")}
                {/if}
              </span>
            </div>
            <div class="audio-picker-actions">
              {#if replacePath}
                <Button
                  variant="ghost"
                  size="small"
                  onclick={toggleReplacePlayback}
                >
                  <Icon name={replacePlaying ? "pause" : "play"} size={16} />
                  {replacePlaying
                    ? $t("voice.pause")
                    : $t("voice.listenSample")}
                </Button>
              {/if}
              <Button
                variant="ghost"
                size="small"
                disabled={busy === "replace"}
                onclick={chooseReplaceAudio}
              >
                <Icon name="folder" size={16} />
                {$t("voice.chooseAudio")}
              </Button>
            </div>
          </div>
          {#if replaceTooShort}
            <p class="sample-error" role="alert">
              {$t("voice.sampleTooShort")}
            </p>
          {:else if replaceTooLong}
            <p class="sample-error" role="alert">
              {$t("voice.sampleTooLong")}
            </p>
          {/if}
          <textarea
            class="replace-reference"
            bind:value={replaceReferenceText}
            rows="3"
            maxlength="2000"
            placeholder={$t("voice.replaceReference")}
            aria-label={$t("voice.replaceReference")}
          ></textarea>
          <label class="check">
            <input type="checkbox" bind:checked={replaceConsent} />
            <span>{consentText || $t("voice.consent")}</span>
          </label>
          <div class="replace-actions">
            <Button
              variant="primary"
              size="small"
              loading={busy === "replace"}
              disabled={!replaceReady}
              onclick={saveReplaceSample}
            >
              {$t("voice.replaceSampleAction")}
            </Button>
            <Button variant="ghost" size="small" onclick={resetReplaceSample}>
              {$t("common.cancel")}
            </Button>
          </div>
        </div>
      {/if}
      <dl class="details-list">
        {#if details.kind === "voice"}
          <div>
            <dt>{$t("common.language")}</dt>
            <dd>{languageLabel(details.voice.language)}</dd>
          </div>
          <div>
            <dt>{$t("voice.filterGender")}</dt>
            <dd>{genderLabel(details.voice.gender ?? "neutral")}</dd>
          </div>
          <div>
            <dt>{$t("voice.engineTitle")}</dt>
            <dd>{details.voice.engine}</dd>
          </div>
          <div>
            <dt>{$t("voice.size")}</dt>
            <dd>{formatBytes(details.voice.sizeBytes)}</dd>
          </div>
          <div>
            <dt>{$t("voice.status")}</dt>
            <dd>
              {details.voice.installed
                ? $t("voice.installed")
                : $t("voice.notInstalled")}
            </dd>
          </div>
        {:else}
          <div>
            <dt>{$t("common.language")}</dt>
            <dd>{languageLabel(details.profile.language)}</dd>
          </div>
          <div>
            <dt>{$t("voice.engineTitle")}</dt>
            <dd>{details.profile.engine}</dd>
          </div>
          <div>
            <dt>{$t("voice.inputFormat")}</dt>
            <dd>{details.profile.inputFormat}</dd>
          </div>
          <div>
            <dt>{$t("voice.sampleDuration")}</dt>
            <dd>{formatDuration(details.profile.referenceDurationMs)}</dd>
          </div>
          <div>
            <dt>{$t("voice.createdAt")}</dt>
            <dd>{formatDate(details.profile.createdAt)}</dd>
          </div>
        {/if}
      </dl>
      <footer class="details-actions">
        <div class="details-actions-group">
          <Button
            variant="ghost"
            size="small"
            onclick={toggleFeaturedFromDetails}
          >
            <Icon name="star" size={16} />
            {detailsFeatured ? $t("voice.unfeature") : $t("voice.feature")}
          </Button>
          {#if details.kind === "profile"}
            <Button
              variant="ghost"
              size="small"
              loading={busy === `sample-${details.profile.id}`}
              onclick={toggleDetailsSampleFromModal}
            >
              <Icon name={detailsSampleActive ? "pause" : "play"} size={16} />
              {detailsSampleActive
                ? $t("voice.pause")
                : $t("voice.listenOriginal")}
            </Button>
            {#if capabilities.cloning}
              <Button variant="ghost" size="small" onclick={startReplaceSample}>
                <Icon name="mic" size={16} />
                {$t("voice.replaceSample")}
              </Button>
            {/if}
            <Button variant="ghost" size="small" onclick={startRename}>
              <Icon name="edit" size={16} />
              {$t("voice.rename")}
            </Button>
          {/if}
        </div>
        <Button variant="primary" size="small" onclick={closeDetails}>
          {$t("common.close")}
        </Button>
      </footer>
    </dialog>
  </div>
{/if}

<style>
  .voice-view {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    background:
      radial-gradient(
        28rem 28rem at 6% -8%,
        color-mix(in srgb, var(--accent) 12%, transparent) 0,
        transparent 70%
      ),
      radial-gradient(
        24rem 24rem at 102% 32%,
        color-mix(in srgb, var(--gold) 10%, transparent) 0,
        transparent 72%
      );
  }

  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1.25rem 2rem;
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 62ch;
  }

  .hero-kicker {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.85rem;
  }

  .studio-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.3rem 0.7rem;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }

  .engine-status {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    color: var(--muted);
    font-size: 0.71rem;
    font-weight: 500;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--border-strong);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--border-strong) 22%, transparent);
  }

  .status-dot.on {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 18%, transparent);
  }

  .hero .page-title {
    font-size: clamp(1.85rem, 3.1vw, 2.55rem);
  }

  .hero .page-subtitle {
    max-width: 60ch;
    margin-top: 0;
  }

  .hero-metrics {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.8rem 0.95rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface) 84%, transparent);
    box-shadow: var(--shadow-sm);
    backdrop-filter: blur(12px);
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 4.4rem;
  }

  .metric span {
    color: var(--muted);
    font-size: 0.58rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .metric strong {
    color: var(--text);
    font-family: var(--font-display);
    font-size: 1.3rem;
    font-weight: 600;
    line-height: 1.1;
  }

  .metric strong .metric-empty {
    color: var(--muted);
    font-family: var(--font-sans);
    font-size: 0.78rem;
    font-weight: 500;
  }

  .metric-average {
    color: var(--muted);
    font-size: 0.6rem;
    font-style: normal;
    font-variant-numeric: tabular-nums;
  }

  .metric-icon {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: var(--radius-md);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .studio-tabs {
    display: inline-flex;
    align-self: flex-start;
    gap: 0.25rem;
    max-width: 100%;
    padding: 0.35rem;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface-2) 90%, transparent);
    box-shadow: var(--shadow-xs);
    scrollbar-width: none;
  }

  .studio-tabs::-webkit-scrollbar {
    display: none;
  }

  .studio-tabs button {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    min-height: 42px;
    padding: 0.5rem 0.85rem;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.78rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .studio-tabs button:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .studio-tabs button.active {
    border-color: color-mix(in srgb, var(--accent-2) 70%, #3b1a00);
    background: linear-gradient(100deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    box-shadow: 0 10px 22px -14px
      color-mix(in srgb, var(--accent-2) 60%, transparent);
  }

  .tab-count {
    display: inline-grid;
    place-items: center;
    min-width: 1.2rem;
    height: 1.2rem;
    padding: 0 0.3rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 7%, transparent);
    color: inherit;
    font-size: 0.61rem;
    font-weight: 700;
  }

  .studio-tabs button.active .tab-count {
    background: color-mix(in srgb, #fff 22%, transparent);
  }

  .tab-count.attention {
    background: color-mix(in srgb, var(--warn) 15%, transparent);
    color: var(--warn);
  }

  .preview-bar {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.55rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface-2) 65%, transparent);
  }

  .preview-bar-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .preview-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0.15rem;
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
  }

  .preview-toggle:hover,
  .preview-toggle.active {
    color: var(--accent);
  }

  .preview-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }

  .preview-state {
    overflow: hidden;
    max-width: 45%;
    color: var(--muted);
    font-size: 0.68rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview-editor {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .preview-editor input {
    flex: 1 1 16rem;
    min-width: 0;
    min-height: 38px;
    padding: 0.45rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
  }

  .preview-editor input:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .preview-count {
    color: var(--muted);
    font-size: 0.64rem;
    font-variant-numeric: tabular-nums;
  }

  .preview-reset {
    padding: 0.25rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--muted);
    font: inherit;
    font-size: 0.68rem;
    font-weight: 600;
  }

  .preview-reset:hover:not(:disabled) {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .preview-reset:disabled {
    opacity: 0.45;
  }

  .preview-hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.45;
  }

  .studio-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.8rem 1rem;
    padding: 0.85rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    box-shadow: var(--shadow-xs);
    backdrop-filter: blur(10px);
  }

  .toolbar-actions,
  .toolbar-filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }

  .filter-select {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.1rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    color: var(--muted);
  }

  .filter-select select {
    min-height: 34px;
    padding: 0.2rem 1.3rem 0.2rem 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: var(--text);
    font-size: 0.75rem;
    font-weight: 600;
  }

  .filter-search {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-width: min(100%, 13rem);
    padding: 0.1rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    color: var(--muted);
  }

  .filter-search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .filter-search input {
    flex: 1;
    min-width: 0;
    min-height: 34px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.75rem;
  }

  .filter-search input:focus-visible {
    outline: none;
  }

  .search-clear {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--muted);
  }

  .search-clear:hover {
    color: var(--text);
  }

  .toolbar-label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .rate-group {
    display: inline-flex;
    align-items: center;
    gap: 0.15rem;
    padding: 0.2rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    color: var(--muted);
  }

  .rate-group button {
    min-height: 32px;
    padding: 0.25rem 0.45rem;
    border: 0;
    border-radius: calc(var(--radius-md) - 4px);
    background: transparent;
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 600;
  }

  .rate-group button.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .filter-segment {
    display: inline-flex;
    gap: 0.15rem;
    padding: 0.2rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .filter-segment button {
    min-height: 34px;
    padding: 0.3rem 0.7rem;
    border: 0;
    border-radius: calc(var(--radius-md) - 4px);
    background: transparent;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .filter-segment button.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-xs);
  }

  .section-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem 1rem;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }

  .section-title h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    font-weight: 550;
    letter-spacing: -0.015em;
  }

  .section-note {
    color: var(--muted);
    font-size: 0.7rem;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    min-height: 22px;
    padding: 0.12rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 0.63rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .pill.ok {
    border-color: color-mix(in srgb, var(--ok) 45%, var(--border));
    background: color-mix(in srgb, var(--ok) 8%, transparent);
    color: var(--ok);
  }

  .studio-grid {
    display: grid;
    grid-template-columns: minmax(0, 2.05fr) minmax(280px, 0.95fr);
    gap: 1.25rem;
    align-items: start;
  }

  .catalog,
  .cloned-main,
  .lab {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    min-width: 0;
  }

  .voice-card {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.9rem 1.20rem;
    padding: 1.05rem 1.15rem;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow: var(--shadow-xs);
    transition:
      border-color 150ms ease,
      box-shadow 150ms ease,
      transform 150ms ease;
  }

  .voice-card:hover {
    transform: translateY(-1px);
    border-color: var(--accent-line);
    box-shadow: var(--shadow-sm);
  }

  .voice-card.featured {
    border-color: color-mix(in srgb, var(--gold) 42%, var(--border));
    box-shadow:
      var(--shadow-sm),
      0 0 0 1px color-mix(in srgb, var(--gold) 14%, transparent);
  }

  .voice-card.featured::before {
    content: "";
    position: absolute;
    inset: 0 auto 0 0;
    width: 5px;
    background: linear-gradient(180deg, var(--gold), var(--accent));
  }

  .voice-card.playing {
    border-color: var(--accent-line);
  }

  .voice-main {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    min-width: 0;
  }

  .voice-avatar {
    position: relative;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 64px;
    height: 64px;
    border-radius: var(--radius-lg);
    background: linear-gradient(145deg, var(--surface-3), var(--surface-2));
    color: var(--muted);
    box-shadow: inset 0 0 0 1px var(--border);
  }

  .voice-avatar.installed {
    background: linear-gradient(
      145deg,
      color-mix(in srgb, var(--accent) 20%, var(--surface-2)),
      color-mix(in srgb, var(--gold) 10%, var(--surface-2))
    );
    color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent-line);
  }

  .voice-avatar .initials {
    font-family: var(--font-display);
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1;
  }

  .voice-dot {
    position: absolute;
    right: 6px;
    bottom: 6px;
    width: 12px;
    height: 12px;
    border: 2px solid var(--surface);
    border-radius: 50%;
    background: var(--border-strong);
  }

  .voice-dot.on {
    background: var(--ok);
  }

  .voice-copy {
    display: flex;
    flex-direction: column;
    gap: 0.26rem;
    min-width: 0;
  }

  .voice-title-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.38rem;
  }

  .voice-title-row h3 {
    margin: 0;
    font-family: var(--font-sans);
    font-size: 1.02rem;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .pill.language {
    border-color: transparent;
    background: var(--surface-3);
    color: var(--muted);
  }

  .pill.featured-pill {
    border-color: transparent;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .voice-desc {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .voice-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 0.7rem;
    color: var(--muted);
    font-size: 0.67rem;
  }

  .voice-meta span {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
  }

  .voice-download {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.45rem;
  }

  .voice-download-track {
    flex: 1;
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .voice-download-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--gold), var(--accent));
    transition: width 150ms linear;
  }

  .voice-download small {
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .voice-actions {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-width: 9.5rem;
  }

  .voice-actions :global(.ui-button) {
    width: 100%;
    justify-content: center;
  }

  .profile-secondary {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .more-menu {
    display: none;
  }

  .more-menu summary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    min-height: 36px;
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    color: var(--text);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    list-style: none;
  }

  .more-menu summary::-webkit-details-marker {
    display: none;
  }

  .more-menu-body {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin-top: 0.4rem;
  }

  .wave-strip {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.5rem 0.65rem;
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .wave-label {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.66rem;
    white-space: nowrap;
  }

  .wave-label strong {
    color: var(--accent);
    font-weight: 700;
  }

  .wave-bars {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 2px;
    height: 26px;
    min-width: 0;
    overflow: hidden;
  }

  .wave-bars span {
    flex: 1;
    min-width: 2px;
    max-width: 4px;
    height: 70%;
    border-radius: 2px;
    background: linear-gradient(180deg, var(--gold), var(--accent));
    opacity: 0.5;
    transform-origin: center;
  }

  .wave-strip.playing .wave-bars span {
    opacity: 1;
    animation: wave 1s ease-in-out infinite alternate;
  }

  .wave-bars span:nth-child(3n) {
    height: 55%;
    animation-delay: -0.2s;
  }

  .wave-bars span:nth-child(3n + 1) {
    height: 35%;
    animation-delay: -0.45s;
  }

  .wave-bars span:nth-child(4n) {
    height: 92%;
    animation-delay: -0.62s;
  }

  .wave-bars span:nth-child(5n) {
    height: 48%;
    animation-delay: -0.3s;
  }

  @keyframes wave {
    from {
      transform: scaleY(0.35);
      opacity: 0.55;
    }
    to {
      transform: scaleY(1);
      opacity: 1;
    }
  }

  .wave-stop {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 28px;
    min-width: 28px;
    height: 28px;
    min-height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--surface);
    color: var(--muted);
    box-shadow: var(--shadow-xs);
  }

  .wave-stop:hover {
    color: var(--accent);
  }

  @media (pointer: coarse) {
    .wave-stop {
      width: 44px;
      min-width: 44px;
      height: 44px;
      min-height: 44px;
    }
  }

  .rail-card,
  .privacy-card,
  .panel {
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow: var(--shadow-sm);
  }

  .rail-card {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    padding: 1.05rem;
  }

  .rail-head,
  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
  }

  .rail-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .rail-title h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.16rem;
    font-weight: 550;
    letter-spacing: -0.01em;
  }

  .rail-icon {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .rail-desc {
    margin: 0;
    color: var(--muted);
    font-size: 0.73rem;
    line-height: 1.5;
  }

  .engine-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .engine-row {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.6rem 0.65rem;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .engine-row.ready {
    background: color-mix(in srgb, var(--ok) 6%, var(--surface-2));
  }

  .engine-state {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 1.3rem;
    height: 1.3rem;
    margin-top: 0.08rem;
    border-radius: 50%;
    background: var(--surface);
    color: var(--muted);
    box-shadow: inset 0 0 0 1px var(--border);
  }

  .engine-row.ready .engine-state {
    background: color-mix(in srgb, var(--ok) 13%, var(--surface));
    color: var(--ok);
    box-shadow: none;
  }

  .engine-row div {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 0.08rem;
    min-width: 0;
  }

  .engine-row strong {
    font-size: 0.75rem;
    font-weight: 650;
    line-height: 1.3;
  }

  .engine-row span {
    color: var(--muted);
    font-size: 0.65rem;
    line-height: 1.4;
  }

  .rail-progress {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding-top: 0.7rem;
    border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  }

  .progress-row,
  .overview-progress {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .progress-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    color: var(--text);
    font-size: 0.68rem;
    font-weight: 500;
  }

  .progress-label strong {
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 700;
  }

  .progress-track {
    height: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-4);
  }

  .progress-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent-2), var(--gold));
    transition: width 300ms ease;
  }

  .privacy-card {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.95rem 1rem;
    background: color-mix(in srgb, var(--surface-3) 72%, transparent);
    box-shadow: none;
  }

  .privacy-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 1.9rem;
    height: 1.9rem;
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .privacy-card div {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .privacy-card h3 {
    margin: 0;
    font-family: var(--font-sans);
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0;
  }

  .privacy-card p {
    margin: 0;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.5;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding: 1.05rem;
  }

  .panel h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.2rem;
    font-weight: 550;
    letter-spacing: -0.01em;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.75rem;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .models-warning {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.65rem;
    padding: 0.7rem 0.75rem;
    border: 1px solid color-mix(in srgb, var(--warn) 38%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--warn) 7%, var(--surface-2));
  }

  .models-warning-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 2.1rem;
    height: 2.1rem;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--warn) 14%, var(--surface));
    color: var(--warn);
  }

  .models-warning > div:first-of-type {
    flex: 1 1 12rem;
    min-width: 0;
  }

  .models-warning strong {
    display: block;
    font-size: 0.76rem;
    line-height: 1.4;
  }

  .models-warning p {
    margin: 0.1rem 0 0;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.45;
  }

  .models-warning-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .mic-select {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .mic-select > span {
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .mic-select select {
    flex: 1;
    min-width: 0;
    min-height: 36px;
    padding: 0.4rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.75rem;
  }

  .mic-select select:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .level-meter {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }

  .level-meter > span {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .level-track {
    flex: 1;
    height: 7px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .level-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--ok), var(--warn));
    transition: width 120ms linear;
  }

  .level-track span.hot {
    background: var(--danger);
  }

  .level-warning {
    margin: 0;
    color: var(--warn);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .audio-picker {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.65rem;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .audio-picker.has-file {
    border-style: solid;
    border-color: var(--accent-line);
  }

  .audio-picker.recording {
    border-style: solid;
    border-color: color-mix(in srgb, var(--danger) 55%, var(--border));
    background: color-mix(in srgb, var(--danger) 5%, var(--surface-2));
  }

  .audio-picker-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 2.35rem;
    height: 2.35rem;
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .audio-picker-icon.live {
    background: color-mix(in srgb, var(--danger) 12%, var(--surface));
  }

  .record-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--danger);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--danger) 16%, transparent);
    animation: recording-pulse 1.2s ease-in-out infinite;
  }

  @keyframes recording-pulse {
    50% {
      opacity: 0.45;
      transform: scale(0.8);
    }
  }

  .audio-picker-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.4rem;
  }

  .sample-error {
    margin: 0;
    color: var(--danger);
    font-size: 0.72rem;
    line-height: 1.45;
  }

  .sample-warnings {
    margin: 0;
    padding-left: 1.1rem;
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.5;
  }

  .audio-picker-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 0.08rem;
    min-width: 0;
  }

  .audio-picker-copy strong {
    font-size: 0.76rem;
  }

  .audio-picker-copy span {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.67rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .form-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 0.8fr);
    gap: 0.7rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .field input,
  .field select,
  .field textarea {
    min-height: 40px;
    padding: 0.48rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.83rem;
    font-weight: 400;
  }

  .field input:focus-visible,
  .field select:focus-visible,
  .field textarea:focus-visible {
    border-color: var(--accent);
    outline: none;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .check {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.6rem 0.7rem;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.72rem;
    font-weight: 400;
    line-height: 1.45;
  }

  .check input {
    width: 17px;
    height: 17px;
    margin-top: 0.1rem;
    flex: 0 0 auto;
  }

  .import-row {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding-top: 0.75rem;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .import-copy {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
  }

  .import-copy strong {
    font-size: 0.78rem;
  }

  .import-copy span {
    color: var(--muted);
    font-size: 0.7rem;
    line-height: 1.45;
  }

  .profiles {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .profile-card .voice-avatar {
    width: 56px;
    height: 56px;
  }

  .profile-card .voice-avatar .initials {
    font-size: 1.3rem;
  }

  .steps {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .steps li {
    display: flex;
    align-items: flex-start;
    gap: 0.55rem;
  }

  .steps li > span {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.64rem;
    font-weight: 700;
  }

  .steps div {
    display: flex;
    flex-direction: column;
    gap: 0.08rem;
  }

  .steps strong {
    font-size: 0.75rem;
    line-height: 1.35;
  }

  .steps p {
    margin: 0;
    color: var(--muted);
    font-size: 0.67rem;
    line-height: 1.45;
  }

  .models-overview {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    box-shadow: var(--shadow-xs);
  }

  .overview-copy h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    font-weight: 550;
    letter-spacing: -0.015em;
  }

  .overview-copy p {
    max-width: 60ch;
    margin: 0.2rem 0 0;
    color: var(--muted);
    font-size: 0.75rem;
    line-height: 1.5;
  }

  .overview-progress {
    flex: 1;
    min-width: min(100%, 16rem);
  }

  .model-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 1rem;
  }

  .model-card {
    gap: 0.7rem;
  }

  .model-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.6rem 0;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .model-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .model-copy strong {
    font-size: 0.79rem;
    line-height: 1.35;
  }

  .model-copy span {
    color: var(--muted);
    font-size: 0.65rem;
    line-height: 1.4;
  }

  .model-progress {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 6.5rem;
  }

  .model-actions {
    display: flex;
    flex-shrink: 0;
    gap: 0.25rem;
  }

  .model-progress .progress-track {
    flex: 1;
  }

  .model-progress small {
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 700;
  }

  .details-backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 1.25rem;
    background: color-mix(in srgb, #120b04 58%, transparent);
    backdrop-filter: blur(6px);
  }

  .details-modal {
    position: static;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    width: min(100%, 26rem);
    margin: 0;
    padding: 1.15rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }

  .details-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .details-head h2 {
    margin: 0.15rem 0 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    font-weight: 550;
    letter-spacing: -0.015em;
  }

  .details-close {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
  }

  .details-close:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .details-list {
    display: flex;
    flex-direction: column;
    margin: 0;
  }

  .details-list div {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.55rem 0;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .details-list div:first-child {
    padding-top: 0;
    border-top: 0;
  }

  .details-list dt {
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 600;
  }

  .details-list dd {
    margin: 0;
    color: var(--text);
    font-size: 0.78rem;
    font-weight: 500;
    text-align: right;
    overflow-wrap: anywhere;
  }

  .details-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding-top: 0.75rem;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .details-actions-group {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .rename-form {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .rename-form input {
    flex: 1 1 12rem;
    min-width: 0;
    min-height: 36px;
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  .rename-form input:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .replace-sample {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.65rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .replace-reference {
    width: 100%;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
    resize: vertical;
  }

  .replace-reference:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .replace-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .platform-note {
    display: flex;
    align-items: flex-start;
    gap: 0.55rem;
    padding: 0.75rem 0.85rem;
    border: 1px solid color-mix(in srgb, var(--warn) 32%, var(--border));
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--warn) 7%, var(--surface-2));
  }

  .platform-note-icon {
    flex: 0 0 auto;
    margin-top: 0.05rem;
    color: var(--warn);
  }

  .platform-note p {
    margin: 0;
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.5;
  }

  @media (max-width: 1020px) {
    .studio-grid {
      grid-template-columns: 1fr;
    }

    .lab {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      align-items: start;
    }
  }

  @media (max-width: 760px) {
    .hero-metrics {
      width: 100%;
      justify-content: space-between;
    }

    .metric {
      flex: 1;
      min-width: 0;
    }

    .voice-card {
      grid-template-columns: 1fr;
      align-items: stretch;
    }

    .voice-actions {
      flex-direction: row;
      flex-wrap: wrap;
      min-width: 0;
    }

    .voice-actions :global(.ui-button) {
      width: auto;
      flex: 1;
      justify-content: center;
    }

    .profile-secondary {
      display: none;
    }

    .more-menu {
      display: block;
      flex: 1 1 100%;
    }

    .more-menu summary {
      width: 100%;
    }

    .more-menu-body :global(.ui-button) {
      width: 100%;
      flex: none;
    }

    .form-grid {
      grid-template-columns: 1fr;
    }

    .audio-picker {
      flex-wrap: wrap;
    }

    .audio-picker-copy {
      flex-basis: calc(100% - 3rem);
    }

    .audio-picker-actions {
      flex-basis: 100%;
      justify-content: flex-start;
    }
  }

  @media (max-width: 620px) {
    .hero-metrics {
      gap: 0.65rem;
      padding: 0.7rem 0.75rem;
    }

    .metric strong {
      font-size: 1.1rem;
    }

    .metric-icon {
      display: none;
    }

    .studio-tabs {
      width: calc(100% + 32px);
      max-width: none;
      margin-inline: -16px;
      padding: 0.3rem 16px;
      border-inline: 0;
      border-radius: 0;
      background: transparent;
      box-shadow: none;
    }

    .studio-tabs button {
      flex: 0 0 auto;
      min-height: 40px;
      background: var(--surface-2);
    }

    .studio-tabs button.active {
      background: linear-gradient(100deg, var(--accent), var(--accent-2));
      color: var(--accent-contrast);
    }

    .studio-toolbar {
      padding: 0.7rem;
    }

    .toolbar-actions,
    .toolbar-filters {
      width: 100%;
    }

    .toolbar-actions :global(.ui-button) {
      flex: 1;
    }

    .filter-select,
    .filter-segment {
      flex: 1;
    }

    .filter-search {
      flex: 1 1 100%;
    }

    .rate-group {
      flex: 1;
      justify-content: center;
    }

    .filter-select select {
      flex: 1;
      min-width: 0;
    }

    .filter-segment button {
      flex: 1;
    }

    .voice-avatar {
      width: 54px;
      height: 54px;
    }

    .voice-avatar .initials {
      font-size: 1.25rem;
    }

    .wave-strip {
      flex-wrap: wrap;
    }

    .wave-bars {
      order: 3;
      flex-basis: 100%;
    }

    .model-grid {
      grid-template-columns: 1fr;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .voice-card:hover {
      transform: none;
    }

    .wave-bars span,
    .wave-strip.playing .wave-bars span {
      animation: none;
      opacity: 0.75;
    }

    .record-dot {
      animation: none;
    }
  }
</style>

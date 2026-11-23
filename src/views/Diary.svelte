<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    diaryCreate,
    diaryDelete,
    diaryUpdate,
    errorMessage,
    exportDiary,
    getAchievements,
    getDiary,
    getPomodoro,
    startPomodoro,
    stopPomodoro,
    ttsVoices,
  } from "../lib/api";
  import {
    formatDate,
    locale,
    t,
    translate,
    type MessageKey,
  } from "../lib/i18n";
  import Icon from "../lib/components/Icon.svelte";
  import AchievementIcon from "../lib/components/AchievementIcon.svelte";
  import type {
    Achievement,
    DiaryDay,
    DiaryEntry,
    DiaryExport,
    DiaryExportProgress,
    DiaryMonth,
    Locator,
    PomodoroConfig,
    PomodoroStatus,
    VoiceDescriptor,
  } from "../lib/types";

  let {
    onRead,
  }: {
    onRead?: (bookId: string, locator?: Locator) => void;
  } = $props();

  function localIsoDate(date = new Date()): string {
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  }

  let todayIso = $state(localIsoDate());
  let year = $state(new Date().getFullYear());
  let month = $state(new Date().getMonth() + 1);
  let selectedDate = $state(localIsoDate());
  let monthLabel = $derived.by(() => {
    const monthName = new Intl.DateTimeFormat($locale, {
      month: "long",
    }).format(new Date(year, month - 1, 1));
    return `${monthName.charAt(0).toLocaleUpperCase($locale)}${monthName.slice(1)} ${year}`;
  });
  let weekdayLabels = $derived.by(() => {
    $locale;
    return Array.from({ length: 7 }, (_, index) =>
      new Intl.DateTimeFormat($locale, { weekday: "short" })
        .format(new Date(2024, 0, 1 + index))
        .replace(".", "")
        .slice(0, 2),
    );
  });

  let diary = $state<DiaryMonth | null>(null);
  let achievements = $state<Achievement[]>([]);
  let pomodoro = $state<PomodoroStatus | null>(null);
  let voices = $state<VoiceDescriptor[]>([]);
  let voiceId = $state("");
  let includeAudiobook = $state(false);
  let exportScope = $state<"month" | "year">("month");
  let exporting = $state(false);
  let exportProgress = $state<DiaryExportProgress | null>(null);
  let exportResult = $state<DiaryExport | null>(null);
  let notice = $state<string | null>(null);
  let error = $state<string | null>(null);
  let diaryRequest = 0;
  let pomodoroBusy = $state(false);
  let noteOpen = $state(false);
  let noteEditingId = $state<string | null>(null);
  let noteDate = $state("");
  let noteBody = $state("");
  let noteMood = $state("");
  let noteBusy = $state(false);
  let nowMs = $state(Date.now());
  let config = $state<PomodoroConfig>({
    focusMinutes: 25,
    breakMinutes: 5,
    longBreakMinutes: 15,
    roundsBeforeLongBreak: 4,
    autoStartBreaks: true,
    autoStartFocus: false,
  });
  let timer: ReturnType<typeof setInterval> | undefined;
  let unlisten: UnlistenFn[] = [];
  let ambience = $state<"none" | "rain" | "white" | "fire">("none");
  let ambienceVolume = $state(35);
  let audioContext: AudioContext | null = null;
  let ambienceSource: AudioBufferSourceNode | null = null;
  let ambienceGain: GainNode | null = null;

  let dayEntries = $derived(
    diary?.entries.filter((entry) => entry.entryDate === selectedDate) ?? [],
  );
  let leadingBlanks = $derived((new Date(year, month - 1, 1).getDay() + 6) % 7);
  let installedVoices = $derived(voices.filter((voice) => voice.installed));
  let remaining = $derived.by(() => {
    if (!pomodoro?.endsAt) return 0;
    return Math.max(
      0,
      Math.floor((new Date(pomodoro.endsAt).getTime() - nowMs) / 1000),
    );
  });
  let pomodoroRunning = $derived(
    pomodoro !== null && pomodoro.state !== "idle",
  );
  let displayRemaining = $derived(
    pomodoroRunning
      ? remaining
      : Math.max(0, Math.round(config.focusMinutes * 60)),
  );
  let phaseKey = $derived<MessageKey>(
    pomodoro?.state === "focus"
      ? "diary.focus"
      : pomodoro?.state === "break"
        ? "diary.break"
        : pomodoro?.state === "long_break"
          ? "diary.longBreak"
          : "diary.idle",
  );
  let phaseTotal = $derived.by(() => {
    if (!pomodoro || pomodoro.state === "idle") return 0;
    const source = pomodoro.config ?? config;
    const minutes =
      pomodoro.state === "focus"
        ? source.focusMinutes
        : pomodoro.state === "break"
          ? source.breakMinutes
          : source.longBreakMinutes;
    return Math.max(1, minutes * 60);
  });
  let phaseRatio = $derived(
    phaseTotal > 0
      ? Math.min(1, Math.max(0, 1 - remaining / phaseTotal))
      : 0,
  );

  onMount(() => {
    void load();
    void initEvents();
    timer = setInterval(() => {
      nowMs = Date.now();
      const currentDay = localIsoDate();
      if (currentDay !== todayIso) todayIso = currentDay;
    }, 1000);
    return () => {
      for (const stop of unlisten) stop();
      if (timer) clearInterval(timer);
      stopAmbience();
      if (audioContext) {
        void audioContext.close();
        audioContext = null;
      }
    };
  });

  async function initEvents() {
    unlisten.push(
      await listen<PomodoroStatus>("pomodoro::state", (event) => {
        pomodoro = event.payload;
        config = { ...event.payload.config };
        nowMs = Date.now();
        void loadAchievements();
      }),
    );
    unlisten.push(
      await listen("diary::updated", () => {
        void loadDiary();
        void loadAchievements();
      }),
    );
    unlisten.push(
      await listen<DiaryExportProgress>("diary::export-progress", (event) => {
        exportProgress = event.payload;
      }),
    );
  }

  async function load() {
    const request = ++diaryRequest;
    error = null;
    const [monthResult, achievementResult, pomodoroResult, voiceResult] =
      await Promise.allSettled([
        getDiary(year, month),
        getAchievements(),
        getPomodoro(),
        ttsVoices(),
      ]);
    if (request === diaryRequest && monthResult.status === "fulfilled") {
      diary = monthResult.value;
      ensureSelection(monthResult.value);
    }
    if (achievementResult.status === "fulfilled") {
      achievements = achievementResult.value;
    }
    if (pomodoroResult.status === "fulfilled" && !pomodoro) {
      pomodoro = pomodoroResult.value;
      config = { ...pomodoroResult.value.config };
    }
    if (voiceResult.status === "fulfilled") {
      voices = voiceResult.value;
      if (!voiceId) {
        voiceId = voiceResult.value.find((voice) => voice.installed)?.id ?? "";
      }
    }
    if (request === diaryRequest) {
      const failure = [
        monthResult,
        achievementResult,
        pomodoroResult,
        voiceResult,
      ].find((result) => result.status === "rejected");
      if (failure) error = errorMessage(failure.reason);
    }
  }

  async function loadDiary() {
    const request = ++diaryRequest;
    try {
      const monthData = await getDiary(year, month);
      if (request !== diaryRequest) return;
      diary = monthData;
      ensureSelection(diary);
    } catch (cause) {
      if (request === diaryRequest) error = errorMessage(cause);
    }
  }

  async function loadAchievements() {
    try {
      achievements = await getAchievements();
    } catch {
      // El catálogo se recupera en la siguiente carga.
    }
  }

  function ensureSelection(monthData: DiaryMonth) {
    const current = monthData.days.find((day) => day.date === selectedDate);
    if (current) return;
    selectedDate =
      monthData.days.find((day) => day.entries > 0)?.date ??
      monthData.days[0]?.date ??
      "";
  }

  async function changeMonth(delta: number) {
    let nextMonth = month + delta;
    let nextYear = year;
    if (nextMonth < 1) {
      nextMonth = 12;
      nextYear -= 1;
    } else if (nextMonth > 12) {
      nextMonth = 1;
      nextYear += 1;
    }
    month = nextMonth;
    year = nextYear;
    selectedDate = "";
    await loadDiary();
  }

  async function goToday() {
    const current = new Date();
    year = current.getFullYear();
    month = current.getMonth() + 1;
    selectedDate = localIsoDate(current);
    todayIso = selectedDate;
    await loadDiary();
  }

  function selectDay(day: DiaryDay) {
    selectedDate = day.date;
  }

  function kindLabel(kind: DiaryEntry["kind"]) {
    if (kind === "reading") return translate("diary.kindReading");
    if (kind === "listening") return translate("diary.kindListening");
    if (kind === "pomodoro") return translate("diary.kindPomodoro");
    return translate("diary.kindManual");
  }

  function entryCountLabel(count: number) {
    return count === 1
      ? translate("diary.entryOne")
      : translate("diary.entriesCount", { count });
  }

  function shortDate(date: string) {
    return formatDate(`${date}T12:00:00`, {
      weekday: "long",
      day: "numeric",
      month: "long",
    });
  }

  function formatCountdown(seconds: number) {
    const safe = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
    const minutes = Math.floor(safe / 60);
    const rest = safe % 60;
    return `${String(minutes).padStart(2, "0")}:${String(rest).padStart(2, "0")}`;
  }

  function applyPreset(focus: number, pause: number, long: number) {
    config = {
      ...config,
      focusMinutes: focus,
      breakMinutes: pause,
      longBreakMinutes: long,
    };
  }

  function stopAmbience() {
    try {
      ambienceSource?.stop();
    } catch {
      // La fuente ya estaba detenida.
    }
    ambienceSource?.disconnect();
    ambienceGain?.disconnect();
    ambienceSource = null;
    ambienceGain = null;
    ambience = "none";
  }

  function startAmbience(kind: "rain" | "white" | "fire") {
    if (ambience === kind) {
      stopAmbience();
      return;
    }
    stopAmbience();
    try {
      audioContext ??= new AudioContext();
      void audioContext.resume();
      const sampleRate = audioContext.sampleRate;
      const buffer = audioContext.createBuffer(1, sampleRate * 4, sampleRate);
      const data = buffer.getChannelData(0);
      let brown = 0;
      for (let index = 0; index < data.length; index += 1) {
        const white = Math.random() * 2 - 1;
        if (kind === "fire") {
          brown = (brown + 0.02 * white) / 1.02;
          const crackle = Math.random() < 0.0009 ? white * 2.2 : 0;
          data[index] = brown * 3.4 + crackle;
        } else {
          data[index] = white * 0.7;
        }
      }
      const source = audioContext.createBufferSource();
      source.buffer = buffer;
      source.loop = true;
      const filter = audioContext.createBiquadFilter();
      filter.type = kind === "white" ? "highpass" : "lowpass";
      filter.frequency.value = kind === "rain" ? 1800 : kind === "fire" ? 900 : 160;
      const gain = audioContext.createGain();
      gain.gain.value = (ambienceVolume / 100) * 0.18;
      source.connect(filter);
      filter.connect(gain);
      gain.connect(audioContext.destination);
      source.start();
      ambienceSource = source;
      ambienceGain = gain;
      ambience = kind;
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function setAmbienceVolume(value: number) {
    ambienceVolume = value;
    if (ambienceGain) {
      ambienceGain.gain.value = (value / 100) * 0.18;
    }
  }

  async function togglePomodoro() {
    if (pomodoroBusy) return;
    pomodoroBusy = true;
    error = null;
    try {
      const status =
        !pomodoro || pomodoro.state === "idle"
          ? await startPomodoro(config)
          : await stopPomodoro();
      if (status) {
        pomodoro = status;
        config = { ...status.config };
      }
      nowMs = Date.now();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      pomodoroBusy = false;
    }
  }

  async function exportNow() {
    exporting = true;
    error = null;
    exportResult = null;
    exportProgress = null;
    try {
      exportResult = await exportDiary(
        year,
        exportScope === "month" ? month : null,
        includeAudiobook,
        includeAudiobook ? voiceId : null,
      );
      notice = translate("diary.exported", { count: exportResult.entries });
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      exporting = false;
      exportProgress = null;
    }
  }

  function openEntry(entry: DiaryEntry) {
    if (entry.bookId) onRead?.(entry.bookId);
  }

  function startManualEntry() {
    noteEditingId = null;
    noteDate = selectedDate || todayIso;
    noteBody = "";
    noteMood = "";
    noteOpen = true;
  }

  function startEditEntry(entry: DiaryEntry) {
    if (entry.kind !== "manual") return;
    noteEditingId = entry.id;
    noteDate = entry.entryDate;
    noteBody = entry.body;
    noteMood = entry.mood ?? "";
    noteOpen = true;
  }

  function cancelManualEntry() {
    noteOpen = false;
    noteEditingId = null;
    noteBody = "";
    noteMood = "";
  }

  async function saveManualEntry() {
    if (!noteBody.trim()) return;
    noteBusy = true;
    error = null;
    try {
      if (noteEditingId) {
        await diaryUpdate(noteEditingId, noteBody, noteMood.trim() || null);
        notice = translate("diary.manualUpdated");
      } else {
        await diaryCreate(noteDate, noteBody, noteMood.trim() || null);
        selectedDate = noteDate;
        const [createdYear, createdMonth] = noteDate.split("-").map(Number);
        if (createdYear && createdMonth) {
          year = createdYear;
          month = createdMonth;
        }
        notice = translate("diary.manualCreated");
      }
      cancelManualEntry();
      await loadDiary();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      noteBusy = false;
    }
  }

  async function deleteEntry(entry: DiaryEntry) {
    if (!window.confirm(translate("diary.manualDeleteConfirm"))) return;
    try {
      await diaryDelete(entry.id);
      if (noteEditingId === entry.id) cancelManualEntry();
      await loadDiary();
      notice = translate("diary.manualDeleted");
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  function unlockLabel(value: string | null) {
    if (!value) return null;
    return formatDate(value, {
      day: "numeric",
      month: "short",
      year: "numeric",
    });
  }
</script>

<section class="diary">
  <header class="hero">
    <div>
      <p class="eyebrow">{$t("diary.eyebrow")}</p>
      <h1 class="page-title">{$t("diary.title")}</h1>
      <p class="page-subtitle">{$t("diary.subtitle")}</p>
    </div>
    <div class="month-nav">
      <button class="ghost" onclick={() => changeMonth(-1)} aria-label={$t("diary.prevMonth")}>‹</button>
      <strong>{monthLabel}</strong>
      <button class="ghost" onclick={() => changeMonth(1)} aria-label={$t("diary.nextMonth")}>›</button>
      <button class="ghost small" onclick={goToday}>{$t("diary.today")}</button>
    </div>
  </header>

  {#if error}
    <p class="notice" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="ok" role="status">{notice}</p>
  {/if}

  <div class="stats">
    <article class="stat">
      <span class="stat-icon"><Icon name="diary" size={16} /></span>
      <span class="stat-value">{diary?.totals.minutes ?? 0}</span>
      <small>{$t("diary.minutes")}</small>
    </article>
    <article class="stat">
      <span class="stat-icon"><Icon name="book" size={16} /></span>
      <span class="stat-value">{diary?.totals.pages ?? 0}</span>
      <small>{$t("diary.pages")}</small>
    </article>
    <article class="stat">
      <span class="stat-icon"><Icon name="list" size={16} /></span>
      <span class="stat-value">{diary?.totals.extracts ?? 0}</span>
      <small>{$t("diary.extracts")}</small>
    </article>
    <article class="stat">
      <span class="stat-icon"><Icon name="diary" size={16} /></span>
      <span class="stat-value">{diary?.totals.notes ?? 0}</span>
      <small>{$t("diary.notes")}</small>
    </article>
    <article class="stat">
      <span class="stat-icon"><Icon name="check" size={16} /></span>
      <span class="stat-value">{diary?.totals.activeDays ?? 0}</span>
      <small>{$t("diary.activeDays")}</small>
    </article>
  </div>

  <div class="layout">
    <article class="panel heatmap-panel">
      <div class="panel-head">
        <h2>{$t("diary.calendar")}</h2>
        <span class="hint">{entryCountLabel(diary?.totals.entries ?? 0)}</span>
      </div>
      <div class="weekdays">
        {#each weekdayLabels as day}<span>{day}</span>{/each}
      </div>
      <div class="heatmap">
        {#each Array(leadingBlanks) as _}<span class="blank"></span>{/each}
        {#each diary?.days ?? [] as day (day.date)}
          <button
            class="day level-{day.level}"
            class:selected={selectedDate === day.date}
            class:today={day.date === todayIso}
            aria-pressed={selectedDate === day.date}
            title={`${shortDate(day.date)} · ${day.minutes} ${$t("diary.minutesShort")} · ${entryCountLabel(day.entries)}`}
            onclick={() => selectDay(day)}
          >
            {Number(day.date.slice(-2))}
          </button>
        {/each}
      </div>
      <div class="legend">
        <span>{$t("diary.legendLess")}</span>
        {#each [0, 1, 2, 3, 4] as level}<i class="level-{level}"></i>{/each}
        <span>{$t("diary.legendMore")}</span>
      </div>
    </article>

    <article class="panel day-panel">
      <div class="panel-head">
        <h2>{selectedDate ? shortDate(selectedDate) : $t("diary.entries")}</h2>
        <div class="day-head-actions">
          <span class="hint">{entryCountLabel(dayEntries.length)}</span>
          <button class="ghost small" onclick={startManualEntry}>
            <Icon name="plus" size={13} /> {$t("diary.manualAdd")}
          </button>
        </div>
      </div>

      {#if noteOpen}
        <div class="manual-editor">
          <div class="manual-editor-head">
            <strong>
              {noteEditingId ? $t("diary.manualEdit") : $t("diary.manualNew")}
            </strong>
            <button
              class="ghost small"
              aria-label={$t("common.close")}
              onclick={cancelManualEntry}
            >
              <Icon name="close" size={13} />
            </button>
          </div>
          {#if !noteEditingId}
            <label>
              <span>{$t("diary.manualDate")}</span>
              <input type="date" bind:value={noteDate} />
            </label>
          {/if}
          <label>
            <span>{$t("diary.manualBody")}</span>
            <textarea
              bind:value={noteBody}
              rows="4"
              maxlength="20000"
              placeholder={$t("diary.manualPlaceholder")}
            ></textarea>
          </label>
          <label>
            <span>{$t("diary.manualMood")}</span>
            <input
              bind:value={noteMood}
              maxlength="40"
              placeholder={$t("diary.manualMoodPlaceholder")}
            />
          </label>
          <div class="manual-actions">
            <button
              class="primary small"
              disabled={noteBusy || !noteBody.trim() || (!noteEditingId && !noteDate)}
              onclick={saveManualEntry}
            >
              {noteBusy ? $t("common.loading") : $t("common.save")}
            </button>
            <button class="ghost small" onclick={cancelManualEntry}>
              {$t("common.cancel")}
            </button>
          </div>
        </div>
      {/if}

      {#if dayEntries.length === 0}
        <p class="empty">{$t("diary.noEntries")}</p>
      {:else}
        <ul class="entries">
          {#each dayEntries as entry (entry.id)}
            <li>
              <div class="entry-head">
                <strong>{kindLabel(entry.kind)}</strong>
                <span class="hint">
                  {#if entry.durationMinutes > 0}{entry.durationMinutes} {$t("diary.minutesShort")}{/if}
                  {#if entry.pagesRead}· {entry.pagesRead} {$t("diary.pagesShort")}{/if}
                  {#if entry.extractsCreated}· {entry.extractsCreated} {$t("diary.extracts")}{/if}
                  {#if entry.notesCreated}· {entry.notesCreated} {$t("diary.notes")}{/if}
                </span>
              </div>
              <p class="entry-body">{entry.body}</p>
              {#if entry.mood}
                <p class="entry-mood">{$t("diary.moodLabel")}: {entry.mood}</p>
              {/if}
              <div class="entry-actions">
                {#if entry.bookId}
                  <button class="ghost small" onclick={() => openEntry(entry)}>
                    {$t("diary.openBook")}
                  </button>
                {/if}
                {#if entry.kind === "manual"}
                  <button class="ghost small" onclick={() => startEditEntry(entry)}>
                    {$t("common.edit")}
                  </button>
                {/if}
                <button class="danger small" onclick={() => deleteEntry(entry)}>
                  {$t("common.delete")}
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </article>
  </div>

  <div class="layout">
    <article class="panel">
      <div class="panel-head">
        <h2>{$t("diary.pomodoro")}</h2>
        <span class="badge">{$t(phaseKey)}</span>
      </div>
      <div class="timer-wrap">
        <div
          class="timer-ring"
          style={`--ring-progress:${Math.round(phaseRatio * 360)}deg`}
        >
          <div class="timer">
            <strong>{formatCountdown(displayRemaining)}</strong>
            <span>{$t(phaseKey)}</span>
          </div>
        </div>
        <p class="timer-meta">
          {$t("diary.completed", { count: pomodoro?.completedFocus ?? 0 })}
          <span aria-hidden="true">·</span>
          {$t("diary.round", { round: pomodoro?.round ?? 1 })}
        </p>
      </div>
      <div class="pomodoro-presets" role="group" aria-label={$t("diary.presets")}>
        {#each ([
          [25, 5, 15, "25/5"],
          [50, 10, 20, "50/10"],
          [90, 15, 30, "90/15"],
        ] as [number, number, number, string][]) as [focus, pause, long, label] (label)}
          <button
            class:active={config.focusMinutes === focus && config.breakMinutes === pause}
            disabled={pomodoroRunning}
            onclick={() => applyPreset(focus, pause, long)}
          >
            {label}
          </button>
        {/each}
      </div>
      <div class="ambience">
        <div class="ambience-head">
          <span>{$t("diary.ambience")}</span>
          {#if ambience !== "none"}<small>{$t("diary.ambienceOn")}</small>{/if}
        </div>
        <div class="ambience-options" role="group" aria-label={$t("diary.ambience")}>
          <button class:active={ambience === "none"} onclick={stopAmbience}>
            <Icon name="close" size={14} /> {$t("diary.ambienceNone")}
          </button>
          <button class:active={ambience === "rain"} onclick={() => startAmbience("rain")}>
            <Icon name="wave" size={14} /> {$t("diary.ambienceRain")}
          </button>
          <button class:active={ambience === "white"} onclick={() => startAmbience("white")}>
            <Icon name="headphones" size={14} /> {$t("diary.ambienceWhite")}
          </button>
          <button class:active={ambience === "fire"} onclick={() => startAmbience("fire")}>
            <Icon name="flame" size={14} /> {$t("diary.ambienceFire")}
          </button>
        </div>
        <label class="ambience-volume">
          <span>{$t("diary.ambienceVolume")}</span>
          <input
            type="range"
            min="0"
            max="100"
            step="5"
            value={ambienceVolume}
            oninput={(event) => setAmbienceVolume(Number(event.currentTarget.value))}
          />
        </label>
      </div>
      <div class="config-grid">
        <label>
          <span>{$t("diary.configFocus")}</span>
          <input type="number" min="1" max="180" bind:value={config.focusMinutes} disabled={pomodoroRunning} />
        </label>
        <label>
          <span>{$t("diary.configBreak")}</span>
          <input type="number" min="1" max="60" bind:value={config.breakMinutes} disabled={pomodoroRunning} />
        </label>
        <label>
          <span>{$t("diary.configLong")}</span>
          <input type="number" min="1" max="120" bind:value={config.longBreakMinutes} disabled={pomodoroRunning} />
        </label>
        <label>
          <span>{$t("diary.configRounds")}</span>
          <input type="number" min="2" max="12" bind:value={config.roundsBeforeLongBreak} disabled={pomodoroRunning} />
        </label>
      </div>
      <label class="check">
        <input type="checkbox" bind:checked={config.autoStartBreaks} disabled={pomodoroRunning} />
        <span>{$t("diary.autoBreak")}</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={config.autoStartFocus} disabled={pomodoroRunning} />
        <span>{$t("diary.autoFocus")}</span>
      </label>
      {#if pomodoroRunning}
        <p class="hint">{$t("diary.configLocked")}</p>
      {/if}
      <button class="primary" onclick={togglePomodoro} disabled={pomodoroBusy} aria-busy={pomodoroBusy}>
        {!pomodoro || pomodoro.state === "idle" ? $t("diary.startFocus") : $t("diary.stop")}
      </button>
      <p class="hint">
        {$t("diary.pomodoro")} · {$t("settings.pomodoroHelp")}
      </p>
    </article>

    <article class="panel">
      <div class="panel-head">
        <h2>{$t("diary.export")}</h2>
      </div>
      <label>
        {$t("diary.export")}
        <select bind:value={exportScope}>
          <option value="month">{$t("diary.exportMonth")}</option>
          <option value="year">{$t("diary.exportYear")}</option>
        </select>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={includeAudiobook} />
        <span>{$t("diary.includeAudiobook")}</span>
      </label>
      {#if includeAudiobook}
        <label>
          {$t("nav.voice")}
          <select bind:value={voiceId}>
            {#each installedVoices as voice (voice.id)}
              <option value={voice.id}>{voice.name}</option>
            {/each}
          </select>
        </label>
        {#if installedVoices.length === 0}
          <p class="notice">{$t("diary.noVoice")}</p>
        {/if}
      {/if}
      <button
        class="primary"
        onclick={exportNow}
        disabled={exporting || (includeAudiobook && installedVoices.length === 0)}
      >
        {exporting ? $t("diary.exporting") : $t("diary.exportNow")}
      </button>
      {#if exportProgress}
        <p class="hint">
          {exportProgress.message} ({Math.round(exportProgress.percent)}%)
        </p>
      {/if}
      {#if exportResult}
        <p class="hint">{$t("diary.epub")}: <code>{exportResult.epubPath}</code></p>
        {#if exportResult.audiobookPath}
          <p class="hint">{$t("diary.audiobook")}: <code>{exportResult.audiobookPath}</code></p>
        {/if}
      {/if}
    </article>
  </div>

  <article class="panel">
    <div class="panel-head">
      <h2>{$t("diary.achievements")}</h2>
      <span class="hint">
        {achievements.filter((item) => item.unlockedAt).length}/{achievements.length}
      </span>
    </div>
    <div class="achievements">
      {#each achievements as achievement (achievement.key)}
        <div class="achievement" class:unlocked={achievement.unlockedAt}>
          <div class="achievement-head">
            <span class="achievement-icon">
              <AchievementIcon achievementKey={achievement.key} size={18} />
            </span>
            <strong>{achievement.title}</strong>
            {#if achievement.unlockedAt}
              <span class="badge">{$t("diary.unlocked")}</span>
            {/if}
          </div>
          <p>{achievement.description}</p>
          <div class="bar">
            <span style:width={`${Math.round(achievement.progress * 100)}%`}></span>
          </div>
          <small>
            {#if achievement.unlockedAt}
              {unlockLabel(achievement.unlockedAt)}
            {:else}
              {Math.round(achievement.progress * 100)}%
            {/if}
          </small>
        </div>
      {/each}
    </div>
  </article>
</section>

<style>
  .diary {
    display: flex;
    flex-direction: column;
    gap: 1.35rem;
  }

  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem 1.5rem;
  }

  .month-nav {
    display: flex;
    align-items: center;
    gap: 0.2rem;
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .month-nav button {
    display: grid;
    place-items: center;
    width: 34px;
    min-width: 34px;
    min-height: 34px;
    height: 34px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 1.15rem;
  }

  .month-nav button:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .month-nav strong {
    min-width: 9.5rem;
    text-align: center;
    font-size: 0.83rem;
    font-weight: 600;
    text-transform: capitalize;
  }

  .month-nav .small {
    width: auto;
    min-width: 0;
    padding: 0 0.65rem;
    border-radius: 999px;
    font-size: 0.7rem;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 0.6rem;
  }

  .stat {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-rows: auto auto;
    align-items: center;
    column-gap: 0.6rem;
    row-gap: 0.05rem;
    padding: 0.75rem 0.85rem;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .stat-icon {
    grid-row: 1 / 3;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .stat-value {
    font-family: var(--font-display);
    font-size: 1.35rem;
    font-weight: 650;
    line-height: 1;
    letter-spacing: -0.02em;
  }

  .stat small {
    color: var(--muted);
    font-size: 0.67rem;
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1.12fr) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    padding: 1.05rem;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
  }

  .weekdays,
  .heatmap {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 0.32rem;
  }

  .weekdays span {
    text-align: center;
    color: var(--muted);
    font-size: 0.66rem;
    font-weight: 600;
  }

  .blank {
    aspect-ratio: 1;
  }

  .day {
    position: relative;
    aspect-ratio: 1;
    border: 1px solid transparent;
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.76rem;
    font-weight: 500;
    cursor: pointer;
    transition:
      transform 140ms ease,
      border-color 140ms ease,
      box-shadow 140ms ease;
  }

  .day:hover {
    transform: translateY(-1px);
    border-color: var(--accent-line);
  }

  .day.today::after {
    content: "";
    position: absolute;
    left: 50%;
    bottom: 4px;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.65;
    transform: translateX(-50%);
  }

  .day.selected {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    box-shadow: 0 0 0 4px var(--accent-soft);
  }

  .level-0 {
    background: var(--surface-2);
  }

  .level-1 {
    background: color-mix(in srgb, var(--accent) 20%, var(--surface-2));
  }

  .level-2 {
    background: color-mix(in srgb, var(--accent) 38%, var(--surface-2));
  }

  .level-3 {
    background: color-mix(in srgb, var(--accent) 58%, var(--surface-2));
  }

  .level-4 {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .legend i {
    width: 13px;
    height: 13px;
    border-radius: 4px;
  }

  .entries {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    max-height: 430px;
    overflow-y: auto;
  }

  .entries li {
    padding: 0.65rem 0.75rem;
    border-left: 2px solid var(--accent-line);
    border-radius: 4px 10px 10px 4px;
    background: var(--surface-2);
  }

  .entry-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.35rem;
  }

  .entry-head strong {
    font-size: 0.84rem;
  }

  .entry-body {
    margin: 0.4rem 0;
    white-space: pre-wrap;
    font-size: 0.84rem;
    line-height: 1.55;
  }

  .entry-mood {
    margin: 0.15rem 0 0.35rem;
    color: var(--muted);
    font-size: 0.72rem;
    font-style: italic;
  }

  .entry-actions,
  .day-head-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .manual-editor {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.7rem;
    border: 1px solid var(--accent-line);
    border-radius: 11px;
    background: var(--accent-soft);
  }

  .manual-editor-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.82rem;
  }

  .manual-editor label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .manual-editor input,
  .manual-editor textarea {
    width: 100%;
    padding: 0.42rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  .manual-editor textarea {
    resize: vertical;
    min-height: 84px;
  }

  .manual-actions {
    display: flex;
    gap: 0.4rem;
  }

  .timer-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.45rem;
    padding: 0.3rem 0 0.15rem;
  }

  .timer-ring {
    width: 188px;
    height: 188px;
    padding: 9px;
    border-radius: 50%;
    background: conic-gradient(
      from -90deg,
      var(--accent) var(--ring-progress, 0deg),
      color-mix(in srgb, var(--text) 9%, transparent) 0
    );
    transition: background 300ms ease;
  }

  .timer {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.1rem;
    width: 100%;
    height: 100%;
    border-radius: 50%;
    background: var(--surface);
    box-shadow:
      inset 0 0 0 1px var(--border),
      var(--shadow-sm);
  }

  .timer strong {
    font-family: var(--font-display);
    font-size: 2.45rem;
    font-weight: 600;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.03em;
  }

  .timer span {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .timer-meta {
    margin: 0;
    color: var(--muted);
    font-size: 0.74rem;
    text-align: center;
  }

  .timer-meta span {
    padding: 0 0.25rem;
  }

  .config-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.55rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    color: var(--muted);
    font-size: 0.74rem;
  }

  label input,
  label select {
    min-height: 40px;
    padding: 0.45rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
  }

  .check {
    flex-direction: row;
    align-items: center;
    gap: 0.45rem;
    color: var(--text);
  }

  .check input {
    width: 17px;
    height: 17px;
    min-height: 0;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    min-height: 22px;
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--muted);
    font-size: 0.66rem;
    white-space: nowrap;
  }

  .achievements {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.65rem;
  }

  .achievement {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
  }

  .achievement.unlocked {
    border-color: var(--accent-line);
    background:
      linear-gradient(
        160deg,
        color-mix(in srgb, var(--accent) 9%, var(--surface)),
        var(--surface)
      );
    box-shadow: var(--shadow-xs);
  }

  .achievement-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .achievement-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    background: var(--surface);
    color: var(--muted);
  }

  .achievement.unlocked .achievement-icon {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .achievement-head strong {
    flex: 1;
    min-width: 0;
    font-size: 0.84rem;
  }

  .achievement p {
    margin: 0;
    color: var(--muted);
    font-size: 0.75rem;
    line-height: 1.5;
  }

  .bar {
    height: 5px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 9%, transparent);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .achievement small {
    color: var(--muted);
    font-size: 0.67rem;
  }

  .hint,
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .notice,
  .ok {
    margin: 0;
    padding: 0.65rem 0.85rem;
    border-radius: 10px;
    font-size: 0.82rem;
  }

  .notice {
    border: 1px solid color-mix(in srgb, var(--danger) 55%, var(--border));
    background: color-mix(in srgb, var(--danger) 9%, transparent);
  }

  .ok {
    border: 1px solid color-mix(in srgb, var(--ok) 55%, var(--border));
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }

  .primary {
    min-height: 40px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.55rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--accent) 82%, #000);
    border-radius: 10px;
    background: linear-gradient(
      180deg,
      var(--accent),
      color-mix(in srgb, var(--accent) 86%, #000)
    );
    color: var(--accent-contrast);
    font: inherit;
    font-size: 0.83rem;
    font-weight: 650;
    box-shadow: 0 8px 18px -12px color-mix(in srgb, var(--accent) 80%, transparent);
  }

  .ghost {
    min-height: 36px;
    padding: 0.35rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
  }

  .ghost:hover {
    border-color: var(--accent-line);
    background: color-mix(in srgb, var(--accent) 6%, var(--surface-2));
  }

  .small {
    min-height: 32px;
    padding: 0.28rem 0.55rem;
    font-size: 0.73rem;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  @media (max-width: 1000px) {
    .stats {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .layout {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 720px) {
    .hero {
      align-items: flex-start;
    }

    .month-nav {
      width: 100%;
      justify-content: space-between;
    }

    .month-nav strong {
      flex: 1;
      min-width: 0;
    }

    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .stat:last-child {
      grid-column: 1 / -1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .day:hover {
      transform: none;
    }

    .timer-ring {
      transition: none;
    }
  }

  .pomodoro-presets {
    display: flex;
    gap: 4px;
    padding: 3px;
    border-radius: 9px;
    background: var(--surface-2);
  }

  .pomodoro-presets button {
    flex: 1;
    min-height: 30px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.66rem;
    font-weight: 600;
  }

  .pomodoro-presets button.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
  }

  .ambience {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 9px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .ambience-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--muted);
    font-size: 0.65rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .ambience-head small {
    color: var(--accent);
    font-size: 0.55rem;
    letter-spacing: 0;
    text-transform: none;
  }

  .ambience-options {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 3px;
  }

  .ambience-options button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    min-height: 30px;
    padding: 0.2rem;
    border: 1px solid transparent;
    border-radius: 6px;
    background: var(--surface);
    color: var(--muted);
    font: inherit;
    font-size: 0.55rem;
    font-weight: 600;
  }

  .ambience-options button.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .ambience-volume {
    flex-direction: row;
    align-items: center;
    gap: 7px;
    font-size: 0.6rem;
  }

  .ambience-volume input {
    flex: 1;
    min-height: 24px;
    padding: 0;
    border: 0;
    background: transparent;
    accent-color: var(--accent);
  }
</style>

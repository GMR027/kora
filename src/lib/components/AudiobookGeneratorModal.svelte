<script lang="ts">
  import { tick } from "svelte";
  import {
    audiobookCharacters,
    ttsCharacterVoices,
    ttsSynthesize,
    voiceClonePreview,
  } from "../api";
  import { t, type MessageKey } from "../i18n";
  import type {
    AudiobookCastInput,
    AudiobookScope,
    BookSummary,
    CharacterInfo,
    NarrationSettings,
    NarrationStyle,
    VoiceDescriptor,
  } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    books,
    voices,
    sections,
    busy = null,
    error = null,
    initialBookId = "",
    initialVoiceId = "",
    onClose,
    onBookChange,
    onGenerate,
    onManageVoices,
  }: {
    open: boolean;
    books: BookSummary[];
    voices: VoiceDescriptor[];
    sections: { index: number; title: string }[];
    busy?: string | null;
    error?: string | null;
    initialBookId?: string;
    initialVoiceId?: string;
    onClose: () => void;
    onBookChange?: (bookId: string) => void;
    onGenerate: (payload: {
      bookId: string;
      scope: AudiobookScope;
      voiceId: string;
      narration: NarrationSettings;
      cast: AudiobookCastInput | null;
    }) => void;
    onManageVoices?: () => void;
  } = $props();

  let selectedBookId = $state("");
  let scopeKind = $state<"fullBook" | "chapter" | "selection">("fullBook");
  let section = $state(0);
  let rangeStart = $state(0);
  let rangeEnd = $state(0);
  let voiceId = $state("");
  let narrationStyle = $state<NarrationStyle>("neutral");
  let narrationSpeed = $state(1);
  let previewing = $state(false);
  let previewError = $state<string | null>(null);
  let modalEl = $state<HTMLElement | null>(null);
  let player: HTMLAudioElement | null = null;
  let wasOpen = false;

  let multiVoice = $state(false);
  let characters = $state<CharacterInfo[]>([]);
  let characterVoices = $state<Record<string, string>>({});
  let castLoading = $state(false);
  let castError = $state<string | null>(null);
  let castRequestId = 0;
  let lastCastBookId = "";

  const NARRATION_STYLES: {
    id: NarrationStyle;
    icon: "wave" | "clock" | "sparkles" | "flame";
    titleKey: MessageKey;
    helpKey: MessageKey;
    rate: number;
    pitch: number;
  }[] = [
    {
      id: "neutral",
      icon: "wave",
      titleKey: "audiobooks.narrationNeutral",
      helpKey: "audiobooks.narrationNeutralHelp",
      rate: 1,
      pitch: 1,
    },
    {
      id: "calm",
      icon: "clock",
      titleKey: "audiobooks.narrationCalm",
      helpKey: "audiobooks.narrationCalmHelp",
      rate: 0.92,
      pitch: 0.97,
    },
    {
      id: "dramatic",
      icon: "sparkles",
      titleKey: "audiobooks.narrationDramatic",
      helpKey: "audiobooks.narrationDramaticHelp",
      rate: 0.9,
      pitch: 1.02,
    },
    {
      id: "lively",
      icon: "flame",
      titleKey: "audiobooks.narrationLively",
      helpKey: "audiobooks.narrationLivelyHelp",
      rate: 1.08,
      pitch: 1.04,
    },
  ];

  let installedVoices = $derived(voices.filter((voice) => voice.installed));
  let selectedVoice = $derived(
    installedVoices.find((voice) => voice.id === voiceId) ?? null,
  );
  let hasVoiceAlternatives = $derived(installedVoices.length > 1);
  let castCharacters = $derived(
    characters.filter((character) => character.name !== "Narrador"),
  );
  let selectedStyle = $derived(
    NARRATION_STYLES.find((style) => style.id === narrationStyle) ??
      NARRATION_STYLES[0],
  );
  let styleLabel = $derived($t(selectedStyle.titleKey));
  let speedLabel = $derived(
    narrationSpeed === 1
      ? "1×"
      : `${narrationSpeed.toFixed(2).replace(/0$/, "")}×`,
  );
  let chapterEstimate = $derived.by(() => {
    if (scopeKind !== "fullBook") return 1;
    return sections.length;
  });
  let summary = $derived.by(() => {
    if (!selectedVoice) return "";
    if (chapterEstimate > 0) {
      return $t("audiobooks.summary", {
        chapters: chapterEstimate,
        voice: selectedVoice.name,
      });
    }
    return $t("audiobooks.summaryFallback");
  });
  let generating = $derived(busy === "generate");

  function currentScope(): AudiobookScope {
    if (scopeKind === "fullBook") {
      return { kind: "fullBook" };
    }
    if (scopeKind === "chapter") {
      return { kind: "chapter", section };
    }
    return { kind: "selection", section, start: rangeStart, end: rangeEnd };
  }

  function castName(name: string) {
    return name === "Diálogo" ? $t("audiobooks.castDialogue") : name;
  }

  function toggleMultiVoice() {
    if (!hasVoiceAlternatives || generating) return;
    multiVoice = !multiVoice;
    previewError = null;
    if (!multiVoice) {
      castRequestId += 1;
      castLoading = false;
      castError = null;
    }
  }

  $effect(() => {
    if (!open) {
      wasOpen = false;
      castRequestId += 1;
      castLoading = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    const firstBook = initialBookId || books[0]?.id || "";
    selectedBookId = firstBook;
    voiceId =
      initialVoiceId ||
      installedVoices.find((voice) => voice.installed)?.id ||
      installedVoices[0]?.id ||
      "";
    scopeKind = "fullBook";
    section = 0;
    rangeStart = 0;
    rangeEnd = 0;
    narrationStyle = "neutral";
    narrationSpeed = 1;
    previewError = null;
    multiVoice = false;
    characters = [];
    characterVoices = {};
    castLoading = false;
    castError = null;
    castRequestId += 1;
    lastCastBookId = "";
    void tick().then(() => modalEl?.focus());
  });

  $effect(() => {
    if (!open) return;
    if (!books.some((book) => book.id === selectedBookId) && books[0]) {
      selectedBookId = books[0].id;
      onBookChange?.(books[0].id);
    }
    if (!installedVoices.some((voice) => voice.id === voiceId) && installedVoices[0]) {
      voiceId = installedVoices[0].id;
    }
  });

  $effect(() => {
    if (!open) {
      player?.pause();
      return;
    }
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
      player?.pause();
      player = null;
    };
  });

  $effect(() => {
    if (!open || !multiVoice) return;
    const scope = currentScope();
    const bookId = selectedBookId;
    if (!bookId) return;
    if (bookId !== lastCastBookId) {
      lastCastBookId = bookId;
      characterVoices = {};
    }
    const requestId = ++castRequestId;
    castLoading = true;
    castError = null;
    characters = [];
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const [detected, stored] = await Promise.all([
            audiobookCharacters(bookId, scope),
            ttsCharacterVoices(bookId),
          ]);
          if (requestId !== castRequestId) return;
          const installedIds = new Set(installedVoices.map((voice) => voice.id));
          const storedMap: Record<string, string> = {};
          for (const entry of stored) {
            if (installedIds.has(entry.voiceId)) {
              storedMap[entry.character] = entry.voiceId;
            }
          }
          const previous = characterVoices;
          characters = detected;
          characterVoices = Object.fromEntries(
            detected
              .filter((character) => character.name !== "Narrador")
              .map((character) => [
                character.name,
                previous[character.name] !== undefined
                  ? previous[character.name]
                  : (storedMap[character.name] ?? ""),
              ]),
          );
        } catch (cause) {
          if (requestId === castRequestId) {
            castError = cause instanceof Error ? cause.message : String(cause);
          }
        } finally {
          if (requestId === castRequestId) castLoading = false;
        }
      })();
    }, 250);
    return () => clearTimeout(timer);
  });

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key !== "Tab" || !modalEl) return;
    const focusable = Array.from(
      modalEl.querySelectorAll<HTMLElement>(
        'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((element) => element.offsetParent !== null);
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

  function changeBook(value: string) {
    selectedBookId = value;
    section = 0;
    onBookChange?.(value);
  }

  async function previewVoice() {
    const voice = selectedVoice;
    if (!voice) return;
    previewError = null;
    previewing = true;
    try {
      player?.pause();
      const sample =
        voice.language === "en"
          ? "Hello, this is a preview of the selected voice."
          : "Hola, esta es una muestra de la voz seleccionada.";
      const output = voice.cloned
        ? await voiceClonePreview(voice.id, sample)
        : await ttsSynthesize({
            text: sample,
            voiceId: voice.id,
            rate: selectedStyle.rate * narrationSpeed,
            pitch: selectedStyle.pitch,
          });
      player = new Audio(`data:audio/wav;base64,${output.audioBase64}`);
      void player.play().catch(() => undefined);
    } catch (cause) {
      previewError = cause instanceof Error ? cause.message : String(cause);
    } finally {
      previewing = false;
    }
  }

  function generate() {
    if (!selectedBookId || !voiceId) return;
    const cast: AudiobookCastInput | null = multiVoice
      ? {
          enabled: true,
          assignments: castCharacters
            .map((character) => ({
              character: character.name,
              voiceId: characterVoices[character.name] ?? "",
            }))
            .filter((assignment) => assignment.voiceId),
        }
      : null;
    onGenerate({
      bookId: selectedBookId,
      scope: currentScope(),
      voiceId,
      narration: {
        style: narrationStyle,
        speed: narrationSpeed,
        multiVoice,
      },
      cast,
    });
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div class="modal-backdrop">
    <button
      class="backdrop-button"
      type="button"
      onclick={onClose}
      aria-label={$t("common.close")}
    ></button>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="audiobook-generator-title"
      tabindex="-1"
      bind:this={modalEl}
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="sparkles" size={21} /></span>
          <div>
            <h2 id="audiobook-generator-title">{$t("audiobooks.modalGenerateTitle")}</h2>
            <p>{$t("audiobooks.modalGenerateHelp")}</p>
          </div>
        </div>
        <button
          class="close-button"
          type="button"
          onclick={onClose}
          aria-label={$t("common.close")}
          title={$t("common.close")}
        >
          <Icon name="close" size={18} />
        </button>
      </header>

      <div class="modal-body">
        {#if error}
          <p class="notice" role="alert">{error}</p>
        {/if}

        <label class="field">
          <span>{$t("audiobooks.stepBook")}</span>
          <select
            value={selectedBookId}
            onchange={(event) => changeBook(event.currentTarget.value)}
            disabled={generating}
          >
            {#each books as book (book.id)}
              <option value={book.id}>
                {book.title}{book.author ? ` — ${book.author}` : ""}
              </option>
            {/each}
          </select>
        </label>

        <div class="field-grid">
          <label class="field">
            <span>{$t("audiobooks.stepScope")}</span>
            <select bind:value={scopeKind} disabled={generating}>
              <option value="fullBook">{$t("audiobooks.fullBook")}</option>
              <option value="chapter">{$t("audiobooks.chapterScope")}</option>
              <option value="selection">{$t("audiobooks.selectionScope")}</option>
            </select>
          </label>

          {#if scopeKind !== "fullBook"}
            <label class="field">
              <span>{$t("audiobooks.sectionPage")}</span>
              {#if sections.length > 0}
                <select bind:value={section} disabled={generating}>
                  {#each sections as item (item.index)}
                    <option value={item.index}>{item.index + 1} · {item.title}</option>
                  {/each}
                </select>
              {:else}
                <input type="number" min="0" bind:value={section} disabled={generating} />
              {/if}
            </label>
          {/if}
        </div>

        {#if scopeKind === "selection"}
          <details class="advanced" open>
            <summary>{$t("audiobooks.advanced")}</summary>
            <div class="field-grid">
              <label class="field">
                <span>{$t("audiobooks.startUtf16")}</span>
                <input type="number" min="0" bind:value={rangeStart} disabled={generating} />
              </label>
              <label class="field">
                <span>{$t("audiobooks.endUtf16")}</span>
                <input type="number" min="0" bind:value={rangeEnd} disabled={generating} />
              </label>
            </div>
          </details>
        {/if}

        <div class="narration-section">
          <div class="narration-head">
            <span class="field-label">{$t("audiobooks.narrationStyle")}</span>
            <span class="narration-current">{styleLabel} · {speedLabel}</span>
          </div>

          <div
            class="style-grid"
            role="radiogroup"
            aria-label={$t("audiobooks.narrationStyle")}
          >
            {#each NARRATION_STYLES as option (option.id)}
              <button
                class="style-option"
                class:active={narrationStyle === option.id}
                type="button"
                role="radio"
                aria-checked={narrationStyle === option.id}
                onclick={() => (narrationStyle = option.id)}
                disabled={generating}
              >
                <span class="style-icon">
                  <Icon name={option.icon} size={17} />
                </span>
                <span class="style-copy">
                  <strong>{$t(option.titleKey)}</strong>
                  <small>{$t(option.helpKey)}</small>
                </span>
              </button>
            {/each}
          </div>

          <div class="speed-row">
            <div class="speed-head">
              <span class="field-label">{$t("audiobooks.baseSpeed")}</span>
              <strong class="speed-value">{speedLabel}</strong>
            </div>
            <input
              type="range"
              min="0.75"
              max="1.5"
              step="0.05"
              bind:value={narrationSpeed}
              disabled={generating}
              aria-label={$t("audiobooks.baseSpeed")}
              aria-valuetext={speedLabel}
            />
            <div class="speed-scale" aria-hidden="true">
              <span>0.75×</span>
              <span>1×</span>
              <span>1.5×</span>
            </div>
            <p class="speed-help">{$t("audiobooks.baseSpeedHelp")}</p>
          </div>
        </div>

        <div class="voice-section">
          <div class="voice-head">
            <span class="field-label">{$t("audiobooks.stepVoice")}</span>
            <div class="voice-actions">
              {#if onManageVoices}
                <button
                  class="manage-button"
                  type="button"
                  onclick={onManageVoices}
                  disabled={generating}
                >
                  {$t("audiobooks.manageVoices")}
                </button>
              {/if}
              <button
                class="preview-button"
                type="button"
                onclick={previewVoice}
                disabled={!selectedVoice || previewing || generating}
              >
                <Icon name={previewing ? "wave" : "play"} size={15} />
                {previewing ? $t("audiobooks.previewing") : $t("audiobooks.previewVoice")}
              </button>
            </div>
          </div>

          {#if installedVoices.length === 0}
            <p class="empty-voices">{$t("audiobooks.engineUnavailable")}</p>
          {:else}
            <div class="voice-grid" role="radiogroup" aria-label={$t("audiobooks.voice")}>
              {#each installedVoices as voice (voice.id)}
                <button
                  class="voice-option"
                  class:active={voice.id === voiceId}
                  type="button"
                  role="radio"
                  aria-checked={voice.id === voiceId}
                  onclick={() => {
                    voiceId = voice.id;
                    previewError = null;
                  }}
                  disabled={generating}
                >
                  <span class="voice-mark">
                    <Icon name={voice.cloned ? "mic" : "voice"} size={18} />
                  </span>
                  <span class="voice-copy">
                    <strong>{voice.name}</strong>
                    <small>
                      {voice.language.toUpperCase()} · {voice.engine}
                      {#if voice.cloned} · {$t("audiobooks.voiceCloned")}{/if}
                    </small>
                  </span>
                  {#if voice.id === voiceId}
                    <span class="voice-check"><Icon name="check" size={15} /></span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}

          {#if previewError}
            <p class="preview-error" role="alert">{previewError}</p>
          {/if}
        </div>

        <div class="cast-section">
          <button
            class="cast-toggle"
            class:active={multiVoice}
            type="button"
            role="switch"
            aria-checked={multiVoice}
            onclick={toggleMultiVoice}
            disabled={!hasVoiceAlternatives || generating}
          >
            <span class="cast-icon"><Icon name="voice" size={19} /></span>
            <span class="cast-copy">
              <strong>{$t("audiobooks.multiVoice")}</strong>
              <small>{$t("audiobooks.multiVoiceHelp")}</small>
            </span>
            <span class="switch" aria-hidden="true"><span></span></span>
          </button>

          {#if !hasVoiceAlternatives}
            <p class="cast-note">{$t("audiobooks.multiVoiceNeedsVoices")}</p>
          {:else if multiVoice}
            {#if castLoading}
              <div class="cast-loading" role="status">
                <span class="cast-spinner" aria-hidden="true"></span>
                {$t("audiobooks.castLoading")}
              </div>
            {:else if castError}
              <p class="cast-error" role="alert">{castError}</p>
            {:else if castCharacters.length === 0}
              <p class="cast-empty">{$t("audiobooks.castNone")}</p>
            {:else}
              <p class="cast-note">
                {$t("audiobooks.castHint", {
                  narrator: selectedVoice?.name ?? "",
                })}
              </p>
              <ul class="cast-list">
                {#each castCharacters as character (character.name)}
                  <li>
                    <span class="cast-info">
                      <strong>{castName(character.name)}</strong>
                      <small title={character.sample}>
                        {$t("audiobooks.castLines", {
                          count: character.lines,
                        })}
                        · {character.sample}
                      </small>
                    </span>
                    <select
                      value={characterVoices[character.name] ?? ""}
                      onchange={(event) => {
                        characterVoices = {
                          ...characterVoices,
                          [character.name]: event.currentTarget.value,
                        };
                      }}
                      disabled={generating}
                      aria-label={castName(character.name)}
                    >
                      <option value="">{$t("audiobooks.castAuto")}</option>
                      {#each installedVoices as voice (voice.id)}
                        <option value={voice.id}>
                          {voice.name} · {voice.language.toUpperCase()}
                        </option>
                      {/each}
                    </select>
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}
        </div>

        <div class="summary-box">
          <span class="summary-icon"><Icon name="chapters" size={19} /></span>
          <div>
            <strong>{$t("audiobooks.stepSummary")}</strong>
            <p>{summary}</p>
            <p class="summary-extra">
              {$t("audiobooks.narrationSummary", {
                style: styleLabel,
                speed: speedLabel,
              })}
            </p>
            {#if multiVoice && castCharacters.length > 0}
              <p class="summary-extra">
                {$t("audiobooks.castSummary", {
                  count: castCharacters.length,
                })}
              </p>
            {/if}
            <p class="queue-hint">{$t("audiobooks.queueHint")}</p>
          </div>
        </div>
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={generating}>
          {$t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={generate}
          loading={generating}
          disabled={generating ||
            (multiVoice && castLoading) ||
            !selectedBookId ||
            !voiceId ||
            books.length === 0}
        >
          <Icon name="sparkles" size={17} />
          {generating ? $t("audiobooks.starting") : $t("audiobooks.startGeneration")}
        </Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(18, 14, 10, 0.46);
    backdrop-filter: blur(6px);
    animation: fade-in 150ms ease;
  }

  .backdrop-button {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: 0;
    border: 0;
    background: transparent;
  }

  .modal {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    width: min(760px, 100%);
    max-height: min(90vh, 900px);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: 0 36px 80px -40px rgba(15, 23, 42, 0.4);
    outline: none;
    animation: rise-in 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 22px 18px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background:
      radial-gradient(
        420px 160px at 0% 0%,
        color-mix(in srgb, var(--accent) 9%, transparent),
        transparent 70%
      ),
      var(--surface);
  }

  .head-copy {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    min-width: 0;
  }

  .head-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 42px;
    height: 42px;
    border: 1px solid var(--accent-line);
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.35rem;
    font-weight: 620;
    letter-spacing: -0.018em;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.8rem;
    line-height: 1.45;
  }

  .close-button {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    min-width: 34px;
    height: 34px;
    min-height: 34px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
  }

  .close-button:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 17px;
    padding: 20px 22px;
    overflow-y: auto;
  }

  .notice {
    margin: 0;
    padding: 0.65rem 0.8rem;
    border: 1px solid color-mix(in srgb, var(--danger) 50%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    font-size: 0.8rem;
  }

  .field,
  .field-grid {
    display: grid;
    gap: 8px;
  }

  .field-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .field span,
  .field-label {
    color: var(--muted);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .field select,
  .field input {
    min-height: 42px;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
  }

  .advanced {
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
  }

  .advanced summary {
    padding: 10px 12px;
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
  }

  .advanced .field-grid {
    padding: 0 12px 12px;
  }

  .narration-section {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .narration-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .narration-current {
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .style-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 7px;
  }

  .style-option {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    min-width: 0;
    min-height: 74px;
    padding: 9px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    text-align: left;
    transition:
      border-color 150ms ease,
      background-color 150ms ease;
  }

  .style-option:hover:not(:disabled) {
    border-color: var(--accent-line);
    background: var(--surface-3);
  }

  .style-option.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .style-option:disabled {
    opacity: 0.6;
  }

  .style-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    background: var(--surface);
    color: var(--accent);
  }

  .style-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .style-copy strong {
    font-size: 0.74rem;
    font-weight: 650;
    line-height: 1.2;
  }

  .style-copy small {
    color: var(--muted);
    font-size: 0.6rem;
    line-height: 1.3;
  }

  .speed-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }

  .speed-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .speed-value {
    color: var(--accent);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .speed-row input[type="range"] {
    width: 100%;
    accent-color: var(--accent);
  }

  .speed-scale {
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--muted);
    font-size: 0.58rem;
    font-variant-numeric: tabular-nums;
  }

  .speed-help {
    margin: 1px 0 0;
    color: var(--muted);
    font-size: 0.63rem;
    line-height: 1.35;
  }

  .voice-section {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .voice-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .voice-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .manage-button {
    min-height: 32px;
    padding: 0.25rem 0.2rem;
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 0.69rem;
    font-weight: 600;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--accent) 35%, transparent);
    text-underline-offset: 3px;
  }

  .manage-button:hover {
    color: var(--accent);
  }

  .preview-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0.25rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--accent);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
  }

  .preview-button:hover:not(:disabled) {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .voice-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    max-height: 260px;
    overflow-y: auto;
    padding: 1px;
  }

  .voice-option {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    min-height: 58px;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .voice-option:hover:not(:disabled) {
    border-color: var(--accent-line);
    background: var(--surface-3);
  }

  .voice-option.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .voice-mark {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: var(--surface);
    color: var(--accent);
  }

  .voice-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .voice-copy strong {
    overflow: hidden;
    font-size: 0.8rem;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .voice-copy small {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.65rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .voice-check {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .empty-voices {
    margin: 0;
    padding: 12px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .preview-error {
    margin: 0;
    color: var(--danger);
    font-size: 0.72rem;
  }

  .cast-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .cast-toggle {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 11px 12px;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .cast-toggle:hover:not(:disabled) {
    border-color: var(--accent-line);
  }

  .cast-toggle.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .cast-toggle:disabled {
    opacity: 0.58;
  }

  .cast-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    background: var(--surface);
    color: var(--accent);
  }

  .cast-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .cast-copy strong {
    font-size: 0.82rem;
    font-weight: 650;
  }

  .cast-copy small {
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.4;
  }

  .switch {
    flex: 0 0 auto;
    width: 38px;
    height: 22px;
    padding: 2px;
    border-radius: 999px;
    background: var(--surface-4);
    transition: background-color 150ms ease;
  }

  .switch span {
    display: block;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface);
    box-shadow: var(--shadow-xs);
    transition: transform 150ms ease;
  }

  .cast-toggle.active .switch {
    background: var(--accent);
  }

  .cast-toggle.active .switch span {
    transform: translateX(16px);
  }

  .cast-note,
  .cast-error,
  .cast-empty {
    margin: 0;
    font-size: 0.7rem;
    line-height: 1.45;
  }

  .cast-note {
    color: var(--muted);
  }

  .cast-error {
    color: var(--danger);
  }

  .cast-empty {
    padding: 10px 12px;
    border: 1px dashed var(--border);
    border-radius: 9px;
    color: var(--muted);
  }

  .cast-loading {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 11px;
    color: var(--muted);
    font-size: 0.72rem;
  }

  .cast-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--surface-4);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: cast-spin 800ms linear infinite;
  }

  .cast-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    max-height: 230px;
    margin: 0;
    padding: 1px;
    overflow-y: auto;
    list-style: none;
  }

  .cast-list li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 180px;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
    border-radius: 9px;
    background: color-mix(in srgb, var(--surface-2) 64%, transparent);
  }

  .cast-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .cast-info strong {
    overflow: hidden;
    font-size: 0.78rem;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cast-info small {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.64rem;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cast-list select {
    min-height: 34px;
    min-width: 0;
    padding: 0.3rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
  }

  @keyframes cast-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .summary-box {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    padding: 12px 14px;
    border: 1px solid var(--accent-line);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 6%, var(--surface));
  }

  .summary-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: var(--surface);
    color: var(--accent);
  }

  .summary-box strong {
    display: block;
    font-size: 0.78rem;
    font-weight: 650;
  }

  .summary-box p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.74rem;
    line-height: 1.4;
  }

  .summary-box .summary-extra {
    margin-top: 3px;
    color: var(--accent);
    font-size: 0.68rem;
  }

  .summary-box .queue-hint {
    margin-top: 2px;
    color: var(--muted);
    font-size: 0.65rem;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 9px;
    padding: 15px 22px;
    border-top: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background: var(--surface-2);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }

  @keyframes rise-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.985);
    }
  }

  @media (max-width: 640px) {
    .modal-backdrop {
      align-items: flex-end;
      padding: 0;
    }

    .modal {
      width: 100%;
      max-height: 94vh;
      border-radius: var(--radius-xl) var(--radius-xl) 0 0;
    }

    .modal-head {
      padding: 16px 16px 14px;
    }

    .modal-body {
      padding: 16px;
    }

    .field-grid,
    .voice-grid {
      grid-template-columns: 1fr;
    }

    .style-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .voice-grid {
      max-height: none;
    }

    .cast-list {
      max-height: 280px;
    }

    .cast-list li {
      grid-template-columns: 1fr;
      gap: 6px;
    }

    .modal-foot {
      padding: 12px 16px calc(12px + var(--safe-bottom));
    }

    .modal-foot :global(.ui-button) {
      flex: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }

    .cast-spinner {
      animation: none;
    }

    .switch,
    .switch span {
      transition: none;
    }
  }
</style>

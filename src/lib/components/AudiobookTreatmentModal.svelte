<script lang="ts">
  import { t, type MessageKey } from "../i18n";
  import type {
    AudioTreatment,
    AudiobookInfo,
    EqPreset,
  } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    audiobooks,
    initialId = null,
    busy = false,
    error = null,
    onClose,
    onApply,
  }: {
    open: boolean;
    audiobooks: AudiobookInfo[];
    initialId?: string | null;
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onApply: (audiobookId: string, treatment: AudioTreatment) => void;
  } = $props();

  const LUFS_OPTIONS: { value: number; key: MessageKey }[] = [
    { value: -16, key: "audiobooks.treatmentLufsPodcast" },
    { value: -18, key: "audiobooks.treatmentLufsBook" },
    { value: -20, key: "audiobooks.treatmentLufsSoft" },
    { value: -23, key: "audiobooks.treatmentLufsNight" },
  ];

  const EQ_PRESETS: {
    id: EqPreset;
    icon: "sliders" | "mic" | "flame" | "sparkles";
    titleKey: MessageKey;
    helpKey: MessageKey;
  }[] = [
    {
      id: "neutral",
      icon: "sliders",
      titleKey: "audiobooks.treatmentEqNeutral",
      helpKey: "audiobooks.treatmentEqNeutralHelp",
    },
    {
      id: "clearVoice",
      icon: "mic",
      titleKey: "audiobooks.treatmentEqClear",
      helpKey: "audiobooks.treatmentEqClearHelp",
    },
    {
      id: "warm",
      icon: "flame",
      titleKey: "audiobooks.treatmentEqWarm",
      helpKey: "audiobooks.treatmentEqWarmHelp",
    },
    {
      id: "bright",
      icon: "sparkles",
      titleKey: "audiobooks.treatmentEqBright",
      helpKey: "audiobooks.treatmentEqBrightHelp",
    },
  ];

  const EQ_KEYS: Record<EqPreset, MessageKey> = {
    neutral: "audiobooks.treatmentEqNeutral",
    clearVoice: "audiobooks.treatmentEqClear",
    warm: "audiobooks.treatmentEqWarm",
    bright: "audiobooks.treatmentEqBright",
  };

  let selectedId = $state("");
  let normalize = $state(true);
  let targetLufs = $state(-18);
  let eq = $state<EqPreset>("neutral");
  let wasOpen = false;

  let selected = $derived(
    audiobooks.find((item) => item.id === selectedId) ?? null,
  );
  let noop = $derived(!normalize && eq === "neutral");
  let summary = $derived.by(() => {
    const loudness = normalize ? `LUFS ${targetLufs}` : $t("audiobooks.treatmentNoNormalize");
    return $t("audiobooks.treatmentSummary", {
      lufs: loudness,
      eq: $t(EQ_KEYS[eq]),
    });
  });
  let currentSummary = $derived.by(() => {
    const treatment = selected?.treatment;
    if (!treatment) return null;
    const loudness = treatment.normalize
      ? `LUFS ${treatment.targetLufs}`
      : $t("audiobooks.treatmentNoNormalize");
    return $t("audiobooks.treatmentCurrent", {
      summary: `${loudness} · ${$t(EQ_KEYS[treatment.eq])}`,
    });
  });

  function hydrate(item: AudiobookInfo | null) {
    const treatment = item?.treatment;
    normalize = treatment?.normalize ?? true;
    targetLufs = treatment?.targetLufs ?? -18;
    eq = treatment?.eq ?? "neutral";
  }

  function selectItem(id: string) {
    selectedId = id;
    hydrate(audiobooks.find((item) => item.id === id) ?? null);
  }

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    const item =
      audiobooks.find((entry) => entry.id === initialId) ?? audiobooks[0] ?? null;
    selectedId = item?.id ?? "";
    hydrate(item);
  });

  $effect(() => {
    if (!open) return;
    if (!audiobooks.some((item) => item.id === selectedId) && audiobooks[0]) {
      selectedId = audiobooks[0].id;
      hydrate(audiobooks[0]);
    }
  });

  $effect(() => {
    if (!open) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
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
      aria-labelledby="audiobook-treatment-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="sliders" size={20} /></span>
          <div>
            <h2 id="audiobook-treatment-title">{$t("audiobooks.treatmentTitle")}</h2>
            <p>{$t("audiobooks.treatmentHelp")}</p>
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

        {#if audiobooks.length === 0}
          <p class="empty">{$t("audiobooks.empty")}</p>
        {:else}
          <label class="field">
            <span>{$t("audiobooks.treatmentBook")}</span>
            <select
              value={selectedId}
              onchange={(event) => selectItem(event.currentTarget.value)}
              disabled={busy}
            >
              {#each audiobooks as item (item.id)}
                <option value={item.id}>
                  {item.bookTitle} · {item.voiceName ?? $t("audiobooks.voice")}
                </option>
              {/each}
            </select>
          </label>

          {#if currentSummary}
            <p class="current-treatment">{currentSummary}</p>
          {/if}

          <button
            class="toggle-row"
            class:active={normalize}
            type="button"
            role="switch"
            aria-checked={normalize}
            onclick={() => (normalize = !normalize)}
            disabled={busy}
          >
            <span class="toggle-icon"><Icon name="waveform" size={19} /></span>
            <span class="toggle-copy">
              <strong>{$t("audiobooks.treatmentNormalize")}</strong>
              <small>{$t("audiobooks.treatmentNormalizeHelp")}</small>
            </span>
            <span class="switch" aria-hidden="true"><span></span></span>
          </button>

          {#if normalize}
            <div class="sub-block">
              <span class="field-label">{$t("audiobooks.treatmentLufs")}</span>
              <div
                class="lufs-grid"
                role="radiogroup"
                aria-label={$t("audiobooks.treatmentLufs")}
              >
                {#each LUFS_OPTIONS as option (option.value)}
                  <button
                    class="lufs-option"
                    class:active={targetLufs === option.value}
                    type="button"
                    role="radio"
                    aria-checked={targetLufs === option.value}
                    onclick={() => (targetLufs = option.value)}
                    disabled={busy}
                  >
                    {$t(option.key)}
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          <div class="sub-block">
            <span class="field-label">{$t("audiobooks.treatmentEq")}</span>
            <div
              class="eq-grid"
              role="radiogroup"
              aria-label={$t("audiobooks.treatmentEq")}
            >
              {#each EQ_PRESETS as preset (preset.id)}
                <button
                  class="eq-option"
                  class:active={eq === preset.id}
                  type="button"
                  role="radio"
                  aria-checked={eq === preset.id}
                  onclick={() => (eq = preset.id)}
                  disabled={busy}
                >
                  <span class="eq-icon"><Icon name={preset.icon} size={17} /></span>
                  <span class="eq-copy">
                    <strong>{$t(preset.titleKey)}</strong>
                    <small>{$t(preset.helpKey)}</small>
                  </span>
                </button>
              {/each}
            </div>
          </div>

          <div class="summary-box">
            <span class="summary-icon"><Icon name="waveform" size={19} /></span>
            <div>
              <strong>{summary}</strong>
              <p>{$t("audiobooks.treatmentWarning")}</p>
            </div>
          </div>

          {#if noop}
            <p class="noop" role="status">{$t("audiobooks.treatmentNoop")}</p>
          {/if}
        {/if}
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={() => {
            if (selectedId && !noop) {
              onApply(selectedId, { normalize, targetLufs, eq });
            }
          }}
          loading={busy}
          disabled={busy || noop || !selectedId || audiobooks.length === 0}
        >
          <Icon name="sliders" size={17} />
          {busy ? $t("audiobooks.treatmentApplying") : $t("audiobooks.treatmentApply")}
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
    width: min(680px, 100%);
    max-height: min(90vh, 900px);
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow: 0 36px 80px -40px rgba(15, 23, 42, 0.4);
    animation: rise-in 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 19px 20px 16px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background:
      radial-gradient(
        360px 150px at 0% 0%,
        color-mix(in srgb, var(--accent) 9%, transparent),
        transparent 70%
      ),
      var(--surface);
  }

  .head-copy {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    min-width: 0;
  }

  .head-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
    border: 1px solid var(--accent-line);
    border-radius: 11px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.25rem;
    font-weight: 620;
    letter-spacing: -0.018em;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.78rem;
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
    gap: 14px;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .notice {
    margin: 0;
    padding: 0.62rem 0.78rem;
    border: 1px solid color-mix(in srgb, var(--danger) 50%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 8%, transparent);
    font-size: 0.78rem;
  }

  .empty {
    margin: 0;
    padding: 18px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    color: var(--muted);
    font-size: 0.8rem;
    text-align: center;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field > span,
  .field-label {
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .field select {
    min-height: 42px;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.84rem;
  }

  .current-treatment {
    margin: -4px 0 0;
    color: var(--muted);
    font-size: 0.7rem;
  }

  .toggle-row {
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

  .toggle-row:hover:not(:disabled) {
    border-color: var(--accent-line);
  }

  .toggle-row.active {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }

  .toggle-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    background: var(--surface);
    color: var(--accent);
  }

  .toggle-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .toggle-copy strong {
    font-size: 0.8rem;
    font-weight: 650;
  }

  .toggle-copy small {
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

  .toggle-row.active .switch {
    background: var(--accent);
  }

  .toggle-row.active .switch span {
    transform: translateX(16px);
  }

  .sub-block {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .lufs-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
  }

  .lufs-option {
    min-height: 34px;
    padding: 0.3rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.68rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .lufs-option:hover:not(:disabled) {
    border-color: var(--accent-line);
    color: var(--text);
  }

  .lufs-option.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .eq-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .eq-option {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
    min-height: 64px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    text-align: left;
  }

  .eq-option:hover:not(:disabled) {
    border-color: var(--accent-line);
    background: var(--surface-3);
  }

  .eq-option.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .eq-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: var(--surface);
    color: var(--accent);
  }

  .eq-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .eq-copy strong {
    font-size: 0.78rem;
    font-weight: 650;
  }

  .eq-copy small {
    color: var(--muted);
    font-size: 0.64rem;
    line-height: 1.35;
  }

  .summary-box {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 11px 13px;
    border: 1px solid var(--accent-line);
    border-radius: 11px;
    background: color-mix(in srgb, var(--accent) 6%, var(--surface));
  }

  .summary-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--surface);
    color: var(--accent);
  }

  .summary-box strong {
    display: block;
    font-size: 0.8rem;
    font-weight: 650;
  }

  .summary-box p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.4;
  }

  .noop {
    margin: 0;
    color: var(--warn);
    font-size: 0.72rem;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 9px;
    padding: 14px 20px;
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
      padding: 15px 16px 13px;
    }

    .modal-body {
      padding: 15px 16px;
    }

    .lufs-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .eq-grid {
      grid-template-columns: 1fr;
    }

    .modal-foot {
      padding: 12px 16px calc(12px + env(safe-area-inset-bottom));
    }

    .modal-foot :global(.ui-button) {
      flex: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal,
    .switch,
    .switch span {
      animation: none;
      transition: none;
    }
  }
</style>

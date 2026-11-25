<script lang="ts">
  import { t, type MessageKey } from "../i18n";
  import type { KaraokePrecision } from "../types";
  import type { PreparedKaraokeLine } from "../karaoke";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    title,
    precision,
    lines,
    activeIndex,
    activeWord,
    startMs,
    onClose,
    onSeek,
    onOpenText,
  }: {
    open: boolean;
    title: string;
    precision: KaraokePrecision;
    lines: PreparedKaraokeLine[];
    activeIndex: number;
    activeWord: number;
    startMs: number;
    onClose: () => void;
    onSeek: (positionMs: number) => void;
    onOpenText: () => void;
  } = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let wasOpen = false;

  const PRECISION_KEYS: Record<KaraokePrecision, MessageKey> = {
    aligned: "audiobooks.karaokeAligned",
    timed: "audiobooks.karaokeTimed",
    estimated: "audiobooks.karaokeEstimated",
  };

  const PRECISION_HELP: Record<KaraokePrecision, MessageKey> = {
    aligned: "audiobooks.karaokeHelp",
    timed: "audiobooks.karaokeTimedHelp",
    estimated: "audiobooks.karaokeEstimatedHelp",
  };

  function formatClock(milliseconds: number) {
    const total = Math.max(0, Math.floor(milliseconds / 1000));
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const seconds = total % 60;
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`
      : `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (!wasOpen) {
      wasOpen = true;
      // Al abrir, centra la línea actual sin animación.
      queueMicrotask(() => {
        if (!scroller || activeIndex < 0) return;
        const row = scroller.querySelector<HTMLElement>(
          `[data-line="${activeIndex}"]`,
        );
        if (row) {
          scroller.scrollTop =
            row.offsetTop - scroller.clientHeight / 2 + row.clientHeight / 2;
        }
      });
    }
  });

  $effect(() => {
    if (!open || activeIndex < 0 || !scroller) return;
    const row = scroller.querySelector<HTMLElement>(
      `[data-line="${activeIndex}"]`,
    );
    if (!row) return;
    const top =
      row.offsetTop - scroller.clientHeight / 2 + row.clientHeight / 2;
    const distance = Math.abs(scroller.scrollTop - top);
    if (distance < 24) return;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    scroller.scrollTo({
      top,
      behavior: reduced ? "auto" : "smooth",
    });
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
      aria-labelledby="karaoke-modal-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="book" size={20} /></span>
          <div>
            <h2 id="karaoke-modal-title">{$t("audiobooks.karaokeModalTitle")}</h2>
            <p>{title}</p>
          </div>
        </div>
        <div class="head-tools">
          <span class="precision" title={$t(PRECISION_HELP[precision])}>
            <Icon name="waveform" size={13} />
            {$t(PRECISION_KEYS[precision])}
          </span>
          <button
            class="close-button"
            type="button"
            onclick={onClose}
            aria-label={$t("audiobooks.karaokeClose")}
            title={$t("audiobooks.karaokeClose")}
          >
            <Icon name="close" size={18} />
          </button>
        </div>
      </header>

      <div class="modal-body" bind:this={scroller}>
        {#if lines.length === 0}
          <p class="empty">{$t("audiobooks.karaokeEmpty")}</p>
        {:else}
          <ol class="karaoke-list" aria-label={$t("audiobooks.karaokeTitle")}>
            {#each lines as line, index (index)}
              <li>
                <button
                  class="karaoke-row"
                  class:current={index === activeIndex}
                  class:past={index < activeIndex}
                  class:upcoming={index > activeIndex}
                  type="button"
                  data-line={index}
                  aria-current={index === activeIndex ? "true" : undefined}
                  title={$t("audiobooks.karaokeJump")}
                  onclick={() => onSeek(startMs + line.startMs)}
                >
                  <span class="row-marker" aria-hidden="true"></span>
                  <span class="row-text">
                    {#each line.segments as segment, segmentIndex (segmentIndex)}
                      {#if segment.wordIndex === null}
                        {segment.text}
                      {:else}
                        <span
                          class="word"
                          class:spoken={index < activeIndex ||
                            (index === activeIndex &&
                              segment.wordIndex < activeWord)}
                          class:active={index === activeIndex &&
                            segment.wordIndex === activeWord}
                        >{segment.text}</span>
                      {/if}
                    {/each}
                  </span>
                  <span class="row-time">
                    {formatClock(line.startMs)}
                  </span>
                </button>
              </li>
            {/each}
          </ol>
        {/if}
      </div>

      <footer class="modal-foot">
        <p class="foot-note">
          {$t(
            precision === "estimated"
              ? "audiobooks.karaokeEstimatedHelp"
              : "audiobooks.karaokeModalHelp",
          )}
        </p>
        <div class="foot-actions">
          <Button variant="subtle" onclick={onOpenText}>
            <Icon name="book" size={16} />
            {$t("audiobooks.openText")}
          </Button>
          <Button variant="primary" onclick={onClose}>
            {$t("common.close")}
          </Button>
        </div>
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
    max-height: min(88vh, 940px);
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
    padding: 18px 20px 15px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background:
      radial-gradient(
        380px 150px at 0% 0%,
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
    font-size: 1.22rem;
    font-weight: 620;
    letter-spacing: -0.018em;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.4;
  }

  .head-tools {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
  }

  .precision {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0.28rem 0.52rem;
    border: 1px solid var(--accent-line);
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
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
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 10px 18px;
    scroll-behavior: smooth;
  }

  .empty {
    margin: 18px;
    padding: 22px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    color: var(--muted);
    font-size: 0.82rem;
    text-align: center;
  }

  .karaoke-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .karaoke-row {
    display: grid;
    grid-template-columns: 3px minmax(0, 1fr) auto;
    align-items: start;
    gap: 12px;
    width: 100%;
    padding: 9px 12px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: color-mix(in srgb, var(--text) 58%, var(--muted));
    font: inherit;
    font-size: 0.98rem;
    line-height: 1.62;
    text-align: left;
    transition:
      background-color 140ms ease,
      color 140ms ease;
  }

  .karaoke-row:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .karaoke-row.past {
    color: color-mix(in srgb, var(--muted) 88%, transparent);
  }

  .karaoke-row.current {
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--accent-line);
  }

  .row-marker {
    width: 3px;
    height: 100%;
    min-height: 20px;
    border-radius: 2px;
    background: transparent;
  }

  .karaoke-row.current .row-marker {
    background: var(--accent);
  }

  .row-text {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .row-time {
    padding-top: 2px;
    color: var(--muted);
    font-size: 0.64rem;
    font-variant-numeric: tabular-nums;
    opacity: 0;
    transition: opacity 140ms ease;
  }

  .karaoke-row:hover .row-time,
  .karaoke-row.current .row-time {
    opacity: 1;
  }

  .word {
    border-radius: 4px;
    transition:
      background-color 110ms ease,
      color 110ms ease;
  }

  .word.spoken {
    color: color-mix(in srgb, var(--text) 68%, var(--muted));
  }

  .word.active {
    padding: 0 3px;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
    font-weight: 650;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 20px;
    border-top: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    background: var(--surface-2);
  }

  .foot-note {
    margin: 0;
    color: var(--muted);
    font-size: 0.68rem;
    line-height: 1.4;
  }

  .foot-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
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
      padding: 14px 15px 12px;
    }

    .head-tools .precision {
      display: none;
    }

    .modal-body {
      padding: 8px 6px 14px;
    }

    .karaoke-row {
      padding: 9px 10px;
      font-size: 0.94rem;
    }

    .modal-foot {
      flex-direction: column;
      align-items: stretch;
      padding: 12px 15px calc(12px + var(--safe-bottom));
    }

    .foot-actions :global(.ui-button) {
      flex: 1;
      padding-inline: 0.6rem;
      white-space: nowrap;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }

    .modal-body {
      scroll-behavior: auto;
    }

    .karaoke-row,
    .row-time,
    .word {
      transition: none;
    }
  }
</style>

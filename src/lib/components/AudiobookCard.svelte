<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { formatDate, formatDuration, t, type MessageKey } from "../i18n";
  import type { AudiobookInfo, EqPreset, NarrationStyle } from "../types";
  import Icon from "./Icon.svelte";

  let {
    item,
    playing = false,
    busy = false,
    favoriteBusy = false,
    onPlay,
    onRead,
    onDownload,
    onChapters,
    onDelete,
    onFavorite,
    onShare,
    onClassify,
    onRetry,
    categoriesLabel = null,
  }: {
    item: AudiobookInfo;
    playing?: boolean;
    busy?: boolean;
    favoriteBusy?: boolean;
    onPlay: () => void;
    onRead: () => void;
    onDownload: () => void;
    onChapters: () => void;
    onDelete: () => void;
    onFavorite: () => void;
    onShare: () => void;
    onClassify: () => void;
    onRetry?: () => void;
    categoriesLabel?: string | null;
  } = $props();

  let broken = $state(false);
  let failed = $derived(item.status === "failed");
  let cancelled = $derived(item.status === "cancelled");
  let cover = $derived(
    item.coverPath && !broken ? convertFileSrc(item.coverPath) : null,
  );
  let percent = $derived(
    Math.round(
      Math.min(1, Math.max(0, Number.isFinite(item.percent) ? item.percent : 0)) *
        100,
    ),
  );
  let originLabel = $derived.by(() => {
    if (item.origin === "tejido") return $t("audiobooks.originTejido");
    if (item.origin === "external") {
      return item.audioExt
        ? `${$t("audiobooks.originImportedShort")} · ${item.audioExt.toUpperCase()}`
        : $t("audiobooks.originImportedShort");
    }
    return $t("audiobooks.originGeneratedShort");
  });
  let stateLabel = $derived.by(() => {
    if (cancelled) return $t("audiobooks.statusCancelled");
    if (failed) return $t("audiobooks.statusFailed");
    if (item.status !== "done") return $t("audiobooks.statusGenerating");
    if (playing) return $t("audiobooks.inProgress");
    if (percent >= 98) return $t("audiobooks.completed");
    if (percent > 0) return $t("audiobooks.percentCompleted", { percent });
    return $t("audiobooks.noStarted");
  });
  let chapterLabel = $derived.by(() => {
    if (item.progressChapterTitle) {
      return $t("audiobooks.chapterNow", {
        chapter: item.progressChapterIndex + 1,
        title: item.progressChapterTitle,
      });
    }
    return $t("audiobooks.chapterCount", { count: item.chapterCount });
  });
  let metaLabel = $derived(
    item.lastListenedAt
      ? $t("audiobooks.lastListened", {
          date: formatDate(item.lastListenedAt, {
            day: "numeric",
            month: "short",
          }),
        })
      : $t("audiobooks.createdOn", {
          date: formatDate(item.createdAt, { day: "numeric", month: "short" }),
        }),
  );

  const NARRATION_KEYS: Record<NarrationStyle, MessageKey> = {
    neutral: "audiobooks.narrationNeutral",
    calm: "audiobooks.narrationCalm",
    dramatic: "audiobooks.narrationDramatic",
    lively: "audiobooks.narrationLively",
  };

  let narrationLabel = $derived.by(() => {
    const narration = item.narration;
    if (!narration) return null;
    if (narration.style === "neutral" && Math.abs(narration.speed - 1) < 0.001) {
      return null;
    }
    const speed =
      Math.abs(narration.speed - 1) < 0.001
        ? "1×"
        : `${narration.speed.toFixed(2).replace(/0$/, "")}×`;
    return $t("audiobooks.narrationBadge", {
      style: $t(NARRATION_KEYS[narration.style]),
      speed,
    });
  });

  const EQ_KEYS: Record<EqPreset, MessageKey> = {
    neutral: "audiobooks.treatmentEqNeutral",
    clearVoice: "audiobooks.treatmentEqClear",
    warm: "audiobooks.treatmentEqWarm",
    bright: "audiobooks.treatmentEqBright",
  };

  let castVoiceCount = $derived(
    new Set(
      [item.voiceId, ...item.cast.map((member) => member.voiceId)].filter(
        (voiceId): voiceId is string => Boolean(voiceId),
      ),
    ).size,
  );
  let castLabel = $derived(
    item.cast.length > 0 && castVoiceCount > 1
      ? $t("audiobooks.castBadge", { count: castVoiceCount })
      : null,
  );
  let castTitle = $derived(
    item.cast
      .map((member) => `${member.character}: ${member.voiceName}`)
      .join("\n"),
  );

  let treatmentLabel = $derived.by(() => {
    const treatment = item.treatment;
    if (!treatment) return null;
    if (treatment.normalize) {
      return $t("audiobooks.lufsBadge", { value: treatment.targetLufs });
    }
    if (treatment.eq !== "neutral") {
      return $t(EQ_KEYS[treatment.eq]);
    }
    return null;
  });
</script>

<article class="audiobook-card" class:playing class:pending={item.status !== "done"}>
  <div class="cover-wrap">
    <button
      class="favorite-button"
      class:active={item.favorite}
      type="button"
      aria-pressed={item.favorite}
      aria-label={item.favorite
        ? $t("audiobooks.removeFavorite")
        : $t("audiobooks.addFavorite")}
      title={item.favorite
        ? $t("audiobooks.removeFavorite")
        : $t("audiobooks.addFavorite")}
      onclick={onFavorite}
      disabled={favoriteBusy}
    >
      <Icon name="star" size={16} />
    </button>

    <button
      class="cover-button"
      onclick={onPlay}
      disabled={item.status !== "done" || busy}
      aria-label={playing ? $t("audiobooks.pause") : $t("audiobooks.listen")}
    >
      {#if cover}
        <img
          src={cover}
          alt=""
          loading="lazy"
          onerror={() => (broken = true)}
        />
      {:else}
        <span class="cover-placeholder">
          <Icon name="headphones" size={24} />
          <strong>{item.bookTitle}</strong>
        </span>
      {/if}
      <span class="cover-shade" aria-hidden="true"></span>
      {#if item.status === "done"}
        <span class="play-orb" class:playing>
          <Icon name={playing ? "pause" : "play"} size={17} />
        </span>
      {:else}
        <span class="state-orb" class:error={failed} class:cancelled>
          <Icon name={failed || cancelled ? "close" : "sparkles"} size={16} />
        </span>
      {/if}
    </button>
  </div>

  <div class="card-body">
    <div class="badge-row">
      <span class="badge origin">{originLabel}</span>
      {#if narrationLabel}
        <span class="badge style">{narrationLabel}</span>
      {/if}
      {#if castLabel}
        <span class="badge cast" title={castTitle}>{castLabel}</span>
      {/if}
      {#if treatmentLabel}
        <span class="badge treatment">{treatmentLabel}</span>
      {/if}
      {#if item.audioOnly}
        <span class="badge audio-only">{$t("audiobooks.audioOnlyBadge")}</span>
      {/if}
      {#if item.watermark}
        <span class="badge warn">{$t("audiobooks.clonedShort")}</span>
      {/if}
      {#if categoriesLabel}
        <span class="badge category">{categoriesLabel}</span>
      {/if}
      <span class="state" class:error={failed}>{stateLabel}</span>
    </div>

    <div class="titles">
      <strong title={item.bookTitle}>{item.bookTitle}</strong>
      <p>{item.author ?? "—"}</p>
    </div>

    <div class="voice-line">
      <Icon name="headphones" size={15} />
      <span class="voice-name">{item.voiceName ?? $t("audiobooks.originImportedShort")}</span>
      <span class="dot" aria-hidden="true">·</span>
      <span class="duration">{formatDuration(item.durationMs)}</span>
    </div>

    <div class="progress-block">
      <div class="progress-head">
        <span class="chapter" title={chapterLabel}>{chapterLabel}</span>
        <span class="percent">{percent}%</span>
      </div>
      <div class="progress-track" aria-hidden="true">
        <span style={`width:${percent}%`}></span>
      </div>
      <div class="progress-foot">
        <span class="meta">{metaLabel}</span>
        <div class="card-actions">
          <button
            class="icon-action"
            type="button"
            onclick={onClassify}
            disabled={busy}
            title={$t("classification.detect")}
            aria-label={$t("classification.detect")}
          >
            <Icon name="sparkles" size={16} />
          </button>
          <button
            class="icon-action"
            type="button"
            onclick={onRead}
            disabled={item.audioOnly}
            title={item.audioOnly
              ? $t("audiobooks.audioOnlyNoText")
              : $t("audiobooks.openText")}
            aria-label={$t("audiobooks.openText")}
          >
            <Icon name="book" size={16} />
          </button>
          <button
            class="icon-action"
            type="button"
            onclick={onShare}
            disabled={item.status !== "done" || busy}
            title={$t("audiobooks.share")}
            aria-label={$t("audiobooks.share")}
          >
            <Icon name="share" size={16} />
          </button>
          <button
            class="icon-action"
            type="button"
            onclick={onDownload}
            disabled={item.status !== "done" || busy}
            title={$t("audiobooks.download")}
            aria-label={$t("audiobooks.download")}
          >
            <Icon name="download" size={16} />
          </button>
          <button
            class="icon-action"
            type="button"
            onclick={onChapters}
            disabled={item.status !== "done"}
            title={$t("audiobooks.openChapters")}
            aria-label={$t("audiobooks.openChapters")}
          >
            <Icon name="chapters" size={16} />
          </button>
          {#if (failed || cancelled) && onRetry}
            <button
              class="icon-action retry"
              type="button"
              onclick={onRetry}
              disabled={busy}
              title={$t("audiobooks.retry")}
              aria-label={$t("audiobooks.retry")}
            >
              <Icon name="sync" size={16} />
            </button>
          {/if}
          <button
            class="icon-action danger"
            type="button"
            onclick={onDelete}
            title={$t("audiobooks.delete")}
            aria-label={$t("audiobooks.delete")}
          >
            <Icon name="trash" size={16} />
          </button>
        </div>
      </div>
    </div>
  </div>
</article>

<style>
  .audiobook-card {
    position: relative;
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr);
    gap: 16px;
    min-width: 0;
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    transition:
      transform 180ms ease,
      border-color 180ms ease,
      box-shadow 180ms ease;
  }

  .cover-wrap {
    position: relative;
    align-self: start;
    width: 112px;
    height: 112px;
  }

  .favorite-button {
    position: absolute;
    top: 6px;
    right: 6px;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    min-width: 32px;
    min-height: 32px;
    padding: 0;
    border: 1px solid color-mix(in srgb, #fff 30%, transparent);
    border-radius: 50%;
    background: color-mix(in srgb, #101418 38%, transparent);
    color: #fff;
    box-shadow: 0 6px 16px -9px rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(7px);
    transition:
      transform 150ms ease,
      background-color 150ms ease,
      color 150ms ease;
  }

  .favorite-button:hover:not(:disabled) {
    transform: scale(1.08);
    background: color-mix(in srgb, #101418 55%, transparent);
  }

  .favorite-button.active {
    border-color: color-mix(in srgb, var(--gold) 55%, transparent);
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    color: #e8a72d;
  }

  .favorite-button.active :global(svg) {
    fill: currentColor;
  }

  .favorite-button:disabled {
    opacity: 0.55;
  }

  .audiobook-card:hover {
    transform: translateY(-2px);
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border));
    box-shadow:
      0 18px 34px -28px var(--shadow-color),
      0 1px 0 color-mix(in srgb, #fff 12%, transparent);
  }

  .audiobook-card.playing {
    border-color: var(--accent-line);
    box-shadow:
      var(--shadow-sm),
      0 0 0 1px color-mix(in srgb, var(--gold) 18%, transparent);
  }

  .cover-button {
    position: relative;
    display: block;
    align-self: start;
    width: 112px;
    height: 112px;
    min-height: 112px;
    padding: 0;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    border-radius: 14px;
    background: var(--surface-3);
    box-shadow:
      0 12px 24px -18px var(--shadow-color),
      inset 0 1px 0 color-mix(in srgb, #fff 18%, transparent);
  }

  .cover-button:disabled {
    cursor: default;
    opacity: 1;
  }

  .cover-button img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 320ms ease;
  }

  .audiobook-card:hover .cover-button img {
    transform: scale(1.045);
  }

  .cover-placeholder {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: flex-start;
    width: 100%;
    height: 100%;
    padding: 12px 10px;
    background:
      linear-gradient(152deg, #f59e0b, #9a3412 58%, #431407);
    color: #fff;
    text-align: left;
  }

  .cover-placeholder strong {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 0.76rem;
    font-weight: 620;
    line-height: 1.25;
  }

  .cover-shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.48), transparent 58%);
    pointer-events: none;
  }

  .play-orb,
  .state-orb {
    position: absolute;
    right: 9px;
    bottom: 9px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 1px solid color-mix(in srgb, #fff 28%, transparent);
    border-radius: 50%;
    background: linear-gradient(135deg, var(--gold), var(--gold-deep));
    color: #fff;
    box-shadow:
      0 8px 18px -8px color-mix(in srgb, var(--gold-deep) 75%, transparent),
      0 0 14px -4px color-mix(in srgb, var(--gold) 45%, transparent);
    transition:
      transform 160ms ease,
      background-color 160ms ease;
  }

  .cover-button:hover .play-orb {
    transform: scale(1.08);
  }

  .play-orb.playing {
    background: var(--surface);
    color: var(--accent);
  }

  .state-orb {
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    color: var(--accent);
    backdrop-filter: blur(6px);
  }

  .state-orb.error {
    color: var(--danger);
  }

  .state-orb.cancelled {
    color: var(--muted);
  }

  .card-body {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
  }

  .badge-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 22px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    min-height: 21px;
    padding: 0.08rem 0.46rem;
    border: 1px solid transparent;
    border-radius: 6px;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    white-space: nowrap;
  }

  .badge.origin {
    border-color: var(--accent-line);
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
    color: var(--accent);
  }

  .badge.warn {
    border-color: color-mix(in srgb, var(--warn) 45%, var(--border));
    background: color-mix(in srgb, var(--warn) 10%, transparent);
    color: var(--warn);
  }

  .badge.style {
    border-color: var(--border);
    background: var(--surface-3);
    color: var(--muted);
  }

  .badge.cast {
    border-color: color-mix(in srgb, var(--accent) 32%, var(--border));
    background: var(--accent-soft);
    color: var(--accent);
    cursor: help;
  }

  .badge.treatment {
    border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    color: var(--accent);
  }

  .badge.audio-only {
    border-color: color-mix(in srgb, var(--muted) 35%, var(--border));
    background: var(--surface-3);
    color: var(--muted);
  }

  .badge.category {
    border-color: color-mix(in srgb, var(--gold) 40%, var(--border));
    background: color-mix(in srgb, var(--gold) 10%, transparent);
    color: color-mix(in srgb, var(--gold) 70%, var(--text));
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    margin-left: auto;
    color: var(--muted);
    font-size: 0.64rem;
    white-space: nowrap;
  }

  .state.error {
    color: var(--danger);
  }

  .titles {
    min-width: 0;
  }

  .titles strong {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 1.14rem;
    font-weight: 620;
    line-height: 1.2;
    letter-spacing: -0.012em;
  }

  .titles p {
    margin: 2px 0 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .voice-line {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    color: var(--muted);
    font-size: 0.68rem;
  }

  .voice-line :global(svg) {
    flex: 0 0 auto;
    color: var(--accent);
  }

  .voice-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    opacity: 0.6;
  }

  .duration {
    flex: 0 0 auto;
    font-variant-numeric: tabular-nums;
  }

  .progress-block {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-top: auto;
    padding-top: 9px;
    border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  }

  .progress-head,
  .progress-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
    font-size: 0.67rem;
  }

  .chapter {
    overflow: hidden;
    color: var(--muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .percent {
    flex: 0 0 auto;
    color: var(--accent);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .progress-track {
    width: 100%;
    height: 5px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .progress-track span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--gold), var(--accent-2));
    box-shadow: 0 0 8px -2px color-mix(in srgb, var(--gold) 60%, transparent);
    transition: width 280ms ease;
  }

  .meta {
    overflow: hidden;
    color: var(--muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 1px;
    flex: 0 0 auto;
  }

  .icon-action {
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
  }

  .icon-action:hover:not(:disabled) {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .icon-action.danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    color: var(--danger);
  }

  .icon-action:disabled {
    opacity: 0.38;
  }

  @media (prefers-reduced-motion: reduce) {
    .audiobook-card,
    .cover-button img,
    .play-orb,
    .progress-track span {
      transition: none;
    }

    .audiobook-card:hover {
      transform: none;
    }

    .audiobook-card:hover .cover-button img {
      transform: none;
    }
  }

  @media (max-width: 640px) {
    .audiobook-card {
      grid-template-columns: 84px minmax(0, 1fr);
      gap: 12px;
      padding: 12px;
    }

    .favorite-button {
      top: 5px;
      right: 5px;
      width: 27px;
      height: 27px;
    }

    .cover-wrap {
      width: 84px;
      height: 84px;
    }

    .cover-button {
      width: 84px;
      height: 84px;
      min-height: 84px;
      border-radius: 11px;
    }

    .cover-placeholder {
      padding: 8px 7px;
    }

    .cover-placeholder strong {
      -webkit-line-clamp: 3;
      line-clamp: 3;
      font-size: 0.68rem;
    }

    .play-orb,
    .state-orb {
      right: 6px;
      bottom: 6px;
      width: 30px;
      height: 30px;
    }

    .titles strong {
      font-size: 0.98rem;
    }

    .voice-line {
      font-size: 0.65rem;
    }

    .state {
      font-size: 0.6rem;
    }

    .progress-head,
    .progress-foot {
      flex-wrap: wrap;
      font-size: 0.61rem;
    }

    .card-actions {
      width: 100%;
      flex-wrap: wrap;
      justify-content: flex-end;
    }

    .icon-action {
      width: 30px;
      min-width: 30px;
      height: 30px;
      min-height: 30px;
    }
  }
</style>

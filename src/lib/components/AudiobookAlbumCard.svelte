<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { formatDuration, t } from "../i18n";
  import type { AudiobookAlbumInfo } from "../types";
  import Icon from "./Icon.svelte";

  let {
    album,
    covers = [],
    durationMs = 0,
    percent = 0,
    playing = false,
    onOpen,
    onPlay,
    onEdit,
    onDelete,
  }: {
    album: AudiobookAlbumInfo;
    covers?: (string | null)[];
    durationMs?: number;
    percent?: number;
    playing?: boolean;
    onOpen: () => void;
    onPlay: () => void;
    onEdit: () => void;
    onDelete: () => void;
  } = $props();

  let count = $derived(album.audiobookIds.length);
  let countLabel = $derived(
    count === 1
      ? $t("albums.oneItem")
      : $t("albums.items", { count }),
  );
  let mosaic = $derived(covers.filter((cover): cover is string => Boolean(cover)).slice(0, 4));
  let roundedPercent = $derived(
    Math.round(Math.min(1, Math.max(0, Number.isFinite(percent) ? percent : 0)) * 100),
  );

  function coverSrc(path: string): string {
    return path.startsWith("data:") ? path : convertFileSrc(path);
  }
</script>

<article class="album-card" class:playing>
  <button class="album-cover" type="button" onclick={onOpen} aria-label={album.title}>
    <div
      class="mosaic"
      class:single={mosaic.length === 1}
      class:two={mosaic.length === 2}
      class:empty={mosaic.length === 0}
    >
      {#each mosaic as cover, index (cover + index)}
        <img src={coverSrc(cover)} alt="" loading="lazy" />
      {/each}
      {#if mosaic.length === 0}
        <span class="cover-empty"><Icon name="headphones" size={28} /></span>
      {/if}
    </div>
    <span class="album-badge"><Icon name="grid" size={13} /> {countLabel}</span>
  </button>

  <div class="album-body">
    <div class="album-titles">
      <strong>{album.title}</strong>
      <p>{album.description ?? $t("albums.subtitle")}</p>
    </div>

    <div class="album-meta">
      <span>{formatDuration(durationMs)}</span>
      {#if durationMs > 0}
        <span class="dot" aria-hidden="true">·</span>
        <span>{$t("albums.progress", { percent: roundedPercent })}</span>
      {/if}
    </div>

    <div class="album-progress" aria-hidden="true">
      <span style={`width:${roundedPercent}%`}></span>
    </div>

    <div class="album-actions">
      <button class="play-button" type="button" onclick={onPlay} disabled={count === 0}>
        <Icon name="play" size={15} />
        {$t("albums.playAll")}
      </button>
      <button
        class="icon-button"
        type="button"
        onclick={onEdit}
        title={$t("albums.edit")}
        aria-label={$t("albums.edit")}
      >
        <Icon name="edit" size={15} />
      </button>
      <button
        class="icon-button danger"
        type="button"
        onclick={onDelete}
        title={$t("albums.delete")}
        aria-label={$t("albums.delete")}
      >
        <Icon name="trash" size={15} />
      </button>
    </div>
  </div>
</article>

<style>
  .album-card {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    transition:
      transform 160ms ease,
      border-color 160ms ease,
      box-shadow 160ms ease;
  }

  .album-card:hover {
    transform: translateY(-2px);
    border-color: var(--accent-line);
    box-shadow: var(--shadow-md);
  }

  .album-card.playing {
    border-color: var(--accent-line);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .album-cover {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    padding: 0;
    border: 0;
    background: var(--surface-2);
    cursor: pointer;
    overflow: hidden;
  }

  .mosaic {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    grid-template-rows: repeat(2, 1fr);
    width: 100%;
    height: 100%;
    gap: 2px;
    background: var(--surface-3);
  }

  .mosaic.single {
    grid-template-columns: 1fr;
    grid-template-rows: 1fr;
  }

  .mosaic.two {
    grid-template-rows: 1fr;
  }

  .mosaic.empty {
    display: grid;
    place-items: center;
    background:
      radial-gradient(circle at 30% 20%, color-mix(in srgb, var(--accent) 16%, transparent), transparent 60%),
      var(--surface-2);
  }

  .mosaic img {
    width: 100%;
    height: 100%;
    min-height: 0;
    object-fit: cover;
  }

  .cover-empty {
    display: grid;
    place-items: center;
    color: var(--accent);
  }

  .album-badge {
    position: absolute;
    left: 10px;
    bottom: 10px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 86%, transparent);
    color: var(--text);
    font-size: 0.66rem;
    font-weight: 650;
    backdrop-filter: blur(8px);
  }

  .album-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 13px 14px 12px;
  }

  .album-titles strong {
    display: block;
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 1.02rem;
    font-weight: 620;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .album-titles p {
    display: -webkit-box;
    margin: 3px 0 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.72rem;
    line-height: 1.4;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .album-meta {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 550;
  }

  .dot {
    opacity: 0.55;
  }

  .album-progress {
    height: 3px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .album-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
  }

  .album-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
  }

  .play-button {
    display: inline-flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 32px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 650;
  }

  .play-button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .play-button:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .icon-button:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .icon-button.danger:hover {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 40%, var(--border));
  }
</style>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { formatDuration, t } from "../i18n";
  import type { AudiobookAlbumInfo, AudiobookInfo } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    album,
    items,
    busy = false,
    error = null,
    onClose,
    onPlay,
    onPlayItem,
    onEdit,
    onAdd,
    onRemove,
    onMove,
  }: {
    open: boolean;
    album: AudiobookAlbumInfo | null;
    items: AudiobookInfo[];
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onPlay: () => void;
    onPlayItem: (item: AudiobookInfo) => void;
    onEdit: () => void;
    onAdd: () => void;
    onRemove: (item: AudiobookInfo) => void;
    onMove: (item: AudiobookInfo, direction: -1 | 1) => void;
  } = $props();

  let totalDuration = $derived(
    items.reduce((sum, item) => sum + Math.max(0, item.durationMs), 0),
  );
  let percent = $derived.by(() => {
    if (totalDuration <= 0) return 0;
    const listened = items.reduce(
      (sum, item) => sum + Math.min(1, Math.max(0, item.percent)) * item.durationMs,
      0,
    );
    return Math.round((listened / totalDuration) * 100);
  });
  let countLabel = $derived(
    items.length === 1
      ? $t("albums.oneItem")
      : $t("albums.items", { count: items.length }),
  );

  function coverSrc(path: string | null): string | null {
    if (!path) return null;
    return path.startsWith("data:") ? path : convertFileSrc(path);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open && album}
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
      aria-labelledby="album-detail-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="grid" size={20} /></span>
          <div>
            <h2 id="album-detail-title">{album.title}</h2>
            <p>
              {countLabel}
              {#if totalDuration > 0}
                · {formatDuration(totalDuration)} · {$t("albums.progress", { percent })}
              {/if}
            </p>
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

        {#if album.description}
          <p class="description">{album.description}</p>
        {/if}

        <div class="album-toolbar">
          <Button
            variant="primary"
            size="small"
            onclick={onPlay}
            disabled={busy || items.length === 0}
          >
            <Icon name="play" size={15} />
            {$t("albums.playAll")}
          </Button>
          <Button variant="subtle" size="small" onclick={onAdd} disabled={busy}>
            <Icon name="plus" size={15} />
            {$t("albums.add")}
          </Button>
          <button class="link-button" type="button" onclick={onEdit} disabled={busy}>
            <Icon name="edit" size={14} />
            {$t("albums.edit")}
          </button>
        </div>

        {#if items.length === 0}
          <div class="empty">
            <span><Icon name="headphones" size={24} /></span>
            <strong>{$t("albums.emptyItems")}</strong>
            <p>{$t("albums.emptyHelp")}</p>
          </div>
        {:else}
          <ol class="album-list">
            {#each items as item, index (item.id)}
              <li>
                <span class="order">{index + 1}</span>
                <button
                  class="item-cover"
                  type="button"
                  onclick={() => onPlayItem(item)}
                  disabled={item.status !== "done" || busy}
                  aria-label={$t("audiobooks.listen")}
                >
                  {#if coverSrc(item.coverPath)}
                    <img src={coverSrc(item.coverPath)} alt="" loading="lazy" />
                  {:else}
                    <Icon name="headphones" size={16} />
                  {/if}
                </button>
                <div class="item-copy">
                  <strong>{item.bookTitle}</strong>
                  <small>
                    {item.author ?? "—"} · {formatDuration(item.durationMs)}
                  </small>
                  <div class="item-progress" aria-hidden="true">
                    <span style={`width:${Math.round(Math.min(1, Math.max(0, item.percent)) * 100)}%`}></span>
                  </div>
                </div>
                <div class="item-actions">
                  <button
                    class="icon-button"
                    type="button"
                    onclick={() => onPlayItem(item)}
                    disabled={item.status !== "done" || busy}
                    title={$t("audiobooks.listen")}
                    aria-label={$t("audiobooks.listen")}
                  >
                    <Icon name="play" size={14} />
                  </button>
                  <button
                    class="icon-button"
                    type="button"
                    onclick={() => onMove(item, -1)}
                    disabled={busy || index === 0}
                    title={$t("albums.moveUp")}
                    aria-label={$t("albums.moveUp")}
                  >
                    ↑
                  </button>
                  <button
                    class="icon-button"
                    type="button"
                    onclick={() => onMove(item, 1)}
                    disabled={busy || index === items.length - 1}
                    title={$t("albums.moveDown")}
                    aria-label={$t("albums.moveDown")}
                  >
                    ↓
                  </button>
                  <button
                    class="icon-button danger"
                    type="button"
                    onclick={() => onRemove(item)}
                    disabled={busy}
                    title={$t("albums.remove")}
                    aria-label={$t("albums.remove")}
                  >
                    <Icon name="close" size={14} />
                  </button>
                </div>
              </li>
            {/each}
          </ol>
        {/if}
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("albums.done")}
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
    padding: 0;
    border: 0;
    background: transparent;
  }

  .modal {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    width: min(720px, 100%);
    max-height: min(88vh, 860px);
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
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.18rem;
    font-weight: 620;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.74rem;
  }

  .close-button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
  }

  .close-button:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 13px;
    padding: 16px 20px 18px;
    overflow-y: auto;
  }

  .description {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .album-toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 7px;
  }

  .link-button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 32px;
    padding: 0 4px;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 0.72rem;
    font-weight: 650;
  }

  .link-button:disabled {
    opacity: 0.5;
  }

  .notice {
    margin: 0;
    padding: 9px 11px;
    border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--danger) 7%, transparent);
    color: var(--danger);
    font-size: 0.75rem;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 32px 20px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    text-align: center;
  }

  .empty > span {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--accent);
  }

  .empty strong {
    font-family: var(--font-display);
    font-size: 1rem;
  }

  .empty p {
    margin: 0;
    color: var(--muted);
    font-size: 0.74rem;
  }

  .album-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .album-list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 9px;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface-2);
  }

  .order {
    flex: 0 0 auto;
    width: 18px;
    color: var(--muted);
    font-size: 0.66rem;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }

  .item-cover {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 40px;
    height: 54px;
    overflow: hidden;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: var(--surface-3);
    color: var(--accent);
  }

  .item-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .item-cover:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .item-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .item-copy strong {
    overflow: hidden;
    font-size: 0.8rem;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-copy small {
    color: var(--muted);
    font-size: 0.68rem;
  }

  .item-progress {
    width: 100%;
    height: 2px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }

  .item-progress span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .item-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--muted);
    font: inherit;
    font-size: 0.76rem;
  }

  .icon-button:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--accent-line);
  }

  .icon-button:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .icon-button.danger:hover:not(:disabled) {
    color: var(--danger);
  }

  .modal-foot {
    display: flex;
    justify-content: flex-end;
    padding: 13px 20px;
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
      max-height: 92vh;
      border-radius: var(--radius-xl) var(--radius-xl) 0 0;
    }

    .album-list li {
      flex-wrap: wrap;
    }

    .item-actions {
      width: 100%;
      justify-content: flex-end;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }
  }
</style>

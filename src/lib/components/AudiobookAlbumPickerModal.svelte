<script lang="ts">
  import { t } from "../i18n";
  import type { AudiobookAlbumInfo, AudiobookInfo } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    albums,
    audiobook,
    busy = false,
    error = null,
    onClose,
    onSave,
  }: {
    open: boolean;
    albums: AudiobookAlbumInfo[];
    audiobook: AudiobookInfo | null;
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onSave: (albumIds: string[]) => void;
  } = $props();

  let selected = $state<Set<string>>(new Set());
  let lastKey = "";

  $effect(() => {
    if (!open || !audiobook) {
      if (!open) lastKey = "";
      return;
    }
    const key = `${audiobook.id}:${albums.map((album) => album.id).join(",")}`;
    if (key === lastKey) return;
    lastKey = key;
    selected = new Set(
      albums
        .filter((album) => album.audiobookIds.includes(audiobook.id))
        .map((album) => album.id),
    );
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  function toggle(albumId: string) {
    const next = new Set(selected);
    if (next.has(albumId)) next.delete(albumId);
    else next.add(albumId);
    selected = next;
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

{#if open && audiobook}
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
      aria-labelledby="album-picker-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="grid" size={20} /></span>
          <div>
            <h2 id="album-picker-title">{$t("albums.pickerTitle")}</h2>
            <p>{$t("albums.pickerHelp", { title: audiobook.bookTitle })}</p>
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

        {#if albums.length === 0}
          <div class="empty">
            <span><Icon name="grid" size={22} /></span>
            <strong>{$t("albums.noAlbums")}</strong>
          </div>
        {:else}
          <ul class="album-options">
            {#each albums as album (album.id)}
              <li>
                <label>
                  <input
                    type="checkbox"
                    checked={selected.has(album.id)}
                    onchange={() => toggle(album.id)}
                    disabled={busy}
                  />
                  <span class="option-copy">
                    <strong>{album.title}</strong>
                    <small>
                      {album.audiobookIds.length === 1
                        ? $t("albums.oneItem")
                        : $t("albums.items", { count: album.audiobookIds.length })}
                    </small>
                  </span>
                  {#if selected.has(album.id)}
                    <Icon name="check" size={16} />
                  {/if}
                </label>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={() => onSave([...selected])}
          loading={busy}
          disabled={busy || albums.length === 0}
        >
          <Icon name="check" size={16} />
          {$t("albums.save")}
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
    width: min(520px, 100%);
    max-height: min(88vh, 720px);
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
    font-size: 1.12rem;
    font-weight: 620;
  }

  .head-copy p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.74rem;
    line-height: 1.4;
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
    gap: 12px;
    padding: 16px 20px 18px;
    overflow-y: auto;
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
    gap: 7px;
    padding: 28px 18px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    text-align: center;
  }

  .empty > span {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--accent);
  }

  .empty strong {
    font-size: 0.82rem;
    font-weight: 650;
  }

  .album-options {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .album-options label {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    cursor: pointer;
  }

  .album-options label:hover {
    border-color: var(--accent-line);
  }

  .album-options input {
    width: 17px;
    height: 17px;
    accent-color: var(--accent);
  }

  .option-copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .option-copy strong {
    overflow: hidden;
    font-size: 0.8rem;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .option-copy small {
    color: var(--muted);
    font-size: 0.68rem;
  }

  .modal-foot {
    display: flex;
    gap: 8px;
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
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-backdrop,
    .modal {
      animation: none;
    }
  }
</style>

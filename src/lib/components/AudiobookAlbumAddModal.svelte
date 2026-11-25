<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { formatDuration, t } from "../i18n";
  import type { AudiobookAlbumInfo, AudiobookInfo } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    album = null,
    audiobooks = [],
    busy = false,
    error = null,
    onClose,
    onAdd,
  }: {
    open: boolean;
    album?: AudiobookAlbumInfo | null;
    audiobooks?: AudiobookInfo[];
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onAdd: (ids: string[]) => void;
  } = $props();

  let query = $state("");
  let selected = $state<Set<string>>(new Set());
  let lastKey = "";

  let candidates = $derived.by(() => {
    if (!album) return [];
    const existing = new Set(album.audiobookIds);
    const needle = query.trim().toLowerCase();
    return audiobooks
      .filter(
        (item) =>
          item.status === "done" &&
          !existing.has(item.id) &&
          (!needle ||
            `${item.bookTitle} ${item.author ?? ""}`
              .toLowerCase()
              .includes(needle)),
      )
      .sort((left, right) => left.bookTitle.localeCompare(right.bookTitle));
  });

  $effect(() => {
    if (!open || !album) {
      if (!open) lastKey = "";
      return;
    }
    if (lastKey === album.id) return;
    lastKey = album.id;
    query = "";
    selected = new Set();
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  function toggle(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

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
      aria-labelledby="album-add-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="plus" size={20} /></span>
          <div>
            <h2 id="album-add-title">{$t("albums.add")}</h2>
            <p>{album.title}</p>
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

        <label class="search-field">
          <Icon name="search" size={16} />
          <input
            type="search"
            bind:value={query}
            placeholder={$t("audiobooks.searchPlaceholder")}
            aria-label={$t("audiobooks.searchPlaceholder")}
          />
        </label>

        {#if candidates.length === 0}
          <p class="empty">{$t("albums.noAvailable")}</p>
        {:else}
          <ul class="candidates">
            {#each candidates as item (item.id)}
              <li>
                <label>
                  <input
                    type="checkbox"
                    checked={selected.has(item.id)}
                    onchange={() => toggle(item.id)}
                    disabled={busy}
                  />
                  <span class="cover">
                    {#if coverSrc(item.coverPath)}
                      <img src={coverSrc(item.coverPath)} alt="" loading="lazy" />
                    {:else}
                      <Icon name="headphones" size={15} />
                    {/if}
                  </span>
                  <span class="copy">
                    <strong>{item.bookTitle}</strong>
                    <small>
                      {item.author ?? "—"} · {formatDuration(item.durationMs)}
                    </small>
                  </span>
                  {#if selected.has(item.id)}
                    <Icon name="check" size={15} />
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
          onclick={() => onAdd([...selected])}
          loading={busy}
          disabled={busy || selected.size === 0}
        >
          <Icon name="plus" size={16} />
          {$t("albums.addSelected")}
        </Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 82;
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
    width: min(560px, 100%);
    max-height: min(88vh, 780px);
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
    gap: 11px;
    padding: 16px 20px 18px;
    overflow-y: auto;
  }

  .search-field {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 38px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .search-field input {
    width: 100%;
    min-height: 34px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
    outline: none;
  }

  .empty {
    margin: 0;
    padding: 20px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    color: var(--muted);
    font-size: 0.76rem;
    text-align: center;
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

  .candidates {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .candidates label {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    cursor: pointer;
  }

  .candidates label:hover {
    border-color: var(--accent-line);
  }

  .candidates input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .cover {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 34px;
    height: 46px;
    overflow: hidden;
    border-radius: 5px;
    background: var(--surface-3);
    color: var(--accent);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .copy strong {
    overflow: hidden;
    font-size: 0.78rem;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .copy small {
    color: var(--muted);
    font-size: 0.66rem;
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

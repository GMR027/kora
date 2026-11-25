<script lang="ts">
  import { t } from "../i18n";
  import type { AudiobookAlbumInfo } from "../types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    album = null,
    busy = false,
    error = null,
    onClose,
    onSave,
  }: {
    open: boolean;
    album?: AudiobookAlbumInfo | null;
    busy?: boolean;
    error?: string | null;
    onClose: () => void;
    onSave: (input: { title: string; description: string | null }) => void;
  } = $props();

  let title = $state("");
  let description = $state("");
  let localError = $state<string | null>(null);
  let wasOpen = false;

  $effect(() => {
    if (!open) {
      wasOpen = false;
      return;
    }
    if (wasOpen) return;
    wasOpen = true;
    title = album?.title ?? "";
    description = album?.description ?? "";
    localError = null;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  });

  function submit() {
    const trimmed = title.trim();
    if (!trimmed) {
      localError = $t("albums.nameRequired");
      return;
    }
    localError = null;
    onSave({
      title: trimmed,
      description: description.trim() ? description.trim() : null,
    });
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
      aria-labelledby="album-editor-title"
    >
      <header class="modal-head">
        <div class="head-copy">
          <span class="head-icon"><Icon name="grid" size={20} /></span>
          <div>
            <h2 id="album-editor-title">
              {album ? $t("albums.editTitle") : $t("albums.createTitle")}
            </h2>
            <p>{$t("albums.subtitle")}</p>
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
        {#if error || localError}
          <p class="notice" role="alert">{error ?? localError}</p>
        {/if}

        <label class="field">
          <span>{$t("albums.name")}</span>
          <input
            bind:value={title}
            maxlength="120"
            placeholder={$t("albums.namePlaceholder")}
            disabled={busy}
          />
        </label>

        <label class="field">
          <span>{$t("albums.description")}</span>
          <textarea
            bind:value={description}
            maxlength="2000"
            rows="4"
            placeholder={$t("albums.descriptionPlaceholder")}
            disabled={busy}
          ></textarea>
        </label>
      </div>

      <footer class="modal-foot">
        <Button variant="subtle" onclick={onClose} disabled={busy}>
          {$t("common.cancel")}
        </Button>
        <Button variant="primary" onclick={submit} loading={busy}>
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
    z-index: 84;
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
    max-height: min(88vh, 700px);
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
    gap: 14px;
    padding: 18px 20px;
    overflow-y: auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .field input,
  .field textarea {
    width: 100%;
    padding: 9px 11px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
    font-weight: 500;
    outline: none;
    resize: vertical;
  }

  .field input:focus,
  .field textarea:focus {
    border-color: var(--accent);
    background: var(--surface);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 10%, transparent);
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

  .modal-foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
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

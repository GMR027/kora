<script lang="ts">
  import { fly } from "svelte/transition";
  import { dismiss, toasts } from "../toast";
</script>

<div class="toasts" aria-live="polite" aria-atomic="false">
  {#each $toasts as toast (toast.id)}
    <div
      class="toast {toast.kind}"
      role={toast.kind === "error" ? "alert" : "status"}
      in:fly={{ y: 12, duration: 180 }}
      out:fly={{ y: 8, duration: 140 }}
    >
      <div class="content">
        <strong>{toast.title}</strong>
        {#if toast.description}
          <p>{toast.description}</p>
        {/if}
      </div>
      <button
        class="close"
        onclick={() => dismiss(toast.id)}
        aria-label="Cerrar notificación"
      >
        ×
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: max(1rem, env(safe-area-inset-right));
    bottom: calc(5.5rem + env(safe-area-inset-bottom));
    z-index: 80;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    width: min(380px, calc(100vw - 2rem));
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    gap: 0.65rem;
    align-items: flex-start;
    justify-content: space-between;
    padding: 0.8rem 0.9rem;
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: 14px;
    background: color-mix(in srgb, var(--surface) 95%, transparent);
    box-shadow: var(--shadow-lg);
    backdrop-filter: blur(14px);
  }

  .toast.error {
    border-left-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 5%, var(--surface));
  }

  .toast.success {
    border-left-color: var(--ok);
  }

  .toast.warning {
    border-left-color: var(--warn);
  }

  .content {
    min-width: 0;
  }

  strong {
    display: block;
    font-size: 0.88rem;
  }

  p {
    margin: 0.25rem 0 0;
    color: var(--muted);
    font-size: 0.78rem;
    overflow-wrap: anywhere;
  }

  .close {
    flex: 0 0 auto;
    width: 32px;
    min-width: 32px;
    height: 32px;
    min-height: 32px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font-size: 1.2rem;
    cursor: pointer;
  }

  .close:hover {
    background: var(--surface-2);
  }

  @media (min-width: 900px) {
    .toasts {
      bottom: 1rem;
    }
  }
</style>

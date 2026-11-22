<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    variant = "ghost",
    size = "normal",
    loading = false,
    disabled = false,
    type = "button",
    ariaLabel = undefined,
    title = undefined,
    onclick,
    children,
  }: {
    variant?: "primary" | "ghost" | "danger" | "subtle";
    size?: "normal" | "small" | "icon";
    loading?: boolean;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    ariaLabel?: string;
    title?: string;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
  } = $props();
</script>

<button
  {type}
  class="ui-button {variant} {size}"
  {disabled}
  aria-busy={loading}
  aria-label={ariaLabel}
  {title}
  {onclick}
>
  {#if loading}
    <span class="spinner" aria-hidden="true"></span>
  {/if}
  {@render children?.()}
</button>

<style>
  .ui-button {
    min-height: 40px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    padding: 0.62rem 1.05rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    box-shadow: var(--shadow-xs);
    transition:
      background-color 140ms ease,
      border-color 140ms ease,
      transform 140ms ease,
      box-shadow 140ms ease;
  }

  .ui-button:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    box-shadow:
      var(--shadow-sm),
      0 0 0 3px color-mix(in srgb, var(--gold) 10%, transparent);
    transform: translateY(-1px);
  }

  .ui-button:active:not(:disabled) {
    transform: translateY(0);
    box-shadow: none;
  }

  .ui-button:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 78%, transparent);
    outline-offset: 2px;
  }

  .ui-button:disabled {
    opacity: 0.52;
    cursor: not-allowed;
    transform: none;
  }

  .ui-button.primary:disabled {
    border-color: var(--border);
    background: var(--surface-2);
    color: var(--muted);
    opacity: 0.78;
    box-shadow: none;
  }

  .primary {
    background: linear-gradient(100deg, var(--accent), var(--accent-2));
    border-color: color-mix(in srgb, var(--accent-2) 76%, #3b1a00);
    color: var(--accent-contrast);
    font-weight: 600;
    box-shadow:
      0 8px 18px -10px color-mix(in srgb, var(--accent-2) 48%, transparent),
      inset 0 1px 0 color-mix(in srgb, #fff 20%, transparent);
  }

  .primary:hover:not(:disabled) {
    background: linear-gradient(100deg, var(--accent-2), var(--accent-3));
    border-color: var(--accent-3);
  }

  .ghost {
    background: var(--surface-3);
    border-color: transparent;
    box-shadow: none;
  }

  .ghost:hover:not(:disabled) {
    background: var(--surface-4);
    border-color: transparent;
  }

  .subtle {
    background: transparent;
    border-color: transparent;
    box-shadow: none;
  }

  .danger {
    background: color-mix(in srgb, var(--danger, #d9534f) 12%, var(--surface));
    border-color: color-mix(in srgb, var(--danger, #d9534f) 45%, var(--border));
    color: color-mix(in srgb, var(--danger, #d9534f) 84%, var(--text));
    box-shadow: none;
  }

  .small {
    min-height: 36px;
    padding: 0.34rem 0.7rem;
    border-radius: var(--radius-sm);
    font-size: 0.78rem;
  }

  .icon {
    width: 40px;
    min-width: 40px;
    height: 40px;
    padding: 0;
  }

  @media (pointer: coarse) {
    .small {
      min-height: 44px;
    }

    .icon {
      width: 44px;
      min-width: 44px;
      height: 44px;
    }
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  :global([data-reduced-motion="true"]) .ui-button,
  :global([data-reduced-motion="true"]) .spinner {
    transition: none;
    animation: none;
  }
</style>

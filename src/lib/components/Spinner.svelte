<script lang="ts">
  let { label = "Cargando…", size = 22 }: { label?: string; size?: number } =
    $props();
</script>

<span class="spinner-wrap" role="status" aria-live="polite">
  <span class="spinner" style:width={`${size}px`} style:height={`${size}px`}></span>
  <span>{label}</span>
</span>

<style>
  .spinner-wrap {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .spinner {
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

  :global([data-reduced-motion="true"]) .spinner {
    animation: none;
  }
</style>

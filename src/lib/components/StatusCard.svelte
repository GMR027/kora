<script lang="ts">
  interface Props {
    title: string;
    value: string;
    hint?: string;
    tone?: "neutral" | "ok" | "warn";
  }

  let { title, value, hint, tone = "neutral" }: Props = $props();
</script>

<section class="card" data-tone={tone}>
  <p class="card-title">{title}</p>
  <p class="card-value">{value}</p>
  {#if hint}
    <p class="card-hint">{hint}</p>
  {/if}
</section>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 1rem 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  .card-title {
    margin: 0;
    font-size: 0.78rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .card-value {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .card-hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  [data-tone="ok"] .card-value {
    color: var(--ok);
  }

  [data-tone="warn"] .card-value {
    color: var(--warn);
  }
</style>

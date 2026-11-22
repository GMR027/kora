<script lang="ts">
  import Icon from "./Icon.svelte";
  import Button from "./Button.svelte";

  let {
    title,
    description,
    action,
    onAction,
    icon = "book",
  }: {
    title: string;
    description: string;
    action?: string;
    onAction?: () => void;
    icon?: "library" | "book" | "headphones" | "diary" | "voice" | "sync" | "search" | "sliders" | "waveform" | "chart";
  } = $props();
</script>

<div class="empty-state">
  <span class="icon"><Icon name={icon} size={28} /></span>
  <strong>{title}</strong>
  <p>{description}</p>
  {#if action && onAction}
    <Button variant="primary" onclick={onAction}>{action}</Button>
  {/if}
</div>

<style>
  .empty-state {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.55rem;
    overflow: hidden;
    text-align: center;
    padding: clamp(2.2rem, 7vw, 3.6rem) 1.25rem;
    border: 1px dashed color-mix(in srgb, var(--border) 84%, var(--accent) 16%);
    border-radius: var(--radius-lg);
    background:
      radial-gradient(
        420px 220px at 50% 0%,
        color-mix(in srgb, var(--accent) 8%, transparent),
        transparent 70%
      ),
      color-mix(in srgb, var(--surface) 72%, transparent);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    margin-bottom: 0.35rem;
    border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--border));
    border-radius: 20px;
    background: color-mix(in srgb, var(--accent) 12%, var(--surface));
    color: var(--accent);
    box-shadow: var(--shadow-sm);
  }

  strong {
    font-family: var(--font-display);
    font-size: 1.22rem;
    font-weight: 650;
    letter-spacing: -0.015em;
  }

  p {
    max-width: 44ch;
    margin: 0 0 0.65rem;
    color: var(--muted);
    font-size: 0.87rem;
    line-height: 1.55;
  }
</style>

<script lang="ts">
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n";
  import type { CategorySuggestion, SuggestionSource } from "../types";

  let {
    suggestions,
    busyId = null,
    onaccept,
    ondismiss,
  }: {
    suggestions: CategorySuggestion[];
    busyId?: string | null;
    onaccept: (suggestion: CategorySuggestion) => void;
    ondismiss: (suggestion: CategorySuggestion) => void;
  } = $props();

  function sourceLabel(source: SuggestionSource): string {
    if (source === "embedding") return $t("classification.sourceEmbedding");
    if (source === "ai") return $t("classification.sourceAi");
    return $t("classification.sourceMetadata");
  }
</script>

<ul class="suggestions">
  {#each suggestions as suggestion (suggestion.id)}
    <li class="suggestion">
      <span
        class="dot"
        style:background={suggestion.categoryColor ?? "var(--accent)"}
        aria-hidden="true"
      ></span>
      <div class="body">
        <div class="line">
          {#if suggestion.categoryName}
            <strong>{suggestion.categoryName}</strong>
          {:else}
            <strong class="new">
              <Icon name="plus" size={13} />
              {$t("classification.newCategory")}: {suggestion.proposedName}
            </strong>
          {/if}
          <span class="score">{Math.round(suggestion.score * 100)}%</span>
        </div>
        {#if suggestion.reason}<p>{suggestion.reason}</p>{/if}
        <span class="source">
          {#if suggestion.audioOnly}
            <Icon name="headphones" size={11} /> {$t("classification.audiobook")} ·
          {/if}
          {sourceLabel(suggestion.source)}
        </span>
      </div>
      <div class="actions">
        <Button
          size="small"
          variant="primary"
          loading={busyId === suggestion.id}
          onclick={() => onaccept(suggestion)}
        >
          {$t("classification.accept")}
        </Button>
        <Button
          size="small"
          variant="ghost"
          disabled={busyId === suggestion.id}
          onclick={() => ondismiss(suggestion)}
        >
          {$t("classification.dismiss")}
        </Button>
      </div>
    </li>
  {/each}
</ul>

<style>
  .suggestions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.55rem;
  }

  .suggestion {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 0.65rem;
    align-items: start;
    padding: 0.7rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--surface) 94%, var(--gold) 6%);
  }

  .dot {
    width: 0.65rem;
    height: 0.65rem;
    border-radius: 999px;
    margin-top: 0.35rem;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--gold) 14%, transparent);
  }

  .body {
    min-width: 0;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .line strong {
    font-size: 0.86rem;
    overflow-wrap: anywhere;
  }

  .line strong.new {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--accent);
  }

  .score {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.05rem 0.4rem;
  }

  .body p {
    margin: 0.25rem 0 0;
    font-size: 0.78rem;
    color: var(--muted);
    line-height: 1.35;
  }

  .source {
    display: inline-block;
    margin-top: 0.3rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: color-mix(in srgb, var(--muted) 78%, transparent);
  }

  .source :global(svg) {
    vertical-align: -1px;
  }

  .actions {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  @media (max-width: 720px) {
    .suggestion {
      grid-template-columns: auto minmax(0, 1fr);
    }

    .actions {
      grid-column: 1 / -1;
      justify-content: flex-start;
    }
  }
</style>

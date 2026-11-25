<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { ReactionKind } from "../types";

  let {
    text,
    subtitle,
    palette,
    note = $bindable(""),
    color = $bindable(palette[0]),
    reactions = [],
    onClose,
    onHighlight,
    onUnderline,
    onSaveNote,
    onCopy,
    onExtract,
    onVoice,
    onTerm,
    onReaction,
  }: {
    text: string;
    subtitle: string;
    palette: string[];
    note?: string;
    color?: string;
    reactions?: { kind: ReactionKind; emoji: string; label: string }[];
    onClose: () => void;
    onHighlight: () => void;
    onUnderline: () => void;
    onSaveNote: () => void;
    onCopy?: () => void;
    onExtract: () => void;
    onVoice: () => void;
    onTerm?: () => void;
    onReaction?: (kind: ReactionKind) => void;
  } = $props();
</script>

<button class="selection-backdrop" aria-label="Cerrar" onclick={onClose}></button>

<div class="selection-layer">
  <div class="selection-modal" role="dialog" aria-modal="true" aria-label="Nota marginal">
  <header class="selection-head">
    <div class="selection-heading">
      <span class="selection-mark"><Icon name="edit" size={18} /></span>
      <div>
        <h2>Nota marginal</h2>
        <p>{subtitle}</p>
      </div>
    </div>
    <button class="selection-close" aria-label="Cerrar" onclick={onClose}>
      <Icon name="close" size={17} />
    </button>
  </header>

  <div class="selection-body">
    <div class="selection-quote">
      <div class="selection-quote-head">
        <span>Texto seleccionado</span>
        <button onclick={onClose}>Cambiar selección</button>
      </div>
      <blockquote>“{text}”</blockquote>
    </div>

    <div class="selection-colors-block">
      <div class="selection-label">
        <span>Tonalidad del resaltador</span>
        <strong>{color}</strong>
      </div>
      <div class="selection-colors" aria-label="Colores del resaltador">
        {#each palette as swatch (swatch)}
          <button
            class="selection-swatch"
            class:active={color === swatch}
            style:background={swatch}
            aria-label={`Usar color ${swatch}`}
            aria-pressed={color === swatch}
            onclick={() => (color = swatch)}
          >
            {#if color === swatch}<Icon name="check" size={13} />{/if}
          </button>
        {/each}
      </div>
    </div>

    <label class="selection-note">
      <span>Nota de lectura</span>
      <textarea
        bind:value={note}
        rows="3"
        maxlength="1200"
        placeholder="Añade una interpretación, una duda o una idea…"
      ></textarea>
    </label>

    <div class="selection-tools" aria-label="Acciones de la selección">
      {#if onTerm}
        <button onclick={onTerm}>
          <Icon name="search" size={14} /> Consultar
        </button>
      {/if}
      {#if onCopy}
        <button onclick={onCopy}>
          <Icon name="book" size={14} /> Copiar cita
        </button>
      {/if}
      <button onclick={onUnderline}>
        <Icon name="edit" size={14} /> Subrayar
      </button>
      <button onclick={onExtract}>
        <Icon name="bookmark" size={14} /> Cita
      </button>
      <button onclick={onVoice}>
        <Icon name="voice" size={14} /> Audio-nota
      </button>
      {#each reactions as reaction (reaction.kind)}
        <button
          title={reaction.label}
          aria-label={`Reaccionar con ${reaction.label}`}
          onclick={() => onReaction?.(reaction.kind)}
        >
          <span aria-hidden="true">{reaction.emoji}</span> {reaction.label}
        </button>
      {/each}
    </div>
  </div>

  <footer class="selection-foot">
    <button class="ghost" onclick={onClose}>Descartar</button>
    <button class="primary" onclick={onSaveNote} disabled={!note.trim()}>
      Guardar nota
    </button>
    <button class="primary" onclick={onHighlight}>Resaltar</button>
  </footer>
  </div>
</div>

<style>
  .selection-backdrop {
    position: fixed;
    inset: 0;
    z-index: 85;
    padding: 0;
    border: 0;
    background: color-mix(in srgb, #1a1008 42%, transparent);
    backdrop-filter: blur(3px);
  }

  .selection-layer {
    position: fixed;
    inset: 0;
    z-index: 86;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    pointer-events: none;
  }

  .selection-modal {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    width: min(34rem, 100%);
    max-height: 88vh;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border-warm) 48%, var(--border));
    border-radius: 22px;
    background: var(--surface);
    box-shadow: 0 30px 80px -28px color-mix(in srgb, #000 46%, transparent);
    pointer-events: auto;
  }

  .selection-head {
    position: relative;
    z-index: 3;
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 14px 16px 11px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: var(--surface);
  }

  .selection-heading {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .selection-mark {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 38px;
    height: 38px;
    border: 1px solid var(--accent-line);
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .selection-heading h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .selection-heading p {
    margin: 2px 0 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.64rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .selection-close {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
  }

  .selection-close:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .selection-body {
    position: relative;
    z-index: 1;
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 16px;
  }

  .selection-quote {
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border));
    border-left: 3px solid var(--accent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 5%, var(--surface));
  }

  .selection-quote-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 4px;
  }

  .selection-quote-head span,
  .selection-label span,
  .selection-note > span {
    color: var(--accent);
    font-size: 0.56rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .selection-quote-head button {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 0.62rem;
    font-weight: 600;
  }

  .selection-quote blockquote {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    margin: 0;
    overflow: hidden;
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.88rem;
    font-style: italic;
    line-height: 1.55;
  }

  .selection-colors-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .selection-label {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .selection-label strong {
    color: var(--muted);
    font-size: 0.6rem;
    font-weight: 500;
  }

  .selection-colors {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .selection-swatch {
    display: grid;
    flex: none;
    place-items: center;
    width: 32px;
    height: 32px;
    min-height: 0;
    padding: 0;
    border: 2px solid var(--surface);
    border-radius: 50%;
    color: #3a2600;
    box-shadow: 0 0 0 1px var(--border);
  }

  .selection-swatch.active {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .selection-note {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .selection-note > span {
    color: var(--muted);
  }

  .selection-note textarea {
    min-height: 5.5rem;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-reading);
    font-size: 0.85rem;
    line-height: 1.55;
    resize: vertical;
  }

  .selection-note textarea:focus {
    outline: 2px solid color-mix(in srgb, var(--accent) 22%, transparent);
    border-color: var(--accent-line);
  }

  .selection-tools {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .selection-tools button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 29px;
    padding: 0.24rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
    font: inherit;
    font-size: 0.64rem;
    font-weight: 600;
  }

  .selection-tools button:hover {
    border-color: var(--accent-line);
    color: var(--accent);
  }

  .selection-foot {
    position: relative;
    z-index: 3;
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 7px;
    padding: 10px 16px calc(10px + var(--safe-bottom));
    border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: color-mix(in srgb, var(--surface-2) 70%, var(--surface));
  }

  .selection-foot button {
    min-height: 38px;
    padding: 0.4rem 0.8rem;
    border-radius: 11px;
    font: inherit;
    font-size: 0.72rem;
    font-weight: 650;
  }

  .selection-foot .ghost {
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
  }

  .selection-foot .primary {
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .selection-foot .primary:disabled {
    opacity: 0.45;
  }

  @media (max-width: 720px) {
    .selection-layer {
      align-items: flex-end;
      padding: 0;
    }

    .selection-modal {
      width: 100%;
      max-height: 92vh;
      border-bottom: 0;
      border-radius: 22px 22px 0 0;
    }

    .selection-modal::before {
      content: "";
      flex: 0 0 auto;
      width: 38px;
      height: 4px;
      margin: 7px auto 0;
      border-radius: 999px;
      background: var(--border-strong);
    }

    .selection-head {
      padding-top: 12px;
    }

    .selection-body {
      gap: 10px;
      padding: 10px 14px;
    }

    .selection-foot {
      justify-content: stretch;
      padding: 9px 14px calc(9px + var(--safe-bottom));
    }

    .selection-foot button {
      flex: 1;
      min-width: 0;
    }

    .selection-foot .primary:last-child {
      flex: 0 0 auto;
    }
  }
</style>

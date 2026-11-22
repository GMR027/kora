<script lang="ts">
  import Quill from "quill";
  import "quill/dist/quill.snow.css";
  import { onDestroy, onMount } from "svelte";

  interface Props {
    initialJson?: string | null;
    initialTitle?: string | null;
    readOnly?: boolean;
    onSave?: (payload: {
      title: string | null;
      contentJson: string;
      plainText: string;
    }) => void;
    onCancel?: () => void;
  }

  let {
    initialJson = null,
    initialTitle = null,
    readOnly = false,
    onSave,
    onCancel,
  }: Props = $props();

  let container = $state<HTMLDivElement | null>(null);
  let title = $state("");
  let editor: Quill | null = null;

  $effect(() => {
    title = initialTitle ?? "";
  });

  onMount(() => {
    if (!container) return;
    editor = new Quill(container, {
      theme: "snow",
      readOnly,
      placeholder: readOnly ? "" : "Escribe una nota…",
      modules: {
        toolbar: readOnly
          ? false
          : [
              [{ header: [1, 2, 3, false] }],
              ["bold", "italic", "underline", "strike"],
              [{ list: "ordered" }, { list: "bullet" }],
              ["blockquote", "code-block"],
              ["link"],
              ["clean"],
            ],
      },
    });

    if (initialJson) {
      try {
        editor.setContents(JSON.parse(initialJson));
      } catch {
        editor.setText(initialJson);
      }
    }
  });

  onDestroy(() => {
    editor = null;
  });

  function save() {
    if (!editor) return;
    const contentJson = JSON.stringify(editor.getContents());
    const plainText = editor.getText().trim();
    onSave?.({
      title: title.trim() || null,
      contentJson,
      plainText,
    });
  }
</script>

<div class="rich-note" class:readonly={readOnly}>
  {#if !readOnly}
    <label>
      <span>Título (opcional)</span>
      <input bind:value={title} maxlength="120" placeholder="Idea, cita, reflexión…" />
    </label>
  {/if}

  <div class="editor-host" bind:this={container}></div>

  {#if !readOnly}
    <div class="actions">
      <button class="primary small" onclick={save}>Guardar nota</button>
      <button class="ghost small" onclick={() => onCancel?.()}>Cancelar</button>
    </div>
  {/if}
</div>

<style>
  .rich-note {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  input {
    font: inherit;
    font-size: 0.85rem;
    padding: 0.45rem 0.55rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .editor-host {
    min-height: 13rem;
    max-height: 50vh;
    overflow-y: auto;
    border-radius: 10px;
    background: var(--surface);
    color: var(--text);
  }

  .editor-host :global(.ql-toolbar) {
    border-color: var(--border);
    border-radius: 10px 10px 0 0;
    background: var(--surface-2);
  }

  .editor-host :global(.ql-container) {
    border-color: var(--border);
    border-radius: 0 0 10px 10px;
    font: inherit;
    font-size: 0.9rem;
  }

  .editor-host :global(.ql-editor) {
    min-height: 11rem;
  }

  .editor-host :global(.ql-editor.ql-blank::before) {
    color: var(--muted);
    font-style: normal;
  }

  .editor-host :global(.ql-snow .ql-stroke) {
    stroke: var(--text);
  }

  .editor-host :global(.ql-snow .ql-fill) {
    fill: var(--text);
  }

  .editor-host :global(.ql-snow .ql-picker) {
    color: var(--text);
  }

  .readonly .editor-host {
    border: 1px solid var(--border);
  }

  .actions {
    display: flex;
    gap: 0.45rem;
  }

  button {
    font: inherit;
    font-size: 0.8rem;
    padding: 0.42rem 0.65rem;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }

  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-contrast);
    font-weight: 600;
  }

  .rich-note {
    gap: 9px;
  }

  label span {
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  input {
    min-height: 38px;
    border-radius: 11px;
    background: var(--surface-2);
    font-size: 0.82rem;
  }

  input:focus {
    outline: 2px solid color-mix(in srgb, var(--accent) 24%, transparent);
    border-color: var(--accent-line);
  }

  .editor-host {
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
  }

  .editor-host :global(.ql-toolbar) {
    border: 0;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: color-mix(in srgb, var(--surface-2) 78%, var(--surface));
  }

  .editor-host :global(.ql-container) {
    border: 0;
    border-radius: 0;
    font-size: 0.86rem;
  }

  .editor-host :global(.ql-editor) {
    min-height: 12rem;
    padding: 12px 14px;
    font-family: var(--font-reading);
    font-size: 0.9rem;
    line-height: 1.55;
  }

  .actions {
    gap: 8px;
    padding-top: 2px;
  }

  .actions .primary {
    flex: 1;
    justify-content: center;
    min-height: 40px;
    border-radius: 12px;
    font-size: 0.76rem;
  }

  .actions .ghost {
    min-height: 40px;
    border-radius: 12px;
    font-size: 0.76rem;
  }

  @media (max-width: 720px) {
    .editor-host :global(.ql-editor) {
      min-height: 9rem;
    }
  }
</style>

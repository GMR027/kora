<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    categoryCreate,
    categoryList,
    errorMessage,
    libraryAssignCategory,
    libraryDelete,
    libraryImport,
    libraryList,
    libraryReplaceCover,
  } from "../lib/api";
  import type { BookSummary, Category } from "../lib/types";

  let books = $state<BookSummary[]>([]);
  let categories = $state<Category[]>([]);
  let loading = $state(true);
  let notice = $state<string | null>(null);
  let busy = $state<string | null>(null);

  let importPath = $state("");
  let categoryName = $state("");
  let dropActive = $state(false);
  let deleting = $state<string | null>(null);
  let coverEditor = $state<string | null>(null);
  let coverPath = $state("");

  async function load() {
    loading = true;
    notice = null;
    try {
      [books, categories] = await Promise.all([libraryList(), categoryList()]);
    } catch (error) {
      notice = errorMessage(error);
    } finally {
      loading = false;
    }
  }

  async function importBook(source: string) {
    const path = source.trim();
    if (!path) {
      notice = "Escribe la ruta de un archivo EPUB o PDF.";
      return;
    }

    notice = null;
    busy = "import";
    try {
      const book = await libraryImport(path);
      books = [book, ...books.filter((item) => item.id !== book.id)];
      importPath = "";
    } catch (error) {
      notice = errorMessage(error);
    } finally {
      busy = null;
    }
  }

  async function removeBook(book: BookSummary) {
    notice = null;
    busy = book.id;
    try {
      await libraryDelete(book.id);
      books = books.filter((item) => item.id !== book.id);
      deleting = null;
    } catch (error) {
      notice = errorMessage(error);
    } finally {
      busy = null;
    }
  }

  async function replaceCover(book: BookSummary) {
    const path = coverPath.trim();
    if (!path) {
      notice = "Escribe la ruta de una imagen JPG, PNG o WebP.";
      return;
    }

    notice = null;
    busy = book.id;
    try {
      const updated = await libraryReplaceCover(book.id, path);
      books = books.map((item) => (item.id === updated.id ? updated : item));
      coverEditor = null;
      coverPath = "";
    } catch (error) {
      notice = errorMessage(error);
    } finally {
      busy = null;
    }
  }

  async function addCategory() {
    const name = categoryName.trim();
    if (!name) {
      notice = "El nombre de la categoría no puede estar vacío.";
      return;
    }

    notice = null;
    busy = "category";
    try {
      const category = await categoryCreate(name);
      if (!categories.some((item) => item.id === category.id)) {
        categories = [...categories, category];
      }
      categoryName = "";
    } catch (error) {
      notice = errorMessage(error);
    } finally {
      busy = null;
    }
  }

  async function assignCategory(bookId: string, categoryId: string) {
    if (!categoryId) return;

    notice = null;
    try {
      await libraryAssignCategory(bookId, categoryId);
      const category = categories.find((item) => item.id === categoryId);
      notice = `«${category?.name ?? "Categoría"}» asignada.`;
    } catch (error) {
      notice = errorMessage(error);
    }
  }

  function coverSrc(book: BookSummary): string | null {
    return book.coverPath ? convertFileSrc(book.coverPath) : null;
  }

  function formatSize(bytes: number): string {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${bytes} B`;
  }

  onMount(() => {
    void load();

    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          dropActive = true;
        } else if (payload.type === "leave") {
          dropActive = false;
        } else if (payload.type === "drop") {
          dropActive = false;
          const path = payload.paths.find((candidate) => /\.(epub|pdf)$/i.test(candidate));
          if (path) {
            void importBook(path);
          } else {
            notice = "Suelta un archivo .epub o .pdf.";
          }
        }
      })
      .then((stop) => {
        unlisten = stop;
      })
      .catch((error) => {
        notice = errorMessage(error);
      });

    return () => unlisten?.();
  });
</script>

<section class="library" class:drop-active={dropActive}>
  <header class="toolbar">
    <div>
      <h1>Biblioteca</h1>
      <p class="subtitle">Tus libros EPUB y PDF viven solo en este dispositivo.</p>
    </div>
    <div class="actions">
      <button class="ghost" onclick={load} disabled={loading}>Recargar</button>
    </div>
  </header>

  <div class="import">
    <div class="import-row">
      <input
        type="text"
        bind:value={importPath}
        placeholder="/ruta/al/libro.epub"
        aria-label="Ruta del archivo a importar"
        onkeydown={(event) => {
          if (event.key === "Enter") void importBook(importPath);
        }}
      />
      <button class="primary" onclick={() => importBook(importPath)} disabled={busy === "import"}>
        {busy === "import" ? "Importando…" : "Importar"}
      </button>
    </div>
    <p class="hint">
      Arrastra un EPUB o PDF a esta ventana, o escribe su ruta. El archivo se copia a
      <code>assets/books/</code> con su hash SHA-256.
    </p>
  </div>

  <div class="categories">
    <div class="import-row">
      <input
        type="text"
        bind:value={categoryName}
        placeholder="Nueva categoría"
        aria-label="Nombre de la nueva categoría"
        onkeydown={(event) => {
          if (event.key === "Enter") void addCategory();
        }}
      />
      <button class="ghost" onclick={addCategory} disabled={busy === "category"}>Añadir</button>
    </div>
    {#if categories.length > 0}
      <ul class="chips">
        {#each categories as category (category.id)}
          <li class="chip" style:border-color={category.color ?? undefined}>
            {#if category.color}
              <span class="dot" style:background={category.color}></span>
            {/if}
            {category.name}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="hint">Aún no hay categorías. Crea una para clasificar tus libros.</p>
    {/if}
  </div>

  {#if notice}
    <p class="notice" role="status">{notice}</p>
  {/if}

  {#if loading}
    <p class="empty">Cargando biblioteca…</p>
  {:else if books.length === 0}
    <div class="empty-state">
      <p class="empty-title">Todavía no hay libros</p>
      <p class="empty">Arrastra un EPUB o PDF, o pega su ruta arriba para empezar.</p>
    </div>
  {:else}
    <ul class="grid">
      {#each books as book (book.id)}
        <li class="book">
          {#if coverSrc(book)}
            <img class="cover" src={coverSrc(book)} alt={`Portada de ${book.title}`} />
          {:else}
            <div class="cover placeholder" aria-hidden="true">{book.format.toUpperCase()}</div>
          {/if}

          <div class="book-meta">
            <p class="book-title">{book.title}</p>
            <p class="book-author">{book.author ?? "Autor desconocido"}</p>
            <p class="book-details">
              {book.format.toUpperCase()} · {formatSize(book.sizeBytes)} ·
              {Math.round(book.progress * 100)}% leído
            </p>

            <div class="book-actions">
              <button
                class="ghost small"
                onclick={() => {
                  coverEditor = coverEditor === book.id ? null : book.id;
                  coverPath = "";
                }}
                disabled={busy === book.id}
              >
                Portada
              </button>
              {#if deleting === book.id}
                <button
                  class="danger small"
                  onclick={() => removeBook(book)}
                  disabled={busy === book.id}
                >
                  Confirmar
                </button>
                <button class="ghost small" onclick={() => (deleting = null)}>Cancelar</button>
              {:else}
                <button
                  class="ghost small"
                  onclick={() => (deleting = book.id)}
                  disabled={busy === book.id}
                >
                  Eliminar
                </button>
              {/if}
            </div>

            {#if coverEditor === book.id}
              <div class="inline-form">
                <input
                  type="text"
                  bind:value={coverPath}
                  placeholder="/ruta/a/portada.jpg"
                  aria-label={`Ruta de la portada de ${book.title}`}
                  onkeydown={(event) => {
                    if (event.key === "Enter") void replaceCover(book);
                  }}
                />
                <button class="primary small" onclick={() => replaceCover(book)} disabled={busy === book.id}>
                  Guardar
                </button>
              </div>
            {/if}

            {#if categories.length > 0}
              <select
                class="category-select"
                aria-label={`Asignar categoría a ${book.title}`}
                onchange={(event) => {
                  const select = event.currentTarget as HTMLSelectElement;
                  void assignCategory(book.id, select.value);
                  select.value = "";
                }}
              >
                <option value="">Asignar categoría…</option>
                {#each categories as category (category.id)}
                  <option value={category.id}>{category.name}</option>
                {/each}
              </select>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if dropActive}
    <div class="drop-overlay" aria-hidden="true">
      <p>Suelta el archivo para importarlo</p>
    </div>
  {/if}
</section>

<style>
  .library {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: flex-end;
    justify-content: space-between;
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
  }

  .subtitle {
    margin: 0.25rem 0 0;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
  }

  button {
    border-radius: 10px;
    border: 1px solid var(--border);
    padding: 0.55rem 0.95rem;
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
    background: var(--surface);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.55;
    cursor: default;
  }

  button.small {
    padding: 0.35rem 0.65rem;
    font-size: 0.8rem;
  }

  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-contrast);
    font-weight: 600;
  }

  .danger {
    border-color: color-mix(in srgb, var(--warn) 60%, var(--border));
    color: var(--warn);
  }

  input,
  select {
    font: inherit;
    font-size: 0.9rem;
    padding: 0.5rem 0.65rem;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    min-width: 0;
  }

  input {
    flex: 1;
  }

  .import,
  .categories {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.9rem;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
  }

  .import-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
  }

  .chips {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin: 0;
    padding: 0;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.3rem 0.6rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-size: 0.8rem;
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }

  .notice {
    margin: 0;
    padding: 0.7rem 0.9rem;
    border-radius: 10px;
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, transparent);
    color: var(--text);
    font-size: 0.85rem;
  }

  .empty-state {
    border: 1px dashed var(--border);
    border-radius: 16px;
    padding: 3rem 1.5rem;
    text-align: center;
  }

  .empty-title {
    margin: 0 0 0.5rem;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
  }

  code {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.82em;
    background: var(--surface-2);
    padding: 0.1rem 0.35rem;
    border-radius: 5px;
  }

  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 1rem;
  }

  .book {
    display: flex;
    gap: 0.85rem;
    padding: 0.85rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
  }

  .cover {
    width: 54px;
    height: 76px;
    flex: 0 0 auto;
    border-radius: 8px;
    object-fit: cover;
  }

  .cover.placeholder {
    background: linear-gradient(160deg, var(--accent), var(--accent-2));
    color: var(--accent-contrast);
    display: grid;
    place-items: center;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.06em;
  }

  .book-meta {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .book-title {
    margin: 0;
    font-weight: 600;
    font-size: 0.95rem;
    overflow-wrap: anywhere;
  }

  .book-author,
  .book-details {
    margin: 0;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .book-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-top: auto;
  }

  .inline-form {
    display: flex;
    gap: 0.35rem;
  }

  .category-select {
    width: 100%;
    font-size: 0.8rem;
    padding: 0.35rem 0.5rem;
  }

  .drop-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border: 2px dashed var(--accent);
    border-radius: 18px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text);
    font-weight: 600;
    pointer-events: none;
    z-index: 10;
  }
</style>

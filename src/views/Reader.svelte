<script lang="ts">
  // Placeholder de la fase de lector. El renderizado EPUB/PDF y la selección
  // de texto se conectarán a reader_open y a las traits de src/reader.
  const sample = [
    "Kora es un lector local-first: el texto, las anotaciones y los perfiles de voz viven en este dispositivo.",
    "En la siguiente fase este panel mostrará el EPUB o PDF real, con paginación, índice y resaltado.",
    "La síntesis de voz leerá el párrafo activo y la sincronización LAN compartirá el progreso entre tus equipos.",
  ];
</script>

<section class="reader">
  <header class="toolbar">
    <div>
      <h1>Lector</h1>
      <p class="subtitle">Vista previa del esqueleto, sin documento cargado.</p>
    </div>
    <div class="controls">
      <button disabled>Anterior</button>
      <button disabled>Siguiente</button>
      <button disabled>Leer en voz alta</button>
    </div>
  </header>

  <article class="page">
    <p class="chapter">Capítulo de muestra</p>
    {#each sample as paragraph}
      <p>{paragraph}</p>
    {/each}
  </article>
</section>

<style>
  .reader {
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

  .controls {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  button {
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    padding: 0.55rem 0.9rem;
    font: inherit;
    font-size: 0.88rem;
  }

  button:disabled {
    opacity: 0.5;
  }

  .page {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: clamp(1.5rem, 4vw, 3rem);
    max-width: 68ch;
    margin: 0 auto;
    line-height: 1.75;
    font-size: 1.02rem;
  }

  .chapter {
    margin: 0 0 1.5rem;
    font-size: 0.78rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .page p {
    margin: 0 0 1.1rem;
  }
</style>

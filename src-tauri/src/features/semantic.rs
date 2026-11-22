use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::error::{KoraError, Result};
use crate::reader::{PdfParser, ReaderService, SourceText, extract_text};
use crate::storage::hash::sha256_bytes;
use crate::storage::{
    AssetKind, AssetRef, Book, BookFormat, NewSemanticChunk, SemanticBookIndex,
    SemanticIndexStatus, Storage,
};

use super::embeddings::{EMBEDDING_DIMENSIONS, EMBEDDING_MODEL_ID, EmbeddingProvider};

const TARGET_WORDS: usize = 500;
const OVERLAP_WORDS: usize = 50;
const MAX_SNIPPET_CHARS: usize = 280;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticStatus {
    pub supported: bool,
    pub available: bool,
    pub model: String,
    pub dimensions: usize,
    pub indexed_books: i64,
    pub total_books: i64,
    pub chunks: i64,
    pub indexing: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticProgress {
    pub book_id: String,
    pub title: String,
    pub indexed: usize,
    pub total: usize,
    pub percent: f64,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticHit {
    pub book_id: String,
    pub book_title: String,
    pub author: Option<String>,
    pub section: i64,
    pub section_title: Option<String>,
    pub offset: i64,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct TextChunk {
    pub section: usize,
    pub chunk_index: usize,
    pub section_title: Option<String>,
    pub content: String,
    pub offset_start: usize,
    pub offset_end: usize,
}

#[derive(Debug)]
struct ExtractedSection {
    section: usize,
    title: Option<String>,
    text: SourceText,
}

pub struct SemanticService {
    storage: Storage,
    owner_account_id: String,
    provider: Arc<dyn EmbeddingProvider>,
    indexing: AtomicBool,
}

struct IndexGuard<'a>(&'a AtomicBool);

impl Drop for IndexGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl SemanticService {
    pub fn new(
        storage: Storage,
        owner_account_id: String,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            storage,
            owner_account_id,
            provider,
            indexing: AtomicBool::new(false),
        }
    }

    pub fn provider(&self) -> &Arc<dyn EmbeddingProvider> {
        &self.provider
    }

    pub async fn status(&self) -> Result<SemanticStatus> {
        let states = self
            .storage
            .semantic
            .list_states(&self.owner_account_id)
            .await?;
        let indexed_books = states
            .iter()
            .filter(|state| {
                state.status == SemanticIndexStatus::Done && state.model == EMBEDDING_MODEL_ID
            })
            .count() as i64;
        let last_error = states
            .iter()
            .find(|state| {
                state.model == EMBEDDING_MODEL_ID && state.status == SemanticIndexStatus::Failed
            })
            .and_then(|state| state.error.clone());

        Ok(SemanticStatus {
            supported: !cfg!(target_os = "android"),
            available: self.provider.ready(),
            model: EMBEDDING_MODEL_ID.to_string(),
            dimensions: EMBEDDING_DIMENSIONS,
            indexed_books,
            total_books: self
                .storage
                .books
                .count_by_owner(&self.owner_account_id)
                .await?,
            chunks: self
                .storage
                .semantic
                .count_chunks(&self.owner_account_id, EMBEDDING_MODEL_ID)
                .await?,
            indexing: self.indexing.load(Ordering::Acquire),
            last_error,
        })
    }

    pub async fn prepare(&self) -> Result<()> {
        self.provider.prepare().await
    }

    pub async fn reset_interrupted(&self) -> Result<()> {
        self.storage
            .semantic
            .reset_interrupted(&self.owner_account_id)
            .await
    }

    /// Indexa los libros pendientes o con un modelo distinto. Si `force` es
    /// true, vuelve a indexar también los que ya estaban listos.
    pub async fn reconcile(&self, app: Option<&AppHandle>, force: bool) -> Result<()> {
        let _guard =
            match self
                .indexing
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => IndexGuard(&self.indexing),
                Err(_) => return Ok(()),
            };

        if !self.provider.ready() {
            return Ok(());
        }

        self.reset_interrupted().await?;

        let books = self
            .storage
            .books
            .list_by_owner(&self.owner_account_id)
            .await?;

        for book in books {
            let state = self.storage.semantic.get_state(&book.id).await?;
            let should_index = force
                || match &state {
                    None => true,
                    Some(state) => {
                        state.model != EMBEDDING_MODEL_ID
                            || matches!(
                                state.status,
                                SemanticIndexStatus::Pending | SemanticIndexStatus::Running
                            )
                    }
                };

            if !should_index {
                continue;
            }

            if let Err(error) = self.index_book(app, &book).await {
                tracing::warn!(book = %book.id, %error, "no se pudo indexar el libro");
            }
        }

        Ok(())
    }

    pub async fn index_book(
        &self,
        app: Option<&AppHandle>,
        book: &Book,
    ) -> Result<SemanticBookIndex> {
        if book.owner_account_id != self.owner_account_id {
            return Err(KoraError::NotFound(format!("libro {}", book.id)));
        }
        if !self.provider.ready() {
            self.storage
                .semantic
                .upsert_state(
                    &book.id,
                    EMBEDDING_MODEL_ID,
                    SemanticIndexStatus::Pending,
                    0,
                    Some("el modelo de embeddings no está preparado"),
                    &self.owner_account_id,
                    book.origin_share_id.as_deref(),
                )
                .await?;
            return Err(KoraError::Unsupported(
                "prepara el modelo de búsqueda semántica antes de indexar".into(),
            ));
        }

        self.storage
            .semantic
            .upsert_state(
                &book.id,
                EMBEDDING_MODEL_ID,
                SemanticIndexStatus::Running,
                0,
                None,
                &self.owner_account_id,
                book.origin_share_id.as_deref(),
            )
            .await?;
        emit_progress(
            app,
            SemanticProgress {
                book_id: book.id.clone(),
                title: book.title.clone(),
                indexed: 0,
                total: 0,
                percent: 0.0,
                status: "running".to_string(),
                message: format!("Extrayendo texto de «{}»", book.title),
            },
        );

        let result = self.index_book_inner(app, book).await;

        match result {
            Ok(chunks) => {
                let state = SemanticBookIndex {
                    book_id: book.id.clone(),
                    model: EMBEDDING_MODEL_ID.to_string(),
                    status: if chunks > 0 {
                        SemanticIndexStatus::Done
                    } else {
                        SemanticIndexStatus::Skipped
                    },
                    chunks,
                    error: if chunks == 0 {
                        Some("el libro no tiene texto indexable".to_string())
                    } else {
                        None
                    },
                    updated_at: Utc::now(),
                    owner_account_id: self.owner_account_id.clone(),
                    origin_share_id: book.origin_share_id.clone(),
                };
                if chunks == 0 {
                    self.storage
                        .semantic
                        .upsert_state(
                            &book.id,
                            EMBEDDING_MODEL_ID,
                            SemanticIndexStatus::Skipped,
                            0,
                            Some("el libro no tiene texto indexable"),
                            &self.owner_account_id,
                            book.origin_share_id.as_deref(),
                        )
                        .await?;
                }
                emit_progress(
                    app,
                    SemanticProgress {
                        book_id: book.id.clone(),
                        title: book.title.clone(),
                        indexed: chunks as usize,
                        total: chunks as usize,
                        percent: 100.0,
                        status: if chunks > 0 { "done" } else { "skipped" }.to_string(),
                        message: if chunks > 0 {
                            format!("«{}» indexado", book.title)
                        } else {
                            format!("«{}» no tiene texto indexable", book.title)
                        },
                    },
                );
                Ok(state)
            }
            Err(error) => {
                self.storage
                    .semantic
                    .upsert_state(
                        &book.id,
                        EMBEDDING_MODEL_ID,
                        SemanticIndexStatus::Failed,
                        0,
                        Some(&error.to_string()),
                        &self.owner_account_id,
                        book.origin_share_id.as_deref(),
                    )
                    .await?;
                emit_progress(
                    app,
                    SemanticProgress {
                        book_id: book.id.clone(),
                        title: book.title.clone(),
                        indexed: 0,
                        total: 0,
                        percent: 0.0,
                        status: "failed".to_string(),
                        message: format!("No se pudo indexar «{}»: {error}", book.title),
                    },
                );
                Err(error)
            }
        }
    }

    async fn index_book_inner(&self, app: Option<&AppHandle>, book: &Book) -> Result<i64> {
        let pdf_pages = if book.format == BookFormat::Pdf {
            self.storage
                .pdf
                .get_info(&book.id)
                .await?
                .map(|info| info.page_count.max(0) as usize)
        } else {
            None
        };

        let storage = self.storage.clone();
        let extraction_book = book.clone();
        let sections = tokio::task::spawn_blocking(move || {
            extract_sections(&storage, &extraction_book, pdf_pages)
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de extracción: {error}")))??;

        let mut chunks = Vec::new();
        for section in &sections {
            chunks.extend(chunk_source_text(
                &section.text,
                section.section,
                section.title.as_deref(),
            ));
        }

        if chunks.is_empty() {
            self.storage
                .semantic
                .replace_chunks(&book.id, EMBEDDING_MODEL_ID, &[])
                .await?;
            return Ok(0);
        }

        let total = chunks.len();
        let mut new_chunks = Vec::with_capacity(total);
        let mut indexed = 0usize;

        for (batch_index, batch) in chunks.chunks(32).enumerate() {
            let texts: Vec<String> = batch
                .iter()
                .map(|chunk| {
                    format!(
                        "{} — {}\n{}",
                        book.title,
                        chunk.section_title.as_deref().unwrap_or("Sin título"),
                        chunk.content
                    )
                })
                .collect();
            let embeddings = self.provider.embed_passages(&texts).await?;

            for (chunk, embedding) in batch.iter().zip(embeddings) {
                if embedding.len() != EMBEDDING_DIMENSIONS {
                    return Err(KoraError::Internal(
                        "dimensión de embedding inesperada".into(),
                    ));
                }
                new_chunks.push(NewSemanticChunk {
                    id: Uuid::now_v7().to_string(),
                    book_id: book.id.clone(),
                    section: chunk.section as i64,
                    chunk_index: chunk.chunk_index as i64,
                    section_title: chunk.section_title.clone(),
                    content: chunk.content.clone(),
                    offset_start: chunk.offset_start as i64,
                    offset_end: chunk.offset_end as i64,
                    content_hash: sha256_bytes(chunk.content.as_bytes()),
                    model: EMBEDDING_MODEL_ID.to_string(),
                    dimensions: EMBEDDING_DIMENSIONS as i64,
                    embedding: encode_embedding(&embedding),
                    owner_account_id: self.owner_account_id.clone(),
                    origin_share_id: book.origin_share_id.clone(),
                });
                indexed += 1;
            }

            let percent = ((indexed as f64 / total as f64) * 100.0).min(100.0);
            emit_progress(
                app,
                SemanticProgress {
                    book_id: book.id.clone(),
                    title: book.title.clone(),
                    indexed,
                    total,
                    percent,
                    status: "running".to_string(),
                    message: format!("Indexando «{}» · {indexed}/{total}", book.title),
                },
            );

            tracing::debug!(
                book = %book.id,
                batch = batch_index + 1,
                indexed,
                total,
                "lote de embeddings indexado"
            );
        }

        self.storage
            .semantic
            .replace_chunks(&book.id, EMBEDDING_MODEL_ID, &new_chunks)
            .await?;
        self.storage
            .semantic
            .upsert_state(
                &book.id,
                EMBEDDING_MODEL_ID,
                SemanticIndexStatus::Done,
                new_chunks.len() as i64,
                None,
                &self.owner_account_id,
                book.origin_share_id.as_deref(),
            )
            .await?;

        Ok(new_chunks.len() as i64)
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SemanticHit>> {
        let query = query.trim();
        if query.chars().count() < 2 {
            return Err(KoraError::InvalidInput(
                "la búsqueda necesita al menos 2 caracteres".into(),
            ));
        }
        if !self.provider.ready() {
            return Err(KoraError::Unsupported(
                "prepara el modelo de búsqueda semántica desde Ajustes".into(),
            ));
        }

        let limit = limit.clamp(1, 50);
        let query_embedding = self.provider.embed_query(query).await?;
        if query_embedding.len() != EMBEDDING_DIMENSIONS {
            return Err(KoraError::Internal(
                "la consulta devolvió una dimensión inesperada".into(),
            ));
        }

        let chunks = self
            .storage
            .semantic
            .list_chunks(&self.owner_account_id, EMBEDDING_MODEL_ID)
            .await?;

        let mut scored = Vec::new();
        for chunk in chunks {
            let embedding = decode_embedding(&chunk.embedding)?;
            if embedding.len() != query_embedding.len() {
                continue;
            }
            let score = dot(&query_embedding, &embedding);
            if score > 0.01 {
                scored.push((score, chunk));
            }
        }
        scored.sort_by(|left, right| right.0.total_cmp(&left.0));
        scored.truncate(limit);

        let mut titles = std::collections::HashMap::new();
        for (_, chunk) in &scored {
            if !titles.contains_key(&chunk.book_id) {
                let book = self.storage.books.get(&chunk.book_id).await?;
                if let Some(book) = book {
                    titles.insert(book.id.clone(), (book.title, book.author));
                }
            }
        }

        Ok(scored
            .into_iter()
            .map(|(score, chunk)| {
                let (book_title, author) = titles
                    .get(&chunk.book_id)
                    .cloned()
                    .unwrap_or_else(|| ("Libro desconocido".to_string(), None));
                SemanticHit {
                    book_id: chunk.book_id,
                    book_title,
                    author,
                    section: chunk.section,
                    section_title: chunk.section_title,
                    offset: chunk.offset_start,
                    snippet: snippet(&chunk.content, query),
                    score,
                }
            })
            .collect())
    }
}

fn emit_progress(app: Option<&AppHandle>, progress: SemanticProgress) {
    if let Some(app) = app {
        let _ = app.emit("semantic::progress", progress);
    }
}

fn extract_sections(
    storage: &Storage,
    book: &Book,
    pdf_pages: Option<usize>,
) -> Result<Vec<ExtractedSection>> {
    let path = book_path(storage, book)?;
    match book.format {
        BookFormat::Epub => extract_epub_sections(&path),
        BookFormat::Pdf => extract_pdf_sections(storage, book, &path, pdf_pages),
    }
}

fn book_path(storage: &Storage, book: &Book) -> Result<PathBuf> {
    let reference = AssetRef {
        kind: AssetKind::Book,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    };
    let path = storage.assets.path(&reference)?;
    if path.is_file() {
        return Ok(path);
    }

    let generated = storage.assets.path(&AssetRef {
        kind: AssetKind::GeneratedBook,
        hash: book.content_hash.clone(),
        extension: book.content_ext.clone(),
    })?;
    if generated.is_file() {
        return Ok(generated);
    }

    Err(KoraError::NotFound(format!(
        "archivo del libro {}",
        book.id
    )))
}

fn extract_epub_sections(path: &Path) -> Result<Vec<ExtractedSection>> {
    let reader = ReaderService::new();
    let inspection = reader.inspect_epub(path)?;
    let mut sections = Vec::new();

    for (index, section) in inspection.sections.iter().enumerate() {
        let chapter = match reader.epub_chapter(path, index) {
            Ok(chapter) => chapter,
            Err(error) => {
                tracing::warn!(section = index, %error, "capítulo EPUB ilegible");
                continue;
            }
        };
        let text = extract_text(&chapter.html);
        if text.is_empty() {
            continue;
        }
        sections.push(ExtractedSection {
            section: section.locator.section,
            title: Some(chapter.title),
            text,
        });
    }

    Ok(sections)
}

fn extract_pdf_sections(
    storage: &Storage,
    book: &Book,
    path: &Path,
    page_count: Option<usize>,
) -> Result<Vec<ExtractedSection>> {
    let page_count = page_count.unwrap_or(0);
    if page_count == 0 {
        return Err(KoraError::Unsupported(
            "el PDF aún no tiene información de páginas; ábrelo una vez antes de indexarlo".into(),
        ));
    }

    let parser = PdfParser::new();
    let mut sections = Vec::new();

    for page in 0..page_count {
        if let Some(layer) = crate::reader::ocr::read_ocr_page(storage, &book.id, page)? {
            let text = layer.text();
            if !text.trim().is_empty() {
                sections.push(ExtractedSection {
                    section: page,
                    title: Some(format!("Página {}", page + 1)),
                    text: SourceText {
                        utf16_len: text.encode_utf16().count(),
                        text,
                        breaks: Vec::new(),
                    },
                });
                continue;
            }
        }

        match parser.page_text(path, page) {
            Ok(layer) => {
                let text = layer.text();
                if !text.trim().is_empty() {
                    sections.push(ExtractedSection {
                        section: page,
                        title: Some(format!("Página {}", page + 1)),
                        text: SourceText {
                            utf16_len: text.encode_utf16().count(),
                            text,
                            breaks: Vec::new(),
                        },
                    });
                }
            }
            Err(error) => {
                tracing::warn!(page, %error, "no se pudo extraer texto del PDF");
                if sections.is_empty() {
                    return Err(error);
                }
            }
        }
    }

    Ok(sections)
}

pub fn chunk_source_text(
    text: &SourceText,
    section: usize,
    section_title: Option<&str>,
) -> Vec<TextChunk> {
    let words = word_ranges(text);
    if words.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut chunk_index = 0usize;

    while start < words.len() {
        let end = (start + TARGET_WORDS).min(words.len());
        let content = words[start..end]
            .iter()
            .map(|word| &text.text[word.0..word.1])
            .collect::<Vec<_>>()
            .join(" ");
        if !content.trim().is_empty() {
            let offset_start = words[start].2;
            let offset_end = words[end - 1].3;
            chunks.push(TextChunk {
                section,
                chunk_index,
                section_title: section_title.map(str::to_string),
                content,
                offset_start,
                offset_end,
            });
            chunk_index += 1;
        }

        if end == words.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP_WORDS);
    }

    chunks
}

/// Rangos de palabras `(byte_inicio, byte_fin, utf16_inicio, utf16_fin)`.
fn word_ranges(text: &SourceText) -> Vec<(usize, usize, usize, usize)> {
    let mut words = Vec::new();
    let mut current: Option<(usize, usize)> = None;
    let mut utf16 = 0usize;

    for (byte, ch) in text.text.char_indices() {
        let width = ch.len_utf16();
        if ch.is_whitespace() {
            if let Some((start_byte, start_utf16)) = current.take() {
                words.push((start_byte, byte, start_utf16, utf16));
            }
        } else if current.is_none() {
            current = Some((byte, utf16));
        }
        utf16 += width;
    }

    if let Some((start_byte, start_utf16)) = current {
        words.push((start_byte, text.text.len(), start_utf16, utf16));
    }

    words
}

fn encode_embedding(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_embedding(bytes: &[u8]) -> Result<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        return Err(KoraError::Storage(
            "embedding con longitud inválida".to_string(),
        ));
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect())
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn snippet(content: &str, query: &str) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_SNIPPET_CHARS {
        return compact;
    }

    let lower = compact.to_lowercase();
    let needle = query
        .split_whitespace()
        .find(|word| word.chars().count() >= 3)
        .unwrap_or(query)
        .to_lowercase();
    let start_char = lower
        .find(&needle)
        .map(|byte| lower[..byte].chars().count())
        .unwrap_or(0)
        .saturating_sub(60);
    let start_byte = compact
        .char_indices()
        .nth(start_char)
        .map(|(byte, _)| byte)
        .unwrap_or(0);
    let snippet: String = compact[start_byte..]
        .chars()
        .take(MAX_SNIPPET_CHARS)
        .collect();
    format!("{}…", snippet.trim())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::features::embeddings::TestEmbeddingProvider;
    use crate::storage::test_support::TestStorage;
    use crate::storage::{BookFormat, BookOrigin, NewBook};
    use crate::tejido::epub::{EpubChapter, build_epub};

    #[tokio::test]
    async fn indexes_an_epub_and_searches_by_meaning() -> Result<()> {
        let (test, identity) = TestStorage::with_identity().await;
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("test.epub");
        build_epub(
            &source,
            "Fauna",
            "Kora",
            "es",
            &[EpubChapter {
                title: "El bosque".to_string(),
                body:
                    "El zorro rápido salta sobre el perro perezoso.\nUn búho observa desde la rama."
                        .to_string(),
                source: None,
            }],
            None,
        )?;
        let asset = test
            .storage
            .assets
            .put_file(AssetKind::Book, &source, "epub")?;
        let book = test
            .storage
            .books
            .create(NewBook {
                title: "Fauna".to_string(),
                author: Some("Kora".to_string()),
                format: BookFormat::Epub,
                content_hash: asset.hash,
                content_ext: "epub".to_string(),
                original_name: "test.epub".to_string(),
                cover_hash: None,
                cover_ext: None,
                size_bytes: 1024,
                language: Some("es".to_string()),
                publisher: None,
                description: None,
                origin: BookOrigin::Imported,
                owner_account_id: identity.account.id.clone(),
                origin_share_id: None,
                audio_only: false,
            })
            .await?;

        let service = SemanticService::new(
            test.storage.clone(),
            identity.account.id.clone(),
            Arc::new(TestEmbeddingProvider),
        );
        let state = service.index_book(None, &book).await?;
        assert_eq!(state.status, SemanticIndexStatus::Done);
        assert!(state.chunks > 0);

        let hits = service.search("zorro", 5).await?;
        assert!(!hits.is_empty());
        assert_eq!(hits[0].book_id, book.id);
        assert!(hits[0].snippet.contains("zorro"));
        Ok(())
    }

    #[test]
    fn chunks_keep_roughly_five_hundred_words_and_overlap() {
        let text = (0..1200)
            .map(|index| format!("palabra{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = SourceText {
            utf16_len: text.encode_utf16().count(),
            text,
            breaks: Vec::new(),
        };

        let chunks = chunk_source_text(&source, 2, Some("Capítulo"));
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content.split_whitespace().count(), 500);
        assert_eq!(chunks[1].content.split_whitespace().count(), 500);
        assert!(chunks[1].content.starts_with("palabra450 "));
        assert_eq!(chunks[0].section, 2);
        assert_eq!(chunks[0].section_title.as_deref(), Some("Capítulo"));
        assert_eq!(chunks[0].offset_start, 0);
        assert!(chunks[2].offset_end > chunks[1].offset_end);
    }

    #[test]
    fn chunks_handle_utf16_offsets() {
        let text = "hola 😀 mundo".to_string();
        let source = SourceText {
            utf16_len: text.encode_utf16().count(),
            text,
            breaks: Vec::new(),
        };
        let chunks = chunk_source_text(&source, 0, None);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].offset_start, 0);
        assert_eq!(chunks[0].offset_end, 13);
        assert_eq!(chunks[0].content, "hola 😀 mundo");
    }

    #[test]
    fn embedding_roundtrip_is_stable() {
        let vector = vec![0.25, -1.5, 3.0];
        let decoded = decode_embedding(&encode_embedding(&vector)).expect("decodificar");
        assert_eq!(decoded, vector);
    }

    #[test]
    fn snippet_centers_on_the_query() {
        let content = format!("{} needle {}", "relleno ".repeat(100), "cola ".repeat(100));
        let result = snippet(&content, "needle");
        assert!(result.to_lowercase().contains("needle"));
        assert!(result.ends_with('…'));
    }
}

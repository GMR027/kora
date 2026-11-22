pub mod cover;
pub mod epub;

use serde::Serialize;
use tauri::AppHandle;

use crate::audiobooks::engine;
use crate::audiobooks::jobs::GenerationControl;
use crate::audiobooks::scope::AudiobookScope;
use crate::error::{KoraError, Result};
use crate::state::AppState;
use crate::storage::AudiobookOrigin;
use crate::storage::{AssetKind, Audiobook, Book, BookFormat, BookOrigin, NewBook};

use epub::EpubChapter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaveResult {
    pub book: Book,
    pub audiobook: Audiobook,
}

/// Convierte extractos en un mini-libro EPUB y su audiolibro. El EPUB queda en
/// `assets/generated_books/` y el M4B en `assets/audiobooks/`.
pub async fn weave(
    state: &AppState,
    app: &AppHandle,
    extract_ids: &[String],
    title: &str,
    voice_id: &str,
    control: Option<&GenerationControl>,
) -> Result<WeaveResult> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(KoraError::InvalidInput(
            "el título de Tejido debe tener entre 1 y 120 caracteres".into(),
        ));
    }
    if extract_ids.is_empty() {
        return Err(KoraError::InvalidInput(
            "selecciona al menos un extracto para tejer".into(),
        ));
    }
    if extract_ids.len() > 200 {
        return Err(KoraError::InvalidInput(
            "Tejido admite como máximo 200 extractos por libro".into(),
        ));
    }

    let mut extracts = Vec::with_capacity(extract_ids.len());
    for id in extract_ids {
        let extract = state
            .storage
            .extracts
            .get(id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("extracto {id}")))?;
        if extract.owner_account_id != state.identity.account.id {
            return Err(KoraError::NotFound(format!("extracto {id}")));
        }
        if extract.text.trim().is_empty() {
            continue;
        }
        extracts.push(extract);
    }
    if extracts.is_empty() {
        return Err(KoraError::InvalidInput(
            "los extractos elegidos están vacíos".into(),
        ));
    }

    let mut chapters = Vec::with_capacity(extracts.len());
    for (index, extract) in extracts.iter().enumerate() {
        let book_title = state
            .storage
            .books
            .get(&extract.book_id)
            .await?
            .map(|book| book.title)
            .unwrap_or_else(|| "Origen desconocido".to_string());

        let mut body = extract.text.trim().to_string();
        if let Some(note) = extract
            .note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
        {
            body.push_str("\n\n");
            body.push_str(note);
        }

        chapters.push(EpubChapter {
            title: extract
                .note
                .as_deref()
                .map(str::trim)
                .filter(|note| !note.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{} · extracto {}", book_title, index + 1)),
            body,
            source: Some(book_title),
        });
    }

    let workspace = state
        .layout
        .root
        .join("tmp")
        .join(format!("tejido-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace)?;
    let cover_path = workspace.join("cover.png");
    let epub_path = workspace.join("tejido.epub");
    let subtitle = format!("Kora · Tejido · {} extractos", chapters.len());

    let result = async {
        cover::render_cover(title, &subtitle, &cover_path)?;
        epub::build_epub(
            &epub_path,
            title,
            "Kora · Tejido",
            "es",
            &chapters,
            Some(&cover_path),
        )?;

        let epub_asset =
            state
                .storage
                .assets
                .put_file(AssetKind::GeneratedBook, &epub_path, "epub")?;
        let cover_asset = state
            .storage
            .assets
            .put_file(AssetKind::Cover, &cover_path, "png")?;
        let size_bytes = std::fs::metadata(&epub_path)
            .map(|meta| meta.len())
            .unwrap_or(0) as i64;

        let book = state
            .storage
            .books
            .create(NewBook {
                title: title.to_string(),
                author: Some("Kora · Tejido".to_string()),
                format: BookFormat::Epub,
                content_hash: epub_asset.hash,
                content_ext: "epub".to_string(),
                original_name: format!("{}.epub", slug(title)),
                cover_hash: Some(cover_asset.hash),
                cover_ext: Some("png".to_string()),
                size_bytes,
                language: Some("es".to_string()),
                publisher: None,
                description: Some(format!(
                    "Mini-libro generado desde {} extractos.",
                    chapters.len()
                )),
                origin: BookOrigin::Tejido,
                owner_account_id: state.identity.account.id.clone(),
                origin_share_id: None,
                audio_only: false,
            })
            .await?;

        let audiobook = engine::generate(
            state,
            app,
            &book.id,
            AudiobookScope::FullBook,
            voice_id,
            AudiobookOrigin::Tejido,
            crate::audiobooks::NarrationSettings::default(),
            crate::audiobooks::CastInput::default(),
            control,
        )
        .await?;

        Ok::<WeaveResult, KoraError>(WeaveResult { book, audiobook })
    }
    .await;

    let _ = std::fs::remove_dir_all(&workspace);
    result
}

fn slug(title: &str) -> String {
    let mut slug: String = title
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "tejido".to_string()
    } else {
        slug.chars().take(60).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_safe_for_filenames() {
        assert_eq!(slug("Mi tejido: ideas & notas"), "mi-tejido-ideas-notas");
        assert_eq!(slug("   "), "tejido");
    }
}

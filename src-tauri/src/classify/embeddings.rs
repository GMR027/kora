use std::collections::HashMap;

use crate::error::{KoraError, Result};
use crate::features::embeddings::{EMBEDDING_DIMENSIONS, EmbeddingProvider};
use crate::storage::Category;

#[derive(Debug, Clone)]
pub struct EmbeddingCandidate {
    pub category_id: String,
    pub score: f64,
}

/// Nivel 1: compara la ficha del libro con el nombre y los alias de cada
/// categoría. Los vectores ya vienen normalizados por el proveedor.
pub async fn rank_categories(
    provider: &dyn EmbeddingProvider,
    book_text: &str,
    categories: &[Category],
    aliases: &HashMap<String, Vec<String>>,
) -> Result<Vec<EmbeddingCandidate>> {
    if categories.is_empty() || book_text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut texts = Vec::with_capacity(categories.len());
    for category in categories {
        let mut parts = vec![format!("Categoría: {}", category.name)];
        if let Some(category_aliases) = aliases.get(&category.id)
            && !category_aliases.is_empty()
        {
            parts.push(format!(
                "También conocida como: {}",
                category_aliases.join(", ")
            ));
        }
        texts.push(parts.join("\n"));
    }

    let query = provider.embed_query(book_text).await?;
    let passages = provider.embed_passages(&texts).await?;
    if query.len() != EMBEDDING_DIMENSIONS || passages.len() != categories.len() {
        return Err(KoraError::Internal(
            "el proveedor de embeddings devolvió dimensiones inesperadas".into(),
        ));
    }

    let mut candidates: Vec<EmbeddingCandidate> = categories
        .iter()
        .zip(passages)
        .map(|(category, passage)| EmbeddingCandidate {
            category_id: category.id.clone(),
            score: dot(&query, &passage) as f64,
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.truncate(5);
    Ok(candidates)
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(a, b)| a * b)
        .sum::<f32>()
        .clamp(-1.0, 1.0)
}

use std::collections::HashMap;

use crate::storage::{Category, CategoryAlias};

use super::normalize::{normalize_key, phrase_position};
use super::profile::BookProfile;

#[derive(Debug, Clone)]
pub struct RuleMatch {
    pub category_id: String,
    pub score: f64,
    pub reason: String,
}

/// Nivel 0: compara materias y palabras clave con los alias de las categorías.
/// Las coincidencias exactas valen más que las parciales y, si dos alias
/// compiten, gana el más largo (por ejemplo, "ciencia ficción" gana a "ciencia").
pub fn match_metadata(
    profile: &BookProfile,
    aliases: &[CategoryAlias],
    categories: &[Category],
) -> Vec<RuleMatch> {
    let category_ids: std::collections::HashSet<&str> = categories
        .iter()
        .map(|category| category.id.as_str())
        .collect();

    let mut index: Vec<(String, String, String)> = Vec::new();
    for category in categories {
        index.push((
            category.id.clone(),
            normalize_key(&category.name),
            category.name.clone(),
        ));
    }
    for alias in aliases {
        if category_ids.contains(alias.category_id.as_str()) {
            index.push((
                alias.category_id.clone(),
                alias.alias_normalized.clone(),
                alias.alias.clone(),
            ));
        }
    }
    // El alias más largo primero evita que un término genérico gane a uno
    // específico dentro del mismo texto.
    index.sort_by_key(|(_, alias, _)| std::cmp::Reverse(alias.len()));

    let mut best: HashMap<String, RuleMatch> = HashMap::new();
    for term in profile.metadata_terms() {
        let normalized_term = normalize_key(term);
        if normalized_term.is_empty() {
            continue;
        }

        // Dentro de una materia BISAC, los segmentos finales son más
        // específicos: "Fiction / Fantasy" debe ganar a "Fiction". Por eso
        // desempata la posición además de la longitud del alias.
        let mut chosen: Option<(&String, &String, &String, usize, usize)> = None;
        for (category_id, alias_normalized, alias_display) in &index {
            let Some(position) = phrase_position(&normalized_term, alias_normalized) else {
                continue;
            };
            let rank = (alias_normalized.len(), position);
            let is_better = chosen
                .as_ref()
                .is_none_or(|(_, _, _, length, best_position)| rank > (*length, *best_position));
            if is_better {
                chosen = Some((category_id, alias_normalized, alias_display, rank.0, rank.1));
            }
        }
        let Some((category_id, alias_normalized, alias_display, _, _)) = chosen else {
            continue;
        };

        let exact = normalized_term == *alias_normalized;
        let score = if exact { 1.0 } else { 0.85 };
        let reason = if exact {
            format!("La materia «{term}» coincide con «{alias_display}»")
        } else {
            format!("La materia «{term}» contiene «{alias_display}»")
        };
        let entry = best.entry(category_id.clone()).or_insert(RuleMatch {
            category_id: category_id.clone(),
            score,
            reason: reason.clone(),
        });
        if score > entry.score {
            entry.score = score;
            entry.reason = reason;
        }
    }

    let mut matches: Vec<RuleMatch> = best.into_values().collect();
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Category, CategoryAlias};
    use chrono::Utc;

    fn alias(category_id: &str, normalized: &str) -> CategoryAlias {
        CategoryAlias {
            id: format!("alias-{normalized}"),
            category_id: category_id.to_string(),
            alias: normalized.to_string(),
            alias_normalized: normalized.to_string(),
            source: crate::storage::AliasSource::Seed,
            created_at: Utc::now(),
            owner_account_id: "owner".to_string(),
            origin_share_id: None,
        }
    }

    fn category(id: &str, name: &str) -> Category {
        Category {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            sort_order: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            owner_account_id: "owner".to_string(),
            origin_share_id: None,
        }
    }

    #[test]
    fn prefers_specific_alias() {
        let categories = vec![
            category("sci", "Ciencia y tecnología"),
            category("fic", "Ciencia ficción"),
        ];
        let aliases = vec![alias("sci", "ciencia"), alias("fic", "ciencia ficcion")];
        let profile = BookProfile {
            title: "Dune".to_string(),
            subjects: vec!["Ficción / Ciencia ficción / Space Opera".to_string()],
            ..BookProfile::default()
        };

        let matches = match_metadata(&profile, &aliases, &categories);
        assert!(matches.iter().any(|entry| entry.category_id == "fic"));
        assert!(
            matches
                .iter()
                .all(|entry| entry.category_id != "sci" || entry.score < 1.0)
        );
    }

    #[test]
    fn matches_category_name_even_without_alias() {
        let categories = vec![category("hist", "Historia")];
        let profile = BookProfile {
            subjects: vec!["Historia".to_string()],
            ..BookProfile::default()
        };
        let matches = match_metadata(&profile, &[], &categories);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].score, 1.0);
    }

    #[test]
    fn bisac_specific_segment_wins_over_generic() {
        let categories = vec![
            category("nov", "Novela y narrativa"),
            category("fan", "Fantasía"),
        ];
        let aliases = vec![alias("nov", "fiction"), alias("fan", "fantasy")];
        let profile = BookProfile {
            subjects: vec!["Fiction / Fantasy / Epic".to_string()],
            ..BookProfile::default()
        };

        let matches = match_metadata(&profile, &aliases, &categories);
        assert!(matches.iter().any(|entry| entry.category_id == "fan"));
        assert!(matches.iter().all(|entry| entry.category_id != "nov"));
    }
}

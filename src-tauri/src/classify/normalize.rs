/// Normaliza texto para comparar alias y nombres de categoría.
///
/// Pasa a minúsculas, elimina acentos y sustituye cualquier carácter que no
/// sea alfanumérico por un espacio. El resultado conserva palabras completas
/// para que una coincidencia no parta términos por la mitad.
pub fn normalize_key(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for character in value.to_lowercase().chars() {
        let base = match character {
            'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            'ý' | 'ÿ' => 'y',
            character if character.is_alphanumeric() => character,
            _ => ' ',
        };
        normalized.push(base);
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Devuelve la posición de la primera aparición de `alias_normalized` como
/// frase completa dentro de `text_normalized`.
pub fn phrase_position(text_normalized: &str, alias_normalized: &str) -> Option<usize> {
    if alias_normalized.is_empty() {
        return None;
    }
    if text_normalized == alias_normalized {
        return Some(0);
    }

    let padded_text = format!(" {text_normalized} ");
    let padded_alias = format!(" {alias_normalized} ");
    padded_text.find(&padded_alias)
}

/// Comprueba si `alias_normalized` aparece como frase completa dentro de
/// `text_normalized`. Evita que "arte" coincida con "apartamento".
pub fn matches_alias(text_normalized: &str, alias_normalized: &str) -> bool {
    phrase_position(text_normalized, alias_normalized).is_some()
}

/// Recorta un texto a un número máximo de caracteres sin partir una palabra
/// cuando es posible.
pub fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated: String = value.chars().take(max_chars).collect();
    if let Some(index) = truncated.rfind(char::is_whitespace) {
        truncated.truncate(index);
    }
    truncated.push('…');
    truncated
}

/// Recorta un texto a un número máximo de palabras.
pub fn truncate_words(value: &str, max_words: usize) -> String {
    let words: Vec<&str> = value.split_whitespace().collect();
    if words.len() <= max_words {
        return words.join(" ");
    }
    let mut truncated = words[..max_words].join(" ");
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_accents_and_punctuation() {
        assert_eq!(
            normalize_key("Ciencia Ficción / Épica"),
            "ciencia ficcion epica"
        );
        assert_eq!(
            normalize_key("  Misterio   y suspense  "),
            "misterio y suspense"
        );
        assert_eq!(normalize_key("NñÇ"), "nnc");
    }

    #[test]
    fn matches_complete_phrases_only() {
        assert!(matches_alias("ficcion fantasy epica", "fantasy"));
        assert!(matches_alias("science fiction", "science fiction"));
        assert!(!matches_alias("apartamento", "arte"));
        assert!(!matches_alias("fan", "fantasy"));
    }
}

use scraper::{ElementRef, Html, Node};

/// Convierte una descripción con HTML en texto plano.
///
/// Algunos EPUB incluyen etiquetas en `dc:description` (a veces mal formadas o
/// con doble escape). La biblioteca debe mostrar la sinopsis, no el marcado.
pub fn plain_text(fragment: &str) -> String {
    let trimmed = fragment.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Camino rápido: texto normal sin etiquetas ni entidades.
    if !trimmed.contains('<') && !trimmed.contains('&') {
        return trimmed.to_string();
    }

    let mut current = trimmed.to_string();
    for _ in 0..3 {
        let document = Html::parse_fragment(&current);
        let mut visible = String::new();
        collect_text(document.root_element(), &mut visible);
        let normalized = normalize_whitespace(&visible);

        if normalized == current || !normalized.contains('<') {
            return normalized;
        }

        // El texto todavía contiene etiquetas: venía con doble escape.
        current = normalized;
    }

    current
}

/// Limpia una descripción opcional. Devuelve `None` si queda vacía.
pub fn clean_description(value: Option<String>) -> Option<String> {
    value
        .map(|value| plain_text(&value))
        .filter(|value| !value.is_empty())
}

fn collect_text(element: ElementRef<'_>, output: &mut String) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => output.push_str(text),
            Node::Element(_) => {
                let Some(child) = ElementRef::wrap(child) else {
                    continue;
                };
                let name = child.value().name();
                let separator = is_block(name);
                if separator {
                    output.push(' ');
                }

                collect_text(child, output);

                if separator {
                    output.push(' ');
                }
            }
            _ => {}
        }
    }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "caption"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "ul"
    )
}

#[cfg(test)]
mod tests {
    use super::{clean_description, plain_text};

    #[test]
    fn strips_html_tags_and_entities() {
        assert_eq!(
            plain_text("<p class=\"sinopsis\">Hola <strong>mundo</strong>.</p>"),
            "Hola mundo."
        );
        assert_eq!(plain_text("Tom &amp; Jerry"), "Tom & Jerry");
        assert_eq!(plain_text("<p>Uno</p><p>Dos</p>"), "Uno Dos");
    }

    #[test]
    fn strips_escaped_html() {
        assert_eq!(
            plain_text("&lt;p class=&quot;sinopsis&quot;&gt;Hola&lt;/p&gt;"),
            "Hola"
        );
    }

    #[test]
    fn keeps_plain_text_and_handles_malformed_tags() {
        assert_eq!(plain_text("  Sinopsis normal.  "), "Sinopsis normal.");
        assert!(!plain_text("<p class=").contains('<'));
    }

    #[test]
    fn cleans_optional_descriptions() {
        assert_eq!(
            clean_description(Some("<p>Uno</p><p>Dos</p>".to_string())),
            Some("Uno Dos".to_string())
        );
        assert_eq!(clean_description(Some("   ".to_string())), None);
        assert_eq!(clean_description(None), None);
    }
}

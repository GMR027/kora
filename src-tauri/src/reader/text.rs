use scraper::{ElementRef, Html, Node};

/// Texto visible de un capítulo con offsets en unidades UTF-16, los mismos
/// que usa el DOM del WebView (`Range.toString().length`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceText {
    pub text: String,
    pub utf16_len: usize,
    /// Posiciones UTF-16 donde empieza o termina un bloque (`p`, `div`, `br`…).
    /// El texto no incluye separadores para no romper los offsets; el
    /// preprocesador expresivo los usa como fronteras de frase.
    pub breaks: Vec<usize>,
}

impl SourceText {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Convierte un offset UTF-16 en un índice de byte. Devuelve `None` si el
    /// offset cae dentro de un par suplente.
    pub fn byte_at(&self, offset: usize) -> Option<usize> {
        if offset > self.utf16_len {
            return None;
        }

        let mut utf16 = 0usize;
        for (byte, ch) in self.text.char_indices() {
            if utf16 == offset {
                return Some(byte);
            }
            utf16 += ch.len_utf16();
            if utf16 > offset {
                return None;
            }
        }

        (utf16 == offset).then_some(self.text.len())
    }

    pub fn slice(&self, start: usize, end: usize) -> Option<&str> {
        let start = self.byte_at(start)?;
        let end = self.byte_at(end)?;
        self.text.get(start..end)
    }
}

/// Extrae el texto de un fragmento HTML ya saneado. Recorre los nodos de
/// texto en orden y usa exactamente el mismo contenido que `textContent`.
pub fn extract_text(html: &str) -> SourceText {
    let document = Html::parse_fragment(html);
    let mut source = SourceText::default();

    walk_element(document.root_element(), &mut source);

    source.breaks.sort_unstable();
    source.breaks.dedup();
    source
}

fn walk_element(element: ElementRef<'_>, source: &mut SourceText) {
    let name = element.value().name();
    let block = is_block(name);
    if block {
        push_break(source);
    }
    if name == "br" {
        push_break(source);
    }

    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                source.text.push_str(text);
                source.utf16_len += text.encode_utf16().count();
            }
            Node::Element(_) => {
                if let Some(child_element) = ElementRef::wrap(child) {
                    walk_element(child_element, source);
                }
            }
            _ => {}
        }
    }

    if block {
        push_break(source);
    }
}

fn push_break(source: &mut SourceText) {
    if source.utf16_len == 0 {
        return;
    }
    if source.breaks.last().copied() != Some(source.utf16_len) {
        source.breaks.push(source.utf16_len);
    }
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "caption"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "footer"
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
            | "section"
            | "table"
            | "tbody"
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
    use super::*;

    #[test]
    fn preserves_dom_text_order_and_utf16_offsets() {
        let html = "<p>Hola 😀</p><p>Mundo</p>";
        let source = extract_text(html);
        assert_eq!(source.text, "Hola 😀Mundo");
        assert_eq!(source.utf16_len, "Hola 😀Mundo".encode_utf16().count());
        assert_eq!(source.breaks, vec![7, 12]);
    }

    #[test]
    fn slices_by_utf16_offsets() {
        let source = extract_text("<p>Hola 😀 mundo</p>");
        assert_eq!(source.slice(0, 4), Some("Hola"));
        assert_eq!(source.slice(5, 7), Some("😀"));
        assert_eq!(source.slice(8, 13), Some("mundo"));
    }

    #[test]
    fn marks_paragraph_and_line_breaks() {
        let source = extract_text("<p>uno</p><p>dos<br>tres</p>");
        assert_eq!(source.text, "unodostres");
        assert!(source.breaks.contains(&3));
        assert!(source.breaks.contains(&6));
    }
}

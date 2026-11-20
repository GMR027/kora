use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use scraper::{Html, Selector};

/// Extrae el contenido de `<body>` del XHTML original. Si el documento no
/// tiene body (por ejemplo, un fragmento), se devuelve tal cual.
pub fn extract_body(xhtml: &str) -> String {
    let document = Html::parse_document(xhtml);
    let selector = Selector::parse("body").expect("selector body válido");

    document
        .select(&selector)
        .next()
        .map(|body| body.inner_html())
        .unwrap_or_else(|| xhtml.to_string())
}

/// Sanea el HTML del capítulo y reescribe las imágenes locales por data URIs.
///
/// El EPUB nunca aporta CSS ni JavaScript: solo se conservan etiquetas de
/// texto, tablas e imágenes. Los enlaces externos pierden el `href`.
pub fn sanitize_chapter(body: &str, images: &HashMap<String, String>) -> String {
    let tags: HashSet<&str> = HashSet::from([
        "a",
        "abbr",
        "article",
        "b",
        "blockquote",
        "br",
        "caption",
        "cite",
        "code",
        "dd",
        "del",
        "div",
        "dl",
        "dt",
        "em",
        "figcaption",
        "figure",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "i",
        "img",
        "ins",
        "li",
        "mark",
        "ol",
        "p",
        "pre",
        "q",
        "rp",
        "rt",
        "ruby",
        "s",
        "section",
        "small",
        "span",
        "strike",
        "strong",
        "sub",
        "sup",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "tr",
        "u",
        "ul",
    ]);

    let mut tag_attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    tag_attributes.insert("img", HashSet::from(["src", "alt", "width", "height"]));
    tag_attributes.insert("a", HashSet::from(["href"]));
    tag_attributes.insert("td", HashSet::from(["colspan", "rowspan"]));
    tag_attributes.insert("th", HashSet::from(["colspan", "rowspan"]));
    tag_attributes.insert("ol", HashSet::from(["start"]));

    let images = images.clone();
    let mut builder = ammonia::Builder::default();

    builder
        .tags(tags)
        .tag_attributes(tag_attributes)
        .generic_attributes(HashSet::from(["id", "lang", "dir", "title"]))
        .url_schemes(HashSet::from(["data"]))
        .url_relative(ammonia::UrlRelative::PassThrough)
        .link_rel(None)
        .attribute_filter(
            move |element, attribute, value| match (element, attribute) {
                ("img", "src") => images
                    .get(value)
                    .map(|data_uri| Cow::Owned(data_uri.clone()))
                    .or_else(|| value.starts_with("data:").then_some(Cow::Borrowed(value))),
                ("a", "href") => value.starts_with('#').then_some(Cow::Borrowed(value)),
                _ => Some(Cow::Borrowed(value)),
            },
        );

    builder.clean(body).to_string()
}

/// Longitud del texto visible del capítulo, usada como referencia para el
/// indicador de avance y para acotar los offsets de las anotaciones.
pub fn text_length(html: &str) -> usize {
    let document = Html::parse_fragment(html);
    document
        .root_element()
        .text()
        .flat_map(|text| text.encode_utf16())
        .count()
}

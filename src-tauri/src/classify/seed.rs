/// Catálogo inicial de categorías. Es editable: el usuario puede renombrar,
/// recolorear o borrar cualquiera. Solo se siembra una vez por cuenta.
pub struct SeedCategory {
    pub name: &'static str,
    pub color: &'static str,
    pub aliases: &'static [&'static str],
}

pub const SEED_CATEGORIES: &[SeedCategory] = &[
    SeedCategory {
        name: "Novela y narrativa",
        color: "#6366f1",
        aliases: &[
            "fiction",
            "novel",
            "novels",
            "narrativa",
            "novela",
            "literatura",
            "literary fiction",
            "general fiction",
        ],
    },
    SeedCategory {
        name: "Misterio y suspense",
        color: "#0ea5e9",
        aliases: &[
            "mystery",
            "mysteries",
            "thriller",
            "thrillers",
            "suspense",
            "detective",
            "crime",
            "policiaco",
            "policiaca",
            "novela negra",
        ],
    },
    SeedCategory {
        name: "Ciencia ficción",
        color: "#8b5cf6",
        aliases: &[
            "science fiction",
            "sci-fi",
            "scifi",
            "ciencia ficcion",
            "space opera",
            "distopia",
            "dystopia",
        ],
    },
    SeedCategory {
        name: "Fantasía",
        color: "#a855f7",
        aliases: &[
            "fantasy",
            "fantasia",
            "epic fantasy",
            "fantasia epica",
            "magia",
            "magic",
        ],
    },
    SeedCategory {
        name: "Romance",
        color: "#ec4899",
        aliases: &["romance", "romantica", "romantico", "love story"],
    },
    SeedCategory {
        name: "Terror",
        color: "#64748b",
        aliases: &["horror", "terror", "gothic", "gotico", "ghost stories"],
    },
    SeedCategory {
        name: "Aventura",
        color: "#14b8a6",
        aliases: &["adventure", "adventures", "aventura", "aventuras"],
    },
    SeedCategory {
        name: "Historia",
        color: "#b45309",
        aliases: &[
            "history",
            "historical",
            "historia",
            "historical fiction",
            "novela historica",
        ],
    },
    SeedCategory {
        name: "Biografía y memorias",
        color: "#f59e0b",
        aliases: &[
            "biography",
            "biographies",
            "autobiography",
            "memoirs",
            "memoir",
            "biografia",
            "memorias",
        ],
    },
    SeedCategory {
        name: "Ensayo y no ficción",
        color: "#10b981",
        aliases: &[
            "nonfiction",
            "non-fiction",
            "essay",
            "essays",
            "ensayo",
            "ensayos",
            "no ficcion",
        ],
    },
    SeedCategory {
        name: "Filosofía",
        color: "#7c3aed",
        aliases: &["philosophy", "filosofia", "ethics", "etica"],
    },
    SeedCategory {
        name: "Poesía",
        color: "#e11d48",
        aliases: &["poetry", "poems", "poesia", "poemas"],
    },
    SeedCategory {
        name: "Teatro",
        color: "#d946ef",
        aliases: &[
            "drama",
            "theatre",
            "theater",
            "plays",
            "teatro",
            "obra de teatro",
        ],
    },
    SeedCategory {
        name: "Ciencia y tecnología",
        color: "#0284c7",
        aliases: &[
            "science",
            "sciences",
            "technology",
            "computer science",
            "engineering",
            "ciencia",
            "tecnologia",
            "informatica",
            "ingenieria",
        ],
    },
    SeedCategory {
        name: "Arte y cultura",
        color: "#db2777",
        aliases: &[
            "art", "arts", "music", "cinema", "film", "arte", "cultura", "musica", "cine",
        ],
    },
    SeedCategory {
        name: "Negocios y economía",
        color: "#059669",
        aliases: &[
            "business",
            "economics",
            "finance",
            "management",
            "negocios",
            "economia",
            "finanzas",
            "empresa",
        ],
    },
    SeedCategory {
        name: "Autoayuda y bienestar",
        color: "#22c55e",
        aliases: &[
            "self-help",
            "health",
            "fitness",
            "wellness",
            "autoayuda",
            "bienestar",
            "salud",
            "desarrollo personal",
        ],
    },
    SeedCategory {
        name: "Infantil y juvenil",
        color: "#f97316",
        aliases: &[
            "juvenile",
            "juvenile fiction",
            "young adult",
            "children",
            "childrens",
            "infantil",
            "juvenil",
            "literatura infantil",
        ],
    },
    SeedCategory {
        name: "Cómics y novela gráfica",
        color: "#eab308",
        aliases: &[
            "comics",
            "comic",
            "graphic novels",
            "graphic novel",
            "manga",
            "comic book",
            "novela grafica",
            "historieta",
        ],
    },
    SeedCategory {
        name: "Viajes",
        color: "#0891b2",
        aliases: &["travel", "travels", "voyages", "viajes", "guia de viaje"],
    },
];

/// Color estable para una categoría nueva detectada por la IA.
pub fn color_for_name(name: &str) -> &'static str {
    const PALETTE: &[&str] = &[
        "#6366f1", "#0ea5e9", "#8b5cf6", "#a855f7", "#ec4899", "#f59e0b", "#10b981", "#22c55e",
        "#14b8a6", "#f97316", "#e11d48", "#0284c7", "#d946ef", "#eab308",
    ];
    let mut hash: u32 = 0;
    for byte in name.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u32::from(*byte));
    }
    PALETTE[(hash as usize) % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::classify::normalize::normalize_key;

    #[test]
    fn seed_catalog_has_unique_names_and_aliases() {
        let mut names = HashSet::new();
        for category in SEED_CATEGORIES {
            assert!(
                names.insert(normalize_key(category.name)),
                "nombre repetido"
            );
            let mut aliases = HashSet::new();
            for alias in category.aliases {
                let normalized = normalize_key(alias);
                assert!(!normalized.is_empty(), "alias vacío");
                assert!(
                    aliases.insert(normalized),
                    "alias repetido en la misma categoría"
                );
            }
        }
    }

    #[test]
    fn color_is_stable() {
        assert_eq!(color_for_name("Fantasía"), color_for_name("Fantasía"));
    }
}

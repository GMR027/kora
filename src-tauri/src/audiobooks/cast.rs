use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Asignación explícita de una voz a un personaje. Si `voice_id` está vacío,
/// el personaje entra en la rotación automática.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastAssignment {
    pub character: String,
    pub voice_id: String,
}

/// Ajustes de multivot que envía el generador. `enabled` activa el reparto y
/// `assignments` contiene solo las elecciones manuales.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastInput {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub assignments: Vec<CastAssignment>,
}

impl CastInput {
    pub fn normalized(mut self) -> Self {
        let mut seen = HashSet::new();
        self.assignments.retain_mut(|assignment| {
            assignment.character = assignment.character.trim().to_string();
            assignment.voice_id = assignment.voice_id.trim().to_string();
            !assignment.character.is_empty() && seen.insert(assignment.character.clone())
        });
        self
    }
}

/// Voz resuelta que se guarda en el manifiesto para que la biblioteca pueda
/// mostrar el reparto sin depender de que las voces sigan instaladas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastMember {
    pub character: String,
    pub voice_id: String,
    pub voice_name: String,
    pub cloned: bool,
}

/// Datos mínimos de una voz instalada que necesita el reparto.
#[derive(Debug, Clone)]
pub struct CastVoice {
    pub id: String,
    pub name: String,
    pub cloned: bool,
}

/// Resuelve la voz de cada fragmento mientras se sintetiza. Las asignaciones
/// manuales mandan; los personajes sin voz reciben una alternativa instalada
/// en rotación. El narrador conserva siempre la voz principal.
pub struct CastResolver {
    enabled: bool,
    base: CastVoice,
    installed: Vec<CastVoice>,
    assignments: HashMap<String, String>,
    members: Vec<CastMember>,
    recorded: HashSet<String>,
    next: usize,
}

impl CastResolver {
    pub fn new(input: CastInput, base: CastVoice, installed: Vec<CastVoice>) -> Self {
        let input = input.normalized();
        let mut assignments = HashMap::new();
        if input.enabled {
            for assignment in input.assignments {
                if installed
                    .iter()
                    .any(|voice| voice.id == assignment.voice_id)
                {
                    assignments.insert(assignment.character, assignment.voice_id);
                }
            }
        }
        Self {
            enabled: input.enabled,
            base,
            installed,
            assignments,
            members: Vec::new(),
            recorded: HashSet::new(),
            next: 0,
        }
    }

    pub fn single(base: CastVoice) -> Self {
        Self::new(CastInput::default(), base, Vec::new())
    }

    /// Vuelve a la voz del narrador, sin tocar el reparto.
    pub fn base_voice(&self) -> &str {
        &self.base.id
    }

    pub fn voice_for(&mut self, character: Option<&str>) -> String {
        if !self.enabled {
            return self.base.id.clone();
        }
        let Some(character) = character.filter(|name| !name.trim().is_empty()) else {
            return self.base.id.clone();
        };

        if let Some(voice_id) = self.assignments.get(character).cloned() {
            self.record(character, &voice_id);
            return voice_id;
        }

        let alternatives: Vec<&CastVoice> = self
            .installed
            .iter()
            .filter(|voice| voice.id != self.base.id)
            .collect();
        if alternatives.is_empty() {
            return self.base.id.clone();
        }

        // Primero se usan voces que todavía no tenga otro personaje; cuando
        // todas están ocupadas, se repiten en rotación.
        let used: HashSet<String> = self.assignments.values().cloned().collect();
        let free: Vec<&CastVoice> = alternatives
            .iter()
            .copied()
            .filter(|voice| !used.contains(&voice.id))
            .collect();
        let voice_id = if free.is_empty() {
            // Todas las alternativas están ocupadas: se elige la menos usada
            // para repartir el desgaste entre las voces disponibles.
            let mut usage: HashMap<&str, usize> = HashMap::new();
            for voice in self.assignments.values() {
                *usage.entry(voice.as_str()).or_default() += 1;
            }
            alternatives
                .iter()
                .min_by_key(|voice| usage.get(voice.id.as_str()).copied().unwrap_or(0))
                .map(|voice| voice.id.clone())
                .unwrap_or_else(|| self.base.id.clone())
        } else {
            let voice = free[self.next % free.len()].id.clone();
            self.next += 1;
            voice
        };
        self.assignments
            .insert(character.to_string(), voice_id.clone());
        self.record(character, &voice_id);
        voice_id
    }

    fn record(&mut self, character: &str, voice_id: &str) {
        if !self.recorded.insert(character.to_string()) {
            return;
        }
        let Some(voice) = self
            .installed
            .iter()
            .find(|voice| voice.id == voice_id)
            .cloned()
        else {
            return;
        };
        self.members.push(CastMember {
            character: character.to_string(),
            voice_id: voice.id,
            voice_name: voice.name,
            cloned: voice.cloned,
        });
    }

    pub fn members(&self) -> &[CastMember] {
        &self.members
    }

    pub fn any_cloned(&self) -> bool {
        self.members.iter().any(|member| member.cloned)
    }

    pub fn into_members(self) -> Vec<CastMember> {
        self.members
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, cloned: bool) -> CastVoice {
        CastVoice {
            id: id.to_string(),
            name: id.to_uppercase(),
            cloned,
        }
    }

    fn input(assignments: &[(&str, &str)]) -> CastInput {
        CastInput {
            enabled: true,
            assignments: assignments
                .iter()
                .map(|(character, voice_id)| CastAssignment {
                    character: (*character).to_string(),
                    voice_id: (*voice_id).to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn disabled_cast_keeps_the_narrator_voice() {
        let base = voice("base", false);
        let mut resolver = CastResolver::new(
            CastInput::default(),
            base,
            vec![voice("base", false), voice("ana", false)],
        );
        assert_eq!(resolver.voice_for(Some("Ana")), "base");
        assert!(resolver.members().is_empty());
    }

    #[test]
    fn manual_assignments_win_and_auto_rotates_the_rest() {
        let base = voice("base", false);
        let mut resolver = CastResolver::new(
            input(&[("Ana", "ana")]),
            base,
            vec![
                voice("base", false),
                voice("ana", false),
                voice("luis", true),
            ],
        );

        assert_eq!(resolver.voice_for(None), "base");
        assert_eq!(resolver.voice_for(Some("Ana")), "ana");
        assert_eq!(resolver.voice_for(Some("Luis")), "luis");
        assert_eq!(resolver.voice_for(Some("Luis")), "luis");
        assert_eq!(resolver.voice_for(Some("Marta")), "ana");
        assert_eq!(resolver.members().len(), 3);
        assert!(resolver.any_cloned());
    }

    #[test]
    fn missing_voices_are_ignored_and_auto_assignment_still_works() {
        let base = voice("base", false);
        let mut resolver = CastResolver::new(
            input(&[("Ana", "missing"), ("Narrador", "ana")]),
            base,
            vec![voice("base", false), voice("ana", false)],
        );
        assert_eq!(resolver.voice_for(Some("Narrador")), "ana");
        assert_eq!(resolver.voice_for(Some("Ana")), "ana");
        assert_eq!(resolver.members().len(), 2);
    }

    #[test]
    fn explicit_narrator_voice_is_respected() {
        let base = voice("base", false);
        let mut resolver = CastResolver::new(
            input(&[("Ana", "base")]),
            base,
            vec![voice("base", false), voice("ana", false)],
        );
        assert_eq!(resolver.voice_for(Some("Ana")), "base");
        assert_eq!(resolver.members().len(), 1);
        assert_eq!(resolver.members()[0].voice_name, "BASE");
    }

    #[test]
    fn normalizes_and_deduplicates_assignments() {
        let input = CastInput {
            enabled: true,
            assignments: vec![
                CastAssignment {
                    character: " Ana ".into(),
                    voice_id: " ana ".into(),
                },
                CastAssignment {
                    character: "Ana".into(),
                    voice_id: "luis".into(),
                },
                CastAssignment {
                    character: "  ".into(),
                    voice_id: "luis".into(),
                },
            ],
        }
        .normalized();
        assert_eq!(input.assignments.len(), 1);
        assert_eq!(input.assignments[0].character, "Ana");
        assert_eq!(input.assignments[0].voice_id, "ana");
    }
}

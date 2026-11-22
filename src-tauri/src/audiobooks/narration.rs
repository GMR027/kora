use serde::{Deserialize, Serialize};

/// Estilo de narración aplicado durante la síntesis. Cada estilo ajusta el
/// ritmo, el tono y el énfasis de los fragmentos; la velocidad base del usuario
/// se multiplica por esos ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NarrationStyle {
    #[default]
    Neutral,
    Calm,
    Dramatic,
    Lively,
}

/// Ajustes de narración que viajan del generador al manifiesto.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrationSettings {
    #[serde(default)]
    pub style: NarrationStyle,
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// Activa el reparto de voces por personaje. El reparto concreto viaja en
    /// `CastInput` y se guarda resuelto en el manifiesto.
    #[serde(default)]
    pub multi_voice: bool,
}

pub const MIN_SPEED: f32 = 0.6;
pub const MAX_SPEED: f32 = 2.0;

fn default_speed() -> f32 {
    1.0
}

impl Default for NarrationSettings {
    fn default() -> Self {
        Self {
            style: NarrationStyle::Neutral,
            speed: 1.0,
            multi_voice: false,
        }
    }
}

impl NarrationSettings {
    pub fn clamped(self) -> Self {
        let speed = if self.speed.is_finite() {
            self.speed.clamp(MIN_SPEED, MAX_SPEED)
        } else {
            1.0
        };
        Self {
            style: self.style,
            speed,
            multi_voice: self.multi_voice,
        }
    }

    /// Multiplicadores que se aplican sobre la prosodia que ya calcula el
    /// analizador de texto para cada fragmento.
    pub fn profile(self) -> NarrationProfile {
        let settings = self.clamped();
        let base = match settings.style {
            NarrationStyle::Neutral => NarrationProfile {
                rate: 1.0,
                pitch: 1.0,
                pause_scale: 1.0,
                emphasis: 1.0,
            },
            NarrationStyle::Calm => NarrationProfile {
                rate: 0.92,
                pitch: 0.97,
                pause_scale: 1.4,
                emphasis: 0.7,
            },
            NarrationStyle::Dramatic => NarrationProfile {
                rate: 0.9,
                pitch: 1.02,
                pause_scale: 1.25,
                emphasis: 1.6,
            },
            NarrationStyle::Lively => NarrationProfile {
                rate: 1.08,
                pitch: 1.04,
                pause_scale: 0.75,
                emphasis: 1.25,
            },
        };
        NarrationProfile {
            rate: base.rate * settings.speed,
            ..base
        }
    }

    pub fn is_default(self) -> bool {
        let settings = self.clamped();
        settings.style == NarrationStyle::Neutral
            && (settings.speed - 1.0).abs() < f32::EPSILON
            && !settings.multi_voice
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NarrationProfile {
    pub rate: f32,
    pub pitch: f32,
    pub pause_scale: f32,
    pub emphasis: f32,
}

impl NarrationProfile {
    /// Aplica el perfil a la prosodia relativa de un fragmento (1.0 = neutro).
    /// `emphasis` amplifica o atenúa las desviaciones por puntuación y diálogo.
    pub fn apply(self, rate: f32, pitch: f32) -> (f32, f32) {
        let rate = (1.0 + (rate - 1.0) * self.emphasis) * self.rate;
        let pitch = (1.0 + (pitch - 1.0) * self.emphasis) * self.pitch;
        (rate.clamp(0.5, 3.0), pitch.clamp(0.5, 1.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_narration_keeps_the_original_prosody() {
        let profile = NarrationSettings::default().profile();
        assert_eq!(profile.rate, 1.0);
        assert_eq!(profile.pitch, 1.0);
        assert_eq!(profile.pause_scale, 1.0);
        assert_eq!(profile.emphasis, 1.0);
        assert!(NarrationSettings::default().is_default());
    }

    #[test]
    fn styles_change_rhythm_and_pauses() {
        let calm = NarrationSettings {
            style: NarrationStyle::Calm,
            speed: 1.0,
            multi_voice: false,
        }
        .profile();
        let lively = NarrationSettings {
            style: NarrationStyle::Lively,
            speed: 1.0,
            multi_voice: false,
        }
        .profile();

        assert!(calm.rate < lively.rate);
        assert!(calm.pause_scale > lively.pause_scale);
        assert!(calm.emphasis < lively.emphasis);
    }

    #[test]
    fn speed_multiplies_the_style_rate_and_clamps() {
        let fast = NarrationSettings {
            style: NarrationStyle::Neutral,
            speed: 1.5,
            multi_voice: false,
        }
        .profile();
        assert!((fast.rate - 1.5).abs() < 0.001);

        let clamped = NarrationSettings {
            style: NarrationStyle::Neutral,
            speed: 9.0,
            multi_voice: false,
        }
        .clamped();
        assert_eq!(clamped.speed, MAX_SPEED);

        let invalid = NarrationSettings {
            style: NarrationStyle::Neutral,
            speed: f32::NAN,
            multi_voice: false,
        }
        .clamped();
        assert_eq!(invalid.speed, 1.0);
    }

    #[test]
    fn emphasis_scales_the_punctuation_prosody() {
        let neutral = NarrationSettings::default().profile();
        let calm = NarrationSettings {
            style: NarrationStyle::Calm,
            speed: 1.0,
            multi_voice: false,
        }
        .profile();
        let dramatic = NarrationSettings {
            style: NarrationStyle::Dramatic,
            speed: 1.0,
            multi_voice: false,
        }
        .profile();

        let (neutral_rate, _) = neutral.apply(1.2, 1.2);
        let (calm_rate, _) = calm.apply(1.2, 1.2);
        let (dramatic_rate, _) = dramatic.apply(1.2, 1.2);

        assert!((neutral_rate - 1.2).abs() < 0.001);
        assert!(calm_rate < neutral_rate);
        assert!(dramatic_rate / dramatic.rate > neutral_rate);
    }

    #[test]
    fn multi_voice_is_preserved_and_not_the_default() {
        let settings = NarrationSettings {
            multi_voice: true,
            ..NarrationSettings::default()
        };
        assert!(!settings.is_default());
        assert!(settings.clamped().multi_voice);
    }

    #[test]
    fn deserializes_missing_fields_with_defaults() {
        let empty: NarrationSettings = serde_json::from_str("{}").expect("vacío");
        assert_eq!(empty, NarrationSettings::default());
    }
}

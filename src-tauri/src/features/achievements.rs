use chrono::{Duration, NaiveDate, Utc};
use sqlx::Row;

use crate::error::Result;
use crate::storage::{Achievement, NewAchievement, Storage};

#[derive(Debug, Clone, Copy)]
pub struct AchievementDefinition {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub target: f64,
}

/// Catálogo de logros de la fase 11. Los valores son acumulativos y nunca se
/// pierden al desbloquearse.
pub const DEFINITIONS: &[AchievementDefinition] = &[
    AchievementDefinition {
        key: "first_book",
        title: "Primer libro",
        description: "Añade tu primer libro a la biblioteca.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "library_10",
        title: "Estantería en marcha",
        description: "Reúne 10 libros en tu biblioteca.",
        target: 10.0,
    },
    AchievementDefinition {
        key: "library_50",
        title: "Bibliotecario",
        description: "Reúne 50 libros en tu biblioteca.",
        target: 50.0,
    },
    AchievementDefinition {
        key: "first_extract",
        title: "Primera chispa",
        description: "Guarda tu primer extracto.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "extracts_50",
        title: "Coleccionista",
        description: "Guarda 50 extractos.",
        target: 50.0,
    },
    AchievementDefinition {
        key: "extracts_200",
        title: "Archivo personal",
        description: "Guarda 200 extractos.",
        target: 200.0,
    },
    AchievementDefinition {
        key: "first_note",
        title: "Nota al margen",
        description: "Escribe tu primera nota.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "notes_50",
        title: "Cuaderno vivo",
        description: "Escribe 50 notas.",
        target: 50.0,
    },
    AchievementDefinition {
        key: "first_diary",
        title: "Primer día",
        description: "Registra tu primera sesión en el diario.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "diary_30",
        title: "Constancia",
        description: "Registra 30 sesiones en el diario.",
        target: 30.0,
    },
    AchievementDefinition {
        key: "minutes_60",
        title: "Una hora",
        description: "Acumula 60 minutos de lectura o escucha.",
        target: 60.0,
    },
    AchievementDefinition {
        key: "minutes_600",
        title: "Diez horas",
        description: "Acumula 600 minutos de lectura o escucha.",
        target: 600.0,
    },
    AchievementDefinition {
        key: "minutes_3000",
        title: "Cincuenta horas",
        description: "Acumula 3000 minutos de lectura o escucha.",
        target: 3000.0,
    },
    AchievementDefinition {
        key: "pages_100",
        title: "Cien páginas",
        description: "Lee 100 páginas.",
        target: 100.0,
    },
    AchievementDefinition {
        key: "pages_1000",
        title: "Mil páginas",
        description: "Lee 1000 páginas.",
        target: 1000.0,
    },
    AchievementDefinition {
        key: "first_pomodoro",
        title: "Enfoque",
        description: "Completa tu primer Pomodoro.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "pomodoros_25",
        title: "Máquina de foco",
        description: "Completa 25 Pomodoros.",
        target: 25.0,
    },
    AchievementDefinition {
        key: "streak_7",
        title: "Siete días",
        description: "Lee o escucha 7 días consecutivos.",
        target: 7.0,
    },
    AchievementDefinition {
        key: "streak_30",
        title: "Un mes de hábito",
        description: "Lee o escucha 30 días consecutivos.",
        target: 30.0,
    },
    AchievementDefinition {
        key: "first_audiobook",
        title: "Voz propia",
        description: "Genera tu primer audiolibro.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "first_share",
        title: "Compartir es vivir",
        description: "Crea tu primer recurso compartido.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "first_voice_note",
        title: "Voz al margen",
        description: "Graba tu primera nota de voz.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "night_reader",
        title: "Lector Nocturno",
        description: "Lee o escucha en cinco sesiones nocturnas.",
        target: 5.0,
    },
    AchievementDefinition {
        key: "bimodal_master",
        title: "Bimodal Master",
        description: "Lee y escucha el mismo libro.",
        target: 1.0,
    },
    AchievementDefinition {
        key: "focus_sentinel",
        title: "Centinela del Enfoque",
        description: "Completa diez Pomodoros.",
        target: 10.0,
    },
    AchievementDefinition {
        key: "book_devourer",
        title: "Devorador de Libros",
        description: "Termina cinco libros.",
        target: 5.0,
    },
];

#[derive(Debug, Clone, Default)]
struct Metrics {
    books: f64,
    extracts: f64,
    notes: f64,
    diary_entries: f64,
    reading_minutes: f64,
    pages: f64,
    pomodoros: f64,
    streak: f64,
    audiobooks: f64,
    shares: f64,
    voice_notes: f64,
    night_sessions: f64,
    bimodal_books: f64,
    finished_books: f64,
}

pub struct AchievementsService {
    storage: Storage,
}

impl AchievementsService {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Devuelve el catálogo completo con el progreso actual sin emitir
    /// notificaciones.
    pub async fn list(&self, owner_account_id: &str) -> Result<Vec<Achievement>> {
        self.refresh(owner_account_id).await?;
        self.storage
            .achievements
            .list_by_owner(owner_account_id)
            .await
    }

    /// Recalcula los logros y devuelve solo los que se han desbloqueado en
    /// esta llamada, para que el comando pueda emitir el evento.
    pub async fn evaluate(&self, owner_account_id: &str) -> Result<Vec<Achievement>> {
        let before = self
            .storage
            .achievements
            .list_by_owner(owner_account_id)
            .await?;
        let before_unlocked = before
            .iter()
            .filter(|achievement| achievement.unlocked_at.is_some())
            .map(|achievement| achievement.key.clone())
            .collect::<std::collections::HashSet<_>>();

        self.refresh(owner_account_id).await?;

        let after = self
            .storage
            .achievements
            .list_by_owner(owner_account_id)
            .await?;
        Ok(after
            .into_iter()
            .filter(|achievement| {
                achievement.unlocked_at.is_some() && !before_unlocked.contains(&achievement.key)
            })
            .collect())
    }

    async fn refresh(&self, owner_account_id: &str) -> Result<()> {
        let metrics = self.metrics(owner_account_id).await?;
        let now = Utc::now();

        for definition in DEFINITIONS {
            let value = match definition.key {
                "first_book" | "library_10" | "library_50" => metrics.books,
                "first_extract" | "extracts_50" | "extracts_200" => metrics.extracts,
                "first_note" | "notes_50" => metrics.notes,
                "first_diary" | "diary_30" => metrics.diary_entries,
                "minutes_60" | "minutes_600" | "minutes_3000" => metrics.reading_minutes,
                "pages_100" | "pages_1000" => metrics.pages,
                "first_pomodoro" | "pomodoros_25" => metrics.pomodoros,
                "streak_7" | "streak_30" => metrics.streak,
                "first_audiobook" => metrics.audiobooks,
                "first_share" => metrics.shares,
                "first_voice_note" => metrics.voice_notes,
                "night_reader" => metrics.night_sessions,
                "bimodal_master" => metrics.bimodal_books,
                "focus_sentinel" => metrics.pomodoros,
                "book_devourer" => metrics.finished_books,
                _ => 0.0,
            };
            let progress = (value / definition.target).clamp(0.0, 1.0);
            let unlocked_at = (progress >= 1.0).then_some(now);

            self.storage
                .achievements
                .upsert(NewAchievement {
                    key: definition.key.to_string(),
                    title: definition.title.to_string(),
                    description: Some(definition.description.to_string()),
                    progress,
                    unlocked_at,
                    owner_account_id: owner_account_id.to_string(),
                    origin_share_id: None,
                })
                .await?;
        }

        Ok(())
    }

    async fn metrics(&self, owner_account_id: &str) -> Result<Metrics> {
        let row = sqlx::query(
            "SELECT
                (SELECT COUNT(*) FROM books
                 WHERE owner_account_id = ? AND deleted_at IS NULL) AS books,
                (SELECT COUNT(*) FROM extracts
                 WHERE owner_account_id = ? AND deleted = 0) AS extracts,
                (SELECT COUNT(*) FROM notes
                 WHERE owner_account_id = ? AND deleted = 0) AS notes,
                (SELECT COUNT(*) FROM diary_entries
                 WHERE owner_account_id = ?) AS diary_entries,
                (SELECT COALESCE(SUM(duration_minutes), 0) FROM diary_entries
                 WHERE owner_account_id = ? AND kind IN ('reading', 'listening')) AS minutes,
                (SELECT COALESCE(SUM(pages_read), 0) FROM diary_entries
                 WHERE owner_account_id = ?) AS pages,
                (SELECT COUNT(*) FROM diary_entries
                 WHERE owner_account_id = ? AND kind = 'pomodoro') AS pomodoros,
                (SELECT COUNT(*) FROM audiobooks
                 WHERE owner_account_id = ?) AS audiobooks,
                (SELECT COUNT(*) FROM shares
                 WHERE owner_account_id = ? AND revoked_at IS NULL) AS shares,
                (SELECT COUNT(*) FROM voice_notes
                 WHERE owner_account_id = ? AND deleted = 0) AS voice_notes,
                (SELECT COUNT(*) FROM diary_entries
                 WHERE owner_account_id = ?
                   AND kind IN ('reading', 'listening')
                   AND session_start IS NOT NULL
                   AND (CAST(strftime('%H', session_start) AS INTEGER) >= 22
                        OR CAST(strftime('%H', session_start) AS INTEGER) < 6)) AS night_sessions,
                (SELECT COUNT(*) FROM (
                    SELECT book_id FROM diary_entries
                    WHERE owner_account_id = ? AND book_id IS NOT NULL
                      AND kind IN ('reading', 'listening')
                    GROUP BY book_id
                    HAVING COUNT(DISTINCT kind) = 2
                 )) AS bimodal_books,
                (SELECT COUNT(*) FROM reading_progress rp
                 JOIN books b ON b.id = rp.book_id
                 WHERE b.owner_account_id = ? AND b.deleted_at IS NULL
                   AND rp.percent >= 0.995) AS finished_books",
        )
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .bind(owner_account_id)
        .fetch_one(self.storage.pool())
        .await?;

        Ok(Metrics {
            books: row.try_get::<i64, _>("books")? as f64,
            extracts: row.try_get::<i64, _>("extracts")? as f64,
            notes: row.try_get::<i64, _>("notes")? as f64,
            diary_entries: row.try_get::<i64, _>("diary_entries")? as f64,
            reading_minutes: row.try_get::<i64, _>("minutes")? as f64,
            pages: row.try_get::<i64, _>("pages")? as f64,
            pomodoros: row.try_get::<i64, _>("pomodoros")? as f64,
            streak: self.streak(owner_account_id).await?,
            audiobooks: row.try_get::<i64, _>("audiobooks")? as f64,
            shares: row.try_get::<i64, _>("shares")? as f64,
            voice_notes: row.try_get::<i64, _>("voice_notes")? as f64,
            night_sessions: row.try_get::<i64, _>("night_sessions")? as f64,
            bimodal_books: row.try_get::<i64, _>("bimodal_books")? as f64,
            finished_books: row.try_get::<i64, _>("finished_books")? as f64,
        })
    }

    async fn streak(&self, owner_account_id: &str) -> Result<f64> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT entry_date FROM diary_entries
             WHERE owner_account_id = ? AND kind IN ('reading', 'listening')
             ORDER BY entry_date",
        )
        .bind(owner_account_id)
        .fetch_all(self.storage.pool())
        .await?;

        let mut best = 0i64;
        let mut current = 0i64;
        let mut previous: Option<NaiveDate> = None;

        for (date,) in rows {
            let Ok(date) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") else {
                continue;
            };
            current = match previous {
                Some(previous) if date == previous + Duration::days(1) => current + 1,
                _ => 1,
            };
            best = best.max(current);
            previous = Some(date);
        }

        Ok(best as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_at_least_fifteen_achievements() {
        assert!(DEFINITIONS.len() >= 15);
        assert!(
            DEFINITIONS
                .iter()
                .all(|definition| !definition.key.is_empty())
        );
    }
}

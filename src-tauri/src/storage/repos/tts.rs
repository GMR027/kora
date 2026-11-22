use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::Result;
use crate::storage::model::{
    CharacterVoice, NewCharacterVoice, NewPracticeAttempt, NewTtsPreferences, NewTtsSequence,
    PracticeAttempt, TtsPreferences, TtsSequenceRecord,
};

/// Persistencia de la fase TTS: caché de secuencias expresivas, voces por
/// personaje, preferencias por libro e intentos del modo ensayo.
#[derive(Clone)]
pub struct TtsRepo {
    pool: SqlitePool,
}

impl TtsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn get_sequence(
        &self,
        book_id: &str,
        section: i64,
        content_hash: &str,
    ) -> Result<Option<TtsSequenceRecord>> {
        Ok(sqlx::query_as::<_, TtsSequenceRecord>(
            "SELECT * FROM tts_sequences
             WHERE book_id = ? AND section = ? AND content_hash = ?",
        )
        .bind(book_id)
        .bind(section)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Guarda la secuencia y descarta las versiones anteriores de la misma
    /// sección para que la caché no crezca sin límite.
    pub async fn save_sequence(&self, new: NewTtsSequence) -> Result<TtsSequenceRecord> {
        sqlx::query(
            "DELETE FROM tts_sequences
             WHERE book_id = ? AND section = ? AND content_hash <> ?",
        )
        .bind(&new.book_id)
        .bind(new.section)
        .bind(&new.content_hash)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO tts_sequences
                (book_id, section, content_hash, unit, sequence, created_at,
                 owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id, section, content_hash) DO UPDATE SET
                unit = excluded.unit,
                sequence = excluded.sequence,
                created_at = excluded.created_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&new.book_id)
        .bind(new.section)
        .bind(&new.content_hash)
        .bind(new.unit)
        .bind(&new.sequence)
        .bind(Utc::now())
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        self.get_sequence(&new.book_id, new.section, &new.content_hash)
            .await?
            .ok_or_else(|| {
                crate::error::KoraError::Internal(
                    "secuencia TTS recién guardada no encontrada".into(),
                )
            })
    }

    pub async fn character_voices(&self, book_id: &str) -> Result<Vec<CharacterVoice>> {
        Ok(sqlx::query_as::<_, CharacterVoice>(
            "SELECT * FROM tts_character_voices WHERE book_id = ? ORDER BY character",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn set_character_voice(&self, new: NewCharacterVoice) -> Result<CharacterVoice> {
        sqlx::query(
            "INSERT INTO tts_character_voices
                (book_id, character, voice_id, updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id, character) DO UPDATE SET
                voice_id = excluded.voice_id,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&new.book_id)
        .bind(&new.character)
        .bind(&new.voice_id)
        .bind(Utc::now())
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, CharacterVoice>(
            "SELECT * FROM tts_character_voices WHERE book_id = ? AND character = ?",
        )
        .bind(&new.book_id)
        .bind(&new.character)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn delete_character_voice(&self, book_id: &str, character: &str) -> Result<()> {
        sqlx::query("DELETE FROM tts_character_voices WHERE book_id = ? AND character = ?")
            .bind(book_id)
            .bind(character)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn preferences(&self, book_id: &str) -> Result<Option<TtsPreferences>> {
        Ok(
            sqlx::query_as::<_, TtsPreferences>("SELECT * FROM tts_preferences WHERE book_id = ?")
                .bind(book_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn save_preferences(&self, new: NewTtsPreferences) -> Result<TtsPreferences> {
        sqlx::query(
            "INSERT INTO tts_preferences
                (book_id, voice_id, rate, pitch, pause_scale, expressive, multi_voice,
                 updated_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (book_id) DO UPDATE SET
                voice_id = excluded.voice_id,
                rate = excluded.rate,
                pitch = excluded.pitch,
                pause_scale = excluded.pause_scale,
                expressive = excluded.expressive,
                multi_voice = excluded.multi_voice,
                updated_at = excluded.updated_at,
                owner_account_id = excluded.owner_account_id,
                origin_share_id = excluded.origin_share_id",
        )
        .bind(&new.book_id)
        .bind(&new.voice_id)
        .bind(new.rate)
        .bind(new.pitch)
        .bind(new.pause_scale)
        .bind(new.expressive)
        .bind(new.multi_voice)
        .bind(Utc::now())
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        self.preferences(&new.book_id).await?.ok_or_else(|| {
            crate::error::KoraError::Internal(
                "preferencias TTS recién guardadas no encontradas".into(),
            )
        })
    }

    pub async fn save_practice_attempt(&self, new: NewPracticeAttempt) -> Result<PracticeAttempt> {
        sqlx::query(
            "INSERT INTO tts_practice_attempts
                (id, book_id, section, offset_start, offset_end, expected, transcript,
                 accuracy, diff, created_at, owner_account_id, origin_share_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&new.id)
        .bind(&new.book_id)
        .bind(new.section)
        .bind(new.offset_start)
        .bind(new.offset_end)
        .bind(&new.expected)
        .bind(&new.transcript)
        .bind(new.accuracy)
        .bind(&new.diff)
        .bind(Utc::now())
        .bind(&new.owner_account_id)
        .bind(&new.origin_share_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, PracticeAttempt>("SELECT * FROM tts_practice_attempts WHERE id = ?")
            .bind(&new.id)
            .fetch_one(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn practice_history(
        &self,
        book_id: &str,
        limit: i64,
    ) -> Result<Vec<PracticeAttempt>> {
        Ok(sqlx::query_as::<_, PracticeAttempt>(
            "SELECT * FROM tts_practice_attempts
             WHERE book_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(book_id)
        .bind(limit.clamp(1, 200))
        .fetch_all(&self.pool)
        .await?)
    }
}

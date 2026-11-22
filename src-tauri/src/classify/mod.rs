pub mod ai;
pub mod embeddings;
pub mod normalize;
pub mod profile;
pub mod rules;
pub mod seed;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

use crate::ai::AiService;
use crate::error::{KoraError, Result};
use crate::features::embeddings::EmbeddingProvider;
use crate::storage::{
    AliasSource, AssetKind, AssetRef, Book, Category, CategoryAlias, CategoryAssignSource,
    CategorySuggestionView, ClassificationConfig, ClassificationProgress, ClassificationRunStatus,
    ClassificationStatus, NewAuditEntry, NewCategory, NewCategoryAlias, NewCategorySuggestion,
    ResourceType, Storage, SuggestionSource, SuggestionStatus,
};

use self::normalize::normalize_key;
use self::profile::BookProfile;

pub const CONFIG_KEY: &str = "classification.config";
pub const SEEDED_KEY: &str = "classification.seeded";
const RUN_MODEL: &str = "kora-classify-v1";

pub struct ClassificationService {
    storage: Storage,
    owner_account_id: String,
    ai: AiService,
    embeddings: Arc<dyn EmbeddingProvider>,
    running: AtomicBool,
    /// Serializa las llamadas a la IA local para no saturar Ollama cuando se
    /// importan varios libros a la vez.
    ai_gate: Semaphore,
    /// Evita lanzar dos clasificaciones simultáneas del mismo libro.
    in_flight: StdMutex<HashSet<String>>,
}

struct BatchGuard<'a>(&'a AtomicBool);

impl Drop for BatchGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct BookGuard<'a> {
    in_flight: &'a StdMutex<HashSet<String>>,
    book_id: String,
}

impl Drop for BookGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = self.in_flight.lock() {
            in_flight.remove(&self.book_id);
        }
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    category_id: String,
    score: f64,
    source: SuggestionSource,
    reason: String,
}

impl ClassificationService {
    pub fn new(
        storage: Storage,
        owner_account_id: String,
        ai: AiService,
        embeddings: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            storage,
            owner_account_id,
            ai,
            embeddings,
            running: AtomicBool::new(false),
            ai_gate: Semaphore::new(1),
            in_flight: StdMutex::new(HashSet::new()),
        }
    }

    // --- Configuración y semilla ------------------------------------------

    pub async fn config(&self) -> Result<ClassificationConfig> {
        let Some(raw) = self.storage.settings.get(CONFIG_KEY).await? else {
            return Ok(ClassificationConfig::default());
        };
        Ok(serde_json::from_str::<ClassificationConfig>(&raw)
            .map(ClassificationConfig::normalize)
            .unwrap_or_default())
    }

    pub async fn save_config(&self, config: ClassificationConfig) -> Result<ClassificationConfig> {
        let config = config.normalize();
        self.storage
            .settings
            .set(CONFIG_KEY, &serde_json::to_string(&config)?)
            .await?;
        Ok(config)
    }

    pub async fn is_seeded(&self) -> Result<bool> {
        Ok(self.storage.settings.get(SEEDED_KEY).await?.is_some())
    }

    /// Siembra el catálogo base una sola vez. En bibliotecas que ya tienen
    /// categorías no se crean categorías nuevas: solo se enlazan alias a las
    /// que coincidan por nombre.
    pub async fn ensure_seed(&self) -> Result<()> {
        if self.is_seeded().await? {
            return Ok(());
        }
        let existing = self
            .storage
            .categories
            .list_by_owner(&self.owner_account_id)
            .await?;
        if existing.is_empty() {
            self.apply_seed_categories().await?;
        } else {
            self.attach_seed_aliases(&existing).await?;
        }
        self.storage.settings.set(SEEDED_KEY, "1").await?;
        Ok(())
    }

    /// Crea las categorías base que falten y enlaza sus alias. Es idempotente
    /// y también se usa desde Ajustes para bibliotecas ya existentes.
    pub async fn apply_seed_categories(&self) -> Result<Vec<Category>> {
        let existing = self
            .storage
            .categories
            .list_by_owner(&self.owner_account_id)
            .await?;
        let mut by_name: HashMap<String, String> = existing
            .iter()
            .map(|category| (normalize_key(&category.name), category.id.clone()))
            .collect();
        let mut next_order = existing
            .iter()
            .map(|category| category.sort_order)
            .max()
            .unwrap_or(-1)
            + 1;
        let mut created = Vec::new();

        for seed in seed::SEED_CATEGORIES {
            let normalized = normalize_key(seed.name);
            let category_id = if let Some(category_id) = by_name.get(&normalized) {
                category_id.clone()
            } else {
                let category = self
                    .storage
                    .categories
                    .create(NewCategory {
                        name: seed.name.to_string(),
                        color: Some(seed.color.to_string()),
                        sort_order: next_order,
                        owner_account_id: self.owner_account_id.clone(),
                        origin_share_id: None,
                    })
                    .await?;
                next_order += 1;
                by_name.insert(normalized, category.id.clone());
                created.push(category.clone());
                category.id
            };

            for alias in seed.aliases {
                self.storage
                    .classification
                    .insert_alias(&NewCategoryAlias {
                        category_id: category_id.clone(),
                        alias: alias.to_string(),
                        alias_normalized: normalize_key(alias),
                        source: AliasSource::Seed,
                        owner_account_id: self.owner_account_id.clone(),
                        origin_share_id: None,
                    })
                    .await?;
            }
        }

        Ok(created)
    }

    async fn attach_seed_aliases(&self, categories: &[Category]) -> Result<()> {
        let by_name: HashMap<String, &str> = categories
            .iter()
            .map(|category| (normalize_key(&category.name), category.id.as_str()))
            .collect();
        for seed in seed::SEED_CATEGORIES {
            let Some(category_id) = by_name.get(&normalize_key(seed.name)) else {
                continue;
            };
            for alias in seed.aliases {
                self.storage
                    .classification
                    .insert_alias(&NewCategoryAlias {
                        category_id: (*category_id).to_string(),
                        alias: alias.to_string(),
                        alias_normalized: normalize_key(alias),
                        source: AliasSource::Seed,
                        owner_account_id: self.owner_account_id.clone(),
                        origin_share_id: None,
                    })
                    .await?;
            }
        }
        Ok(())
    }

    // --- Estado ------------------------------------------------------------

    pub async fn status(&self) -> Result<ClassificationStatus> {
        let config = self.config().await?;
        let ai = self.ai.status().await?;
        let pending_suggestions = self
            .storage
            .classification
            .count_pending(&self.owner_account_id)
            .await?;
        let classified_books = self
            .storage
            .classification
            .count_done(&self.owner_account_id)
            .await?;
        let total_books = self
            .storage
            .books
            .count_by_owner(&self.owner_account_id)
            .await?;

        Ok(ClassificationStatus {
            enabled: config.enabled,
            seeded: self.is_seeded().await?,
            ai_available: ai.available,
            embeddings_ready: self.embeddings.ready(),
            pending_suggestions,
            classified_books,
            total_books,
        })
    }

    // --- Clasificación -----------------------------------------------------

    fn claim_book(&self, book_id: &str) -> Option<BookGuard<'_>> {
        let mut in_flight = self.in_flight.lock().ok()?;
        if !in_flight.insert(book_id.to_string()) {
            return None;
        }
        Some(BookGuard {
            in_flight: &self.in_flight,
            book_id: book_id.to_string(),
        })
    }

    pub async fn classify_book(
        &self,
        app: Option<&AppHandle>,
        book: &Book,
        force: bool,
        manual: bool,
    ) -> Result<Vec<CategorySuggestionView>> {
        self.classify_book_inner(app, book, force, manual, None)
            .await
    }

    /// Clasifica los libros sin categorías (o todos si `force`) en segundo
    /// plano. Solo se ejecuta un lote a la vez.
    pub async fn classify_pending(&self, app: Option<&AppHandle>, force: bool) -> Result<usize> {
        let _guard =
            match self
                .running
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => BatchGuard(&self.running),
                Err(_) => return Ok(0),
            };

        let config = self.config().await?;
        if !config.enabled {
            return Ok(0);
        }
        let ai_available = if config.use_ai {
            self.ai
                .status()
                .await
                .map(|status| status.available)
                .unwrap_or(false)
        } else {
            false
        };

        let books = self
            .storage
            .books
            .list_by_owner(&self.owner_account_id)
            .await?;
        let mut processed = 0usize;

        for book in books {
            if !force {
                let categories = self.storage.book_categories.list_by_book(&book.id).await?;
                if !categories.is_empty() {
                    continue;
                }
                if self
                    .storage
                    .classification
                    .has_pending_for_book(&book.id)
                    .await?
                {
                    continue;
                }
            }

            match self
                .classify_book_inner(app, &book, force, false, Some(ai_available))
                .await
            {
                Ok(_) => processed += 1,
                Err(error) => {
                    tracing::warn!(book = %book.id, %error, "no se pudo clasificar el libro");
                }
            }
            tokio::task::yield_now().await;
        }

        Ok(processed)
    }

    async fn classify_book_inner(
        &self,
        app: Option<&AppHandle>,
        book: &Book,
        force: bool,
        manual: bool,
        ai_available: Option<bool>,
    ) -> Result<Vec<CategorySuggestionView>> {
        if book.owner_account_id != self.owner_account_id {
            return Err(KoraError::NotFound(format!("libro {}", book.id)));
        }

        let Some(_book_guard) = self.claim_book(&book.id) else {
            return self.pending_for_book(&book.id).await;
        };

        let config = self.config().await?;
        if !config.enabled && !manual {
            return self.pending_for_book(&book.id).await;
        }

        if !manual
            && !force
            && self
                .storage
                .book_categories
                .has_locked_category(&book.id)
                .await?
        {
            return self.pending_for_book(&book.id).await;
        }

        if !force
            && self
                .storage
                .classification
                .has_pending_for_book(&book.id)
                .await?
        {
            return self.pending_for_book(&book.id).await;
        }

        self.emit_progress(
            app,
            book,
            "metadata",
            10.0,
            "running",
            "Leyendo metadatos y texto",
        );

        let book_path = self.book_path(book);
        let audio_path = self.audio_path(book).await;
        let profile_book = book.clone();
        let profile = tokio::task::spawn_blocking(move || {
            profile::build_profile(&profile_book, book_path.as_deref(), audio_path.as_deref())
        })
        .await
        .map_err(|error| KoraError::Internal(format!("tarea de perfil: {error}")))?;

        let input_hash = profile.input_hash();
        if !force
            && let Some(run) = self.storage.classification.get_run(&book.id).await?
            && run.input_hash == input_hash
            && run.status == ClassificationRunStatus::Done
            && run.model.as_deref() == Some(RUN_MODEL)
        {
            return self.pending_for_book(&book.id).await;
        }

        self.storage
            .classification
            .upsert_run(
                &book.id,
                &input_hash,
                Some(RUN_MODEL),
                ClassificationRunStatus::Running,
                None,
                &self.owner_account_id,
            )
            .await?;

        let result = self
            .classify_profile(app, book, &profile, &config, manual, ai_available)
            .await;

        match result {
            Ok(()) => {
                self.storage
                    .classification
                    .upsert_run(
                        &book.id,
                        &input_hash,
                        Some(RUN_MODEL),
                        ClassificationRunStatus::Done,
                        None,
                        &self.owner_account_id,
                    )
                    .await?;
                self.emit_progress(app, book, "done", 100.0, "done", "Clasificación terminada");
                let pending = self.pending_for_book(&book.id).await?;
                self.emit_suggestions(app, &book.id, pending.len());
                Ok(pending)
            }
            Err(error) => {
                self.storage
                    .classification
                    .upsert_run(
                        &book.id,
                        &input_hash,
                        Some(RUN_MODEL),
                        ClassificationRunStatus::Failed,
                        Some(&error.to_string()),
                        &self.owner_account_id,
                    )
                    .await?;
                self.emit_progress(
                    app,
                    book,
                    "failed",
                    100.0,
                    "failed",
                    &format!("No se pudo clasificar: {error}"),
                );
                Err(error)
            }
        }
    }

    async fn classify_profile(
        &self,
        app: Option<&AppHandle>,
        book: &Book,
        profile: &BookProfile,
        config: &ClassificationConfig,
        manual: bool,
        ai_available: Option<bool>,
    ) -> Result<()> {
        let ai_available = match ai_available {
            Some(value) => value,
            None => {
                if config.use_ai {
                    self.ai
                        .status()
                        .await
                        .map(|status| status.available)
                        .unwrap_or(false)
                } else {
                    false
                }
            }
        };

        let mut categories = self
            .storage
            .categories
            .list_by_owner(&self.owner_account_id)
            .await?;
        if categories.is_empty() {
            self.ensure_seed().await?;
            categories = self
                .storage
                .categories
                .list_by_owner(&self.owner_account_id)
                .await?;
        }

        let aliases = self
            .storage
            .classification
            .list_aliases(&self.owner_account_id)
            .await?;
        let alias_map = alias_map(&aliases);

        let assigned: HashSet<String> = self
            .storage
            .book_categories
            .list_by_book(&book.id)
            .await?
            .into_iter()
            .map(|category| category.id)
            .collect();

        self.emit_progress(
            app,
            book,
            "rules",
            30.0,
            "running",
            "Comparando materias y alias",
        );

        let rule_matches = rules::match_metadata(profile, &aliases, &categories);
        let mut candidates: HashMap<String, Candidate> = HashMap::new();
        let mut exact_rule = false;
        for rule in rule_matches {
            if rule.score >= 0.99 {
                exact_rule = true;
            }
            if assigned.contains(&rule.category_id) {
                continue;
            }
            insert_candidate(
                &mut candidates,
                Candidate {
                    category_id: rule.category_id,
                    score: rule.score,
                    source: SuggestionSource::Metadata,
                    reason: rule.reason,
                },
            );
        }

        let mut embedding_candidates: Vec<(String, f64)> = Vec::new();
        if self.embeddings.ready() && (manual || !exact_rule) {
            self.emit_progress(
                app,
                book,
                "embeddings",
                50.0,
                "running",
                "Buscando similitud semántica",
            );
            match embeddings::rank_categories(
                self.embeddings.as_ref(),
                &profile.embedding_text(),
                &categories,
                &alias_map,
            )
            .await
            {
                Ok(ranked) => {
                    embedding_candidates = ranked
                        .iter()
                        .map(|candidate| (candidate.category_id.clone(), candidate.score))
                        .collect();
                    for candidate in ranked {
                        if candidate.score < f64::from(config.embedding_threshold)
                            || assigned.contains(&candidate.category_id)
                        {
                            continue;
                        }
                        let name = categories
                            .iter()
                            .find(|category| category.id == candidate.category_id)
                            .map(|category| category.name.as_str())
                            .unwrap_or("categoría");
                        insert_candidate(
                            &mut candidates,
                            Candidate {
                                category_id: candidate.category_id,
                                score: candidate.score,
                                source: SuggestionSource::Embedding,
                                reason: format!(
                                    "Similitud semántica del {:.0}% con «{name}»",
                                    candidate.score * 100.0
                                ),
                            },
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(book = %book.id, %error, "falló la clasificación por embeddings");
                }
            }
        }

        let top_embedding = embedding_candidates
            .first()
            .map(|(_, score)| *score)
            .unwrap_or(0.0);
        let second_embedding = embedding_candidates
            .get(1)
            .map(|(_, score)| *score)
            .unwrap_or(0.0);
        let strong_embedding = top_embedding >= 0.90 && (top_embedding - second_embedding) >= 0.06;

        let mut ai_new_category: Option<(String, f64, String)> = None;
        if config.use_ai && ai_available && (manual || (!exact_rule && !strong_embedding)) {
            self.emit_progress(
                app,
                book,
                "ai",
                70.0,
                "running",
                "Consultando el modelo local",
            );
            let _permit = self.ai_gate.acquire().await;
            match ai::classify(
                &self.ai,
                profile,
                &categories,
                &embedding_candidates,
                config.allow_new_categories,
            )
            .await
            {
                Ok(matches) => {
                    let mut ai_candidates: Vec<Candidate> = Vec::new();
                    let mut new_category = None;
                    for entry in matches {
                        if entry.confidence < f64::from(config.ai_threshold) {
                            continue;
                        }
                        if let Some(category_id) = entry.category_id {
                            if assigned.contains(&category_id) {
                                continue;
                            }
                            ai_candidates.push(Candidate {
                                category_id,
                                score: entry.confidence,
                                source: SuggestionSource::Ai,
                                reason: entry.reason,
                            });
                        } else if let Some(name) = entry.proposed_name {
                            let normalized = normalize_key(&name);
                            if !normalized.is_empty() {
                                new_category = Some((name, entry.confidence, entry.reason));
                            }
                        }
                    }
                    ai_candidates.sort_by(|a, b| {
                        b.score
                            .partial_cmp(&a.score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    ai_candidates.truncate(2);
                    if ai_candidates.is_empty() {
                        if let Some((name, score, reason)) = new_category {
                            ai_new_category = Some((name, score, reason));
                        }
                    } else {
                        for candidate in ai_candidates {
                            insert_candidate(&mut candidates, candidate);
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(book = %book.id, %error, "falló la clasificación con IA");
                }
            }
        }

        let mut candidates: Vec<Candidate> = candidates.into_values().collect();
        let has_existing_candidates = !candidates.is_empty();
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(config.max_suggestions as usize);

        self.emit_progress(
            app,
            book,
            "saving",
            90.0,
            "running",
            "Guardando sugerencias",
        );

        for candidate in candidates {
            self.storage
                .classification
                .insert_suggestion(&NewCategorySuggestion::for_category(
                    &book.id,
                    &candidate.category_id,
                    candidate.score,
                    candidate.source,
                    Some(candidate.reason),
                    &self.owner_account_id,
                    book.origin_share_id.as_deref(),
                ))
                .await?;
        }

        if let Some((name, score, reason)) = ai_new_category
            && !has_existing_candidates
        {
            let normalized = normalize_key(&name);
            if !normalized.is_empty()
                && !self
                    .storage
                    .categories
                    .list_by_owner(&self.owner_account_id)
                    .await?
                    .iter()
                    .any(|category| normalize_key(&category.name) == normalized)
            {
                self.storage
                    .classification
                    .insert_suggestion(&NewCategorySuggestion::for_new_category(
                        &book.id,
                        &name,
                        &normalized,
                        score,
                        SuggestionSource::Ai,
                        Some(reason),
                        &self.owner_account_id,
                        book.origin_share_id.as_deref(),
                    ))
                    .await?;
            }
        }

        Ok(())
    }

    // --- Sugerencias -------------------------------------------------------

    pub async fn pending_for_book(&self, book_id: &str) -> Result<Vec<CategorySuggestionView>> {
        self.storage
            .classification
            .list_pending_by_book(&self.owner_account_id, book_id)
            .await
    }

    pub async fn pending_all(&self) -> Result<Vec<CategorySuggestionView>> {
        self.storage
            .classification
            .list_pending_by_owner(&self.owner_account_id)
            .await
    }

    pub async fn accept_suggestion(
        &self,
        app: Option<&AppHandle>,
        suggestion_id: &str,
    ) -> Result<()> {
        let suggestion = self
            .storage
            .classification
            .get_suggestion(suggestion_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("sugerencia {suggestion_id}")))?;

        if suggestion.owner_account_id != self.owner_account_id {
            return Err(KoraError::NotFound(format!("sugerencia {suggestion_id}")));
        }
        if suggestion.status != SuggestionStatus::Pending {
            return Err(KoraError::InvalidInput(
                "la sugerencia ya fue resuelta".into(),
            ));
        }

        let (category, created) = if let Some(category_id) = &suggestion.category_id {
            let category = self
                .storage
                .categories
                .get(category_id)
                .await?
                .ok_or_else(|| KoraError::NotFound(format!("categoría {category_id}")))?;
            if category.owner_account_id != self.owner_account_id {
                return Err(KoraError::NotFound(format!("categoría {category_id}")));
            }
            (category, false)
        } else {
            let proposed_name = suggestion
                .proposed_name
                .clone()
                .ok_or_else(|| KoraError::InvalidInput("sugerencia sin categoría".into()))?;
            let normalized = suggestion
                .proposed_name_normalized
                .clone()
                .unwrap_or_else(|| normalize_key(&proposed_name));
            let existing = self
                .storage
                .categories
                .list_by_owner(&self.owner_account_id)
                .await?;
            if let Some(category) = existing
                .into_iter()
                .find(|category| normalize_key(&category.name) == normalized)
            {
                (category, false)
            } else {
                let next_order = self
                    .storage
                    .categories
                    .list_by_owner(&self.owner_account_id)
                    .await?
                    .iter()
                    .map(|category| category.sort_order)
                    .max()
                    .unwrap_or(-1)
                    + 1;
                let category = self
                    .storage
                    .categories
                    .create(NewCategory {
                        name: proposed_name.clone(),
                        color: Some(seed::color_for_name(&proposed_name).to_string()),
                        sort_order: next_order,
                        owner_account_id: self.owner_account_id.clone(),
                        origin_share_id: None,
                    })
                    .await?;
                self.storage
                    .audit
                    .append(NewAuditEntry {
                        actor_account_id: Some(self.owner_account_id.clone()),
                        action: "category.create".to_string(),
                        resource_type: Some(ResourceType::Category),
                        resource_id: Some(category.id.clone()),
                        metadata: serde_json::json!({ "name": category.name }).to_string(),
                    })
                    .await?;
                (category, true)
            }
        };

        let source = match suggestion.source {
            SuggestionSource::Metadata => CategoryAssignSource::Metadata,
            SuggestionSource::Embedding => CategoryAssignSource::Embedding,
            SuggestionSource::Ai => CategoryAssignSource::Ai,
        };

        self.storage
            .book_categories
            .assign_detailed(
                &suggestion.book_id,
                &category.id,
                &self.owner_account_id,
                suggestion.origin_share_id.as_deref(),
                source,
                Some(suggestion.score.clamp(0.0, 1.0)),
                false,
            )
            .await?;

        self.storage
            .audit
            .append(NewAuditEntry {
                actor_account_id: Some(self.owner_account_id.clone()),
                action: "book.category_assign".to_string(),
                resource_type: Some(ResourceType::Book),
                resource_id: Some(suggestion.book_id.clone()),
                metadata: serde_json::json!({
                    "categoryId": category.id,
                    "source": source,
                })
                .to_string(),
            })
            .await?;

        if created {
            let mut aliases = vec![category.name.clone()];
            if let Ok(Some(book)) = self.storage.books.get(&suggestion.book_id).await {
                let book_path = self.book_path(&book);
                let audio_path = self.audio_path(&book).await;
                let profile_book = book.clone();
                if let Ok(profile) = tokio::task::spawn_blocking(move || {
                    profile::build_profile(
                        &profile_book,
                        book_path.as_deref(),
                        audio_path.as_deref(),
                    )
                })
                .await
                {
                    aliases.extend(profile.learning_aliases());
                }
            }
            self.storage
                .classification
                .insert_learned_aliases(&category.id, &aliases, &self.owner_account_id)
                .await?;
        }

        self.storage
            .classification
            .resolve_suggestion(
                suggestion_id,
                &self.owner_account_id,
                SuggestionStatus::Accepted,
            )
            .await?;
        let pending = self.pending_for_book(&suggestion.book_id).await?;
        self.emit_suggestions(app, &suggestion.book_id, pending.len());
        Ok(())
    }

    pub async fn dismiss_suggestion(
        &self,
        app: Option<&AppHandle>,
        suggestion_id: &str,
    ) -> Result<()> {
        let suggestion = self
            .storage
            .classification
            .get_suggestion(suggestion_id)
            .await?
            .ok_or_else(|| KoraError::NotFound(format!("sugerencia {suggestion_id}")))?;
        if suggestion.owner_account_id != self.owner_account_id {
            return Err(KoraError::NotFound(format!("sugerencia {suggestion_id}")));
        }
        self.storage
            .classification
            .resolve_suggestion(
                suggestion_id,
                &self.owner_account_id,
                SuggestionStatus::Dismissed,
            )
            .await?;
        let pending = self.pending_for_book(&suggestion.book_id).await?;
        self.emit_suggestions(app, &suggestion.book_id, pending.len());
        Ok(())
    }

    pub async fn accept_all_for_book(
        &self,
        app: Option<&AppHandle>,
        book_id: &str,
    ) -> Result<usize> {
        let suggestions = self.pending_for_book(book_id).await?;
        let mut accepted = 0usize;
        for suggestion in suggestions {
            if self.accept_suggestion(app, &suggestion.id).await.is_ok() {
                accepted += 1;
            }
        }
        Ok(accepted)
    }

    // --- Rutas y utilidades -----------------------------------------------

    fn book_path(&self, book: &Book) -> Option<PathBuf> {
        for kind in [AssetKind::Book, AssetKind::GeneratedBook] {
            let reference = AssetRef {
                kind,
                hash: book.content_hash.clone(),
                extension: book.content_ext.clone(),
            };
            if let Ok(path) = self.storage.assets.path(&reference)
                && path.is_file()
            {
                return Some(path);
            }
        }
        None
    }

    async fn audio_path(&self, book: &Book) -> Option<PathBuf> {
        let audiobooks = self.storage.audiobooks.list_by_book(&book.id).await.ok()?;
        for audiobook in audiobooks {
            let (Some(hash), Some(extension)) = (audiobook.audio_hash, audiobook.audio_ext) else {
                continue;
            };
            let reference = AssetRef {
                kind: AssetKind::Audiobook,
                hash,
                extension,
            };
            if let Ok(path) = self.storage.assets.path(&reference)
                && path.is_file()
            {
                return Some(path);
            }
        }
        None
    }

    fn emit_progress(
        &self,
        app: Option<&AppHandle>,
        book: &Book,
        stage: &str,
        percent: f64,
        status: &str,
        message: &str,
    ) {
        let Some(app) = app else {
            return;
        };
        let _ = app.emit(
            "classification::progress",
            ClassificationProgress {
                book_id: book.id.clone(),
                title: book.title.clone(),
                stage: stage.to_string(),
                percent,
                status: status.to_string(),
                message: message.to_string(),
            },
        );
    }

    fn emit_suggestions(&self, app: Option<&AppHandle>, book_id: &str, pending: usize) {
        let Some(app) = app else {
            return;
        };
        let _ = app.emit(
            "classification::suggestions",
            serde_json::json!({ "bookId": book_id, "pending": pending }),
        );
    }
}

fn alias_map(aliases: &[CategoryAlias]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for alias in aliases {
        map.entry(alias.category_id.clone())
            .or_default()
            .push(alias.alias.clone());
    }
    map
}

fn insert_candidate(candidates: &mut HashMap<String, Candidate>, candidate: Candidate) {
    match candidates.get(&candidate.category_id) {
        Some(existing) if existing.score >= candidate.score => {}
        _ => {
            candidates.insert(candidate.category_id.clone(), candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::features::embeddings::TestEmbeddingProvider;
    use crate::storage::test_support::TestStorage;
    use crate::storage::{BookFormat, BookOrigin, NewBook};

    fn new_book(owner: &str, title: &str, description: &str) -> NewBook {
        NewBook {
            title: title.to_string(),
            author: Some("Autora".to_string()),
            format: BookFormat::Epub,
            content_hash: crate::storage::hash::sha256_bytes(title.as_bytes()),
            content_ext: "epub".to_string(),
            original_name: format!("{title}.epub"),
            cover_hash: None,
            cover_ext: None,
            size_bytes: 10,
            language: Some("es".to_string()),
            publisher: None,
            description: Some(description.to_string()),
            origin: BookOrigin::Imported,
            owner_account_id: owner.to_string(),
            origin_share_id: None,
            audio_only: false,
        }
    }

    #[tokio::test]
    async fn seeds_catalog_and_suggests_a_category() {
        let (test, identity) = TestStorage::with_identity().await;
        let owner = identity.account.id.clone();
        let ai = AiService::new(test.storage.clone()).expect("IA");
        let service = ClassificationService::new(
            test.storage.clone(),
            owner.clone(),
            ai,
            Arc::new(TestEmbeddingProvider),
        );

        service.ensure_seed().await.expect("sembrar");
        let seeded = test
            .storage
            .categories
            .list_by_owner(&owner)
            .await
            .expect("categorías");
        assert!(seeded.len() >= 20);
        service.ensure_seed().await.expect("sembrar de nuevo");
        assert_eq!(
            test.storage
                .categories
                .list_by_owner(&owner)
                .await
                .expect("categorías")
                .len(),
            seeded.len()
        );

        let config = ClassificationConfig {
            use_ai: false,
            embedding_threshold: 0.0,
            ..Default::default()
        };
        service.save_config(config).await.expect("configuración");

        let book = test
            .storage
            .books
            .create(new_book(&owner, "Dune", "Ciencia ficción"))
            .await
            .expect("libro");
        let suggestions = service
            .classify_book(None, &book, true, true)
            .await
            .expect("clasificar");
        let target = suggestions
            .iter()
            .find(|suggestion| suggestion.category_name.as_deref() == Some("Ciencia ficción"))
            .expect("sugerencia de ciencia ficción");

        service
            .accept_suggestion(None, &target.id)
            .await
            .expect("aceptar");
        let assigned = test
            .storage
            .book_categories
            .list_by_book(&book.id)
            .await
            .expect("asignadas");
        assert!(
            assigned
                .iter()
                .any(|category| category.name == "Ciencia ficción")
        );
        assert!(
            !test
                .storage
                .book_categories
                .has_locked_category(&book.id)
                .await
                .expect("bloqueo")
        );
    }

    #[tokio::test]
    async fn accepting_a_new_category_creates_it_and_learns_an_alias() {
        let (test, identity) = TestStorage::with_identity().await;
        let owner = identity.account.id.clone();
        let ai = AiService::new(test.storage.clone()).expect("IA");
        let service = ClassificationService::new(
            test.storage.clone(),
            owner.clone(),
            ai,
            Arc::new(TestEmbeddingProvider),
        );
        service.ensure_seed().await.expect("sembrar");

        let book = test
            .storage
            .books
            .create(new_book(
                &owner,
                "Libro raro",
                "Una historia inclasificable",
            ))
            .await
            .expect("libro");
        let suggestion = test
            .storage
            .classification
            .insert_suggestion(&NewCategorySuggestion::for_new_category(
                &book.id,
                "Fantasía épica",
                "fantasia epica",
                0.82,
                SuggestionSource::Ai,
                Some("Ninguna categoría existente encaja".to_string()),
                &owner,
                None,
            ))
            .await
            .expect("sugerencia");

        service
            .accept_suggestion(None, &suggestion.id)
            .await
            .expect("aceptar");

        let categories = test
            .storage
            .categories
            .list_by_owner(&owner)
            .await
            .expect("categorías");
        let created = categories
            .iter()
            .find(|category| category.name == "Fantasía épica")
            .expect("categoría creada");
        let assigned = test
            .storage
            .book_categories
            .list_by_book(&book.id)
            .await
            .expect("asignadas");
        assert!(assigned.iter().any(|category| category.id == created.id));
        let aliases = test
            .storage
            .classification
            .list_aliases(&owner)
            .await
            .expect("alias");
        assert!(aliases.iter().any(|alias| {
            alias.category_id == created.id && alias.alias_normalized == "fantasia epica"
        }));
    }
}

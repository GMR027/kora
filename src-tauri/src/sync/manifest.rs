use std::collections::HashSet;

use serde_json::json;

use crate::annotations::note_audio_path;
use crate::error::{KoraError, Result};
use crate::storage::{Audiobook, Book, ResourceType, Storage};

use super::crdt::CrdtStore;
use super::protocol::{Manifest, ResourceRef, SyncResource};

/// Construye el manifiesto local aplicando el filtro de colecciones.
pub async fn build(
    storage: &Storage,
    crdt: &CrdtStore,
    owner_account_id: &str,
    account_hint: &str,
    device_id: &str,
) -> Result<Manifest> {
    build_inner(
        storage,
        crdt,
        owner_account_id,
        account_hint,
        device_id,
        true,
    )
    .await
}

/// Manifiesto sin el filtro de colecciones. Los shares explícitos no deben
/// quedar ocultos por la selección de sync del dueño.
pub async fn build_for_share(
    storage: &Storage,
    crdt: &CrdtStore,
    owner_account_id: &str,
    account_hint: &str,
    device_id: &str,
) -> Result<Manifest> {
    build_inner(
        storage,
        crdt,
        owner_account_id,
        account_hint,
        device_id,
        false,
    )
    .await
}

async fn build_inner(
    storage: &Storage,
    crdt: &CrdtStore,
    owner_account_id: &str,
    account_hint: &str,
    device_id: &str,
    apply_collection_filter: bool,
) -> Result<Manifest> {
    let mut resources = Vec::new();
    let mode = storage
        .settings
        .get(&format!("sync.collections.mode.{device_id}"))
        .await?;
    let has_selection = apply_collection_filter
        && (mode.as_deref() == Some("selected")
            || storage
                .sync_collections
                .has_selection(owner_account_id, device_id)
                .await?);
    let enabled = storage
        .sync_collections
        .enabled_category_ids(owner_account_id, device_id)
        .await?;
    let enabled: HashSet<String> = enabled.into_iter().collect();

    let all_books = if apply_collection_filter {
        storage.books.list_by_owner(owner_account_id).await?
    } else {
        storage.books.list_all_by_owner(owner_account_id).await?
    };
    let mut books = Vec::new();
    let mut referenced_categories = HashSet::new();

    for book in all_books {
        let categories = storage.book_categories.list_by_book(&book.id).await?;
        let allowed = !has_selection
            || categories
                .iter()
                .any(|category| enabled.contains(&category.id));
        if !allowed {
            continue;
        }
        for category in &categories {
            referenced_categories.insert(category.id.clone());
        }
        books.push((book, categories));
    }

    let all_categories = storage.categories.list_by_owner(owner_account_id).await?;
    for category in all_categories {
        let allowed = !has_selection
            || referenced_categories.contains(&category.id)
            || enabled.contains(&category.id);
        if !allowed {
            continue;
        }
        let payload = json!({ "category": category });
        resources.push(SyncResource {
            reference: ResourceRef {
                kind: ResourceType::Category,
                id: category.id.clone(),
            },
            hash: hash_json(&payload)?,
            updated_at: category.updated_at,
            payload,
        });
    }

    for (book, categories) in &books {
        let mut category_ids: Vec<String> = categories
            .iter()
            .map(|category| category.id.clone())
            .collect();
        category_ids.sort();
        let payload = json!({ "book": book, "categoryIds": category_ids });
        resources.push(SyncResource {
            reference: ResourceRef {
                kind: ResourceType::Book,
                id: book.id.clone(),
            },
            hash: hash_json(&payload)?,
            updated_at: book.updated_at,
            payload,
        });

        if let Some(progress) = storage.progress.get(&book.id).await? {
            let payload = json!({ "progress": progress });
            resources.push(SyncResource {
                reference: ResourceRef {
                    kind: ResourceType::ReadingProgress,
                    id: book.id.clone(),
                },
                hash: hash_json(&payload)?,
                updated_at: progress.updated_at,
                payload,
            });
        }

        for annotation in storage
            .annotations
            .list_by_book_including_deleted(&book.id)
            .await?
        {
            if let Some(document) = crdt
                .ensure(ResourceType::Annotation, &annotation.id)
                .await?
            {
                resources.push(SyncResource {
                    reference: ResourceRef {
                        kind: ResourceType::Annotation,
                        id: annotation.id.clone(),
                    },
                    hash: document.hash,
                    updated_at: annotation.updated_at,
                    payload: json!({ "bookId": annotation.book_id }),
                });
            }
        }

        for extract in storage
            .extracts
            .list_by_book_including_deleted(&book.id)
            .await?
        {
            if let Some(document) = crdt.ensure(ResourceType::Extract, &extract.id).await? {
                resources.push(SyncResource {
                    reference: ResourceRef {
                        kind: ResourceType::Extract,
                        id: extract.id.clone(),
                    },
                    hash: document.hash,
                    updated_at: extract.updated_at,
                    payload: json!({ "bookId": extract.book_id }),
                });
            }
        }

        for reaction in storage
            .reactions
            .list_by_book_including_deleted(&book.id)
            .await?
        {
            if let Some(document) = crdt.ensure(ResourceType::Reaction, &reaction.id).await? {
                resources.push(SyncResource {
                    reference: ResourceRef {
                        kind: ResourceType::Reaction,
                        id: reaction.id.clone(),
                    },
                    hash: document.hash,
                    updated_at: reaction.updated_at,
                    payload: json!({ "bookId": reaction.book_id }),
                });
            }
        }

        for mut note in storage
            .voice_notes
            .list_by_book_including_deleted(&book.id)
            .await?
        {
            let audio_path = note_audio_path(storage.layout(), &note.id);
            let audio_hash = match note.audio_hash.clone() {
                Some(hash) => Some(hash),
                None if audio_path.is_file() => {
                    let bytes = std::fs::read(&audio_path)?;
                    let hash = crate::storage::hash::sha256_bytes(&bytes);
                    storage.voice_notes.set_audio_hash(&note.id, &hash).await?;
                    note.audio_hash = Some(hash.clone());
                    Some(hash)
                }
                None => None,
            };
            let audio_size = std::fs::metadata(&audio_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            if let Some(document) = crdt.ensure(ResourceType::VoiceNote, &note.id).await? {
                resources.push(SyncResource {
                    reference: ResourceRef {
                        kind: ResourceType::VoiceNote,
                        id: note.id.clone(),
                    },
                    hash: document.hash,
                    updated_at: note.updated_at,
                    payload: json!({
                        "voiceNote": note,
                        "audioHash": audio_hash,
                        "audioSize": audio_size,
                    }),
                });
            }
        }
    }

    let allowed_books: HashSet<String> = books.iter().map(|(book, _)| book.id.clone()).collect();
    for note in storage
        .notes
        .list_by_owner_including_deleted(owner_account_id)
        .await?
    {
        if note
            .book_id
            .as_ref()
            .is_some_and(|book_id| !allowed_books.contains(book_id))
        {
            continue;
        }
        if let Some(document) = crdt.ensure(ResourceType::Note, &note.id).await? {
            resources.push(SyncResource {
                reference: ResourceRef {
                    kind: ResourceType::Note,
                    id: note.id.clone(),
                },
                hash: document.hash,
                updated_at: note.updated_at,
                payload: json!({ "bookId": note.book_id }),
            });
        }
    }

    Ok(Manifest {
        device_id: device_id.to_string(),
        account_hint: account_hint.to_string(),
        resources,
    })
}

fn hash_json(value: &serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(crate::storage::hash::sha256_bytes(&bytes))
}

/// Payload mínimo para los libros que viaja en el manifiesto.
pub fn book_from_payload(payload: &serde_json::Value) -> Result<(Book, Vec<String>)> {
    let book: Book = serde_json::from_value(
        payload
            .get("book")
            .cloned()
            .ok_or_else(|| KoraError::InvalidInput("libro sin metadatos".into()))?,
    )?;
    let category_ids = payload
        .get("categoryIds")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    Ok((book, category_ids))
}

/// Recurso de un audiolibro para shares explícitos. No se incluye en la sync
/// normal porque el M4B puede pesar cientos de MB y solo debe viajar cuando el
/// usuario comparte ese audiolibro.
pub fn audiobook_resource(audiobook: &Audiobook) -> Result<SyncResource> {
    let payload = json!({ "audiobook": audiobook });
    Ok(SyncResource {
        reference: ResourceRef {
            kind: ResourceType::Audiobook,
            id: audiobook.id.clone(),
        },
        hash: hash_json(&payload)?,
        updated_at: audiobook.updated_at,
        payload,
    })
}

pub fn audiobook_from_payload(payload: &serde_json::Value) -> Result<Audiobook> {
    Ok(serde_json::from_value(
        payload
            .get("audiobook")
            .cloned()
            .ok_or_else(|| KoraError::InvalidInput("audiolibro sin metadatos".into()))?,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{AudiobookOrigin, AudiobookStatus};
    use chrono::Utc;

    #[test]
    fn audiobook_payload_round_trips() {
        let audiobook = Audiobook {
            id: "audio-1".into(),
            book_id: "book-1".into(),
            voice_id: Some("voice-1".into()),
            status: AudiobookStatus::Done,
            manifest_hash: Some("manifest".into()),
            manifest_ext: Some("json".into()),
            audio_hash: Some("audio".into()),
            audio_ext: Some("m4b".into()),
            scope: Some("full_book".into()),
            origin: AudiobookOrigin::Generated,
            duration_ms: Some(60_000),
            narration: None,
            cast: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            owner_account_id: "owner".into(),
            origin_share_id: None,
        };

        let resource = audiobook_resource(&audiobook).expect("recurso");
        assert_eq!(resource.reference.id, "audio-1");
        assert_eq!(resource.reference.kind, ResourceType::Audiobook);
        let parsed = audiobook_from_payload(&resource.payload).expect("payload");
        assert_eq!(parsed.id, audiobook.id);
        assert_eq!(parsed.book_id, audiobook.book_id);
        assert_eq!(parsed.audio_hash, audiobook.audio_hash);
    }
}

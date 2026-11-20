use crate::storage::AssetKind;
use crate::storage::model::{
    AnnotationKind, AudiobookStatus, BookFormat, NewAchievement, NewAnnotation, NewAudiobook,
    NewBook, NewCategory, NewDiaryEntry, NewExtract, NewNote, NewReaderSettings,
    NewReadingProgress, NewShare, NewVoice, ResourceType, SyncState,
};
use crate::storage::test_support::TestStorage;

fn new_book(owner: &str, hash: &str, title: &str) -> NewBook {
    NewBook {
        title: title.to_string(),
        author: Some("Autora".to_string()),
        format: BookFormat::Epub,
        content_hash: hash.to_string(),
        content_ext: "epub".to_string(),
        original_name: format!("{title}.epub"),
        cover_hash: None,
        cover_ext: None,
        size_bytes: 1234,
        language: Some("es".to_string()),
        publisher: None,
        description: None,
        owner_account_id: owner.to_string(),
        origin_share_id: None,
    }
}

fn hash(seed: &str) -> String {
    crate::storage::hash::sha256_bytes(seed.as_bytes())
}

#[tokio::test]
async fn creates_local_identity_only_once() {
    let test = TestStorage::new().await;

    let first = test
        .storage
        .ensure_local_identity("Biblioteca", "Portátil")
        .await
        .expect("identidad");
    let second = test
        .storage
        .ensure_local_identity("Otro nombre", "Otro equipo")
        .await
        .expect("identidad idempotente");

    assert_eq!(first.account.id, second.account.id);
    assert_eq!(first.device.id, second.device.id);
    assert_eq!(second.account.display_name, "Biblioteca");
    assert_eq!(
        test.storage.accounts.list().await.expect("cuentas").len(),
        1
    );
    assert_eq!(
        test.storage
            .devices
            .count_by_account(&first.account.id)
            .await
            .expect("dispositivos"),
        1
    );
}

#[tokio::test]
async fn creates_reads_and_deletes_books() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("libro"), "El libro"))
        .await
        .expect("crear libro");

    assert_eq!(book.title, "El libro");
    assert_eq!(book.format, BookFormat::Epub);
    assert_eq!(book.owner_account_id, owner);

    let found = test
        .storage
        .books
        .find_by_hash(&owner, &hash("libro"))
        .await
        .expect("buscar")
        .expect("libro por hash");
    assert_eq!(found.id, book.id);

    let summaries = test
        .storage
        .books
        .list_summaries(&owner)
        .await
        .expect("listar");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].progress, 0.0);

    let owner_row = test
        .storage
        .resource_owners
        .get(ResourceType::Book, &book.id)
        .await
        .expect("propietario")
        .expect("fila de propietario");
    assert_eq!(owner_row.owner_account_id, owner);

    test.storage.books.delete(&book.id).await.expect("borrar");
    assert!(
        test.storage
            .books
            .get(&book.id)
            .await
            .expect("consultar")
            .is_none()
    );
    assert!(
        test.storage
            .resource_owners
            .get(ResourceType::Book, &book.id)
            .await
            .expect("propietario")
            .is_none()
    );
    assert!(test.storage.books.delete(&book.id).await.is_err());
}

#[tokio::test]
async fn duplicate_hash_is_rejected_per_owner() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = new_book(&owner, &hash("mismo"), "Uno");

    test.storage
        .books
        .create(book.clone())
        .await
        .expect("primer libro");
    assert!(test.storage.books.create(book).await.is_err());

    let other = test
        .storage
        .accounts
        .create(crate::storage::NewAccount {
            display_name: "Otra".to_string(),
            email: None,
            is_local: false,
        })
        .await
        .expect("otra cuenta");

    test.storage
        .books
        .create(new_book(&other.id, &hash("mismo"), "Dos"))
        .await
        .expect("mismo hash, otra cuenta");
}

#[tokio::test]
async fn reading_progress_upserts_and_cascades() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("progreso"), "Progreso"))
        .await
        .expect("libro");

    test.storage
        .progress
        .upsert(NewReadingProgress {
            book_id: book.id.clone(),
            locator: "epubcfi(/6/2)".to_string(),
            percent: 0.25,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("progreso");

    test.storage
        .progress
        .upsert(NewReadingProgress {
            book_id: book.id.clone(),
            locator: "epubcfi(/6/8)".to_string(),
            percent: 0.75,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("actualizar progreso");

    let progress = test
        .storage
        .progress
        .get(&book.id)
        .await
        .expect("leer")
        .expect("progreso");
    assert_eq!(progress.percent, 0.75);
    assert_eq!(progress.locator, "epubcfi(/6/8)");

    let summaries = test
        .storage
        .books
        .list_summaries(&owner)
        .await
        .expect("listar");
    assert!((summaries[0].progress - 0.75).abs() < f64::EPSILON);

    test.storage.books.delete(&book.id).await.expect("borrar");
    assert!(
        test.storage
            .progress
            .get(&book.id)
            .await
            .expect("leer")
            .is_none()
    );
}

#[tokio::test]
async fn annotations_extracts_and_notes_crud() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("anotaciones"), "Anotado"))
        .await
        .expect("libro");

    let annotation = test
        .storage
        .annotations
        .create(NewAnnotation {
            book_id: book.id.clone(),
            kind: AnnotationKind::Highlight,
            locator: "epubcfi(/6/4)".to_string(),
            chapter_index: Some(0),
            offset_start: Some(3),
            offset_end: Some(8),
            text: Some("texto".to_string()),
            note: None,
            color: Some("#ffcc00".to_string()),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("anotación");

    assert_eq!(annotation.chapter_index, Some(0));
    assert_eq!(annotation.offset_start, Some(3));
    assert_eq!(annotation.offset_end, Some(8));

    let mut updated = annotation.clone();
    updated.note = Some("nota".to_string());
    test.storage
        .annotations
        .update(&updated)
        .await
        .expect("actualizar anotación");
    assert_eq!(
        test.storage
            .annotations
            .get(&annotation.id)
            .await
            .expect("leer")
            .expect("anotación")
            .note
            .as_deref(),
        Some("nota")
    );

    let extract = test
        .storage
        .extracts
        .create(NewExtract {
            book_id: book.id.clone(),
            locator: "epubcfi(/6/6)".to_string(),
            text: "fragmento".to_string(),
            note: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("extracto");

    let note = test
        .storage
        .notes
        .create(NewNote {
            book_id: Some(book.id.clone()),
            title: Some("Idea".to_string()),
            body: "cuerpo".to_string(),
            audio_hash: None,
            audio_ext: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("nota");

    assert_eq!(
        test.storage
            .annotations
            .list_by_book(&book.id)
            .await
            .expect("anotaciones")
            .len(),
        1
    );
    assert_eq!(
        test.storage
            .extracts
            .list_by_book(&book.id)
            .await
            .expect("extractos")
            .len(),
        1
    );
    assert_eq!(
        test.storage
            .notes
            .list_by_book(&book.id)
            .await
            .expect("notas")
            .len(),
        1
    );

    test.storage.books.delete(&book.id).await.expect("borrar");

    assert!(
        test.storage
            .annotations
            .get(&annotation.id)
            .await
            .expect("leer")
            .is_none()
    );
    assert!(
        test.storage
            .extracts
            .get(&extract.id)
            .await
            .expect("leer")
            .is_none()
    );
    // `notes.book_id` usa ON DELETE SET NULL: la nota sobrevive.
    let orphan = test
        .storage
        .notes
        .get(&note.id)
        .await
        .expect("leer")
        .expect("nota huérfana");
    assert!(orphan.book_id.is_none());
    assert!(
        test.storage
            .resource_owners
            .get(ResourceType::Annotation, &annotation.id)
            .await
            .expect("propietario anotación")
            .is_none()
    );
    assert!(
        test.storage
            .resource_owners
            .get(ResourceType::Extract, &extract.id)
            .await
            .expect("propietario extracto")
            .is_none()
    );

    test.storage
        .notes
        .delete(&note.id)
        .await
        .expect("borrar nota");
    assert!(
        test.storage
            .resource_owners
            .get(ResourceType::Note, &note.id)
            .await
            .expect("propietario")
            .is_none()
    );
}

#[tokio::test]
async fn reader_settings_are_saved_per_book_and_cascade() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("ajustes"), "Ajustes"))
        .await
        .expect("libro");

    assert!(
        test.storage
            .reader_settings
            .get(&book.id)
            .await
            .expect("leer ajustes")
            .is_none()
    );

    let saved = test
        .storage
        .reader_settings
        .save(NewReaderSettings {
            book_id: book.id.clone(),
            font_size: 22.0,
            line_height: 1.9,
            margin: 48,
            font_family: "sans".to_string(),
            theme: "sepia".to_string(),
            texture: "paper".to_string(),
            mode: "paged".to_string(),
            zoom: 1.2,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("guardar ajustes");

    assert_eq!(saved.font_size, 22.0);
    assert_eq!(saved.theme, "sepia");
    assert_eq!(saved.mode, "paged");

    let updated = test
        .storage
        .reader_settings
        .save(NewReaderSettings {
            book_id: book.id.clone(),
            font_size: 16.0,
            line_height: 1.5,
            margin: 20,
            font_family: "mono".to_string(),
            theme: "dark".to_string(),
            texture: "grid".to_string(),
            mode: "scroll".to_string(),
            zoom: 0.8,
            owner_account_id: owner,
            origin_share_id: None,
        })
        .await
        .expect("actualizar ajustes");

    assert_eq!(updated.font_size, 16.0);
    assert_eq!(updated.font_family, "mono");

    test.storage.books.delete(&book.id).await.expect("borrar");
    assert!(
        test.storage
            .reader_settings
            .get(&book.id)
            .await
            .expect("leer ajustes")
            .is_none()
    );
}

#[tokio::test]
async fn categories_assign_and_filter_books() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("categoria"), "Categorizado"))
        .await
        .expect("libro");

    let category = test
        .storage
        .categories
        .create(NewCategory {
            name: "Ensayo".to_string(),
            color: Some("#123456".to_string()),
            sort_order: 1,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("categoría");

    test.storage
        .book_categories
        .assign(&book.id, &category.id, &owner, None)
        .await
        .expect("asignar");
    test.storage
        .book_categories
        .assign(&book.id, &category.id, &owner, None)
        .await
        .expect("asignar de nuevo");

    assert_eq!(
        test.storage
            .book_categories
            .list_by_book(&book.id)
            .await
            .expect("categorías del libro")
            .len(),
        1
    );
    assert_eq!(
        test.storage
            .book_categories
            .list_by_category(&category.id)
            .await
            .expect("libros de la categoría")
            .len(),
        1
    );

    test.storage
        .book_categories
        .unassign(&book.id, &category.id)
        .await
        .expect("quitar");
    assert!(
        test.storage
            .book_categories
            .list_by_book(&book.id)
            .await
            .expect("categorías")
            .is_empty()
    );

    test.storage
        .categories
        .delete(&category.id)
        .await
        .expect("borrar categoría");
    assert!(
        test.storage
            .resource_owners
            .get(ResourceType::Category, &category.id)
            .await
            .expect("propietario")
            .is_none()
    );
}

#[tokio::test]
async fn voices_audiobooks_diary_and_achievements() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("voz"), "Para escuchar"))
        .await
        .expect("libro");

    let voice = test
        .storage
        .voices
        .create(NewVoice {
            name: "Narradora".to_string(),
            engine: "piper".to_string(),
            language: "es".to_string(),
            profile_hash: None,
            profile_ext: None,
            sample_hash: None,
            sample_ext: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("voz");

    let audiobook = test
        .storage
        .audiobooks
        .create(NewAudiobook {
            book_id: book.id.clone(),
            voice_id: Some(voice.id.clone()),
            status: AudiobookStatus::Pending,
            manifest_hash: None,
            manifest_ext: None,
            duration_ms: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("audiolibro");

    test.storage
        .audiobooks
        .update(&crate::storage::Audiobook {
            status: AudiobookStatus::Done,
            duration_ms: Some(60_000),
            ..audiobook.clone()
        })
        .await
        .expect("actualizar audiolibro");

    assert_eq!(
        test.storage
            .audiobooks
            .list_by_book(&book.id)
            .await
            .expect("audiolibros")[0]
            .status,
        AudiobookStatus::Done
    );

    let entry = test
        .storage
        .diary
        .create(NewDiaryEntry {
            book_id: Some(book.id.clone()),
            entry_date: "2026-11-19".to_string(),
            body: "Buen día de lectura".to_string(),
            mood: Some("tranquilo".to_string()),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("diario");

    assert_eq!(
        test.storage
            .diary
            .list_by_owner(&owner)
            .await
            .expect("diario")
            .len(),
        1
    );

    let achievement = test
        .storage
        .achievements
        .upsert(NewAchievement {
            key: "first_book".to_string(),
            title: "Primer libro".to_string(),
            description: None,
            progress: 0.5,
            unlocked_at: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("logro");

    let again = test
        .storage
        .achievements
        .upsert(NewAchievement {
            key: "first_book".to_string(),
            title: "Primer libro".to_string(),
            description: None,
            progress: 1.0,
            unlocked_at: Some(chrono::Utc::now()),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("actualizar logro");

    assert_eq!(achievement.id, again.id);
    assert_eq!(again.progress, 1.0);

    // Borrar el libro deja la entrada de diario sin libro, pero viva.
    test.storage.books.delete(&book.id).await.expect("borrar");
    let entry = test
        .storage
        .diary
        .get(&entry.id)
        .await
        .expect("diario")
        .expect("entrada");
    assert!(entry.book_id.is_none());
    assert!(
        test.storage
            .audiobooks
            .get(&audiobook.id)
            .await
            .expect("audiolibro")
            .is_none()
    );
}

#[tokio::test]
async fn shares_and_sync_state_are_tracked() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("compartido"), "Compartido"))
        .await
        .expect("libro");

    let share = test
        .storage
        .shares
        .create(NewShare {
            owner_account_id: owner.clone(),
            resource_type: ResourceType::Book,
            resource_id: book.id.clone(),
        })
        .await
        .expect("share");

    let key = test
        .storage
        .share_keys
        .create(crate::storage::NewShareKey {
            share_id: share.id.clone(),
            recipient_account_id: None,
            wrapped_key: "base64".to_string(),
            expires_at: None,
        })
        .await
        .expect("clave");
    assert_eq!(
        test.storage
            .share_keys
            .list_by_share(&share.id)
            .await
            .expect("claves")
            .len(),
        1
    );

    test.storage
        .share_keys
        .revoke(&key.id)
        .await
        .expect("revocar clave");
    test.storage
        .shares
        .revoke(&share.id)
        .await
        .expect("revocar share");
    assert!(
        test.storage
            .share_keys
            .get(&key.id)
            .await
            .expect("clave")
            .expect("clave")
            .revoked_at
            .is_some()
    );

    test.storage
        .sync_state
        .upsert(crate::storage::NewSyncEntry {
            resource_type: ResourceType::Book,
            resource_id: book.id.clone(),
            peer_device_id: "peer-1".to_string(),
            version: 1,
            state: SyncState::Pending,
        })
        .await
        .expect("sync");

    test.storage
        .sync_state
        .set_state(ResourceType::Book, &book.id, "peer-1", SyncState::Synced)
        .await
        .expect("estado");

    assert_eq!(
        test.storage
            .sync_state
            .get(ResourceType::Book, &book.id, "peer-1")
            .await
            .expect("leer")
            .expect("estado")
            .state,
        SyncState::Synced
    );

    test.storage.books.delete(&book.id).await.expect("borrar");
    assert!(
        test.storage
            .sync_state
            .list_by_resource(ResourceType::Book, &book.id)
            .await
            .expect("sync")
            .is_empty()
    );
    // El share se revoca en lugar de borrarse para conservar la auditoría.
    let share = test
        .storage
        .shares
        .get(&share.id)
        .await
        .expect("share")
        .expect("share");
    assert!(share.revoked_at.is_some());
}

#[tokio::test]
async fn settings_and_audit_round_trip() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    test.storage
        .settings
        .set("theme", "sepia")
        .await
        .expect("guardar ajuste");
    assert_eq!(
        test.storage.settings.get("theme").await.expect("leer"),
        Some("sepia".to_string())
    );

    let entry = test
        .storage
        .audit
        .append(crate::storage::NewAuditEntry {
            actor_account_id: Some(owner.clone()),
            action: "book.import".to_string(),
            resource_type: Some(ResourceType::Book),
            resource_id: Some("book-1".to_string()),
            metadata: "{}".to_string(),
        })
        .await
        .expect("auditoría");

    assert_eq!(
        test.storage
            .audit
            .list_by_resource(ResourceType::Book, "book-1", 10)
            .await
            .expect("listar")
            .len(),
        1
    );
    assert_eq!(entry.actor_account_id.as_deref(), Some(owner.as_str()));
}

#[tokio::test]
async fn import_flow_stores_assets_and_replaces_cover() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    let source = test.temp.path().join("Mi libro.epub");
    std::fs::write(&source, b"contenido epub").expect("escribir libro");

    let asset = test
        .storage
        .assets
        .put_file(AssetKind::Book, &source, "epub")
        .expect("guardar libro");

    let book = test
        .storage
        .books
        .create(NewBook {
            title: "Mi libro".to_string(),
            author: None,
            format: BookFormat::Epub,
            content_hash: asset.hash.clone(),
            content_ext: "epub".to_string(),
            original_name: "Mi libro.epub".to_string(),
            cover_hash: None,
            cover_ext: None,
            size_bytes: 13,
            language: None,
            publisher: None,
            description: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("libro");

    assert!(test.storage.assets.exists(&asset).expect("existe asset"));

    // Guardar el mismo archivo de nuevo reutiliza el asset.
    let again = test
        .storage
        .assets
        .put_file(AssetKind::Book, &source, "epub")
        .expect("deduplicar");
    assert_eq!(asset, again);

    let cover_source = test.temp.path().join("portada.png");
    std::fs::write(&cover_source, b"png").expect("escribir portada");
    let cover = test
        .storage
        .assets
        .put_file(AssetKind::Cover, &cover_source, "png")
        .expect("guardar portada");

    test.storage
        .books
        .replace_cover(&book.id, &cover.hash, &cover.extension)
        .await
        .expect("sustituir portada");

    let updated = test
        .storage
        .books
        .get(&book.id)
        .await
        .expect("leer")
        .expect("libro");
    assert_eq!(updated.cover_hash.as_deref(), Some(cover.hash.as_str()));
    assert!(
        test.storage
            .asset_is_referenced(AssetKind::Cover, &cover.hash)
            .await
            .expect("referencia")
    );

    test.storage.assets.remove(&cover).expect("borrar portada");
    assert!(!test.storage.assets.exists(&cover).expect("existe"));
}

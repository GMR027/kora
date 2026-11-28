use crate::storage::AssetKind;
use crate::storage::model::{
    AnnotationKind, AudiobookDelivery, AudiobookOrigin, AudiobookStatus, BookFormat, BookOrigin,
    CategoryAssignSource, NewAchievement, NewAnnotation, NewAudiobook, NewAudiobookAlbum, NewBook,
    NewCategory, NewCategorySuggestion, NewCharacterVoice, NewDiaryEntry, NewExtract, NewNote,
    NewPracticeAttempt, NewReaction, NewReaderSettings, NewReadingProgress, NewShare,
    NewTtsPreferences, NewTtsSequence, NewVoice, NewVoiceNote, OffsetUnit, ReactionKind,
    ResourceType, SuggestionSource, SuggestionStatus, SyncState, VoiceKind,
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
        origin: BookOrigin::Imported,
        owner_account_id: owner.to_string(),
        origin_share_id: None,
        audio_only: false,
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
async fn account_avatar_fields_round_trip() {
    let (test, identity) = TestStorage::with_identity().await;
    let mut account = test
        .storage
        .accounts
        .get(&identity.account.id)
        .await
        .expect("cuenta")
        .expect("cuenta local");
    assert!(account.avatar_hash.is_none());

    let avatar_hash = "a".repeat(64);
    account.avatar_hash = Some(avatar_hash.clone());
    account.avatar_ext = Some("png".to_string());
    test.storage
        .accounts
        .update(&account)
        .await
        .expect("guardar avatar");

    let stored = test
        .storage
        .accounts
        .get(&identity.account.id)
        .await
        .expect("leer cuenta")
        .expect("cuenta local");
    assert_eq!(stored.avatar_hash.as_deref(), Some(avatar_hash.as_str()));
    assert_eq!(stored.avatar_ext.as_deref(), Some("png"));

    account.avatar_hash = None;
    account.avatar_ext = None;
    test.storage
        .accounts
        .update(&account)
        .await
        .expect("quitar avatar");
    let cleared = test
        .storage
        .accounts
        .get(&identity.account.id)
        .await
        .expect("leer cuenta")
        .expect("cuenta local");
    assert!(cleared.avatar_hash.is_none());
    assert!(cleared.avatar_ext.is_none());
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
async fn repairs_book_descriptions_with_embedded_html() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    let mut dirty = new_book(&owner, &hash("descripcion"), "Con HTML");
    dirty.description = Some("<p class=\"sinopsis\">Hola &amp; adiós</p>".to_string());

    let book = test.storage.books.create(dirty).await.expect("crear libro");

    let repaired = test
        .storage
        .books
        .repair_descriptions()
        .await
        .expect("reparar descripciones");
    assert_eq!(repaired, 1);

    let found = test
        .storage
        .books
        .get(&book.id)
        .await
        .expect("leer libro")
        .expect("libro existente");
    assert_eq!(found.description.as_deref(), Some("Hola & adiós"));

    let again = test
        .storage
        .books
        .repair_descriptions()
        .await
        .expect("reparar de nuevo");
    assert_eq!(again, 0);
}

#[tokio::test]
async fn book_trash_hides_but_keeps_the_record_restorable() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("papelera"), "En papelera"))
        .await
        .expect("crear libro");

    test.storage
        .books
        .set_deleted(&book.id, true)
        .await
        .expect("mover a papelera");

    assert!(
        test.storage
            .books
            .list_summaries(&owner)
            .await
            .expect("listar activos")
            .is_empty()
    );
    assert!(
        test.storage
            .books
            .find_by_hash(&owner, &hash("papelera"))
            .await
            .expect("buscar activo")
            .is_none()
    );
    assert!(
        test.storage
            .books
            .find_trashed_by_hash(&owner, &hash("papelera"))
            .await
            .expect("buscar papelera")
            .is_some()
    );
    let trash = test
        .storage
        .books
        .list_trash(&owner)
        .await
        .expect("listar papelera");
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].id, book.id);
    assert!(trash[0].deleted_at.is_some());

    test.storage
        .books
        .set_deleted(&book.id, false)
        .await
        .expect("restaurar");
    let restored = test
        .storage
        .books
        .list_summaries(&owner)
        .await
        .expect("listar restaurados");
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].id, book.id);
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
            chapter_index: Some(1),
            offset_start: Some(0),
            offset_end: Some(8),
            citation: Some("Anotado · capítulo 2".to_string()),
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
            content_json: Some(r#"{"ops":[{"insert":"cuerpo\n"}]}"#.to_string()),
            chapter_index: Some(1),
            offset_start: Some(0),
            offset_end: Some(8),
            text: Some("fragmento".to_string()),
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
    assert_eq!(
        test.storage
            .annotations
            .list_by_owner(&owner)
            .await
            .expect("anotaciones de la cuenta")
            .len(),
        1
    );
    assert_eq!(
        test.storage
            .extracts
            .list_by_owner(&owner)
            .await
            .expect("extractos de la cuenta")
            .len(),
        1
    );
    assert_eq!(
        test.storage
            .notes
            .list_by_owner(&owner)
            .await
            .expect("notas de la cuenta")
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
    // El borrado suave conserva el propietario y la tumba para CRDT.
    assert!(
        test.storage
            .resource_owners
            .get(ResourceType::Note, &note.id)
            .await
            .expect("propietario")
            .is_some()
    );
    assert!(
        test.storage
            .notes
            .get_any(&note.id)
            .await
            .expect("leer tumba")
            .expect("tumba")
            .deleted
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
            font_face: "jakarta".to_string(),
            column_width: 840,
            text_align: "left".to_string(),
            paragraph_indent: 0,
            hyphenate: false,
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
            font_face: "literata".to_string(),
            column_width: 680,
            text_align: "justify".to_string(),
            paragraph_indent: 1,
            hyphenate: true,
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
async fn library_aggregates_categories_and_audiobook_ids() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("agregado"), "Agregado"))
        .await
        .expect("libro");
    let other = test
        .storage
        .books
        .create(new_book(&owner, &hash("sin-categoria"), "Sin categoría"))
        .await
        .expect("otro libro");
    let category = test
        .storage
        .categories
        .create(NewCategory {
            name: "Agregados".to_string(),
            color: None,
            sort_order: 0,
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

    let pairs = test
        .storage
        .book_categories
        .list_pairs_by_owner(&owner)
        .await
        .expect("pares");
    assert_eq!(pairs, vec![(book.id.clone(), category.id.clone())]);

    test.storage
        .audiobooks
        .create(NewAudiobook {
            book_id: book.id.clone(),
            voice_id: None,
            status: AudiobookStatus::Done,
            manifest_hash: None,
            manifest_ext: None,
            audio_hash: None,
            audio_ext: None,
            scope: None,
            origin: AudiobookOrigin::Generated,
            duration_ms: None,
            narration: None,
            cast: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("audiolibro");

    let ids = test
        .storage
        .audiobooks
        .list_book_ids_by_owner(&owner)
        .await
        .expect("ids con audio");
    assert_eq!(ids, vec![book.id.clone()]);
    assert!(!ids.contains(&other.id));
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
async fn classification_suggestions_lifecycle_and_manual_lock() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("clasificable"), "Clasificable"))
        .await
        .expect("libro");
    let category = test
        .storage
        .categories
        .create(NewCategory {
            name: "Ciencia ficción".to_string(),
            color: None,
            sort_order: 0,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("categoría");

    let suggestion = test
        .storage
        .classification
        .insert_suggestion(&NewCategorySuggestion::for_category(
            &book.id,
            &category.id,
            0.91,
            SuggestionSource::Embedding,
            Some("prueba".to_string()),
            &owner,
            None,
        ))
        .await
        .expect("sugerencia");
    assert_eq!(
        test.storage
            .classification
            .count_pending(&owner)
            .await
            .expect("pendientes"),
        1
    );
    assert_eq!(
        test.storage
            .classification
            .list_pending_by_book(&owner, &book.id)
            .await
            .expect("vista")
            .len(),
        1
    );

    // Insertar la misma sugerencia pendiente es idempotente.
    let repeated = test
        .storage
        .classification
        .insert_suggestion(&NewCategorySuggestion::for_category(
            &book.id,
            &category.id,
            0.50,
            SuggestionSource::Ai,
            None,
            &owner,
            None,
        ))
        .await
        .expect("sugerencia repetida");
    assert_eq!(repeated.id, suggestion.id);

    // Aceptar una sugerencia no bloquea la asignación: sigue siendo automática.
    test.storage
        .book_categories
        .assign_detailed(
            &book.id,
            &category.id,
            &owner,
            None,
            CategoryAssignSource::Embedding,
            Some(0.91),
            false,
        )
        .await
        .expect("asignar sugerencia");
    assert!(
        !test
            .storage
            .book_categories
            .has_locked_category(&book.id)
            .await
            .expect("bloqueo")
    );
    test.storage
        .classification
        .resolve_suggestion(&suggestion.id, &owner, SuggestionStatus::Accepted)
        .await
        .expect("resolver");

    // Una asignación manual marca el libro como bloqueado.
    test.storage
        .book_categories
        .assign(&book.id, &category.id, &owner, None)
        .await
        .expect("asignación manual");
    assert!(
        test.storage
            .book_categories
            .has_locked_category(&book.id)
            .await
            .expect("bloqueo manual")
    );

    // Una sugerencia de categoría nueva se puede repetir tras descartarla.
    let new_suggestion = test
        .storage
        .classification
        .insert_suggestion(&NewCategorySuggestion::for_new_category(
            &book.id,
            "Fantasía épica",
            "fantasia epica",
            0.77,
            SuggestionSource::Ai,
            None,
            &owner,
            None,
        ))
        .await
        .expect("nueva categoría");
    test.storage
        .classification
        .resolve_suggestion(&new_suggestion.id, &owner, SuggestionStatus::Dismissed)
        .await
        .expect("descartar");
    let again = test
        .storage
        .classification
        .insert_suggestion(&NewCategorySuggestion::for_new_category(
            &book.id,
            "Fantasía épica",
            "fantasia epica",
            0.80,
            SuggestionSource::Ai,
            None,
            &owner,
            None,
        ))
        .await
        .expect("reinsertar");
    assert_ne!(again.id, new_suggestion.id);
    assert_eq!(
        test.storage
            .classification
            .count_pending(&owner)
            .await
            .expect("pendientes finales"),
        1
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
            id: None,
            name: "Narradora".to_string(),
            engine: "piper".to_string(),
            language: "es".to_string(),
            profile_hash: None,
            profile_ext: None,
            sample_hash: None,
            sample_ext: None,
            kind: VoiceKind::System,
            reference_duration_ms: 0,
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
            audio_hash: None,
            audio_ext: None,
            scope: None,
            origin: AudiobookOrigin::Generated,
            duration_ms: None,
            narration: None,
            cast: None,
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
            scope_type: crate::storage::ShareScopeType::Book,
            permission: crate::storage::SharePermission::Read,
            expires_at: None,
            max_uses: Some(3),
            share_key_hash: None,
            share_key_ciphertext: None,
            share_key_nonce: None,
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
            hash: None,
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
            origin: BookOrigin::Imported,
            owner_account_id: owner.clone(),
            origin_share_id: None,
            audio_only: false,
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

#[tokio::test]
async fn pdf_info_ocr_state_and_auto_index_round_trip() {
    use crate::storage::model::{AutoIndexEntry, OcrStatus, PdfDocumentInfo, PdfOcrState};

    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();

    let mut new = new_book(&owner, &hash("pdf"), "Escaneado");
    new.format = BookFormat::Pdf;
    new.content_ext = "pdf".to_string();
    new.original_name = "Escaneado.pdf".to_string();
    let book = test.storage.books.create(new).await.expect("crear PDF");

    let info = PdfDocumentInfo {
        book_id: book.id.clone(),
        page_count: 12,
        is_scanned: true,
        has_native_outline: false,
        inspected_at: chrono::Utc::now(),
        owner_account_id: owner.clone(),
        origin_share_id: None,
    };
    test.storage.pdf.upsert_info(&info).await.expect("info");
    let stored = test
        .storage
        .pdf
        .get_info(&book.id)
        .await
        .expect("leer info")
        .expect("info");
    assert_eq!(stored.page_count, 12);
    assert!(stored.is_scanned);
    assert!(!stored.has_native_outline);

    let mut state = PdfOcrState::pending(book.id.clone(), 12, owner.clone());
    state.status = OcrStatus::Done;
    state.pages_done = 12;
    state.language = "spa+eng".to_string();
    test.storage
        .pdf
        .upsert_ocr_state(&state)
        .await
        .expect("estado OCR");
    let stored = test
        .storage
        .pdf
        .get_ocr_state(&book.id)
        .await
        .expect("leer estado")
        .expect("estado");
    assert_eq!(stored.status, OcrStatus::Done);
    assert_eq!(stored.pages_done, 12);
    assert!(stored.progress() >= 0.99);

    let entries = vec![
        AutoIndexEntry {
            title: "Capítulo 1".to_string(),
            page: 0,
            level: 0,
        },
        AutoIndexEntry {
            title: "1.1 Sección".to_string(),
            page: 3,
            level: 1,
        },
    ];
    let index = test
        .storage
        .pdf
        .save_auto_index(&book.id, "ocr", &entries, &owner, None)
        .await
        .expect("índice");
    assert_eq!(index.source, "ocr");

    let stored = test
        .storage
        .pdf
        .get_auto_index(&book.id)
        .await
        .expect("leer índice")
        .expect("índice");
    assert_eq!(stored.entries, entries);

    test.storage
        .books
        .delete(&book.id)
        .await
        .expect("borrar libro");
    assert!(
        test.storage
            .pdf
            .get_info(&book.id)
            .await
            .expect("info borrada")
            .is_none()
    );
    assert!(
        test.storage
            .pdf
            .get_auto_index(&book.id)
            .await
            .expect("índice borrado")
            .is_none()
    );
}

#[tokio::test]
async fn tts_sequences_preferences_and_practice_round_trip() {
    let test = TestStorage::new().await;
    let identity = test
        .storage
        .ensure_local_identity("Biblioteca", "Portátil")
        .await
        .expect("identidad");
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("tts"), "TTS"))
        .await
        .expect("libro");

    let sequence = test
        .storage
        .tts
        .save_sequence(NewTtsSequence {
            book_id: book.id.clone(),
            section: 0,
            content_hash: "hash-a".to_string(),
            unit: OffsetUnit::Utf16,
            sequence: "{\"chunks\":[]}".to_string(),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("secuencia");
    assert_eq!(sequence.unit, OffsetUnit::Utf16);

    test.storage
        .tts
        .save_sequence(NewTtsSequence {
            book_id: book.id.clone(),
            section: 0,
            content_hash: "hash-b".to_string(),
            unit: OffsetUnit::Utf16,
            sequence: "{\"chunks\":[]}".to_string(),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("secuencia nueva");
    assert!(
        test.storage
            .tts
            .get_sequence(&book.id, 0, "hash-a")
            .await
            .expect("buscar vieja")
            .is_none()
    );

    test.storage
        .tts
        .set_character_voice(NewCharacterVoice {
            book_id: book.id.clone(),
            character: "Ana".to_string(),
            voice_id: "es_ES-davefx-medium".to_string(),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("voz de personaje");
    assert_eq!(
        test.storage
            .tts
            .character_voices(&book.id)
            .await
            .expect("voces")
            .len(),
        1
    );

    test.storage
        .tts
        .save_preferences(NewTtsPreferences {
            book_id: book.id.clone(),
            voice_id: Some("es_ES-davefx-medium".to_string()),
            rate: 1.2,
            pitch: 1.0,
            pause_scale: 0.8,
            expressive: true,
            multi_voice: true,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("preferencias");
    let preferences = test
        .storage
        .tts
        .preferences(&book.id)
        .await
        .expect("leer preferencias")
        .expect("preferencias");
    assert!((preferences.rate - 1.2).abs() < f64::EPSILON);

    let attempt = test
        .storage
        .tts
        .save_practice_attempt(NewPracticeAttempt {
            id: uuid::Uuid::new_v4().to_string(),
            book_id: book.id.clone(),
            section: 0,
            offset_start: 0,
            offset_end: 10,
            expected: "hola mundo".to_string(),
            transcript: "hola mundo".to_string(),
            accuracy: 1.0,
            diff: "[]".to_string(),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("intento");
    assert_eq!(
        test.storage
            .tts
            .practice_history(&book.id, 10)
            .await
            .expect("historial")
            .len(),
        1
    );

    test.storage
        .books
        .delete(&book.id)
        .await
        .expect("borrar libro");
    assert!(
        test.storage
            .tts
            .preferences(&book.id)
            .await
            .expect("preferencias borradas")
            .is_none()
    );
    assert!(
        test.storage
            .tts
            .practice_history(&book.id, 10)
            .await
            .expect("historial borrado")
            .is_empty()
    );
    assert_eq!(attempt.accuracy, 1.0);
}

#[tokio::test]
async fn audiobook_origin_scope_and_progress_round_trip() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("tejido"), "Tejido"))
        .await
        .expect("libro");

    let audiobook = test
        .storage
        .audiobooks
        .create(NewAudiobook {
            book_id: book.id.clone(),
            voice_id: None,
            status: AudiobookStatus::Running,
            manifest_hash: None,
            manifest_ext: None,
            audio_hash: Some(hash("audio")),
            audio_ext: Some("m4b".to_string()),
            scope: Some(r#"{"kind":"fullBook"}"#.to_string()),
            origin: AudiobookOrigin::Tejido,
            duration_ms: None,
            narration: None,
            cast: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("audiolibro");

    assert_eq!(audiobook.origin, AudiobookOrigin::Tejido);
    assert_eq!(audiobook.delivery, AudiobookDelivery::File);
    assert_eq!(audiobook.scope.as_deref(), Some(r#"{"kind":"fullBook"}"#));
    assert!(
        test.storage
            .asset_is_referenced(AssetKind::Audiobook, &hash("audio"))
            .await
            .expect("referencia")
    );

    test.storage
        .audiobooks
        .save_progress(&audiobook.id, &owner, 12_500, 2)
        .await
        .expect("guardar progreso");

    let progress = test
        .storage
        .audiobooks
        .progress(&audiobook.id)
        .await
        .expect("leer progreso")
        .expect("progreso");
    assert_eq!(progress.position_ms, 12_500);
    assert_eq!(progress.chapter_index, 2);

    let latest = test
        .storage
        .audiobooks
        .latest_progress_by_owner(&owner)
        .await
        .expect("último progreso")
        .expect("progreso");
    assert_eq!(latest.audiobook_id, audiobook.id);
}

#[tokio::test]
async fn on_demand_audiobook_links_book_without_audio() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("on-demand"), "Bajo demanda"))
        .await
        .expect("libro");

    let audiobook = test
        .storage
        .audiobooks
        .create_on_demand(NewAudiobook {
            book_id: book.id.clone(),
            voice_id: None,
            status: AudiobookStatus::Done,
            manifest_hash: None,
            manifest_ext: None,
            audio_hash: None,
            audio_ext: None,
            scope: Some(r#"{"kind":"fullBook"}"#.to_string()),
            origin: AudiobookOrigin::Generated,
            duration_ms: None,
            narration: None,
            cast: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("audiolibro bajo demanda");

    assert_eq!(audiobook.delivery, AudiobookDelivery::OnDemand);
    assert!(audiobook.audio_hash.is_none());
    assert!(audiobook.manifest_hash.is_none());

    let linked = test
        .storage
        .audiobooks
        .get_on_demand_by_book(&book.id, &owner)
        .await
        .expect("buscar vínculo")
        .expect("vínculo");
    assert_eq!(linked.id, audiobook.id);

    let ids = test
        .storage
        .audiobooks
        .list_book_ids_by_owner(&owner)
        .await
        .expect("ids con audio");
    assert!(ids.contains(&book.id));

    test.storage
        .audiobooks
        .save_progress(&audiobook.id, &owner, 0, 3)
        .await
        .expect("guardar capítulo");
    let progress = test
        .storage
        .audiobooks
        .progress(&audiobook.id)
        .await
        .expect("leer progreso")
        .expect("progreso");
    assert_eq!(progress.chapter_index, 3);
    assert!(
        test.storage
            .audiobooks
            .latest_progress_by_owner(&owner)
            .await
            .expect("último progreso de archivo")
            .is_none(),
        "el progreso bajo demanda no debe alimentar al reproductor de archivos"
    );
}

#[tokio::test]
async fn annotation_ids_are_uuidv7_and_soft_delete_keeps_tombstones() {
    assert_eq!(
        uuid::Uuid::parse_str(&crate::storage::repos::new_id())
            .expect("uuid")
            .get_version_num(),
        7
    );

    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("soft-delete"), "Soft delete"))
        .await
        .expect("libro");

    let annotation = test
        .storage
        .annotations
        .create(NewAnnotation {
            book_id: book.id.clone(),
            kind: AnnotationKind::Highlight,
            locator: "chapter:0:0-5".to_string(),
            chapter_index: Some(0),
            offset_start: Some(0),
            offset_end: Some(5),
            text: Some("hola".to_string()),
            note: None,
            color: Some("#ffd166".to_string()),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("anotación");

    assert_eq!(
        uuid::Uuid::parse_str(&annotation.id)
            .expect("uuid")
            .get_version_num(),
        7
    );

    test.storage
        .annotations
        .delete(&annotation.id)
        .await
        .expect("borrado suave");
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
            .annotations
            .get_any(&annotation.id)
            .await
            .expect("leer tumba")
            .expect("tumba")
            .deleted
    );
    assert!(
        test.storage
            .annotations
            .list_by_book(&book.id)
            .await
            .expect("listar")
            .is_empty()
    );

    let extract = test
        .storage
        .extracts
        .create(NewExtract {
            book_id: book.id.clone(),
            locator: "chapter:0:0-8".to_string(),
            text: "fragmento buscable".to_string(),
            note: Some("una nota".to_string()),
            chapter_index: Some(0),
            offset_start: Some(0),
            offset_end: Some(8),
            citation: Some("Soft delete · capítulo 1".to_string()),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("extracto");

    let found = test
        .storage
        .extracts
        .search(&owner, Some(&book.id), "buscable", 10)
        .await
        .expect("buscar");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, extract.id);

    test.storage
        .extracts
        .delete(&extract.id)
        .await
        .expect("borrar extracto");
    assert!(
        test.storage
            .extracts
            .search(&owner, Some(&book.id), "buscable", 10)
            .await
            .expect("buscar tras borrar")
            .is_empty()
    );

    let note = test
        .storage
        .notes
        .create(NewNote {
            book_id: Some(book.id.clone()),
            title: Some("Rica".to_string()),
            body: "texto plano".to_string(),
            content_json: Some(r#"{"ops":[{"insert":"texto plano\n"}]}"#.to_string()),
            chapter_index: Some(0),
            offset_start: Some(0),
            offset_end: Some(8),
            text: Some("fragmento".to_string()),
            audio_hash: None,
            audio_ext: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("nota");
    test.storage
        .notes
        .delete(&note.id)
        .await
        .expect("borrar nota");
    assert!(
        test.storage
            .notes
            .get(&note.id)
            .await
            .expect("leer")
            .is_none()
    );
    assert!(
        test.storage
            .notes
            .get_any(&note.id)
            .await
            .expect("leer tumba")
            .expect("tumba")
            .deleted
    );
}

#[tokio::test]
async fn voice_notes_and_reactions_round_trip() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(
            &owner,
            &hash("voz-reacciones"),
            "Voz y reacciones",
        ))
        .await
        .expect("libro");

    let voice = test
        .storage
        .voice_notes
        .create(NewVoiceNote {
            id: None,
            book_id: book.id.clone(),
            chapter_index: 2,
            offset_start: 10,
            offset_end: 20,
            text: "pasaje".to_string(),
            duration_ms: 1_500,
            audio_hash: None,
            audiobook_id: None,
            position_ms: 0,
            quote: None,
            transcript: None,
            source: "manual".to_string(),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("nota de voz");
    assert_eq!(voice.duration_ms, 1_500);
    assert_eq!(voice.chapter_index, 2);
    assert!(voice.audiobook_id.is_none());
    assert_eq!(voice.position_ms, 0);
    assert_eq!(voice.source, "manual");
    assert_eq!(
        test.storage
            .voice_notes
            .list_by_book(&book.id)
            .await
            .expect("listar voz")
            .len(),
        1
    );

    for kind in [
        ReactionKind::Like,
        ReactionKind::Like,
        ReactionKind::Surprise,
        ReactionKind::Tension,
    ] {
        test.storage
            .reactions
            .create(NewReaction {
                book_id: book.id.clone(),
                chapter_index: 2,
                offset_start: 10,
                offset_end: 20,
                kind,
                text: None,
                owner_account_id: owner.clone(),
                origin_share_id: None,
            })
            .await
            .expect("reacción");
    }

    let counts = test
        .storage
        .reactions
        .counts_by_book(&book.id)
        .await
        .expect("mapa");
    assert_eq!(counts.len(), 3);
    assert_eq!(
        counts
            .iter()
            .find(|(_, kind, _)| *kind == ReactionKind::Like)
            .expect("like")
            .2,
        2
    );
    assert_eq!(
        test.storage
            .reactions
            .list_by_book(&book.id)
            .await
            .expect("listar reacciones")
            .len(),
        4
    );
    assert_eq!(
        test.storage
            .voice_notes
            .list_by_owner(&owner)
            .await
            .expect("voz de la cuenta")
            .len(),
        1
    );
    assert_eq!(
        test.storage
            .reactions
            .list_by_owner(&owner)
            .await
            .expect("reacciones de la cuenta")
            .len(),
        4
    );

    let mut edited_voice = voice.clone();
    edited_voice.text = "pasaje corregido".to_string();
    edited_voice.transcript = Some("transcripción nueva".to_string());
    edited_voice.quote = Some("cita del libro".to_string());
    test.storage
        .voice_notes
        .update(&edited_voice)
        .await
        .expect("editar voz");
    let stored_voice = test
        .storage
        .voice_notes
        .get(&voice.id)
        .await
        .expect("consultar voz")
        .expect("voz editada");
    assert_eq!(stored_voice.text, "pasaje corregido");
    assert_eq!(
        stored_voice.transcript.as_deref(),
        Some("transcripción nueva")
    );

    let mut reaction = test
        .storage
        .reactions
        .list_by_book(&book.id)
        .await
        .expect("listar reacciones")
        .into_iter()
        .next()
        .expect("reacción existente");
    reaction.kind = ReactionKind::Emotion;
    reaction.text = Some("me llegó".to_string());
    test.storage
        .reactions
        .update(&reaction)
        .await
        .expect("editar reacción");
    let stored_reaction = test
        .storage
        .reactions
        .get(&reaction.id)
        .await
        .expect("consultar reacción")
        .expect("reacción editada");
    assert_eq!(stored_reaction.kind, ReactionKind::Emotion);
    assert_eq!(stored_reaction.text.as_deref(), Some("me llegó"));

    test.storage
        .voice_notes
        .delete(&voice.id)
        .await
        .expect("borrar voz");
    assert!(
        test.storage
            .voice_notes
            .list_by_book(&book.id)
            .await
            .expect("listar voz")
            .is_empty()
    );
    assert!(
        test.storage
            .voice_notes
            .list_by_book_including_deleted(&book.id)
            .await
            .expect("listar tumbas")
            .len()
            == 1
    );
}

#[tokio::test]
async fn audiobook_favorites_survive_round_trip_and_cascade() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let book = test
        .storage
        .books
        .create(new_book(&owner, &hash("favorito-audio"), "Favorito"))
        .await
        .expect("libro");
    let audiobook = test
        .storage
        .audiobooks
        .create(NewAudiobook {
            book_id: book.id.clone(),
            voice_id: None,
            status: AudiobookStatus::Done,
            manifest_hash: Some(hash("manifiesto-favorito")),
            manifest_ext: Some("json".to_string()),
            audio_hash: Some(hash("audio-favorito")),
            audio_ext: Some("m4b".to_string()),
            scope: None,
            origin: AudiobookOrigin::Generated,
            duration_ms: Some(60_000),
            narration: None,
            cast: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("audiolibro");

    assert!(
        !test
            .storage
            .audiobooks
            .is_favorite(&audiobook.id)
            .await
            .expect("favorito inicial")
    );
    test.storage
        .audiobooks
        .set_favorite(&owner, &audiobook.id, true)
        .await
        .expect("marcar favorito");
    assert!(
        test.storage
            .audiobooks
            .is_favorite(&audiobook.id)
            .await
            .expect("favorito")
    );

    test.storage
        .audiobooks
        .delete(&audiobook.id)
        .await
        .expect("borrar audiolibro");
    assert!(
        !test
            .storage
            .audiobooks
            .is_favorite(&audiobook.id)
            .await
            .expect("favorito tras borrar")
    );
}

#[tokio::test]
async fn replaces_pdf_content_with_epub_and_cleans_pdf_tables() {
    use crate::storage::conversion::{AnchorUpdate, EpubReplacement, ProgressUpdate};
    use crate::storage::model::{AutoIndexEntry, PdfDocumentInfo, PdfOcrState};

    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let mut new_book = new_book(&owner, &hash("pdf-a-epub"), "PDF original");
    new_book.format = BookFormat::Pdf;
    new_book.content_ext = "pdf".to_string();
    new_book.original_name = "original.pdf".to_string();
    let book = test
        .storage
        .books
        .create(new_book)
        .await
        .expect("crear PDF");

    test.storage
        .pdf
        .upsert_info(&PdfDocumentInfo {
            book_id: book.id.clone(),
            page_count: 10,
            is_scanned: false,
            has_native_outline: false,
            inspected_at: chrono::Utc::now(),
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("info PDF");
    test.storage
        .pdf
        .upsert_ocr_state(&PdfOcrState::pending(book.id.clone(), 10, owner.clone()))
        .await
        .expect("estado OCR");
    test.storage
        .pdf
        .save_auto_index(
            &book.id,
            "heuristic",
            &[AutoIndexEntry {
                title: "Uno".to_string(),
                page: 0,
                level: 0,
            }],
            &owner,
            None,
        )
        .await
        .expect("índice PDF");

    test.storage
        .progress
        .upsert(NewReadingProgress {
            book_id: book.id.clone(),
            locator: "{\"section\":4,\"offset\":0}".to_string(),
            percent: 0.4,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("progreso");

    let annotation = test
        .storage
        .annotations
        .create(NewAnnotation {
            book_id: book.id.clone(),
            kind: AnnotationKind::Highlight,
            locator: "chapter:4:10-20".to_string(),
            chapter_index: Some(4),
            offset_start: Some(10),
            offset_end: Some(20),
            text: Some("texto subrayado".to_string()),
            note: None,
            color: None,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("anotación");

    let category = test
        .storage
        .categories
        .create(NewCategory {
            name: "Convertidos".to_string(),
            color: None,
            sort_order: 0,
            owner_account_id: owner.clone(),
            origin_share_id: None,
        })
        .await
        .expect("categoría");
    test.storage
        .book_categories
        .assign(&book.id, &category.id, &owner, None)
        .await
        .expect("categoría asignada");
    test.storage
        .books
        .set_favorite(&owner, &book.id, true)
        .await
        .expect("favorito");

    let new_hash = hash("epub-convertido");
    crate::storage::conversion::replace_book_with_epub(
        &test.storage,
        &EpubReplacement {
            book_id: &book.id,
            title: "PDF convertido",
            author: Some("Autora"),
            language: Some("es"),
            description: Some("Convertido desde PDF."),
            content_hash: &new_hash,
            content_ext: "epub",
            original_name: "convertido.epub",
            size_bytes: 2048,
            cover_hash: None,
            cover_ext: None,
            progress: Some(ProgressUpdate {
                locator: "{\"section\":2,\"offset\":0}".to_string(),
                percent: 0.4,
            }),
            annotations: vec![AnchorUpdate {
                id: annotation.id.clone(),
                chapter_index: 2,
                offset_start: 30,
                offset_end: 40,
            }],
            extracts: Vec::new(),
            notes: Vec::new(),
            voice_notes: Vec::new(),
            reactions: Vec::new(),
        },
    )
    .await
    .expect("reemplazar");

    let updated = test
        .storage
        .books
        .get(&book.id)
        .await
        .expect("leer libro")
        .expect("libro convertido");
    assert_eq!(updated.id, book.id);
    assert_eq!(updated.format, BookFormat::Epub);
    assert_eq!(updated.title, "PDF convertido");
    assert_eq!(updated.content_hash, new_hash);
    assert_eq!(updated.content_ext, "epub");
    assert_eq!(updated.original_name, "convertido.epub");
    assert_eq!(updated.size_bytes, 2048);

    assert!(
        test.storage
            .pdf
            .get_info(&book.id)
            .await
            .expect("info")
            .is_none()
    );
    assert!(
        test.storage
            .pdf
            .get_ocr_state(&book.id)
            .await
            .expect("ocr")
            .is_none()
    );
    assert!(
        test.storage
            .pdf
            .get_auto_index(&book.id)
            .await
            .expect("índice")
            .is_none()
    );

    let relocated = test
        .storage
        .annotations
        .get(&annotation.id)
        .await
        .expect("anotación")
        .expect("anotación reubicada");
    assert_eq!(relocated.chapter_index, Some(2));
    assert_eq!(relocated.offset_start, Some(30));
    assert_eq!(relocated.offset_end, Some(40));
    assert_eq!(relocated.locator, "chapter:2:30-40");

    let progress = test
        .storage
        .progress
        .get(&book.id)
        .await
        .expect("progreso")
        .expect("progreso actualizado");
    assert_eq!(progress.locator, "{\"section\":2,\"offset\":0}");
    assert!((progress.percent - 0.4).abs() < f64::EPSILON);

    assert_eq!(
        test.storage
            .book_categories
            .list_by_book(&book.id)
            .await
            .expect("categorías")
            .len(),
        1
    );
    assert!(
        test.storage
            .books
            .is_favorite(&book.id)
            .await
            .expect("favorito")
    );
}

#[tokio::test]
async fn audiobook_albums_group_order_hide_and_cascade() {
    let (test, identity) = TestStorage::with_identity().await;
    let owner = identity.account.id.clone();
    let first_book = test
        .storage
        .books
        .create(new_book(&owner, &hash("album-first"), "Primero"))
        .await
        .expect("libro uno");
    let second_book = test
        .storage
        .books
        .create(new_book(&owner, &hash("album-second"), "Segundo"))
        .await
        .expect("libro dos");

    let create_audiobook = |book_id: String| {
        let storage = test.storage.clone();
        let owner = owner.clone();
        async move {
            storage
                .audiobooks
                .create(NewAudiobook {
                    book_id,
                    voice_id: None,
                    status: AudiobookStatus::Done,
                    manifest_hash: None,
                    manifest_ext: None,
                    audio_hash: None,
                    audio_ext: None,
                    scope: None,
                    origin: AudiobookOrigin::Generated,
                    duration_ms: Some(60_000),
                    narration: None,
                    cast: None,
                    owner_account_id: owner,
                    origin_share_id: None,
                })
                .await
                .expect("audiolibro")
        }
    };
    let first_audio = create_audiobook(first_book.id.clone()).await;
    let second_audio = create_audiobook(second_book.id.clone()).await;

    let album = test
        .storage
        .audiobook_albums
        .create(NewAudiobookAlbum {
            title: "Colección".into(),
            description: Some("Dos audiolibros".into()),
            sort_order: 0,
            owner_account_id: owner.clone(),
        })
        .await
        .expect("álbum");
    assert!(
        test.storage
            .audiobook_albums
            .get_owned(&album.id, "otra-cuenta")
            .await
            .expect("propietario")
            .is_none()
    );

    test.storage
        .audiobook_albums
        .add_items(
            &album.id,
            &owner,
            &[first_audio.id.clone(), second_audio.id.clone()],
        )
        .await
        .expect("añadir");
    test.storage
        .audiobook_albums
        .add_items(&album.id, &owner, std::slice::from_ref(&first_audio.id))
        .await
        .expect("añadir idempotente");
    assert_eq!(
        test.storage
            .audiobook_albums
            .list_visible_items(&album.id, &owner)
            .await
            .expect("elementos"),
        vec![first_audio.id.clone(), second_audio.id.clone()]
    );

    test.storage
        .audiobook_albums
        .reorder(
            &album.id,
            &[second_audio.id.clone(), first_audio.id.clone()],
        )
        .await
        .expect("reordenar");
    assert_eq!(
        test.storage
            .audiobook_albums
            .list_visible_items(&album.id, &owner)
            .await
            .expect("reordenado"),
        vec![second_audio.id.clone(), first_audio.id.clone()]
    );

    test.storage
        .books
        .set_deleted(&first_book.id, true)
        .await
        .expect("papelera");
    assert_eq!(
        test.storage
            .audiobook_albums
            .list_visible_items(&album.id, &owner)
            .await
            .expect("visibles con papelera"),
        vec![second_audio.id.clone()]
    );
    assert_eq!(
        test.storage
            .audiobook_albums
            .list_items(&album.id)
            .await
            .expect("pertenencia conservada"),
        vec![second_audio.id.clone(), first_audio.id.clone()]
    );
    test.storage
        .books
        .set_deleted(&first_book.id, false)
        .await
        .expect("restaurar");

    test.storage
        .audiobook_albums
        .remove_item(&album.id, &first_audio.id)
        .await
        .expect("quitar");
    assert_eq!(
        test.storage
            .audiobook_albums
            .list_visible_items(&album.id, &owner)
            .await
            .expect("tras quitar"),
        vec![second_audio.id.clone()]
    );

    test.storage
        .audiobook_albums
        .add_items(&album.id, &owner, std::slice::from_ref(&first_audio.id))
        .await
        .expect("volver a añadir");
    test.storage
        .audiobooks
        .delete(&first_audio.id)
        .await
        .expect("borrar audiolibro");
    assert_eq!(
        test.storage
            .audiobook_albums
            .list_items(&album.id)
            .await
            .expect("cascada de audiolibro"),
        vec![second_audio.id.clone()]
    );

    test.storage
        .audiobook_albums
        .delete(&album.id)
        .await
        .expect("borrar álbum");
    assert!(
        test.storage
            .audiobooks
            .get(&second_audio.id)
            .await
            .expect("audiolibro tras álbum")
            .is_some()
    );
}

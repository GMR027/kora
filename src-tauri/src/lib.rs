pub mod accounts;
pub mod ai;
pub mod annotations;
pub mod audiobooks;
pub mod classify;
pub mod commands;
pub mod error;
pub mod features;
pub mod reader;
pub mod state;
pub mod storage;
pub mod sync;
pub mod tejido;
pub mod tts;

use tauri::Manager;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn init_tracing(log_dir: &std::path::Path) -> anyhow::Result<()> {
    let file_appender = tracing_appender::rolling::daily(log_dir, "kora.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    // El guard debe vivir tanto como el proceso. Se filtra para no arrastrar
    // el estado en AppState (que exige Send + Sync).
    Box::leak(Box::new(guard));

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("kora_lib=info,tauri=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false),
        )
        .try_init()?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());

    #[cfg(target_os = "android")]
    {
        builder = builder
            .plugin(tauri_plugin_tts::init())
            .plugin(audiobooks::player::init_mobile_plugin());
    }

    builder
        .setup(|app| {
            eprintln!("Kora: inicializando estado");
            let state = match state::AppState::new(app.handle()) {
                Ok(state) => state,
                Err(error) => {
                    eprintln!("Kora: fallo al inicializar el estado: {error:#}");
                    return Err(Box::new(error));
                }
            };
            let logs = state.layout.logs.clone();
            // El estado debe quedar registrado antes de cualquier inicialización
            // que pueda fallar (por ejemplo, el suscriptor global de tracing en
            // una recreación de la actividad Android).
            app.manage(state);
            if let Err(error) = init_tracing(&logs) {
                eprintln!("Kora: no se pudo iniciar el registro de logs: {error:#}");
            }
            eprintln!("Kora: estado listo");

            tracing::info!(
                root = %app.state::<state::AppState>().layout.root.display(),
                database = %app.state::<state::AppState>().layout.database_file().display(),
                "Kora iniciado"
            );

            let handle = app.handle().clone();
            let semantic = app.state::<state::AppState>().semantic.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = semantic.reconcile(Some(&handle), false).await {
                    tracing::warn!(%error, "no se pudo reconciliar el índice semántico al iniciar");
                }
            });

            #[cfg(not(target_os = "android"))]
            audiobooks::queue::start_worker(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ai::ai_get_config,
            commands::ai::ai_save_config,
            commands::ai::ai_status,
            commands::ai::ai_chat,
            commands::app::app_info,
            commands::app::storage_layout,
            commands::storage::storage_usage,
            commands::storage::backup_library,
            commands::storage::restore_library,
            commands::storage::purge_storage,
            commands::appearance::get_appearance,
            commands::appearance::save_appearance,
            commands::appearance::save_custom_theme,
            commands::appearance::delete_custom_theme,
            commands::library::library_list,
            commands::library::library_overview,
            commands::library::library_audiobook_book_ids,
            commands::library::library_import,
            commands::convert::pdf_conversion_preview,
            commands::convert::convert_pdf_to_epub,
            commands::convert::pdf_conversion_status,
            commands::library::library_delete,
            commands::library::library_trash,
            commands::library::library_restore,
            commands::library::library_purge_book,
            commands::library::library_empty_trash,
            commands::library::library_ensure_cover,
            commands::library::library_replace_cover,
            commands::library::library_assign_category,
            commands::library::library_unassign_category,
            commands::library::library_update,
            commands::library::library_set_favorite,
            commands::library::category_create,
            commands::library::category_update,
            commands::library::category_delete,
            commands::library::category_list,
            commands::classification::classification_get_config,
            commands::classification::classification_save_config,
            commands::classification::classification_status,
            commands::classification::library_classify,
            commands::classification::library_classify_pending,
            commands::classification::category_suggestions,
            commands::classification::category_suggestion_list_pending,
            commands::classification::category_suggestion_accept,
            commands::classification::category_suggestion_dismiss,
            commands::classification::category_suggestion_accept_all,
            commands::classification::category_seed_apply,
            commands::reader::reader_open,
            commands::reader::reader_get_chapter,
            commands::reader::reader_update_progress,
            commands::reader::reader_add_annotation,
            commands::reader::reader_update_annotation,
            commands::reader::reader_list_annotations,
            commands::reader::reader_delete_annotation,
            commands::reader::reader_add_extract,
            commands::reader::reader_list_extracts,
            commands::reader::reader_delete_extract,
            commands::reader::list_all_extracts,
            commands::annotations::add_marker,
            commands::annotations::update_marker,
            commands::annotations::add_note,
            commands::annotations::update_note,
            commands::annotations::add_voice_note,
            commands::annotations::start_voice_note_recording,
            commands::annotations::cancel_voice_note_recording,
            commands::annotations::add_extract,
            commands::annotations::update_extract,
            commands::annotations::update_voice_note,
            commands::annotations::add_reaction,
            commands::annotations::update_reaction,
            commands::annotations::list_annotations,
            commands::annotations::list_library_annotations,
            commands::annotations::search_extracts,
            commands::annotations::reaction_map,
            commands::annotations::delete_annotation,
            commands::reader::reader_get_settings,
            commands::reader::reader_get_defaults,
            commands::reader::reader_save_settings,
            commands::reader::reader_apply_to_all,
            commands::reader::ocr_pdf,
            commands::reader::get_ocr_page,
            commands::reader::search_pdf,
            commands::reader::build_auto_index,
            commands::reader::get_auto_index,
            commands::reader::reader_pdf_bytes,
            commands::tts::tts_engines,
            commands::tts::tts_voices,
            commands::tts::tts_synthesize,
            commands::tts::tts_download_voice,
            commands::tts::start_tts,
            commands::tts::pause_tts,
            commands::tts::resume_tts,
            commands::tts::stop_tts,
            commands::tts::set_rate,
            commands::tts::set_pitch,
            commands::tts::tts_status,
            commands::tts::tts_characters,
            commands::tts::tts_character_voices,
            commands::tts::assign_character_voice,
            commands::tts::tts_preferences,
            commands::tts::save_tts_preferences,
            commands::practice::practice_start,
            commands::practice::practice_stop,
            commands::practice::practice_cancel,
            commands::practice::practice_models,
            commands::practice::practice_history,
            commands::practice::practice_download_model,
            commands::voice::voice_consent_text,
            commands::voice::voice_capabilities,
            commands::voice::voice_sample_start,
            commands::voice::voice_sample_stop,
            commands::voice::voice_sample_cancel,
            commands::voice::voice_sample_discard,
            commands::voice::voice_sample_level,
            commands::voice::voice_input_devices,
            commands::voice::voice_models,
            commands::voice::voice_download_models,
            commands::voice::voice_download_model,
            commands::voice::delete_voice_model,
            commands::voice::create_voice_profile,
            commands::voice::list_voice_profiles,
            commands::voice::delete_voice_profile,
            commands::voice::update_voice_profile,
            commands::voice::voice_profile_sample,
            commands::voice::replace_voice_sample,
            commands::voice::export_voice_profile,
            commands::voice::import_voice_profile,
            commands::voice::voice_clone_preview,
            commands::audiobooks::list_audiobooks,
            commands::audiobooks::audiobook_set_favorite,
            commands::audiobooks::generate_audiobook,
            commands::audiobooks::enqueue_audiobook,
            commands::audiobooks::retry_audiobook,
            commands::audiobooks::audiobook_queue_status,
            commands::audiobooks::remove_queued_audiobook,
            commands::audiobooks::move_queued_audiobook,
            commands::audiobooks::clear_audiobook_queue,
            commands::audiobooks::pause_audiobook_generation,
            commands::audiobooks::resume_audiobook_generation,
            commands::audiobooks::cancel_audiobook_generation,
            commands::audiobooks::import_external_audiobook,
            commands::audiobooks::import_audio_audiobook,
            commands::audiobooks::reanalyze_audiobook_chapters,
            commands::audiobooks::update_audiobook_chapters,
            commands::audiobooks::transcribe_audiobook_later,
            commands::audiobooks::play_audiobook,
            commands::audiobooks::pause_audiobook,
            commands::audiobooks::seek_audiobook,
            commands::audiobooks::set_audiobook_speed,
            commands::audiobooks::set_audiobook_keep_screen_on,
            commands::audiobooks::set_audiobook_sleep_timer,
            commands::audiobooks::audiobook_chapters,
            commands::audiobooks::audiobook_characters,
            commands::audiobooks::audiobook_karaoke,
            commands::audiobook_voice::audiobook_voice_markers,
            commands::audiobook_voice::save_voice_marker_phrase,
            commands::audiobook_voice::start_voice_marker_listening,
            commands::audiobook_voice::stop_voice_marker_listening,
            commands::audiobook_voice::voice_marker_status,
            commands::audiobooks::stop_audiobook,
            commands::audiobooks::get_audiobook_progress,
            commands::audiobooks::export_audiobook,
            commands::audiobooks::apply_audiobook_treatment,
            commands::audiobooks::audiobook_waveform,
            commands::audiobooks::audiobook_locator,
            commands::audiobooks::audiobook_capabilities,
            commands::audiobooks::delete_audiobook,
            commands::tejido::weave_extracts,
            commands::sync::start_sync,
            commands::sync::stop_sync,
            commands::sync::get_sync_status,
            commands::sync::get_peers,
            commands::sync::sync_now,
            commands::sync::start_share,
            commands::sync::stop_share,
            commands::sync::revoke_peer,
            commands::sync::get_sync_collections,
            commands::sync::set_sync_categories,
            commands::sync::set_sync_collections,
            commands::sync::list_sync_devices,
            commands::sync::start_mirror,
            commands::sync::stop_mirror,
            commands::sync::join_mirror,
            commands::sync::leave_mirror,
            commands::sync::get_mirror_status,
            commands::sync::sync_status,
            commands::sync::sync_peers,
            commands::sync::sync_start,
            commands::sync::list_audit_log,
            commands::sync::export_audit_log,
            commands::updates::check_app_update,
            commands::updates::open_external_url,
            commands::accounts::account_status,
            commands::accounts::create_account,
            commands::accounts::get_account,
            commands::accounts::update_display_name,
            commands::accounts::update_account_avatar,
            commands::accounts::remove_account_avatar,
            commands::accounts::get_connection_key,
            commands::accounts::rotate_connection_key,
            commands::accounts::change_pin,
            commands::accounts::recover_account_pin,
            commands::accounts::delete_account,
            commands::accounts::create_share,
            commands::accounts::create_audiobook_share,
            commands::accounts::list_shares,
            commands::accounts::reveal_share_key,
            commands::accounts::revoke_share,
            commands::accounts::accept_share,
            commands::accounts::remove_received_share,
            commands::accounts::list_received_shares,
            commands::accounts::list_devices,
            commands::accounts::revoke_device,
            commands::accounts::start_pairing,
            commands::accounts::confirm_sas,
            commands::accounts::stop_pairing,
            commands::features::features_status,
            commands::features::semantic_status,
            commands::features::prepare_semantic_model,
            commands::features::reindex_library,
            commands::features::search_semantic,
            commands::diary::get_diary,
            commands::diary::diary_create,
            commands::diary::diary_update,
            commands::diary::diary_delete,
            commands::diary::export_diary,
            commands::notes::export_notes,
            commands::diary::get_achievements,
            commands::diary::session_start,
            commands::diary::session_end,
            commands::pomodoro::get_pomodoro,
            commands::pomodoro::start_pomodoro,
            commands::pomodoro::stop_pomodoro,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Kora");
}

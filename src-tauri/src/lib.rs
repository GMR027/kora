pub mod accounts;
pub mod commands;
pub mod error;
pub mod features;
pub mod reader;
pub mod state;
pub mod storage;
pub mod sync;
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
    tauri::Builder::default()
        .setup(|app| {
            let state = state::AppState::new(app.handle())?;
            init_tracing(&state.layout.logs)?;

            tracing::info!(
                root = %state.layout.root.display(),
                database = %state.layout.database_file().display(),
                "Kora iniciado"
            );

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::storage_layout,
            commands::library::library_list,
            commands::library::library_import,
            commands::library::library_delete,
            commands::library::library_replace_cover,
            commands::library::library_assign_category,
            commands::library::category_create,
            commands::library::category_list,
            commands::reader::reader_open,
            commands::reader::reader_update_progress,
            commands::tts::tts_engines,
            commands::tts::tts_voices,
            commands::tts::tts_synthesize,
            commands::sync::sync_status,
            commands::sync::sync_peers,
            commands::sync::sync_start,
            commands::accounts::account_status,
            commands::features::features_status,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Kora");
}

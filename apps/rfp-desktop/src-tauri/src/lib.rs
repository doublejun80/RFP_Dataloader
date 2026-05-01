use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub mod analysis;
pub mod block_normalizer;
pub mod candidate_extractor;
mod commands;
pub mod db;
pub mod document_ingestion;
pub mod domain;
pub mod error;
pub mod opendataloader_adapter;
mod state;
mod validation;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = state::AppState::from_app_handle(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::documents::register_document_by_path,
            commands::documents::list_documents,
            commands::extraction::diagnose_opendataloader,
            commands::extraction::run_fast_extraction,
            commands::pipeline::analyze_document_baseline,
            commands::pipeline::analyze_document_candidates
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

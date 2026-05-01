use std::path::PathBuf;

use tauri::State;

use crate::document_ingestion;
use crate::domain::DocumentSummary;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn register_document_by_path(
    path: String,
    state: State<'_, AppState>,
) -> AppResult<DocumentSummary> {
    let conn = state.connect()?;
    document_ingestion::register_document(&conn, &PathBuf::from(path))
}

#[tauri::command]
pub fn list_documents(state: State<'_, AppState>) -> AppResult<Vec<DocumentSummary>> {
    let conn = state.connect()?;
    document_ingestion::list_documents(&conn)
}

use std::path::PathBuf;

use tauri::State;

use crate::domain::ExtractionRunSummary;
use crate::error::AppResult;
use crate::opendataloader_adapter::{self, OpenDataLoaderDiagnostic};
use crate::state::AppState;

#[tauri::command]
pub fn diagnose_opendataloader(cli_path: Option<String>) -> OpenDataLoaderDiagnostic {
    opendataloader_adapter::diagnose(cli_path.map(PathBuf::from))
}

#[tauri::command]
pub fn run_fast_extraction(
    document_id: String,
    cli_path: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<ExtractionRunSummary> {
    let conn = state.connect()?;
    opendataloader_adapter::run_fast_extraction(
        &conn,
        &state.app_data_dir,
        &document_id,
        cli_path.map(PathBuf::from),
    )
}

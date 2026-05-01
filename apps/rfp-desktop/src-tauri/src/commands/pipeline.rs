use rusqlite::Connection;
use tauri::State;

use crate::analysis;
use crate::document_ingestion;
use crate::domain::PipelineSummary;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn analyze_document_baseline(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<PipelineSummary> {
    let conn = state.connect()?;
    run_baseline_analysis_for_document(&conn, &document_id)
}

pub fn run_baseline_analysis_for_document(
    conn: &Connection,
    document_id: &str,
) -> AppResult<PipelineSummary> {
    analysis::create_or_update_baseline_project(conn, document_id)?;
    let document = document_ingestion::load_document_summary(conn, document_id)?;
    let ready_count = count_documents_by_status(conn, "ready")?;
    let review_needed_count = count_documents_by_status(conn, "review_needed")?;
    let failed_count = count_documents_by_status(conn, "failed")?;

    Ok(PipelineSummary {
        document,
        extraction: None,
        ready_count,
        review_needed_count,
        failed_count,
    })
}

fn count_documents_by_status(conn: &Connection, status: &str) -> AppResult<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE status = ?",
        [status],
        |row| row.get(0),
    )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::db;
    use crate::document_ingestion;

    #[test]
    fn summarize_document_reports_review_needed_after_blocks_and_validation() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let pdf_path = temp.path().join("sample.pdf");
        fs::write(&pdf_path, b"%PDF-1.7\nsample").expect("write pdf");
        let conn = db::open_database(&db_path).expect("open db");
        let doc = document_ingestion::register_document(&conn, &pdf_path).expect("register");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-1', ?, 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
            [&doc.id],
        )
        .expect("insert run");
        conn.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (
                'block-1', 'run-1', ?, 'el-1', 1, 0, 'paragraph', NULL,
                '요구사항 SFR-001 통합 로그인 기능', NULL, '{}'
             )",
            [&doc.id],
        )
        .expect("insert block");

        let summary = run_baseline_analysis_for_document(&conn, &doc.id).expect("baseline");

        assert_eq!(summary.document.status, "review_needed");
        assert_eq!(summary.review_needed_count, 1);
        assert_eq!(summary.failed_count, 0);
    }
}

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};
use tauri::State;

use crate::analysis;
use crate::block_normalizer;
use crate::document_ingestion;
use crate::domain::{
    CandidateBundleSummaryDto, CandidateExtractionSummary, EvidenceLinkDto, PipelineSummary,
    RfpFieldDto,
};
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

#[tauri::command]
pub fn analyze_document_candidates(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<CandidateExtractionSummary> {
    let conn = state.connect()?;
    run_candidate_analysis_for_document(&conn, &document_id)
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

pub fn run_candidate_analysis_for_document(
    conn: &Connection,
    document_id: &str,
) -> AppResult<CandidateExtractionSummary> {
    normalize_latest_successful_extraction_if_needed(conn, document_id)?;
    let project_id = analysis::create_or_update_candidate_project(conn, document_id)?;
    load_candidate_extraction_summary(conn, document_id, &project_id)
}

fn normalize_latest_successful_extraction_if_needed(
    conn: &Connection,
    document_id: &str,
) -> AppResult<()> {
    let run: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT id, json_path FROM extraction_runs
             WHERE document_id = ? AND status = 'succeeded'
             ORDER BY finished_at DESC, started_at DESC
             LIMIT 1",
            [document_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let Some((run_id, Some(json_path))) = run else {
        return Ok(());
    };

    let existing_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM document_blocks WHERE extraction_run_id = ?",
        [&run_id],
        |row| row.get(0),
    )?;
    if existing_count == 0 {
        block_normalizer::normalize_extraction_json(
            conn,
            document_id,
            &run_id,
            &PathBuf::from(json_path),
        )?;
    }

    Ok(())
}

pub fn load_candidate_extraction_summary(
    conn: &Connection,
    document_id: &str,
    project_id: &str,
) -> AppResult<CandidateExtractionSummary> {
    Ok(CandidateExtractionSummary {
        document: document_ingestion::load_document_summary(conn, document_id)?,
        project_id: project_id.to_string(),
        fields: load_project_fields(conn, project_id)?,
        bundles: load_candidate_bundle_summaries(conn, project_id)?,
        ready_count: count_documents_by_status(conn, "ready")?,
        review_needed_count: count_documents_by_status(conn, "review_needed")?,
        failed_count: count_documents_by_status(conn, "failed")?,
    })
}

fn load_project_fields(conn: &Connection, project_id: &str) -> AppResult<Vec<RfpFieldDto>> {
    let mut statement = conn.prepare(
        "SELECT id, field_key, label, raw_value, normalized_value, confidence, source
         FROM rfp_fields
         WHERE rfp_project_id = ?
         ORDER BY CASE field_key
           WHEN 'business_name' THEN 1
           WHEN 'client' THEN 2
           WHEN 'budget' THEN 3
           WHEN 'period' THEN 4
           WHEN 'contract_method' THEN 5
           WHEN 'deadline' THEN 6
           ELSE 99
         END",
    )?;
    let fields = statement
        .query_map([project_id], |row| {
            Ok(RfpFieldDto {
                id: row.get(0)?,
                field_key: row.get(1)?,
                label: row.get(2)?,
                raw_value: row.get(3)?,
                normalized_value: row.get(4)?,
                confidence: row.get(5)?,
                source: row.get(6)?,
                evidence: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    fields
        .into_iter()
        .map(|mut field| {
            field.evidence = load_field_evidence(conn, &field.id)?;
            Ok(field)
        })
        .collect()
}

fn load_field_evidence(conn: &Connection, field_id: &str) -> AppResult<Vec<EvidenceLinkDto>> {
    let mut statement = conn.prepare(
        "SELECT document_block_id, quote, confidence
         FROM evidence_links
         WHERE target_table = 'rfp_fields' AND target_id = ?
         ORDER BY confidence DESC",
    )?;
    let evidence = statement
        .query_map([field_id], |row| {
            Ok(EvidenceLinkDto {
                id: None,
                document_block_id: row.get(0)?,
                quote: row.get(1)?,
                confidence: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(evidence)
}

fn load_candidate_bundle_summaries(
    conn: &Connection,
    project_id: &str,
) -> AppResult<Vec<CandidateBundleSummaryDto>> {
    let mut statement = conn.prepare(
        "SELECT bundle_key, candidate_count
         FROM candidate_bundles
         WHERE rfp_project_id = ?
         ORDER BY bundle_key",
    )?;
    let bundles = statement
        .query_map([project_id], |row| {
            Ok(CandidateBundleSummaryDto {
                bundle_key: row.get(0)?,
                candidate_count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(bundles)
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

    #[test]
    fn candidate_pipeline_normalizes_blocks_and_returns_fields() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let json_path = temp.path().join("sample-output.json");
        fs::write(
            &json_path,
            r#"[
              {"id":"b1","type":"paragraph","page_number":1,"text":"사업명: 서울시 통합 유지관리 사업"},
              {"id":"b2","type":"paragraph","page_number":1,"text":"발주기관: 서울특별시"},
              {"id":"b3","type":"paragraph","page_number":1,"text":"사업예산: 1,200,000,000원"},
              {"id":"b4","type":"paragraph","page_number":1,"text":"사업기간: 계약일로부터 12개월"}
            ]"#,
        )
        .expect("write json");
        let conn = db::open_database(&db_path).expect("open db");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'created')",
            [],
        )
        .expect("insert doc");
        conn.execute(
            "INSERT INTO extraction_runs (
                id, document_id, provider, mode, status, json_path, started_at, finished_at
             ) VALUES (
                'run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', ?, '2026-05-02T00:00:00Z', '2026-05-02T00:00:01Z'
             )",
            [json_path.to_string_lossy().to_string()],
        )
        .expect("insert run");

        let summary =
            run_candidate_analysis_for_document(&conn, "doc-1").expect("candidate analysis");

        assert_eq!(summary.fields.len(), 4);
        assert_eq!(summary.bundles.len(), 7);
        assert_eq!(summary.document.status, "review_needed");
        assert_eq!(summary.review_needed_count, 1);
        assert!(summary
            .fields
            .iter()
            .any(|field| field.field_key == "business_name"));
    }
}

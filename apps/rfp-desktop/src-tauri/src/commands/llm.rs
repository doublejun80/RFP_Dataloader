use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension};
use tauri::State;

use crate::analysis;
use crate::candidate_extractor::CandidateBundle;
use crate::domain_writer::DomainWriteSummary;
use crate::error::{AppError, AppResult};
use crate::llm_adapter::contracts::{
    CandidateBlock, LlmInputEnvelope, LlmInstructions, LlmProvider, LlmRunSummary, LlmSchemaName,
};
use crate::llm_adapter::draft::{domain_draft_from_outputs, StructuredOutputs};
use crate::llm_adapter::http::ReqwestTransport;
use crate::llm_adapter::runner::{run_structured_extraction, RunLlmRequest};
use crate::llm_adapter::settings::{self, LlmSettings, SaveLlmSettingsRequest};
use crate::state::AppState;

#[tauri::command]
pub fn get_llm_settings(state: State<'_, AppState>) -> AppResult<LlmSettings> {
    let conn = state.connect()?;
    settings::load_llm_settings(&conn, &settings::KeyringSecretStore)
}

#[tauri::command]
pub fn save_llm_settings(
    request: SaveLlmSettingsRequest,
    state: State<'_, AppState>,
) -> AppResult<LlmSettings> {
    let conn = state.connect()?;
    settings::save_llm_settings(&conn, &settings::KeyringSecretStore, request)?;
    settings::load_llm_settings(&conn, &settings::KeyringSecretStore)
}

#[tauri::command]
pub fn clear_llm_api_key(
    provider: LlmProvider,
    state: State<'_, AppState>,
) -> AppResult<LlmSettings> {
    let conn = state.connect()?;
    settings::clear_api_key(&conn, &settings::KeyringSecretStore, provider)?;
    settings::load_llm_settings(&conn, &settings::KeyringSecretStore)
}

#[tauri::command]
pub fn run_llm_structuring(
    document_id: String,
    schema_name: LlmSchemaName,
    state: State<'_, AppState>,
) -> AppResult<LlmRunSummary> {
    let conn = state.connect()?;
    let envelope = load_candidate_envelope_for_document(&conn, &document_id, schema_name)?;
    run_structured_extraction(
        &conn,
        &settings::KeyringSecretStore,
        &ReqwestTransport::new()?,
        RunLlmRequest {
            schema_name,
            input: envelope,
        },
    )
}

#[tauri::command]
pub fn run_llm_domain_analysis(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<DomainWriteSummary> {
    let mut conn = state.connect()?;
    let transport = ReqwestTransport::new()?;
    let mut outputs = StructuredOutputs::default();

    for schema_name in [
        LlmSchemaName::ProjectInfo,
        LlmSchemaName::Requirements,
        LlmSchemaName::Procurement,
        LlmSchemaName::RiskClassification,
    ] {
        let envelope = load_candidate_envelope_for_document(&conn, &document_id, schema_name)?;
        let summary = run_structured_extraction(
            &conn,
            &settings::KeyringSecretStore,
            &transport,
            RunLlmRequest {
                schema_name,
                input: envelope,
            },
        )?;
        let output = load_llm_run_output(&conn, &summary.id)?;
        match schema_name {
            LlmSchemaName::ProjectInfo => outputs.project_info = Some(output),
            LlmSchemaName::Requirements => outputs.requirements = Some(output),
            LlmSchemaName::Procurement => outputs.procurement = Some(output),
            LlmSchemaName::RiskClassification => outputs.risk_classification = Some(output),
        }
    }

    let draft = domain_draft_from_outputs(outputs)?;
    analysis::write_domain_analysis(&mut conn, &document_id, draft)
}

pub(crate) fn load_candidate_envelope_for_document(
    conn: &Connection,
    document_id: &str,
    schema_name: LlmSchemaName,
) -> AppResult<LlmInputEnvelope> {
    let project_id = ensure_candidate_project(conn, document_id)?;
    let extraction_run_id = latest_successful_extraction_run_id(conn, document_id)?;
    let mut seen = BTreeSet::new();
    let mut candidate_blocks = Vec::new();

    for bundle_key in bundle_keys_for_schema(schema_name) {
        if let Some(bundle_json) = load_bundle_json(conn, &project_id, bundle_key)? {
            let bundle: CandidateBundle = serde_json::from_str(&bundle_json)?;
            for snippet in bundle.snippets {
                if snippet.quote.trim().is_empty()
                    || !seen.insert(snippet.document_block_id.clone())
                {
                    continue;
                }
                candidate_blocks.push(CandidateBlock {
                    block_id: snippet.document_block_id.clone(),
                    page_number: snippet.page_number,
                    kind: snippet.kind,
                    text: snippet.quote,
                    bbox: load_block_bbox(conn, document_id, &snippet.document_block_id)?,
                });
            }
        }
    }

    Ok(LlmInputEnvelope {
        document_id: document_id.to_string(),
        rfp_project_id: project_id,
        extraction_run_id,
        language: "ko".into(),
        candidate_blocks,
        instructions: LlmInstructions {
            preserve_korean_terms: true,
            do_not_invent_values: true,
            require_evidence_block_ids: true,
        },
    })
}

fn ensure_candidate_project(conn: &Connection, document_id: &str) -> AppResult<String> {
    let project_id: Option<String> = conn
        .query_row(
            "SELECT id FROM rfp_projects WHERE document_id = ?",
            [document_id],
            |row| row.get(0),
        )
        .optional()?;
    let needs_candidates = if let Some(project_id) = project_id.as_deref() {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM candidate_bundles WHERE rfp_project_id = ?",
            [project_id],
            |row| row.get(0),
        )?;
        count == 0
    } else {
        true
    };

    if needs_candidates {
        let summary = super::pipeline::run_candidate_analysis_for_document(conn, document_id)?;
        Ok(summary.project_id)
    } else {
        Ok(project_id.expect("project id checked"))
    }
}

fn latest_successful_extraction_run_id(conn: &Connection, document_id: &str) -> AppResult<String> {
    conn.query_row(
        "SELECT id FROM extraction_runs
         WHERE document_id = ? AND status = 'succeeded'
         ORDER BY finished_at DESC, started_at DESC
         LIMIT 1",
        [document_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| AppError::LlmDisabled("successful extraction is unavailable".into()))
}

fn bundle_keys_for_schema(schema_name: LlmSchemaName) -> &'static [&'static str] {
    match schema_name {
        LlmSchemaName::ProjectInfo => &["project_info_candidates"],
        LlmSchemaName::Requirements => &["requirement_candidates"],
        LlmSchemaName::Procurement => &[
            "requirement_candidates",
            "procurement_candidates",
            "staffing_candidates",
            "deliverable_candidates",
            "acceptance_candidates",
            "risk_candidates",
        ],
        LlmSchemaName::RiskClassification => &["risk_candidates"],
    }
}

fn load_bundle_json(
    conn: &Connection,
    project_id: &str,
    bundle_key: &str,
) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT bundle_json FROM candidate_bundles
             WHERE rfp_project_id = ? AND bundle_key = ?",
            [project_id, bundle_key],
            |row| row.get(0),
        )
        .optional()?)
}

fn load_block_bbox(
    conn: &Connection,
    document_id: &str,
    block_id: &str,
) -> AppResult<Option<Vec<f64>>> {
    let bbox_json: Option<Option<String>> = conn
        .query_row(
            "SELECT bbox_json FROM document_blocks WHERE id = ? AND document_id = ?",
            [block_id, document_id],
            |row| row.get(0),
        )
        .optional()?;
    bbox_json
        .flatten()
        .map(|value| serde_json::from_str(&value).map_err(AppError::from))
        .transpose()
}

fn load_llm_run_output(conn: &Connection, run_id: &str) -> AppResult<serde_json::Value> {
    let response_json: String = conn.query_row(
        "SELECT response_json FROM llm_runs WHERE id = ? AND status = 'succeeded'",
        [run_id],
        |row| row.get(0),
    )?;
    Ok(serde_json::from_str(&response_json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_candidate_envelope_from_candidate_bundles_without_paths() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'created')",
            [],
        )
        .expect("doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at, finished_at)
             VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-02T00:00:00Z', '2026-05-02T00:01:00Z')",
            [],
        )
        .expect("run");
        for (id, index, text) in [
            ("block-1", 0, "사업명: RFP 분석 시스템"),
            ("block-2", 1, "요구사항 고유번호 SFR-001 검색 기능"),
        ] {
            conn.execute(
                "INSERT INTO document_blocks (
                    id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                    kind, heading_level, text, bbox_json, raw_json
                 ) VALUES (?, 'run-1', 'doc-1', ?, 1, ?, 'paragraph', NULL, ?, '[1.0,2.0,3.0,4.0]', '{}')",
                rusqlite::params![id, id, index, text],
            )
            .expect("block");
        }

        let envelope =
            load_candidate_envelope_for_document(&conn, "doc-1", LlmSchemaName::ProjectInfo)
                .expect("envelope");

        assert_eq!(envelope.document_id, "doc-1");
        assert_eq!(envelope.extraction_run_id, "run-1");
        assert!(envelope
            .candidate_blocks
            .iter()
            .any(|block| block.block_id == "block-1" && block.text.contains("사업명")));
        let serialized = serde_json::to_string(&envelope).expect("json");
        assert!(!serialized.contains("sample.pdf"));
        assert!(!serialized.contains("raw_json"));
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::validation;

use super::contracts::{
    LlmInputEnvelope, LlmProvider, LlmRunSummary, LlmSchemaName, ProviderStructuredResponse,
};
use super::gemini;
use super::http::LlmHttpTransport;
use super::openai;
use super::prompts::PROMPT_VERSION;
use super::schema_validation::validate_structured_output;
use super::settings::{self, SecretStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunLlmRequest {
    pub schema_name: LlmSchemaName,
    pub input: LlmInputEnvelope,
}

pub fn run_structured_extraction(
    conn: &rusqlite::Connection,
    secret_store: &dyn SecretStore,
    transport: &dyn LlmHttpTransport,
    mut request: RunLlmRequest,
) -> AppResult<LlmRunSummary> {
    request
        .input
        .candidate_blocks
        .retain(|block| !block.text.trim().is_empty());

    let settings = settings::load_llm_settings(conn, secret_store)?;
    if !settings.enabled {
        return Err(AppError::LlmDisabled("LLM opt-in is disabled".into()));
    }
    if settings.offline_mode {
        return Err(AppError::LlmDisabled("LLM offline mode is enabled".into()));
    }
    if settings.model.trim().is_empty() {
        return Err(AppError::LlmDisabled("LLM model is not configured".into()));
    }
    if request.input.candidate_blocks.is_empty() {
        return Err(AppError::LlmDisabled(
            "candidate blocks are unavailable".into(),
        ));
    }
    let api_key = settings::load_api_key(secret_store, &settings.provider)?.ok_or_else(|| {
        AppError::LlmDisabled(format!(
            "{} API key is not configured",
            settings.provider.as_str()
        ))
    })?;

    let run_id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let request_json = request_snapshot(
        &settings.provider,
        &settings.model,
        request.schema_name,
        &request.input,
    );
    conn.execute(
        "INSERT INTO llm_runs (
            id, extraction_run_id, provider, model, schema_name, prompt_version, status,
            input_token_count, output_token_count, request_json, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, 'running', 0, 0, ?, ?)",
        rusqlite::params![
            run_id,
            request.input.extraction_run_id,
            settings.provider.as_str(),
            settings.model,
            request.schema_name.as_str(),
            PROMPT_VERSION,
            serde_json::to_string(&request_json)?,
            created_at,
        ],
    )?;

    match call_with_retry(
        &settings.provider,
        transport,
        &api_key,
        &settings.model,
        request.schema_name,
        &request.input,
    ) {
        Ok(provider_response) => {
            if let Err(error) = validate_structured_output(
                request.schema_name,
                &provider_response.output_json,
                &request.input.candidate_blocks,
            ) {
                persist_rejected_run(
                    conn,
                    &run_id,
                    Some(&provider_response.raw_response_json),
                    &error.to_string(),
                    &request.input.rfp_project_id,
                )?;
                return Err(error);
            }
            persist_succeeded_run(conn, &run_id, &provider_response)?;
            load_run_summary(conn, &run_id)
        }
        Err(error @ AppError::LlmRejected(_)) => {
            persist_rejected_run(
                conn,
                &run_id,
                None,
                &error.to_string(),
                &request.input.rfp_project_id,
            )?;
            Err(error)
        }
        Err(error) => {
            persist_failed_run(conn, &run_id, &error.to_string())?;
            Err(error)
        }
    }
}

fn request_snapshot(
    provider: &LlmProvider,
    model: &str,
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> Value {
    match provider {
        LlmProvider::OpenAi => openai::request_snapshot(model, schema_name, envelope),
        LlmProvider::Gemini => gemini::request_snapshot(model, schema_name, envelope),
    }
}

fn call_with_retry(
    provider: &LlmProvider,
    transport: &dyn LlmHttpTransport,
    api_key: &str,
    model: &str,
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> AppResult<ProviderStructuredResponse> {
    let mut last_error = None;
    for attempt in 1..=3 {
        let result = match provider {
            LlmProvider::OpenAi => openai::call_openai_structured_output(
                transport,
                api_key,
                model,
                schema_name,
                envelope,
            ),
            LlmProvider::Gemini => gemini::call_gemini_structured_output(
                transport,
                api_key,
                model,
                schema_name,
                envelope,
            ),
        };
        match result {
            Ok(response) => return Ok(response),
            Err(error) if attempt < 3 && is_retryable_provider_error(&error) => {
                last_error = Some(error);
                retry_backoff(attempt);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::LlmProvider("provider call failed".into())))
}

fn is_retryable_provider_error(error: &AppError) -> bool {
    let message = error.to_string();
    matches!(error, AppError::LlmProvider(_))
        && ([
            "http_status=408",
            "http_status=409",
            "http_status=429",
            "http_status=500",
            "http_status=502",
            "http_status=503",
            "http_status=504",
        ]
        .iter()
        .any(|status| message.contains(status))
            || message.to_ascii_lowercase().contains("timeout")
            || message.to_ascii_lowercase().contains("connection reset"))
}

fn retry_backoff(attempt: usize) {
    #[cfg(not(test))]
    {
        let millis = if attempt == 1 { 500 } else { 1500 };
        std::thread::sleep(std::time::Duration::from_millis(millis));
    }
    #[cfg(test)]
    {
        let _ = attempt;
    }
}

fn persist_succeeded_run(
    conn: &rusqlite::Connection,
    run_id: &str,
    response: &ProviderStructuredResponse,
) -> AppResult<()> {
    conn.execute(
        "UPDATE llm_runs
         SET status = 'succeeded',
             input_token_count = ?,
             output_token_count = ?,
             response_json = ?,
             error_message = NULL,
             finished_at = ?
         WHERE id = ?",
        rusqlite::params![
            response.input_token_count,
            response.output_token_count,
            serde_json::to_string(&response.output_json)?,
            chrono::Utc::now().to_rfc3339(),
            run_id,
        ],
    )?;
    Ok(())
}

fn persist_rejected_run(
    conn: &rusqlite::Connection,
    run_id: &str,
    response_json: Option<&Value>,
    error_message: &str,
    rfp_project_id: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE llm_runs
         SET status = 'rejected',
             response_json = ?,
             error_message = ?,
             finished_at = ?
         WHERE id = ?",
        rusqlite::params![
            response_json.map(serde_json::to_string).transpose()?,
            error_message,
            chrono::Utc::now().to_rfc3339(),
            run_id,
        ],
    )?;
    if error_message.contains("missing_evidence") {
        validation::insert_llm_rejection_finding(
            conn,
            rfp_project_id,
            "missing_evidence",
            "LLM 구조화 결과가 원문 근거 검증을 통과하지 못했습니다.",
        )?;
    } else if error_message.contains("schema_invalid") {
        validation::insert_llm_rejection_finding(
            conn,
            rfp_project_id,
            "schema_invalid",
            "LLM 구조화 결과가 schema를 통과하지 못했습니다.",
        )?;
    }
    Ok(())
}

fn persist_failed_run(
    conn: &rusqlite::Connection,
    run_id: &str,
    error_message: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE llm_runs
         SET status = 'failed',
             error_message = ?,
             finished_at = ?
         WHERE id = ?",
        rusqlite::params![error_message, chrono::Utc::now().to_rfc3339(), run_id],
    )?;
    Ok(())
}

fn load_run_summary(conn: &rusqlite::Connection, run_id: &str) -> AppResult<LlmRunSummary> {
    Ok(conn.query_row(
        "SELECT id, extraction_run_id, provider, model, schema_name, prompt_version, status,
                input_token_count, output_token_count, error_message, created_at, finished_at
         FROM llm_runs
         WHERE id = ?",
        [run_id],
        |row| {
            Ok(LlmRunSummary {
                id: row.get(0)?,
                extraction_run_id: row.get(1)?,
                provider: row.get(2)?,
                model: row.get(3)?,
                schema_name: row.get(4)?,
                prompt_version: row.get(5)?,
                status: row.get(6)?,
                input_token_count: row.get(7)?,
                output_token_count: row.get(8)?,
                error_message: row.get(9)?,
                created_at: row.get(10)?,
                finished_at: row.get(11)?,
            })
        },
    )?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::llm_adapter::contracts::{CandidateBlock, LlmInstructions, LlmProvider};
    use crate::llm_adapter::http::test_support::SequenceTransport;
    use crate::llm_adapter::http::HttpJsonResponse;
    use crate::llm_adapter::settings::test_support::InMemorySecretStore;
    use crate::llm_adapter::settings::{save_llm_settings, SaveLlmSettingsRequest};

    fn seeded_conn_with_project_and_extraction() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrate(&conn).expect("migrate");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'created')",
            [],
        )
        .expect("doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-02T00:00:00Z')",
            [],
        )
        .expect("run");
        conn.execute(
            "INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
             VALUES ('project-1', 'doc-1', 'test', 'draft', 'summary', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z')",
            [],
        )
        .expect("project");
        conn
    }

    fn sample_envelope() -> LlmInputEnvelope {
        LlmInputEnvelope {
            document_id: "doc-1".into(),
            rfp_project_id: "project-1".into(),
            extraction_run_id: "run-1".into(),
            language: "ko".into(),
            candidate_blocks: vec![CandidateBlock {
                block_id: "block-1".into(),
                page_number: 1,
                kind: "paragraph".into(),
                text: "사업명: RFP 분석 시스템".into(),
                bbox: None,
            }],
            instructions: LlmInstructions {
                preserve_korean_terms: true,
                do_not_invent_values: true,
                require_evidence_block_ids: true,
            },
        }
    }

    fn save_enabled_settings(conn: &rusqlite::Connection, provider: LlmProvider, model: &str) {
        save_llm_settings(
            conn,
            &InMemorySecretStore::with_key(provider.clone(), "test-key"),
            SaveLlmSettingsRequest {
                enabled: true,
                offline_mode: false,
                provider,
                model: model.into(),
                api_key: None,
            },
        )
        .expect("settings");
    }

    fn save_offline_settings(conn: &rusqlite::Connection) {
        save_llm_settings(
            conn,
            &InMemorySecretStore::default(),
            SaveLlmSettingsRequest {
                enabled: false,
                offline_mode: true,
                provider: LlmProvider::OpenAi,
                model: String::new(),
                api_key: None,
            },
        )
        .expect("settings");
    }

    fn project_info_success_response() -> serde_json::Value {
        json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "{\"fields\":[{\"field_key\":\"business_name\",\"raw_value\":\"RFP 분석 시스템\",\"normalized_value\":\"RFP 분석 시스템\",\"confidence\":0.91,\"evidence_block_ids\":[\"block-1\"]}]}"
                }]
            }],
            "usage": { "input_tokens": 10, "output_tokens": 4 }
        })
    }

    fn project_info_missing_required_response() -> serde_json::Value {
        json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "{\"fields\":[{\"field_key\":\"business_name\",\"raw_value\":\"RFP 분석 시스템\",\"confidence\":0.91,\"evidence_block_ids\":[\"block-1\"]}]}"
                }]
            }]
        })
    }

    fn retryable_status_response(status: u16) -> HttpJsonResponse {
        HttpJsonResponse {
            status,
            body: json!({ "error": { "message": "retry later" } }),
        }
    }

    #[test]
    fn successful_run_persists_succeeded_llm_run() {
        let conn = seeded_conn_with_project_and_extraction();
        let store = InMemorySecretStore::with_key(LlmProvider::OpenAi, "test-key");
        save_enabled_settings(&conn, LlmProvider::OpenAi, "gpt-5.5");
        let transport = SequenceTransport::single_success(project_info_success_response());

        let summary = run_structured_extraction(
            &conn,
            &store,
            &transport,
            RunLlmRequest {
                schema_name: LlmSchemaName::ProjectInfo,
                input: sample_envelope(),
            },
        )
        .expect("run");

        assert_eq!(summary.status, "succeeded");

        let stored_status: String = conn
            .query_row(
                "SELECT status FROM llm_runs WHERE id = ?",
                [&summary.id],
                |row| row.get(0),
            )
            .expect("stored run");
        assert_eq!(stored_status, "succeeded");
    }

    #[test]
    fn offline_mode_skips_without_persisting_run() {
        let conn = seeded_conn_with_project_and_extraction();
        let store = InMemorySecretStore::default();
        save_offline_settings(&conn);
        let transport = SequenceTransport::unused();

        let error = run_structured_extraction(
            &conn,
            &store,
            &transport,
            RunLlmRequest {
                schema_name: LlmSchemaName::ProjectInfo,
                input: sample_envelope(),
            },
        )
        .expect_err("offline");

        assert!(error.to_string().contains("llm disabled"));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM llm_runs", [], |row| row.get(0))
            .expect("run count");
        assert_eq!(count, 0);
    }

    #[test]
    fn schema_rejection_persists_rejected_run_and_schema_invalid_finding() {
        let conn = seeded_conn_with_project_and_extraction();
        let store = InMemorySecretStore::with_key(LlmProvider::OpenAi, "test-key");
        save_enabled_settings(&conn, LlmProvider::OpenAi, "gpt-5.5");
        let transport = SequenceTransport::single_success(project_info_missing_required_response());

        let error = run_structured_extraction(
            &conn,
            &store,
            &transport,
            RunLlmRequest {
                schema_name: LlmSchemaName::ProjectInfo,
                input: sample_envelope(),
            },
        )
        .expect_err("schema rejection");

        assert!(error.to_string().contains("schema_invalid"));
        let rejected_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM llm_runs WHERE status = 'rejected'",
                [],
                |row| row.get(0),
            )
            .expect("rejected count");
        assert_eq!(rejected_count, 1);
        let finding_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings WHERE finding_type = 'schema_invalid'",
                [],
                |row| row.get(0),
            )
            .expect("finding count");
        assert_eq!(finding_count, 1);
    }

    #[test]
    fn retryable_provider_error_retries_then_succeeds() {
        let conn = seeded_conn_with_project_and_extraction();
        let store = InMemorySecretStore::with_key(LlmProvider::OpenAi, "test-key");
        save_enabled_settings(&conn, LlmProvider::OpenAi, "gpt-5.5");
        let transport = SequenceTransport::new(vec![
            retryable_status_response(429),
            HttpJsonResponse {
                status: 200,
                body: project_info_success_response(),
            },
        ]);

        let summary = run_structured_extraction(
            &conn,
            &store,
            &transport,
            RunLlmRequest {
                schema_name: LlmSchemaName::ProjectInfo,
                input: sample_envelope(),
            },
        )
        .expect("retry success");

        assert_eq!(summary.status, "succeeded");
        assert_eq!(transport.call_count(), 2);
    }
}

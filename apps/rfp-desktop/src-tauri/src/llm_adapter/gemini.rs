use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

use super::contracts::{LlmInputEnvelope, LlmSchemaName, ProviderStructuredResponse};
use super::http::{sanitize_provider_message, LlmHttpTransport};
use super::prompts::{system_prompt, user_prompt, PROMPT_VERSION};
use super::schemas::schema_for;

pub fn call_gemini_structured_output(
    transport: &dyn LlmHttpTransport,
    api_key: &str,
    model: &str,
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> AppResult<ProviderStructuredResponse> {
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let response = transport.post_json(
        &url,
        vec![
            ("x-goog-api-key".into(), api_key.to_string()),
            ("Content-Type".into(), "application/json".into()),
        ],
        gemini_request_body(schema_name, envelope)?,
    )?;

    if !(200..=299).contains(&response.status) {
        return Err(AppError::LlmProvider(provider_status_message(
            response.status,
            &response.body,
        )));
    }

    let output_json = parse_gemini_output_json(&response.body)?;
    Ok(ProviderStructuredResponse {
        output_json,
        raw_response_json: response.body.clone(),
        input_token_count: response.body["usageMetadata"]["promptTokenCount"]
            .as_i64()
            .unwrap_or(0),
        output_token_count: response.body["usageMetadata"]["candidatesTokenCount"]
            .as_i64()
            .unwrap_or(0),
    })
}

pub fn request_snapshot(
    model: &str,
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> Value {
    json!({
        "endpoint": "gemini.generateContent",
        "model": model,
        "schema_name": schema_name.as_str(),
        "prompt_version": PROMPT_VERSION,
        "input": envelope
    })
}

fn gemini_request_body(
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> AppResult<Value> {
    let text = format!(
        "{}\n\n{}",
        system_prompt(schema_name),
        user_prompt(envelope)?
    );
    Ok(json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": text }]
        }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema_for(schema_name)
        }
    }))
}

fn parse_gemini_output_json(body: &Value) -> AppResult<Value> {
    if let Some(reason) = body["promptFeedback"]["blockReason"].as_str() {
        return Err(provider_refusal(reason));
    }

    let first_candidate = body["candidates"]
        .as_array()
        .and_then(|values| values.first());
    let candidate = first_candidate
        .ok_or_else(|| AppError::LlmRejected("schema_invalid: missing Gemini candidate".into()))?;
    if let Some(reason) = candidate["finishReason"].as_str() {
        if matches!(reason, "SAFETY" | "RECITATION" | "PROHIBITED_CONTENT") {
            return Err(provider_refusal(reason));
        }
    }

    let text = candidate["content"]["parts"]
        .as_array()
        .and_then(|parts| parts.first())
        .and_then(|part| part["text"].as_str())
        .ok_or_else(|| AppError::LlmRejected("schema_invalid: missing Gemini text".into()))?;

    serde_json::from_str(text)
        .map_err(|error| AppError::LlmRejected(format!("schema_invalid: {error}")))
}

fn provider_refusal(reason: &str) -> AppError {
    AppError::LlmRejected(format!(
        "provider_refusal: {}",
        sanitize_provider_message(reason)
    ))
}

fn provider_status_message(status: u16, body: &Value) -> String {
    let message = body["error"]["message"]
        .as_str()
        .or_else(|| body["message"].as_str())
        .unwrap_or("provider returned non-success status");
    format!(
        "http_status={status}: {}",
        sanitize_provider_message(message)
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::llm_adapter::contracts::{CandidateBlock, LlmInstructions};
    use crate::llm_adapter::http::test_support::RecordingTransport;

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

    #[test]
    fn gemini_request_uses_response_json_schema_and_excludes_api_key() {
        let transport = RecordingTransport::new(json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "{\"fields\":[]}" }]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 8,
                "candidatesTokenCount": 3
            }
        }));

        let result = call_gemini_structured_output(
            &transport,
            "test-key",
            "gemini-2.5-flash",
            LlmSchemaName::ProjectInfo,
            &sample_envelope(),
        )
        .expect("gemini call");

        let request = transport.last_body().expect("request body");
        assert_eq!(
            request["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert!(request["generationConfig"]["responseJsonSchema"].is_object());
        assert_eq!(result.output_json, json!({ "fields": [] }));
        assert!(!serde_json::to_string(&request)
            .unwrap()
            .contains("test-key"));
    }

    #[test]
    fn gemini_safety_finish_reason_becomes_rejected() {
        let transport = RecordingTransport::new(json!({
            "candidates": [{ "finishReason": "SAFETY" }]
        }));

        let error = call_gemini_structured_output(
            &transport,
            "test-key",
            "gemini-2.5-flash",
            LlmSchemaName::ProjectInfo,
            &sample_envelope(),
        )
        .expect_err("safety refusal");

        assert!(error.to_string().contains("provider_refusal"));
    }

    #[test]
    #[ignore]
    fn gemini_live_structured_output_roundtrip() {
        let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY");
        let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".into());
        let transport = crate::llm_adapter::http::ReqwestTransport::new().expect("transport");
        let envelope = sample_envelope();

        let response = call_gemini_structured_output(
            &transport,
            &api_key,
            &model,
            LlmSchemaName::ProjectInfo,
            &envelope,
        )
        .expect("live gemini");

        crate::llm_adapter::schema_validation::validate_structured_output(
            LlmSchemaName::ProjectInfo,
            &response.output_json,
            &envelope.candidate_blocks,
        )
        .expect("schema valid");
    }
}

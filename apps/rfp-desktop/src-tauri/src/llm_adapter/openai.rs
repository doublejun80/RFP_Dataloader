use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

use super::contracts::{LlmInputEnvelope, LlmSchemaName, ProviderStructuredResponse};
use super::http::{sanitize_provider_message, LlmHttpTransport};
use super::prompts::{system_prompt, user_prompt, PROMPT_VERSION};
use super::schemas::schema_for;

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

pub fn call_openai_structured_output(
    transport: &dyn LlmHttpTransport,
    api_key: &str,
    model: &str,
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> AppResult<ProviderStructuredResponse> {
    let request = openai_request_body(model, schema_name, envelope)?;
    let response = transport.post_json(
        OPENAI_RESPONSES_URL,
        vec![
            ("Authorization".into(), format!("Bearer {api_key}")),
            ("Content-Type".into(), "application/json".into()),
        ],
        request,
    )?;

    if !(200..=299).contains(&response.status) {
        return Err(AppError::LlmProvider(provider_status_message(
            response.status,
            &response.body,
        )));
    }

    let output_json = parse_openai_output_json(&response.body)?;
    Ok(ProviderStructuredResponse {
        output_json,
        raw_response_json: response.body.clone(),
        input_token_count: response.body["usage"]["input_tokens"].as_i64().unwrap_or(0),
        output_token_count: response.body["usage"]["output_tokens"]
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
        "endpoint": "openai.responses",
        "model": model,
        "schema_name": schema_name.as_str(),
        "prompt_version": PROMPT_VERSION,
        "input": envelope
    })
}

fn openai_request_body(
    model: &str,
    schema_name: LlmSchemaName,
    envelope: &LlmInputEnvelope,
) -> AppResult<Value> {
    Ok(json!({
        "model": model,
        "input": [
            { "role": "system", "content": system_prompt(schema_name) },
            { "role": "user", "content": user_prompt(envelope)? }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": schema_name.as_str(),
                "strict": true,
                "schema": schema_for(schema_name)
            }
        }
    }))
}

fn parse_openai_output_json(body: &Value) -> AppResult<Value> {
    if let Some(parsed) = body.get("output_parsed") {
        return Ok(parsed.clone());
    }

    for output in body["output"].as_array().into_iter().flatten() {
        for content in output["content"].as_array().into_iter().flatten() {
            if content["type"] == "refusal" {
                let message = content["refusal"]
                    .as_str()
                    .unwrap_or("provider refused structured output");
                return Err(AppError::LlmRejected(format!(
                    "provider_refusal: {}",
                    sanitize_provider_message(message)
                )));
            }
            if let Some(parsed) = content.get("output_parsed") {
                return Ok(parsed.clone());
            }
            if content["type"] == "output_text" {
                let text = content["text"].as_str().ok_or_else(|| {
                    AppError::LlmRejected("schema_invalid: missing output_text text".into())
                })?;
                return serde_json::from_str(text)
                    .map_err(|error| AppError::LlmRejected(format!("schema_invalid: {error}")));
            }
        }
    }

    Err(AppError::LlmRejected(
        "schema_invalid: no structured output text".into(),
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
    fn openai_request_uses_json_schema_and_excludes_api_key_from_snapshot() {
        let transport = RecordingTransport::new(json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "{\"fields\":[]}"
                }]
            }],
            "usage": { "input_tokens": 10, "output_tokens": 4 }
        }));

        let result = call_openai_structured_output(
            &transport,
            "test-key",
            "gpt-4o-mini",
            LlmSchemaName::ProjectInfo,
            &sample_envelope(),
        )
        .expect("openai call");

        let request = transport.last_body().expect("request body");
        assert_eq!(request["text"]["format"]["type"], "json_schema");
        assert_eq!(request["text"]["format"]["strict"], true);
        assert_eq!(result.output_json, json!({ "fields": [] }));
        assert!(!serde_json::to_string(&request)
            .unwrap()
            .contains("test-key"));
    }

    #[test]
    fn openai_refusal_becomes_rejected() {
        let transport = RecordingTransport::new(json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "refusal",
                    "refusal": "Cannot process this request"
                }]
            }]
        }));

        let error = call_openai_structured_output(
            &transport,
            "test-key",
            "gpt-4o-mini",
            LlmSchemaName::ProjectInfo,
            &sample_envelope(),
        )
        .expect_err("refusal");

        assert!(error.to_string().contains("provider_refusal"));
    }

    #[test]
    #[ignore]
    fn openai_live_structured_output_roundtrip() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY");
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
        let transport = crate::llm_adapter::http::ReqwestTransport::new().expect("transport");
        let envelope = sample_envelope();

        let response = call_openai_structured_output(
            &transport,
            &api_key,
            &model,
            LlmSchemaName::ProjectInfo,
            &envelope,
        )
        .expect("live openai");

        crate::llm_adapter::schema_validation::validate_structured_output(
            LlmSchemaName::ProjectInfo,
            &response.output_json,
            &envelope.candidate_blocks,
        )
        .expect("schema valid");
    }
}

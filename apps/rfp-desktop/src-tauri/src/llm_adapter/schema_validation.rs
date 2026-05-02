use std::collections::HashSet;

use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::contracts::{CandidateBlock, LlmSchemaName};
use super::schemas::schema_for;

pub fn validate_structured_output(
    schema_name: LlmSchemaName,
    output: &Value,
    candidate_blocks: &[CandidateBlock],
) -> AppResult<()> {
    let schema = schema_for(schema_name);
    let compiled = jsonschema::validator_for(&schema)
        .map_err(|error| AppError::LlmRejected(format!("invalid local schema: {error}")))?;

    if !compiled.is_valid(output) {
        let messages = compiled
            .iter_errors(output)
            .map(|error| format!("{} at {}", error, error.instance_path()))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(AppError::LlmRejected(format!("schema_invalid: {messages}")));
    }

    let allowed_block_ids = candidate_blocks
        .iter()
        .map(|block| block.block_id.as_str())
        .collect::<HashSet<_>>();
    validate_evidence_ids(output, &allowed_block_ids)
}

fn validate_evidence_ids(value: &Value, allowed_block_ids: &HashSet<&str>) -> AppResult<()> {
    match value {
        Value::Object(map) => {
            if let Some(ids) = map.get("evidence_block_ids") {
                let array = ids.as_array().ok_or_else(|| {
                    AppError::LlmRejected("evidence_block_ids must be an array".into())
                })?;
                if array.is_empty() {
                    return Err(AppError::LlmRejected(
                        "missing_evidence: evidence_block_ids is empty".into(),
                    ));
                }
                for id in array {
                    let id = id.as_str().ok_or_else(|| {
                        AppError::LlmRejected("evidence_block_ids entries must be strings".into())
                    })?;
                    if !allowed_block_ids.contains(id) {
                        return Err(AppError::LlmRejected(format!(
                            "missing_evidence: unknown evidence block id {id}"
                        )));
                    }
                }
            }

            for child in map.values() {
                validate_evidence_ids(child, allowed_block_ids)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for child in values {
                validate_evidence_ids(child, allowed_block_ids)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::llm_adapter::contracts::CandidateBlock;

    fn blocks() -> Vec<CandidateBlock> {
        vec![CandidateBlock {
            block_id: "block-1".into(),
            page_number: 1,
            kind: "paragraph".into(),
            text: "사업명: RFP 분석 시스템".into(),
            bbox: None,
        }]
    }

    #[test]
    fn accepts_valid_project_info_with_known_evidence() {
        let output = json!({
            "fields": [{
                "field_key": "business_name",
                "raw_value": "RFP 분석 시스템",
                "normalized_value": "RFP 분석 시스템",
                "confidence": 0.91,
                "evidence_block_ids": ["block-1"]
            }]
        });

        validate_structured_output(LlmSchemaName::ProjectInfo, &output, &blocks())
            .expect("valid output");
    }

    #[test]
    fn rejects_schema_mismatch() {
        let output = json!({
            "fields": [{
                "field_key": "business_name",
                "raw_value": "RFP 분석 시스템",
                "confidence": 0.91,
                "evidence_block_ids": ["block-1"]
            }]
        });

        let error = validate_structured_output(LlmSchemaName::ProjectInfo, &output, &blocks())
            .expect_err("schema rejection");
        assert!(error.to_string().contains("schema_invalid"));
    }

    #[test]
    fn rejects_unknown_evidence_block_id() {
        let output = json!({
            "fields": [{
                "field_key": "business_name",
                "raw_value": "RFP 분석 시스템",
                "normalized_value": "RFP 분석 시스템",
                "confidence": 0.91,
                "evidence_block_ids": ["block-missing"]
            }]
        });

        let error = validate_structured_output(LlmSchemaName::ProjectInfo, &output, &blocks())
            .expect_err("evidence rejection");
        assert!(error.to_string().contains("unknown evidence block id"));
    }
}

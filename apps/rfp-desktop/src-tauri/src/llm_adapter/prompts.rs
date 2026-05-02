use super::contracts::{LlmInputEnvelope, LlmSchemaName};

pub const PROMPT_VERSION: &str = "rfp-v2-llm-2026-05-02";

pub fn system_prompt(schema_name: LlmSchemaName) -> String {
    format!(
        "You are a structured RFP extraction component. Return only JSON for schema '{}'. Preserve Korean source terms. Do not invent values. Every extracted item must cite evidence_block_ids from the provided candidate_blocks. Empty arrays are allowed when evidence is insufficient.",
        schema_name.as_str()
    )
}

pub fn user_prompt(envelope: &LlmInputEnvelope) -> crate::error::AppResult<String> {
    Ok(serde_json::to_string_pretty(envelope)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_adapter::contracts::{CandidateBlock, LlmInstructions};

    #[test]
    fn user_prompt_contains_candidate_blocks_but_no_file_paths() {
        let envelope = LlmInputEnvelope {
            document_id: "doc-1".into(),
            rfp_project_id: "project-1".into(),
            extraction_run_id: "run-1".into(),
            language: "ko".into(),
            candidate_blocks: vec![CandidateBlock {
                block_id: "block-1".into(),
                page_number: 12,
                kind: "table".into(),
                text: "요구사항 고유번호 SFR-001".into(),
                bbox: Some(vec![72.0, 400.0, 540.0, 650.0]),
            }],
            instructions: LlmInstructions {
                preserve_korean_terms: true,
                do_not_invent_values: true,
                require_evidence_block_ids: true,
            },
        };

        let prompt = user_prompt(&envelope).expect("prompt");

        assert!(prompt.contains("SFR-001"));
        assert!(prompt.contains("block-1"));
        assert!(!prompt.contains(".pdf"));
        assert!(!prompt.contains("OPENAI_API_KEY"));
        assert!(!prompt.contains("GEMINI_API_KEY"));
    }
}

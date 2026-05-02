use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmInputEnvelope {
    pub document_id: String,
    pub rfp_project_id: String,
    pub extraction_run_id: String,
    pub language: String,
    pub candidate_blocks: Vec<CandidateBlock>,
    pub instructions: LlmInstructions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBlock {
    pub block_id: String,
    pub page_number: i64,
    pub kind: String,
    pub text: String,
    pub bbox: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmInstructions {
    pub preserve_korean_terms: bool,
    pub do_not_invent_values: bool,
    pub require_evidence_block_ids: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmSchemaName {
    ProjectInfo,
    Requirements,
    Procurement,
    RiskClassification,
}

impl LlmSchemaName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectInfo => "project_info",
            Self::Requirements => "requirements",
            Self::Procurement => "procurement",
            Self::RiskClassification => "risk_classification",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    #[serde(rename = "openai")]
    OpenAi,
    Gemini,
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStructuredResponse {
    pub output_json: serde_json::Value,
    pub raw_response_json: serde_json::Value,
    pub input_token_count: i64,
    pub output_token_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmRunSummary {
    pub id: String,
    pub extraction_run_id: String,
    pub provider: String,
    pub model: String,
    pub schema_name: String,
    pub prompt_version: String,
    pub status: String,
    pub input_token_count: i64,
    pub output_token_count: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

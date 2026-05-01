use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub file_name: Option<String>,
    pub blocker_count: i64,
    pub warning_count: i64,
    pub block_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockPreview {
    pub id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionRunSummary {
    pub id: String,
    pub document_id: String,
    pub status: String,
    pub mode: String,
    pub json_path: Option<String>,
    pub markdown_path: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSummary {
    pub document: DocumentSummary,
    pub extraction: Option<ExtractionRunSummary>,
    pub ready_count: i64,
    pub review_needed_count: i64,
    pub failed_count: i64,
}

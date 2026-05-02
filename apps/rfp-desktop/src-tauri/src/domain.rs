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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLinkDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub document_block_id: String,
    pub quote: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RfpFieldDto {
    pub id: String,
    pub field_key: String,
    pub label: String,
    pub raw_value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub source: String,
    pub evidence: Vec<EvidenceLinkDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBundleSummaryDto {
    pub bundle_key: String,
    pub candidate_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateExtractionSummary {
    pub document: DocumentSummary,
    pub project_id: String,
    pub fields: Vec<RfpFieldDto>,
    pub bundles: Vec<CandidateBundleSummaryDto>,
    pub ready_count: i64,
    pub review_needed_count: i64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProjectDto {
    pub document: DocumentSummary,
    pub project: Option<ReviewProjectSummary>,
    pub overview_fields: Vec<ReviewFieldDto>,
    pub requirements: Vec<RequirementReviewRow>,
    pub procurement_items: Vec<ProcurementItemReviewRow>,
    pub staffing_requirements: Vec<StaffingReviewRow>,
    pub deliverables: Vec<DeliverableReviewRow>,
    pub acceptance_criteria: Vec<AcceptanceReviewRow>,
    pub risk_clauses: Vec<RiskReviewRow>,
    pub findings: Vec<ValidationFindingDto>,
    pub metrics: ReviewMetricsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProjectSummary {
    pub id: String,
    pub status: String,
    pub summary: String,
    pub analysis_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFieldDto {
    pub id: String,
    pub field_key: String,
    pub label: String,
    pub raw_value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub source: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RequirementReviewRow {
    pub id: String,
    pub requirement_code: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub mandatory: bool,
    pub confidence: f64,
    pub source: String,
    pub evidence_count: i64,
    pub blocker_count: i64,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcurementItemReviewRow {
    pub id: String,
    pub item_type: String,
    pub name: String,
    pub spec: String,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub required: bool,
    pub confidence: f64,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StaffingReviewRow {
    pub id: String,
    pub role: String,
    pub grade: String,
    pub headcount: Option<f64>,
    pub mm: Option<f64>,
    pub onsite: Option<bool>,
    pub period_text: String,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeliverableReviewRow {
    pub id: String,
    pub name: String,
    pub due_text: String,
    pub format_text: String,
    pub description: String,
    pub confidence: f64,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceReviewRow {
    pub id: String,
    pub criterion_type: String,
    pub description: String,
    pub threshold: String,
    pub due_text: String,
    pub confidence: f64,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RiskReviewRow {
    pub id: String,
    pub risk_type: String,
    pub severity: String,
    pub description: String,
    pub recommended_action: String,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFindingDto {
    pub id: String,
    pub severity: String,
    pub finding_type: String,
    pub message: String,
    pub target_table: Option<String>,
    pub target_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewMetricsDto {
    pub requirement_count: i64,
    pub procurement_count: i64,
    pub staffing_count: i64,
    pub total_mm: Option<f64>,
    pub high_risk_count: i64,
    pub blocker_count: i64,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceContextDto {
    pub target_table: String,
    pub target_id: String,
    pub evidence: Vec<EvidenceLinkDto>,
    pub blocks: Vec<SourceBlockDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlockDto {
    pub id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub text: String,
    pub bbox_json: Option<String>,
    pub is_direct_evidence: bool,
}

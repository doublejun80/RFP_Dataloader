mod evidence;
mod numeric;

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftSource {
    Rule,
    Llm,
}

impl DraftSource {
    pub fn as_db_value(self) -> &'static str {
        match self {
            DraftSource::Rule => "rule",
            DraftSource::Llm => "llm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDraft {
    pub source: DraftSource,
    pub fields: Vec<FieldDraft>,
    pub requirements: Vec<RequirementDraft>,
    pub procurement_items: Vec<ProcurementItemDraft>,
    pub staffing_requirements: Vec<StaffingRequirementDraft>,
    pub deliverables: Vec<DeliverableDraft>,
    pub acceptance_criteria: Vec<AcceptanceCriterionDraft>,
    pub risk_clauses: Vec<RiskClauseDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDraft {
    pub block_id: String,
    pub quote: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDraft {
    pub field_key: String,
    pub label: String,
    pub raw_value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDraft {
    pub requirement_code: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub mandatory: bool,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcurementItemDraft {
    pub requirement_code: String,
    pub item_type: String,
    pub name: String,
    pub spec: String,
    pub quantity_text: String,
    pub unit: String,
    pub required: bool,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaffingRequirementDraft {
    pub requirement_code: String,
    pub role: String,
    pub grade: String,
    pub headcount_text: String,
    pub mm_text: String,
    pub onsite_text: String,
    pub period_text: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliverableDraft {
    pub requirement_code: String,
    pub name: String,
    pub due_text: String,
    pub format_text: String,
    pub description: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCriterionDraft {
    pub requirement_code: String,
    pub criterion_type: String,
    pub description: String,
    pub threshold: String,
    pub due_text: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskClauseDraft {
    pub requirement_code: String,
    pub risk_type: String,
    pub severity: String,
    pub description: String,
    pub recommended_action: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainRejection {
    pub severity: String,
    pub finding_type: String,
    pub message: String,
    pub target_table: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainWriteSummary {
    pub rfp_project_id: String,
    pub fields_written: usize,
    pub requirements_written: usize,
    pub procurement_items_written: usize,
    pub staffing_requirements_written: usize,
    pub deliverables_written: usize,
    pub acceptance_criteria_written: usize,
    pub risk_clauses_written: usize,
    pub evidence_links_written: usize,
    pub rejected_records: usize,
    pub rejections: Vec<DomainRejection>,
}

pub fn write_domain_draft(
    conn: &Connection,
    rfp_project_id: &str,
    draft: DomainDraft,
) -> AppResult<DomainWriteSummary> {
    let tx = conn.unchecked_transaction()?;
    let document_id: String = tx.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?;

    clear_existing_domain_rows(&tx, rfp_project_id)?;

    let mut writer = WriterState::new(rfp_project_id.to_string(), document_id, draft.source);
    writer.declared_requirement_codes = draft
        .requirements
        .iter()
        .filter_map(|requirement| normalize_code(&requirement.requirement_code))
        .collect();
    writer.write_fields(&tx, &draft.fields)?;
    writer.write_requirements(&tx, &draft.requirements)?;
    writer.write_children(&tx, &draft)?;

    let audit_document_id = writer.document_id.clone();
    let summary = writer.summary();
    tx.execute(
        "INSERT INTO audit_events (
            id, rfp_project_id, document_id, event_type, payload_json, created_at
         ) VALUES (?, ?, ?, 'analysis_completed', ?, ?)",
        params![
            Uuid::new_v4().to_string(),
            rfp_project_id,
            audit_document_id,
            serde_json::to_string(&summary)?,
            Utc::now().to_rfc3339(),
        ],
    )?;
    tx.commit()?;
    Ok(summary)
}

pub fn clear_project_domain_rows(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    clear_existing_domain_rows(&tx, rfp_project_id)?;
    tx.commit()?;
    Ok(())
}

struct WriterState {
    rfp_project_id: String,
    document_id: String,
    source: DraftSource,
    requirement_ids: BTreeMap<String, String>,
    declared_requirement_codes: BTreeSet<String>,
    generated_requirement_index: usize,
    default_generated_requirement_id: Option<String>,
    fields_written: usize,
    requirements_written: usize,
    procurement_items_written: usize,
    staffing_requirements_written: usize,
    deliverables_written: usize,
    acceptance_criteria_written: usize,
    risk_clauses_written: usize,
    evidence_links_written: usize,
    rejected_records: usize,
    rejections: Vec<DomainRejection>,
}

impl WriterState {
    fn new(rfp_project_id: String, document_id: String, source: DraftSource) -> Self {
        Self {
            rfp_project_id,
            document_id,
            source,
            requirement_ids: BTreeMap::new(),
            declared_requirement_codes: BTreeSet::new(),
            generated_requirement_index: 0,
            default_generated_requirement_id: None,
            fields_written: 0,
            requirements_written: 0,
            procurement_items_written: 0,
            staffing_requirements_written: 0,
            deliverables_written: 0,
            acceptance_criteria_written: 0,
            risk_clauses_written: 0,
            evidence_links_written: 0,
            rejected_records: 0,
            rejections: Vec::new(),
        }
    }

    fn write_fields(&mut self, tx: &Transaction<'_>, fields: &[FieldDraft]) -> AppResult<()> {
        let mut seen = BTreeSet::new();
        for draft in fields {
            let field_key = draft.field_key.trim();
            if !is_allowed(field_key, FIELD_KEYS) || !valid_confidence(draft.confidence) {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "사업 기본정보 draft가 domain writer enum 또는 confidence 규칙을 통과하지 못했습니다.",
                    Some("rfp_fields"),
                );
                continue;
            }
            if !seen.insert(field_key.to_string()) {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "중복 field_key가 있어 사업 기본정보 row를 저장하지 않았습니다.",
                    Some("rfp_fields"),
                );
                continue;
            }

            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "사업 기본정보에 같은 문서의 원문 근거가 없습니다.",
                    Some("rfp_fields"),
                );
                continue;
            }

            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO rfp_fields (
                    id, rfp_project_id, field_key, label, raw_value, normalized_value,
                    confidence, source, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    self.rfp_project_id,
                    field_key,
                    draft.label.trim(),
                    draft.raw_value.trim(),
                    draft.normalized_value.trim(),
                    draft.confidence,
                    self.source.as_db_value(),
                    now,
                    now,
                ],
            )?;
            self.evidence_links_written +=
                evidence::insert_evidence_links(tx, "rfp_fields", &id, &evidence_blocks)?;
            self.fields_written += 1;
        }
        Ok(())
    }

    fn write_requirements(
        &mut self,
        tx: &Transaction<'_>,
        requirements: &[RequirementDraft],
    ) -> AppResult<()> {
        let mut seen = BTreeSet::new();
        for draft in requirements {
            let Some(code) = normalize_code(&draft.requirement_code) else {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "명시적 요구사항 draft에 requirement_code가 없습니다.",
                    Some("requirements"),
                );
                continue;
            };
            if !seen.insert(code.clone()) {
                self.record_rejection(
                    true,
                    "blocker",
                    "duplicate_requirement_code",
                    "중복 요구사항 ID가 있어 뒤쪽 row를 저장하지 않았습니다.",
                    Some("requirements"),
                );
                continue;
            }
            if !is_allowed(draft.category.trim(), REQUIREMENT_CATEGORIES)
                || !valid_confidence(draft.confidence)
            {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "요구사항 draft가 domain writer enum 또는 confidence 규칙을 통과하지 못했습니다.",
                    Some("requirements"),
                );
                continue;
            }

            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "요구사항에 같은 문서의 원문 근거가 없습니다.",
                    Some("requirements"),
                );
                continue;
            }

            let id = self.insert_requirement(
                tx,
                &code,
                draft.title.trim(),
                draft.description.trim(),
                draft.category.trim(),
                draft.mandatory,
                draft.confidence,
                &evidence_blocks,
            )?;
            self.requirement_ids.insert(code, id);
        }
        Ok(())
    }

    fn write_children(&mut self, tx: &Transaction<'_>, draft: &DomainDraft) -> AppResult<()> {
        self.write_procurement_items(tx, &draft.procurement_items)?;
        self.write_staffing_requirements(tx, &draft.staffing_requirements)?;
        self.write_deliverables(tx, &draft.deliverables)?;
        self.write_acceptance_criteria(tx, &draft.acceptance_criteria)?;
        self.write_risk_clauses(tx, &draft.risk_clauses)?;
        Ok(())
    }

    fn write_procurement_items(
        &mut self,
        tx: &Transaction<'_>,
        items: &[ProcurementItemDraft],
    ) -> AppResult<()> {
        for draft in items {
            if !is_allowed(draft.item_type.trim(), PROCUREMENT_ITEM_TYPES)
                || !valid_confidence(draft.confidence)
            {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "구매 항목 draft가 domain writer enum 또는 confidence 규칙을 통과하지 못했습니다.",
                    Some("procurement_items"),
                );
                continue;
            }
            let quantity = numeric::parse_number(&draft.quantity_text);
            if quantity.is_some_and(|value| value <= 0.0) {
                self.record_rejection(
                    true,
                    "blocker",
                    "invalid_quantity",
                    "구매 항목 수량이 0 이하로 해석되어 저장하지 않았습니다.",
                    Some("procurement_items"),
                );
                continue;
            }
            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "구매 항목에 같은 문서의 원문 근거가 없습니다.",
                    Some("procurement_items"),
                );
                continue;
            }
            let Some(requirement_id) = self.resolve_requirement_for_child(
                tx,
                &draft.requirement_code,
                "other",
                draft.confidence,
                &evidence_blocks,
                Some("procurement_items"),
            )?
            else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            let unit = if draft.unit.trim().is_empty() {
                numeric::parse_unit(&draft.quantity_text).unwrap_or_default()
            } else {
                draft.unit.trim().to_string()
            };
            tx.execute(
                "INSERT INTO procurement_items (
                    id, requirement_id, item_type, name, spec, quantity, quantity_text,
                    unit, required, confidence, source, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    requirement_id,
                    draft.item_type.trim(),
                    draft.name.trim(),
                    draft.spec.trim(),
                    quantity,
                    draft.quantity_text.trim(),
                    unit,
                    bool_to_i64(draft.required),
                    draft.confidence,
                    self.source.as_db_value(),
                    now,
                    now,
                ],
            )?;
            self.evidence_links_written +=
                evidence::insert_evidence_links(tx, "procurement_items", &id, &evidence_blocks)?;
            self.procurement_items_written += 1;
        }
        Ok(())
    }

    fn write_staffing_requirements(
        &mut self,
        tx: &Transaction<'_>,
        items: &[StaffingRequirementDraft],
    ) -> AppResult<()> {
        for draft in items {
            if !valid_confidence(draft.confidence) {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "투입 인력 draft confidence가 범위를 벗어났습니다.",
                    Some("staffing_requirements"),
                );
                continue;
            }
            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "투입 인력에 같은 문서의 원문 근거가 없습니다.",
                    Some("staffing_requirements"),
                );
                continue;
            }
            let Some(requirement_id) = self.resolve_requirement_for_child(
                tx,
                &draft.requirement_code,
                "staffing",
                draft.confidence,
                &evidence_blocks,
                Some("staffing_requirements"),
            )?
            else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO staffing_requirements (
                    id, requirement_id, role, grade, headcount, headcount_text, mm, mm_text,
                    onsite, onsite_text, period_text, confidence, source, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    requirement_id,
                    draft.role.trim(),
                    draft.grade.trim(),
                    numeric::parse_number(&draft.headcount_text),
                    draft.headcount_text.trim(),
                    numeric::parse_mm(&draft.mm_text),
                    draft.mm_text.trim(),
                    numeric::parse_onsite(&draft.onsite_text),
                    draft.onsite_text.trim(),
                    draft.period_text.trim(),
                    draft.confidence,
                    self.source.as_db_value(),
                    now,
                    now,
                ],
            )?;
            self.evidence_links_written += evidence::insert_evidence_links(
                tx,
                "staffing_requirements",
                &id,
                &evidence_blocks,
            )?;
            self.staffing_requirements_written += 1;
        }
        Ok(())
    }

    fn write_deliverables(
        &mut self,
        tx: &Transaction<'_>,
        items: &[DeliverableDraft],
    ) -> AppResult<()> {
        for draft in items {
            if !valid_confidence(draft.confidence) {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "납품물 draft confidence가 범위를 벗어났습니다.",
                    Some("deliverables"),
                );
                continue;
            }
            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "납품물에 같은 문서의 원문 근거가 없습니다.",
                    Some("deliverables"),
                );
                continue;
            }
            let Some(requirement_id) = self.resolve_requirement_for_child(
                tx,
                &draft.requirement_code,
                "management",
                draft.confidence,
                &evidence_blocks,
                Some("deliverables"),
            )?
            else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO deliverables (
                    id, requirement_id, name, due_text, format_text, description,
                    confidence, source, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    requirement_id,
                    draft.name.trim(),
                    draft.due_text.trim(),
                    draft.format_text.trim(),
                    draft.description.trim(),
                    draft.confidence,
                    self.source.as_db_value(),
                    now,
                    now,
                ],
            )?;
            self.evidence_links_written +=
                evidence::insert_evidence_links(tx, "deliverables", &id, &evidence_blocks)?;
            self.deliverables_written += 1;
        }
        Ok(())
    }

    fn write_acceptance_criteria(
        &mut self,
        tx: &Transaction<'_>,
        items: &[AcceptanceCriterionDraft],
    ) -> AppResult<()> {
        for draft in items {
            if !is_allowed(draft.criterion_type.trim(), ACCEPTANCE_CRITERION_TYPES)
                || !valid_confidence(draft.confidence)
            {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "검수 조건 draft가 domain writer enum 또는 confidence 규칙을 통과하지 못했습니다.",
                    Some("acceptance_criteria"),
                );
                continue;
            }
            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "검수 조건에 같은 문서의 원문 근거가 없습니다.",
                    Some("acceptance_criteria"),
                );
                continue;
            }
            let Some(requirement_id) = self.resolve_requirement_for_child(
                tx,
                &draft.requirement_code,
                "management",
                draft.confidence,
                &evidence_blocks,
                Some("acceptance_criteria"),
            )?
            else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO acceptance_criteria (
                    id, requirement_id, criterion_type, description, threshold, due_text,
                    confidence, source, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    requirement_id,
                    draft.criterion_type.trim(),
                    draft.description.trim(),
                    draft.threshold.trim(),
                    draft.due_text.trim(),
                    draft.confidence,
                    self.source.as_db_value(),
                    now,
                    now,
                ],
            )?;
            self.evidence_links_written +=
                evidence::insert_evidence_links(tx, "acceptance_criteria", &id, &evidence_blocks)?;
            self.acceptance_criteria_written += 1;
        }
        Ok(())
    }

    fn write_risk_clauses(
        &mut self,
        tx: &Transaction<'_>,
        items: &[RiskClauseDraft],
    ) -> AppResult<()> {
        for draft in items {
            if !is_allowed(draft.risk_type.trim(), RISK_TYPES)
                || !is_allowed(draft.severity.trim(), RISK_SEVERITIES)
                || !valid_confidence(draft.confidence)
            {
                self.record_rejection(
                    true,
                    "blocker",
                    "schema_invalid",
                    "리스크 조항 draft가 domain writer enum 또는 confidence 규칙을 통과하지 못했습니다.",
                    Some("risk_clauses"),
                );
                continue;
            }
            let evidence_blocks =
                evidence::load_valid_evidence_blocks(tx, &self.document_id, &draft.evidence)?;
            if evidence_blocks.is_empty() {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "리스크 조항에 같은 문서의 원문 근거가 없습니다.",
                    Some("risk_clauses"),
                );
                continue;
            }
            let Some(requirement_id) = self.resolve_requirement_for_child(
                tx,
                &draft.requirement_code,
                "other",
                draft.confidence,
                &evidence_blocks,
                Some("risk_clauses"),
            )?
            else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO risk_clauses (
                    id, requirement_id, risk_type, severity, description, recommended_action,
                    confidence, source, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    requirement_id,
                    draft.risk_type.trim(),
                    draft.severity.trim(),
                    draft.description.trim(),
                    draft.recommended_action.trim(),
                    draft.confidence,
                    self.source.as_db_value(),
                    now,
                    now,
                ],
            )?;
            self.evidence_links_written +=
                evidence::insert_evidence_links(tx, "risk_clauses", &id, &evidence_blocks)?;
            self.risk_clauses_written += 1;
        }
        Ok(())
    }

    fn resolve_requirement_for_child(
        &mut self,
        tx: &Transaction<'_>,
        requirement_code: &str,
        generated_category: &str,
        child_confidence: f64,
        evidence_blocks: &[(EvidenceDraft, evidence::EvidenceBlock)],
        target_table: Option<&'static str>,
    ) -> AppResult<Option<String>> {
        if let Some(code) = normalize_code(requirement_code) {
            if let Some(id) = self.requirement_ids.get(&code) {
                return Ok(Some(id.clone()));
            }
            if self.declared_requirement_codes.contains(&code) {
                self.record_rejection(
                    true,
                    "blocker",
                    "missing_evidence",
                    "참조한 요구사항이 저장되지 않아 하위 row를 저장하지 않았습니다.",
                    target_table,
                );
                return Ok(None);
            }
        }

        if evidence_blocks.is_empty() {
            self.record_rejection(
                true,
                "blocker",
                "missing_evidence",
                "요구사항 참조가 없는 하위 row에 원문 근거가 없습니다.",
                target_table,
            );
            return Ok(None);
        }

        if let Some(id) = &self.default_generated_requirement_id {
            if let Some(original_code) = normalize_code(requirement_code) {
                self.requirement_ids.insert(original_code, id.clone());
            }
            return Ok(Some(id.clone()));
        }

        self.generated_requirement_index += 1;
        let generated_code = format!("GEN-{index:03}", index = self.generated_requirement_index);
        let title = "프로젝트 공통 요구사항";
        let description = evidence_blocks
            .first()
            .map(|(draft, block)| evidence::build_quote(draft, block))
            .unwrap_or_else(|| "프로젝트 공통 요구사항".to_string());
        let id = self.insert_requirement(
            tx,
            &generated_code,
            title,
            &description,
            generated_category,
            true,
            child_confidence.min(0.6),
            evidence_blocks,
        )?;
        self.requirement_ids
            .insert(generated_code.clone(), id.clone());
        if let Some(original_code) = normalize_code(requirement_code) {
            self.requirement_ids.insert(original_code, id.clone());
        }
        self.default_generated_requirement_id = Some(id.clone());
        self.record_rejection(
            false,
            "warning",
            "unknown_requirement_reference",
            "하위 row가 알 수 없는 요구사항을 참조해 GEN 요구사항을 생성했습니다.",
            Some("requirements"),
        );
        Ok(Some(id))
    }

    fn insert_requirement(
        &mut self,
        tx: &Transaction<'_>,
        requirement_code: &str,
        title: &str,
        description: &str,
        category: &str,
        mandatory: bool,
        confidence: f64,
        evidence_blocks: &[(EvidenceDraft, evidence::EvidenceBlock)],
    ) -> AppResult<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO requirements (
                id, rfp_project_id, requirement_code, title, description, category,
                mandatory, confidence, source, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                self.rfp_project_id,
                requirement_code,
                title,
                description,
                category,
                bool_to_i64(mandatory),
                confidence,
                self.source.as_db_value(),
                now,
                now,
            ],
        )?;
        self.evidence_links_written +=
            evidence::insert_evidence_links(tx, "requirements", &id, evidence_blocks)?;
        self.requirements_written += 1;
        Ok(id)
    }

    fn record_rejection(
        &mut self,
        rejected: bool,
        severity: &str,
        finding_type: &str,
        message: &str,
        target_table: Option<&str>,
    ) {
        if rejected {
            self.rejected_records += 1;
        }
        self.rejections.push(DomainRejection {
            severity: severity.to_string(),
            finding_type: finding_type.to_string(),
            message: message.to_string(),
            target_table: target_table.map(ToOwned::to_owned),
        });
    }

    fn summary(self) -> DomainWriteSummary {
        DomainWriteSummary {
            rfp_project_id: self.rfp_project_id,
            fields_written: self.fields_written,
            requirements_written: self.requirements_written,
            procurement_items_written: self.procurement_items_written,
            staffing_requirements_written: self.staffing_requirements_written,
            deliverables_written: self.deliverables_written,
            acceptance_criteria_written: self.acceptance_criteria_written,
            risk_clauses_written: self.risk_clauses_written,
            evidence_links_written: self.evidence_links_written,
            rejected_records: self.rejected_records,
            rejections: self.rejections,
        }
    }
}

fn clear_existing_domain_rows(tx: &Transaction<'_>, rfp_project_id: &str) -> AppResult<()> {
    for (target_table, target_query) in [
        (
            "risk_clauses",
            "SELECT rc.id FROM risk_clauses rc
             JOIN requirements r ON r.id = rc.requirement_id
             WHERE r.rfp_project_id = ?",
        ),
        (
            "acceptance_criteria",
            "SELECT ac.id FROM acceptance_criteria ac
             JOIN requirements r ON r.id = ac.requirement_id
             WHERE r.rfp_project_id = ?",
        ),
        (
            "deliverables",
            "SELECT d.id FROM deliverables d
             JOIN requirements r ON r.id = d.requirement_id
             WHERE r.rfp_project_id = ?",
        ),
        (
            "staffing_requirements",
            "SELECT sr.id FROM staffing_requirements sr
             JOIN requirements r ON r.id = sr.requirement_id
             WHERE r.rfp_project_id = ?",
        ),
        (
            "procurement_items",
            "SELECT pi.id FROM procurement_items pi
             JOIN requirements r ON r.id = pi.requirement_id
             WHERE r.rfp_project_id = ?",
        ),
        (
            "requirements",
            "SELECT id FROM requirements WHERE rfp_project_id = ?",
        ),
        (
            "rfp_fields",
            "SELECT id FROM rfp_fields WHERE rfp_project_id = ?",
        ),
    ] {
        tx.execute(
            &format!(
                "DELETE FROM evidence_links
                 WHERE target_table = ? AND target_id IN ({target_query})"
            ),
            params![target_table, rfp_project_id],
        )?;
    }

    tx.execute(
        "DELETE FROM risk_clauses
         WHERE requirement_id IN (SELECT id FROM requirements WHERE rfp_project_id = ?)",
        [rfp_project_id],
    )?;
    tx.execute(
        "DELETE FROM acceptance_criteria
         WHERE requirement_id IN (SELECT id FROM requirements WHERE rfp_project_id = ?)",
        [rfp_project_id],
    )?;
    tx.execute(
        "DELETE FROM deliverables
         WHERE requirement_id IN (SELECT id FROM requirements WHERE rfp_project_id = ?)",
        [rfp_project_id],
    )?;
    tx.execute(
        "DELETE FROM staffing_requirements
         WHERE requirement_id IN (SELECT id FROM requirements WHERE rfp_project_id = ?)",
        [rfp_project_id],
    )?;
    tx.execute(
        "DELETE FROM procurement_items
         WHERE requirement_id IN (SELECT id FROM requirements WHERE rfp_project_id = ?)",
        [rfp_project_id],
    )?;
    tx.execute(
        "DELETE FROM requirements WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;
    tx.execute(
        "DELETE FROM rfp_fields WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;
    Ok(())
}

fn normalize_code(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn valid_confidence(value: f64) -> bool {
    (0.0..=1.0).contains(&value)
}

fn is_allowed(value: &str, allowed: &[&str]) -> bool {
    allowed.iter().any(|candidate| *candidate == value)
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

const FIELD_KEYS: &[&str] = &[
    "business_name",
    "client",
    "budget",
    "period",
    "contract_method",
    "deadline",
    "evaluation_ratio",
    "requirement_count",
];

const REQUIREMENT_CATEGORIES: &[&str] = &[
    "functional",
    "technical",
    "security",
    "data",
    "staffing",
    "management",
    "quality",
    "performance",
    "other",
];

const PROCUREMENT_ITEM_TYPES: &[&str] = &[
    "hardware", "software", "license", "cloud", "network", "database", "security", "service",
    "other",
];

const ACCEPTANCE_CRITERION_TYPES: &[&str] = &[
    "test",
    "performance",
    "security",
    "inspection",
    "sla",
    "warranty",
    "other",
];

const RISK_TYPES: &[&str] = &[
    "scope_creep",
    "free_work",
    "short_schedule",
    "liability",
    "ambiguous_spec",
    "vendor_lock",
    "payment",
    "security",
    "other",
];

const RISK_SEVERITIES: &[&str] = &["low", "medium", "high", "blocker"];

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use rusqlite::{params, Connection};

    pub(crate) fn seed_document_project_and_blocks(conn: &Connection) {
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'created')",
            [],
        )
        .expect("insert doc");
        conn.execute(
            "INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
             VALUES ('project-1', 'doc-1', 'test-version', 'draft', '', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z')",
            [],
        )
        .expect("insert project");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
            [],
        )
        .expect("insert run");
        for index in 1..=8 {
            conn.execute(
                "INSERT INTO document_blocks (
                    id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                    kind, heading_level, text, bbox_json, raw_json
                 ) VALUES (?, 'run-1', 'doc-1', ?, 1, ?, 'paragraph', NULL, ?, NULL, '{}')",
                params![
                    format!("block-{index}"),
                    format!("el-{index}"),
                    index,
                    format!("테스트 근거 문장 {index}")
                ],
            )
            .expect("insert block");
        }
    }

    pub(crate) fn evidence(block_id: &str) -> Vec<EvidenceDraft> {
        vec![EvidenceDraft {
            block_id: block_id.to_string(),
            quote: None,
            confidence: 0.9,
        }]
    }

    pub(crate) fn full_domain_draft() -> DomainDraft {
        DomainDraft {
            source: DraftSource::Llm,
            fields: vec![
                FieldDraft {
                    field_key: "business_name".to_string(),
                    label: "사업명".to_string(),
                    raw_value: "AI 서비스 플랫폼 구축".to_string(),
                    normalized_value: "AI 서비스 플랫폼 구축".to_string(),
                    confidence: 0.9,
                    evidence: evidence("block-1"),
                },
                FieldDraft {
                    field_key: "client".to_string(),
                    label: "발주기관".to_string(),
                    raw_value: "테스트 기관".to_string(),
                    normalized_value: "테스트 기관".to_string(),
                    confidence: 0.9,
                    evidence: evidence("block-2"),
                },
                FieldDraft {
                    field_key: "budget".to_string(),
                    label: "예산".to_string(),
                    raw_value: "100,000,000원".to_string(),
                    normalized_value: "100000000 KRW".to_string(),
                    confidence: 0.8,
                    evidence: evidence("block-3"),
                },
                FieldDraft {
                    field_key: "period".to_string(),
                    label: "사업기간".to_string(),
                    raw_value: "계약일로부터 6개월".to_string(),
                    normalized_value: "6개월".to_string(),
                    confidence: 0.8,
                    evidence: evidence("block-4"),
                },
            ],
            requirements: vec![RequirementDraft {
                requirement_code: "SFR-001".to_string(),
                title: "통합 로그인".to_string(),
                description: "통합 로그인 기능을 제공한다.".to_string(),
                category: "functional".to_string(),
                mandatory: true,
                confidence: 0.9,
                evidence: evidence("block-5"),
            }],
            procurement_items: vec![ProcurementItemDraft {
                requirement_code: "SFR-001".to_string(),
                item_type: "software".to_string(),
                name: "인증 솔루션".to_string(),
                spec: "SSO 지원".to_string(),
                quantity_text: "1식".to_string(),
                unit: "식".to_string(),
                required: true,
                confidence: 0.8,
                evidence: evidence("block-6"),
            }],
            staffing_requirements: vec![StaffingRequirementDraft {
                requirement_code: "SFR-001".to_string(),
                role: "PM".to_string(),
                grade: "고급".to_string(),
                headcount_text: "1명".to_string(),
                mm_text: "6MM".to_string(),
                onsite_text: "상주".to_string(),
                period_text: "착수부터 종료까지".to_string(),
                confidence: 0.8,
                evidence: evidence("block-6"),
            }],
            deliverables: vec![DeliverableDraft {
                requirement_code: "SFR-001".to_string(),
                name: "설계서".to_string(),
                due_text: "설계 단계 종료 시".to_string(),
                format_text: "문서".to_string(),
                description: "시스템 설계서를 제출한다.".to_string(),
                confidence: 0.8,
                evidence: evidence("block-7"),
            }],
            acceptance_criteria: vec![AcceptanceCriterionDraft {
                requirement_code: "SFR-001".to_string(),
                criterion_type: "test".to_string(),
                description: "통합 로그인 시험 통과".to_string(),
                threshold: "성공률 100%".to_string(),
                due_text: "검수 시".to_string(),
                confidence: 0.8,
                evidence: evidence("block-7"),
            }],
            risk_clauses: vec![RiskClauseDraft {
                requirement_code: "SFR-001".to_string(),
                risk_type: "ambiguous_spec".to_string(),
                severity: "medium".to_string(),
                description: "세부 연동 범위가 모호하다.".to_string(),
                recommended_action: "질의서로 연동 대상 확정".to_string(),
                confidence: 0.7,
                evidence: evidence("block-8"),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{evidence, full_domain_draft, seed_document_project_and_blocks};
    use super::*;

    #[test]
    fn draft_source_maps_to_db_value() {
        assert_eq!(DraftSource::Rule.as_db_value(), "rule");
        assert_eq!(DraftSource::Llm.as_db_value(), "llm");
    }

    #[test]
    fn writes_full_domain_graph_with_evidence_links() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        let draft = full_domain_draft();
        let summary =
            write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

        assert_eq!(summary.requirements_written, 1);
        assert_eq!(summary.procurement_items_written, 1);
        assert_eq!(summary.staffing_requirements_written, 1);
        assert_eq!(summary.deliverables_written, 1);
        assert_eq!(summary.acceptance_criteria_written, 1);
        assert_eq!(summary.risk_clauses_written, 1);
        assert_eq!(summary.rejected_records, 0);

        let evidence_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM evidence_links", [], |row| row.get(0))
            .expect("evidence count");
        assert!(evidence_count >= 7);
    }

    #[test]
    fn rejects_domain_record_without_evidence() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        let mut draft = full_domain_draft();
        draft.requirements[0].evidence = vec![];

        let summary =
            write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

        assert_eq!(summary.requirements_written, 0);
        assert!(summary.rejected_records >= 1);
        assert!(summary
            .rejections
            .iter()
            .any(|rejection| rejection.finding_type == "missing_evidence"));
    }

    #[test]
    fn rejects_evidence_from_another_document() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-2', 'other.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'created')",
            [],
        )
        .expect("insert other doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-2', 'doc-2', 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
            [],
        )
        .expect("insert other run");
        conn.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES ('foreign-block', 'run-2', 'doc-2', 'el-x', 1, 1, 'paragraph', NULL, '다른 문서 근거', NULL, '{}')",
            [],
        )
        .expect("insert foreign block");

        let mut draft = full_domain_draft();
        draft.requirements[0].evidence = evidence("foreign-block");

        let summary =
            write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

        assert_eq!(summary.requirements_written, 0);
        assert!(summary.rejected_records >= 1);
    }

    #[test]
    fn creates_generated_requirement_for_evidenced_orphan_child() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        let mut draft = full_domain_draft();
        draft.requirements = vec![];
        draft.procurement_items[0].requirement_code = "".to_string();

        let summary =
            write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

        assert_eq!(summary.requirements_written, 1);
        assert_eq!(summary.procurement_items_written, 1);
        let code: String = conn
            .query_row("SELECT requirement_code FROM requirements", [], |row| {
                row.get(0)
            })
            .expect("generated code");
        assert_eq!(code, "GEN-001");
    }

    #[test]
    fn rejects_duplicate_requirement_codes_before_sql_unique_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        let mut draft = full_domain_draft();
        draft.requirements.push(RequirementDraft {
            requirement_code: "SFR-001".to_string(),
            title: "중복 요구사항".to_string(),
            description: "같은 코드".to_string(),
            category: "functional".to_string(),
            mandatory: true,
            confidence: 0.9,
            evidence: evidence("block-5"),
        });

        let summary =
            write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

        assert_eq!(summary.requirements_written, 1);
        assert!(summary.rejected_records >= 1);
        assert!(summary
            .rejections
            .iter()
            .any(|rejection| rejection.finding_type == "duplicate_requirement_code"));
    }

    #[test]
    fn domain_write_records_analysis_completed_audit_event() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        let summary =
            write_domain_draft(&mut conn, "project-1", full_domain_draft()).expect("write");

        let payload: String = conn
            .query_row(
                "SELECT payload_json FROM audit_events
                 WHERE rfp_project_id = 'project-1' AND event_type = 'analysis_completed'",
                [],
                |row| row.get(0),
            )
            .expect("audit payload");
        assert!(payload.contains(&format!(
            "\"requirementsWritten\":{}",
            summary.requirements_written
        )));
    }
}

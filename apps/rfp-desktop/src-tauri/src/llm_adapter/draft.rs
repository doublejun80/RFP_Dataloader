use serde_json::Value;

use crate::domain_writer::{
    AcceptanceCriterionDraft, DeliverableDraft, DomainDraft, DraftSource, EvidenceDraft,
    FieldDraft, ProcurementItemDraft, RequirementDraft, RiskClauseDraft, StaffingRequirementDraft,
};
use crate::error::AppResult;

#[derive(Debug, Clone, Default)]
pub struct StructuredOutputs {
    pub project_info: Option<Value>,
    pub requirements: Option<Value>,
    pub procurement: Option<Value>,
    pub risk_classification: Option<Value>,
}

pub fn domain_draft_from_outputs(outputs: StructuredOutputs) -> AppResult<DomainDraft> {
    let mut draft = DomainDraft {
        source: DraftSource::Llm,
        fields: Vec::new(),
        requirements: Vec::new(),
        procurement_items: Vec::new(),
        staffing_requirements: Vec::new(),
        deliverables: Vec::new(),
        acceptance_criteria: Vec::new(),
        risk_clauses: Vec::new(),
    };

    if let Some(project_info) = outputs.project_info {
        for item in array(&project_info, "fields") {
            draft.fields.push(FieldDraft {
                field_key: text(item, "field_key"),
                label: field_label(&text(item, "field_key")).to_string(),
                raw_value: text(item, "raw_value"),
                normalized_value: text(item, "normalized_value"),
                confidence: confidence(item),
                evidence: evidence(item),
            });
        }
    }

    if let Some(requirements) = outputs.requirements {
        for item in array(&requirements, "requirements") {
            draft.requirements.push(RequirementDraft {
                requirement_code: text(item, "requirement_code"),
                title: text(item, "title"),
                description: text(item, "description"),
                category: text(item, "category"),
                mandatory: item["mandatory"].as_bool().unwrap_or(false),
                confidence: confidence(item),
                evidence: evidence(item),
            });
        }
    }

    if let Some(procurement) = outputs.procurement {
        append_procurement_output(&mut draft, &procurement);
    }

    if let Some(risks) = outputs.risk_classification {
        for item in array(&risks, "risk_clauses") {
            draft.risk_clauses.push(risk_clause(item));
        }
    }

    Ok(draft)
}

fn append_procurement_output(draft: &mut DomainDraft, value: &Value) {
    for item in array(value, "procurement_items") {
        draft.procurement_items.push(ProcurementItemDraft {
            requirement_code: text(item, "requirement_code"),
            item_type: text(item, "item_type"),
            name: text(item, "name"),
            spec: text(item, "spec"),
            quantity_text: text(item, "quantity_text"),
            unit: text(item, "unit"),
            required: true,
            confidence: confidence(item),
            evidence: evidence(item),
        });
    }
    for item in array(value, "staffing_requirements") {
        draft.staffing_requirements.push(StaffingRequirementDraft {
            requirement_code: text(item, "requirement_code"),
            role: text(item, "role"),
            grade: text(item, "grade"),
            headcount_text: text(item, "headcount_text"),
            mm_text: text(item, "mm_text"),
            onsite_text: text(item, "onsite_text"),
            period_text: String::new(),
            confidence: confidence(item),
            evidence: evidence(item),
        });
    }
    for item in array(value, "deliverables") {
        draft.deliverables.push(DeliverableDraft {
            requirement_code: text(item, "requirement_code"),
            name: text(item, "name"),
            due_text: text(item, "due_text"),
            format_text: text(item, "format_text"),
            description: text(item, "description"),
            confidence: confidence(item),
            evidence: evidence(item),
        });
    }
    for item in array(value, "acceptance_criteria") {
        draft.acceptance_criteria.push(AcceptanceCriterionDraft {
            requirement_code: text(item, "requirement_code"),
            criterion_type: text(item, "criterion_type"),
            description: text(item, "description"),
            threshold: text(item, "threshold"),
            due_text: String::new(),
            confidence: confidence(item),
            evidence: evidence(item),
        });
    }
    for item in array(value, "risk_clauses") {
        draft.risk_clauses.push(risk_clause(item));
    }
}

fn risk_clause(item: &Value) -> RiskClauseDraft {
    RiskClauseDraft {
        requirement_code: text(item, "requirement_code"),
        risk_type: text(item, "risk_type"),
        severity: text(item, "severity"),
        description: text(item, "description"),
        recommended_action: text(item, "recommended_action"),
        confidence: confidence(item),
        evidence: evidence(item),
    }
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().trim().to_string()
}

fn confidence(value: &Value) -> f64 {
    value["confidence"].as_f64().unwrap_or(0.0)
}

fn evidence(value: &Value) -> Vec<EvidenceDraft> {
    let confidence = confidence(value);
    value["evidence_block_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| id.as_str())
        .map(|block_id| EvidenceDraft {
            block_id: block_id.to_string(),
            quote: None,
            confidence,
        })
        .collect()
}

fn field_label(field_key: &str) -> &'static str {
    match field_key {
        "business_name" => "사업명",
        "client" => "발주기관",
        "budget" => "사업예산",
        "period" => "사업기간",
        "contract_method" => "계약방식",
        "deadline" => "제출마감",
        "evaluation_ratio" => "평가비율",
        _ => "기본정보",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_project_requirements_procurement_and_risks_to_domain_draft() {
        let draft = domain_draft_from_outputs(StructuredOutputs {
            project_info: Some(json!({
                "fields": [{
                    "field_key": "business_name",
                    "raw_value": "RFP 분석 시스템",
                    "normalized_value": "RFP 분석 시스템",
                    "confidence": 0.91,
                    "evidence_block_ids": ["block-1"]
                }]
            })),
            requirements: Some(json!({
                "requirements": [{
                    "requirement_code": "SFR-001",
                    "title": "검색 기능",
                    "description": "문서 검색 기능을 제공한다.",
                    "category": "functional",
                    "mandatory": true,
                    "confidence": 0.88,
                    "evidence_block_ids": ["block-2"]
                }]
            })),
            procurement: Some(json!({
                "procurement_items": [{
                    "requirement_code": "SFR-001",
                    "item_type": "software",
                    "name": "검색 엔진",
                    "spec": "한국어 검색",
                    "quantity_text": "1식",
                    "unit": "식",
                    "confidence": 0.82,
                    "evidence_block_ids": ["block-3"]
                }],
                "staffing_requirements": [],
                "deliverables": [],
                "acceptance_criteria": [],
                "risk_clauses": []
            })),
            risk_classification: Some(json!({
                "risk_clauses": [{
                    "requirement_code": "SFR-001",
                    "risk_type": "scope_creep",
                    "severity": "medium",
                    "description": "추가 요청 가능성이 있음",
                    "recommended_action": "범위 질의 필요",
                    "confidence": 0.7,
                    "evidence_block_ids": ["block-4"]
                }]
            })),
        })
        .expect("draft");

        assert_eq!(draft.fields.len(), 1);
        assert_eq!(draft.requirements.len(), 1);
        assert_eq!(draft.procurement_items.len(), 1);
        assert_eq!(draft.risk_clauses.len(), 1);
        assert_eq!(draft.source.as_db_value(), "llm");
        assert_eq!(draft.fields[0].evidence[0].block_id, "block-1");
    }
}

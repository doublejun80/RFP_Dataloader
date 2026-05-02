use serde_json::{json, Value};

use super::contracts::LlmSchemaName;

pub fn schema_for(schema_name: LlmSchemaName) -> Value {
    match schema_name {
        LlmSchemaName::ProjectInfo => project_info_schema(),
        LlmSchemaName::Requirements => requirements_schema(),
        LlmSchemaName::Procurement => procurement_schema(),
        LlmSchemaName::RiskClassification => risk_classification_schema(),
    }
}

fn project_info_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["fields"],
        "properties": {
            "fields": {
                "type": "array",
                "items": object_schema(
                    &["field_key", "raw_value", "normalized_value", "confidence", "evidence_block_ids"],
                    json!({
                        "field_key": {
                            "type": "string",
                            "enum": [
                                "business_name",
                                "client",
                                "budget",
                                "period",
                                "contract_method",
                                "deadline",
                                "evaluation_ratio"
                            ]
                        },
                        "raw_value": { "type": "string" },
                        "normalized_value": { "type": "string" },
                        "confidence": confidence_schema(),
                        "evidence_block_ids": evidence_ids_schema()
                    })
                )
            }
        }
    })
}

fn requirements_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["requirements"],
        "properties": {
            "requirements": {
                "type": "array",
                "items": object_schema(
                    &["requirement_code", "title", "description", "category", "mandatory", "confidence", "evidence_block_ids"],
                    json!({
                        "requirement_code": { "type": "string" },
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "category": {
                            "type": "string",
                            "enum": [
                                "functional",
                                "technical",
                                "security",
                                "data",
                                "staffing",
                                "management",
                                "quality",
                                "performance",
                                "other"
                            ]
                        },
                        "mandatory": { "type": "boolean" },
                        "confidence": confidence_schema(),
                        "evidence_block_ids": evidence_ids_schema()
                    })
                )
            }
        }
    })
}

fn procurement_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "procurement_items",
            "staffing_requirements",
            "deliverables",
            "acceptance_criteria",
            "risk_clauses"
        ],
        "properties": {
            "procurement_items": {
                "type": "array",
                "items": procurement_item_schema()
            },
            "staffing_requirements": {
                "type": "array",
                "items": staffing_requirement_schema()
            },
            "deliverables": {
                "type": "array",
                "items": deliverable_schema()
            },
            "acceptance_criteria": {
                "type": "array",
                "items": acceptance_criterion_schema()
            },
            "risk_clauses": {
                "type": "array",
                "items": risk_clause_schema()
            }
        }
    })
}

fn risk_classification_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["risk_clauses"],
        "properties": {
            "risk_clauses": {
                "type": "array",
                "items": risk_clause_schema()
            }
        }
    })
}

fn procurement_item_schema() -> Value {
    object_schema(
        &[
            "requirement_code",
            "item_type",
            "name",
            "spec",
            "quantity_text",
            "unit",
            "confidence",
            "evidence_block_ids",
        ],
        json!({
            "requirement_code": { "type": "string" },
            "item_type": {
                "type": "string",
                "enum": [
                    "hardware",
                    "software",
                    "license",
                    "cloud",
                    "network",
                    "database",
                    "security",
                    "service",
                    "other"
                ]
            },
            "name": { "type": "string" },
            "spec": { "type": "string" },
            "quantity_text": { "type": "string" },
            "unit": { "type": "string" },
            "confidence": confidence_schema(),
            "evidence_block_ids": evidence_ids_schema()
        }),
    )
}

fn staffing_requirement_schema() -> Value {
    object_schema(
        &[
            "requirement_code",
            "role",
            "grade",
            "headcount_text",
            "mm_text",
            "onsite_text",
            "confidence",
            "evidence_block_ids",
        ],
        json!({
            "requirement_code": { "type": "string" },
            "role": { "type": "string" },
            "grade": { "type": "string" },
            "headcount_text": { "type": "string" },
            "mm_text": { "type": "string" },
            "onsite_text": { "type": "string" },
            "confidence": confidence_schema(),
            "evidence_block_ids": evidence_ids_schema()
        }),
    )
}

fn deliverable_schema() -> Value {
    object_schema(
        &[
            "requirement_code",
            "name",
            "due_text",
            "format_text",
            "description",
            "confidence",
            "evidence_block_ids",
        ],
        json!({
            "requirement_code": { "type": "string" },
            "name": { "type": "string" },
            "due_text": { "type": "string" },
            "format_text": { "type": "string" },
            "description": { "type": "string" },
            "confidence": confidence_schema(),
            "evidence_block_ids": evidence_ids_schema()
        }),
    )
}

fn acceptance_criterion_schema() -> Value {
    object_schema(
        &[
            "requirement_code",
            "criterion_type",
            "description",
            "threshold",
            "confidence",
            "evidence_block_ids",
        ],
        json!({
            "requirement_code": { "type": "string" },
            "criterion_type": {
                "type": "string",
                "enum": ["test", "performance", "security", "inspection", "sla", "warranty", "other"]
            },
            "description": { "type": "string" },
            "threshold": { "type": "string" },
            "confidence": confidence_schema(),
            "evidence_block_ids": evidence_ids_schema()
        }),
    )
}

fn risk_clause_schema() -> Value {
    object_schema(
        &[
            "requirement_code",
            "risk_type",
            "severity",
            "description",
            "recommended_action",
            "confidence",
            "evidence_block_ids",
        ],
        json!({
            "requirement_code": { "type": "string" },
            "risk_type": {
                "type": "string",
                "enum": [
                    "scope_creep",
                    "free_work",
                    "short_schedule",
                    "liability",
                    "ambiguous_spec",
                    "vendor_lock",
                    "payment",
                    "security",
                    "other"
                ]
            },
            "severity": {
                "type": "string",
                "enum": ["low", "medium", "high", "blocker"]
            },
            "description": { "type": "string" },
            "recommended_action": { "type": "string" },
            "confidence": confidence_schema(),
            "evidence_block_ids": evidence_ids_schema()
        }),
    )
}

fn object_schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn confidence_schema() -> Value {
    json!({ "type": "number", "minimum": 0, "maximum": 1 })
}

fn evidence_ids_schema() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_info_schema_requires_fields_and_blocks_extra_properties() {
        let schema = schema_for(LlmSchemaName::ProjectInfo);

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"][0], "fields");
    }

    #[test]
    fn procurement_schema_contains_all_domain_arrays() {
        let schema = schema_for(LlmSchemaName::Procurement);
        let required = schema["required"].as_array().expect("required array");

        for key in [
            "procurement_items",
            "staffing_requirements",
            "deliverables",
            "acceptance_criteria",
            "risk_clauses",
        ] {
            assert!(required.iter().any(|value| value == key));
            assert_eq!(schema["properties"][key]["type"], "array");
        }
    }
}

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct FindingInput {
    pub severity: &'static str,
    pub finding_type: &'static str,
    pub message: &'static str,
    pub target_table: Option<&'static str>,
    pub target_id: Option<String>,
}

pub fn evaluate_baseline_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM validation_findings WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;

    let document_id: String = conn.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let block_count = count_document_blocks(conn, &document_id)?;

    let project_target = Some(rfp_project_id.to_string());
    let mut findings = vec![
        FindingInput {
            severity: "blocker",
            finding_type: "missing_business_name",
            message: "사업명이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target.clone(),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "missing_client",
            message: "발주기관이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target.clone(),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "missing_budget",
            message: "사업예산이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target.clone(),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "missing_period",
            message: "사업기간이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target.clone(),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "zero_requirements",
            message: "요구사항이 0건입니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target.clone(),
        },
        FindingInput {
            severity: "warning",
            finding_type: "llm_not_used",
            message: "LLM opt-in이 꺼져 구조화가 제한됩니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target,
        },
    ];

    if block_count == 0 {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "missing_evidence",
            message: "원문 근거 block이 없습니다.",
            target_table: Some("document_blocks"),
            target_id: None,
        });
    }

    for finding in findings {
        insert_finding(conn, rfp_project_id, finding)?;
    }

    update_status_from_findings(conn, rfp_project_id, &document_id)
}

pub fn evaluate_candidate_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM validation_findings WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;

    let document_id: String = conn.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let block_count = count_document_blocks(conn, &document_id)?;
    let project_target = Some(rfp_project_id.to_string());
    let mut findings = Vec::new();

    for (field_key, finding_type, message) in [
        (
            "business_name",
            "missing_business_name",
            "사업명이 추출되지 않았습니다.",
        ),
        (
            "client",
            "missing_client",
            "발주기관이 추출되지 않았습니다.",
        ),
        (
            "budget",
            "missing_budget",
            "사업예산이 추출되지 않았습니다.",
        ),
        (
            "period",
            "missing_period",
            "사업기간이 추출되지 않았습니다.",
        ),
    ] {
        if !has_field(conn, rfp_project_id, field_key)? {
            findings.push(FindingInput {
                severity: "blocker",
                finding_type,
                message,
                target_table: Some("rfp_projects"),
                target_id: project_target.clone(),
            });
        }
    }

    findings.push(FindingInput {
        severity: "blocker",
        finding_type: "zero_requirements",
        message: "요구사항이 0건입니다.",
        target_table: Some("rfp_projects"),
        target_id: project_target.clone(),
    });

    if block_count == 0 || has_field_without_evidence(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "missing_evidence",
            message: "원문 근거가 없는 항목이 있습니다.",
            target_table: Some("rfp_fields"),
            target_id: None,
        });
    }

    let low_confidence_field_ids = low_confidence_fields(conn, rfp_project_id)?;
    for field_id in low_confidence_field_ids {
        findings.push(FindingInput {
            severity: "warning",
            finding_type: "low_confidence",
            message: "신뢰도가 낮은 추출값이 있습니다.",
            target_table: Some("rfp_fields"),
            target_id: Some(field_id),
        });
    }

    findings.push(FindingInput {
        severity: "warning",
        finding_type: "llm_not_used",
        message: "LLM opt-in이 꺼져 구조화가 제한됩니다.",
        target_table: Some("rfp_projects"),
        target_id: project_target,
    });

    for finding in findings {
        insert_finding(conn, rfp_project_id, finding)?;
    }

    update_status_from_findings(conn, rfp_project_id, &document_id)
}

fn count_document_blocks(conn: &Connection, document_id: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM document_blocks WHERE document_id = ?",
        [document_id],
        |row| row.get(0),
    )?)
}

fn has_field(conn: &Connection, rfp_project_id: &str, field_key: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM rfp_fields
         WHERE rfp_project_id = ? AND field_key = ? AND TRIM(normalized_value) <> ''",
        [rfp_project_id, field_key],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn has_field_without_evidence(conn: &Connection, rfp_project_id: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM rfp_fields f
         LEFT JOIN evidence_links e ON e.target_table = 'rfp_fields' AND e.target_id = f.id
         WHERE f.rfp_project_id = ? AND e.id IS NULL",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn low_confidence_fields(conn: &Connection, rfp_project_id: &str) -> AppResult<Vec<String>> {
    let mut statement =
        conn.prepare("SELECT id FROM rfp_fields WHERE rfp_project_id = ? AND confidence < 0.6")?;
    let ids = statement
        .query_map([rfp_project_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

fn update_status_from_findings(
    conn: &Connection,
    rfp_project_id: &str,
    document_id: &str,
) -> AppResult<()> {
    let blocker_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings
         WHERE rfp_project_id = ? AND severity = 'blocker'",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let status = if blocker_count > 0 {
        "review_needed"
    } else {
        "ready"
    };
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE rfp_projects SET status = ?, updated_at = ? WHERE id = ?",
        params![status, now, rfp_project_id],
    )?;
    conn.execute(
        "UPDATE documents SET status = ?, updated_at = ? WHERE id = ?",
        params![status, now, document_id],
    )?;
    Ok(())
}

fn insert_finding(conn: &Connection, rfp_project_id: &str, finding: FindingInput) -> AppResult<()> {
    conn.execute(
        "INSERT INTO validation_findings (
            id, rfp_project_id, severity, finding_type, message, target_table, target_id, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            Uuid::new_v4().to_string(),
            rfp_project_id,
            finding.severity,
            finding.finding_type,
            finding.message,
            finding.target_table,
            finding.target_id,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

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
    let block_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM document_blocks WHERE document_id = ?",
        [&document_id],
        |row| row.get(0),
    )?;

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

    let blocker_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'blocker'",
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

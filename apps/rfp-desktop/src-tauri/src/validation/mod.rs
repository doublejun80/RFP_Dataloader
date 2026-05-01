use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::domain_writer::DomainRejection;
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
    evaluate_project(conn, rfp_project_id)
}

pub fn evaluate_candidate_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    evaluate_project(conn, rfp_project_id)
}

pub fn evaluate_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM validation_findings WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;

    let document_id = project_document_id(conn, rfp_project_id)?;
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

    let requirement_count = count_requirements(conn, rfp_project_id)?;
    if requirement_count == 0 {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "zero_requirements",
            message: "요구사항이 0건입니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target.clone(),
        });
    }

    if has_duplicate_requirement_code(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "duplicate_requirement_code",
            message: "중복 요구사항 ID가 있습니다.",
            target_table: Some("requirements"),
            target_id: None,
        });
    }

    if is_over_extracted(conn, rfp_project_id, requirement_count)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "over_extraction",
            message: "요구사항 과다 추출 가능성이 있습니다.",
            target_table: Some("requirements"),
            target_id: None,
        });
    }

    if block_count == 0 {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "missing_evidence",
            message: "원문 근거 block이 없습니다.",
            target_table: Some("document_blocks"),
            target_id: None,
        });
    }

    for (target_table, target_id) in rows_without_evidence(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "missing_evidence",
            message: "원문 근거가 없는 항목이 있습니다.",
            target_table: Some(target_table),
            target_id: Some(target_id),
        });
    }

    for target_id in procurement_items_missing_quantity(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "warning",
            finding_type: "missing_quantity",
            message: "구매 항목 이름은 있으나 수량/스펙이 비어 있습니다.",
            target_table: Some("procurement_items"),
            target_id: Some(target_id),
        });
    }

    for target_id in procurement_items_invalid_quantity(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "invalid_quantity",
            message: "구매 항목 수량이 0 이하입니다.",
            target_table: Some("procurement_items"),
            target_id: Some(target_id),
        });
    }

    for (target_table, target_id) in low_confidence_domain_rows(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "warning",
            finding_type: "low_confidence",
            message: "신뢰도가 낮은 추출값이 있습니다.",
            target_table: Some(target_table),
            target_id: Some(target_id),
        });
    }

    for target_id in blocker_risk_clause_ids(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "risk_clause_blocker",
            message: "blocker 등급 리스크 조항이 있습니다.",
            target_table: Some("risk_clauses"),
            target_id: Some(target_id),
        });
    }

    if !has_llm_sourced_rows(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "warning",
            finding_type: "llm_not_used",
            message: "LLM opt-in이 꺼져 구조화가 제한됩니다.",
            target_table: Some("rfp_projects"),
            target_id: project_target,
        });
    }

    for finding in findings {
        insert_finding(conn, rfp_project_id, finding)?;
    }

    update_status_from_findings(conn, rfp_project_id, &document_id)
}

pub fn insert_domain_rejections(
    conn: &Connection,
    rfp_project_id: &str,
    rejections: &[DomainRejection],
) -> AppResult<()> {
    for rejection in rejections {
        insert_finding_values(
            conn,
            rfp_project_id,
            &rejection.severity,
            &rejection.finding_type,
            &rejection.message,
            rejection.target_table.as_deref(),
            None,
        )?;
    }
    Ok(())
}

pub fn refresh_project_status_from_findings(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<()> {
    let document_id = project_document_id(conn, rfp_project_id)?;
    update_status_from_findings(conn, rfp_project_id, &document_id)
}

fn project_document_id(conn: &Connection, rfp_project_id: &str) -> AppResult<String> {
    Ok(conn.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?)
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

fn count_requirements(conn: &Connection, rfp_project_id: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM requirements WHERE rfp_project_id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?)
}

fn has_duplicate_requirement_code(conn: &Connection, rfp_project_id: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT requirement_code
            FROM requirements
            WHERE rfp_project_id = ?
            GROUP BY requirement_code
            HAVING COUNT(*) > 1
         )",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn is_over_extracted(
    conn: &Connection,
    rfp_project_id: &str,
    actual_requirement_count: i64,
) -> AppResult<bool> {
    let count_text: Option<String> = conn
        .query_row(
            "SELECT COALESCE(NULLIF(TRIM(normalized_value), ''), raw_value)
             FROM rfp_fields
             WHERE rfp_project_id = ? AND field_key = 'requirement_count'
             LIMIT 1",
            [rfp_project_id],
            |row| row.get(0),
        )
        .ok();
    let Some(expected) = count_text.and_then(|value| parse_first_number(&value)) else {
        return Ok(false);
    };
    let expected = expected.ceil() as i64;
    let threshold = std::cmp::max(expected + 5, ((expected as f64) * 1.5).ceil() as i64);
    Ok(actual_requirement_count > threshold)
}

fn rows_without_evidence(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<Vec<(&'static str, String)>> {
    let mut rows = Vec::new();
    collect_ids(
        conn,
        rfp_project_id,
        "rfp_fields",
        "SELECT f.id
         FROM rfp_fields f
         LEFT JOIN evidence_links e ON e.target_table = 'rfp_fields' AND e.target_id = f.id
         WHERE f.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    collect_ids(
        conn,
        rfp_project_id,
        "requirements",
        "SELECT r.id
         FROM requirements r
         LEFT JOIN evidence_links e ON e.target_table = 'requirements' AND e.target_id = r.id
         WHERE r.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "procurement_items",
        "SELECT pi.id
         FROM procurement_items pi
         JOIN requirements r ON r.id = pi.requirement_id
         LEFT JOIN evidence_links e ON e.target_table = 'procurement_items' AND e.target_id = pi.id
         WHERE r.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "staffing_requirements",
        "SELECT sr.id
         FROM staffing_requirements sr
         JOIN requirements r ON r.id = sr.requirement_id
         LEFT JOIN evidence_links e ON e.target_table = 'staffing_requirements' AND e.target_id = sr.id
         WHERE r.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "deliverables",
        "SELECT d.id
         FROM deliverables d
         JOIN requirements r ON r.id = d.requirement_id
         LEFT JOIN evidence_links e ON e.target_table = 'deliverables' AND e.target_id = d.id
         WHERE r.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "acceptance_criteria",
        "SELECT ac.id
         FROM acceptance_criteria ac
         JOIN requirements r ON r.id = ac.requirement_id
         LEFT JOIN evidence_links e ON e.target_table = 'acceptance_criteria' AND e.target_id = ac.id
         WHERE r.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "risk_clauses",
        "SELECT rc.id
         FROM risk_clauses rc
         JOIN requirements r ON r.id = rc.requirement_id
         LEFT JOIN evidence_links e ON e.target_table = 'risk_clauses' AND e.target_id = rc.id
         WHERE r.rfp_project_id = ? AND e.id IS NULL",
        &mut rows,
    )?;
    Ok(rows)
}

fn procurement_items_missing_quantity(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<Vec<String>> {
    query_ids(
        conn,
        rfp_project_id,
        "SELECT pi.id
         FROM procurement_items pi
         JOIN requirements r ON r.id = pi.requirement_id
         WHERE r.rfp_project_id = ?
           AND TRIM(pi.name) <> ''
           AND TRIM(pi.spec) = ''
           AND pi.quantity IS NULL",
    )
}

fn procurement_items_invalid_quantity(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<Vec<String>> {
    query_ids(
        conn,
        rfp_project_id,
        "SELECT pi.id
         FROM procurement_items pi
         JOIN requirements r ON r.id = pi.requirement_id
         WHERE r.rfp_project_id = ? AND pi.quantity <= 0.0",
    )
}

fn low_confidence_domain_rows(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<Vec<(&'static str, String)>> {
    let mut rows = Vec::new();
    collect_ids(
        conn,
        rfp_project_id,
        "rfp_fields",
        "SELECT id FROM rfp_fields WHERE rfp_project_id = ? AND confidence < 0.6",
        &mut rows,
    )?;
    collect_ids(
        conn,
        rfp_project_id,
        "requirements",
        "SELECT id FROM requirements WHERE rfp_project_id = ? AND confidence < 0.6",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "procurement_items",
        "SELECT pi.id
         FROM procurement_items pi
         JOIN requirements r ON r.id = pi.requirement_id
         WHERE r.rfp_project_id = ? AND pi.confidence < 0.6",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "staffing_requirements",
        "SELECT sr.id
         FROM staffing_requirements sr
         JOIN requirements r ON r.id = sr.requirement_id
         WHERE r.rfp_project_id = ? AND sr.confidence < 0.6",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "deliverables",
        "SELECT d.id
         FROM deliverables d
         JOIN requirements r ON r.id = d.requirement_id
         WHERE r.rfp_project_id = ? AND d.confidence < 0.6",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "acceptance_criteria",
        "SELECT ac.id
         FROM acceptance_criteria ac
         JOIN requirements r ON r.id = ac.requirement_id
         WHERE r.rfp_project_id = ? AND ac.confidence < 0.6",
        &mut rows,
    )?;
    collect_child_ids_without_evidence(
        conn,
        rfp_project_id,
        "risk_clauses",
        "SELECT rc.id
         FROM risk_clauses rc
         JOIN requirements r ON r.id = rc.requirement_id
         WHERE r.rfp_project_id = ? AND rc.confidence < 0.6",
        &mut rows,
    )?;
    Ok(rows)
}

fn blocker_risk_clause_ids(conn: &Connection, rfp_project_id: &str) -> AppResult<Vec<String>> {
    query_ids(
        conn,
        rfp_project_id,
        "SELECT rc.id
         FROM risk_clauses rc
         JOIN requirements r ON r.id = rc.requirement_id
         WHERE r.rfp_project_id = ? AND rc.severity = 'blocker'",
    )
}

fn has_llm_sourced_rows(conn: &Connection, rfp_project_id: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM rfp_fields WHERE rfp_project_id = ? AND source = 'llm') +
            (SELECT COUNT(*) FROM requirements WHERE rfp_project_id = ? AND source = 'llm') +
            (SELECT COUNT(*) FROM procurement_items pi JOIN requirements r ON r.id = pi.requirement_id WHERE r.rfp_project_id = ? AND pi.source = 'llm') +
            (SELECT COUNT(*) FROM staffing_requirements sr JOIN requirements r ON r.id = sr.requirement_id WHERE r.rfp_project_id = ? AND sr.source = 'llm') +
            (SELECT COUNT(*) FROM deliverables d JOIN requirements r ON r.id = d.requirement_id WHERE r.rfp_project_id = ? AND d.source = 'llm') +
            (SELECT COUNT(*) FROM acceptance_criteria ac JOIN requirements r ON r.id = ac.requirement_id WHERE r.rfp_project_id = ? AND ac.source = 'llm') +
            (SELECT COUNT(*) FROM risk_clauses rc JOIN requirements r ON r.id = rc.requirement_id WHERE r.rfp_project_id = ? AND rc.source = 'llm')",
        params![
            rfp_project_id,
            rfp_project_id,
            rfp_project_id,
            rfp_project_id,
            rfp_project_id,
            rfp_project_id,
            rfp_project_id
        ],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn collect_ids(
    conn: &Connection,
    rfp_project_id: &str,
    target_table: &'static str,
    sql: &str,
    rows: &mut Vec<(&'static str, String)>,
) -> AppResult<()> {
    for id in query_ids(conn, rfp_project_id, sql)? {
        rows.push((target_table, id));
    }
    Ok(())
}

fn collect_child_ids_without_evidence(
    conn: &Connection,
    rfp_project_id: &str,
    target_table: &'static str,
    sql: &str,
    rows: &mut Vec<(&'static str, String)>,
) -> AppResult<()> {
    collect_ids(conn, rfp_project_id, target_table, sql, rows)
}

fn query_ids(conn: &Connection, rfp_project_id: &str, sql: &str) -> AppResult<Vec<String>> {
    let mut statement = conn.prepare(sql)?;
    let ids = statement
        .query_map([rfp_project_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

fn parse_first_number(text: &str) -> Option<f64> {
    let mut value = String::new();
    let mut started = false;
    let mut seen_digit = false;
    for ch in text.chars() {
        if (ch == '-' || ch == '+') && !started {
            value.push(ch);
            started = true;
        } else if ch.is_ascii_digit() {
            value.push(ch);
            started = true;
            seen_digit = true;
        } else if ch == '.' && started {
            value.push(ch);
        } else if ch == ',' && started {
            continue;
        } else if started && seen_digit {
            break;
        } else if started {
            value.clear();
            started = false;
        }
    }
    if !seen_digit {
        None
    } else {
        value.parse::<f64>().ok()
    }
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
    insert_finding_values(
        conn,
        rfp_project_id,
        finding.severity,
        finding.finding_type,
        finding.message,
        finding.target_table,
        finding.target_id,
    )
}

fn insert_finding_values(
    conn: &Connection,
    rfp_project_id: &str,
    severity: &str,
    finding_type: &str,
    message: &str,
    target_table: Option<&str>,
    target_id: Option<String>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO validation_findings (
            id, rfp_project_id, severity, finding_type, message, target_table, target_id, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            Uuid::new_v4().to_string(),
            rfp_project_id,
            severity,
            finding_type,
            message,
            target_table,
            target_id,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain_writer::test_support::{full_domain_draft, seed_document_project_and_blocks};

    use super::*;

    #[test]
    fn domain_project_with_required_fields_requirement_and_evidence_becomes_ready() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);
        let draft = full_domain_draft();

        crate::domain_writer::write_domain_draft(&mut conn, "project-1", draft).expect("write");
        evaluate_project(&conn, "project-1").expect("evaluate");

        let status: String = conn
            .query_row(
                "SELECT status FROM rfp_projects WHERE id = 'project-1'",
                [],
                |row| row.get(0),
            )
            .expect("project status");
        assert_eq!(status, "ready");
    }

    #[test]
    fn domain_project_missing_required_field_stays_review_needed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut conn =
            crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);
        let mut draft = full_domain_draft();
        draft.fields.retain(|field| field.field_key != "budget");

        crate::domain_writer::write_domain_draft(&mut conn, "project-1", draft).expect("write");
        evaluate_project(&conn, "project-1").expect("evaluate");

        let blocker_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings
                 WHERE rfp_project_id = 'project-1' AND finding_type = 'missing_budget'",
                [],
                |row| row.get(0),
            )
            .expect("blocker count");
        assert_eq!(blocker_count, 1);
    }
}

use rusqlite::{Connection, OptionalExtension};
use tauri::State;

use crate::document_ingestion;
use crate::domain::{
    AcceptanceReviewRow, CandidateBundleSummaryDto, DeliverableReviewRow, EvidenceContextDto,
    EvidenceLinkDto, ProcurementItemReviewRow, RequirementReviewRow, ReviewFieldDto,
    ReviewMetricsDto, ReviewProjectDto, ReviewProjectSummary, RiskReviewRow, SourceBlockDto,
    StaffingReviewRow, ValidationFindingDto,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const EVIDENCE_TARGET_TABLES: &[&str] = &[
    "rfp_fields",
    "requirements",
    "procurement_items",
    "staffing_requirements",
    "deliverables",
    "acceptance_criteria",
    "risk_clauses",
];

#[tauri::command]
pub fn get_review_project(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<ReviewProjectDto> {
    let conn = state.connect()?;
    load_review_project(&conn, &document_id)
}

#[tauri::command]
pub fn get_evidence_context(
    target_table: String,
    target_id: String,
    state: State<'_, AppState>,
) -> AppResult<EvidenceContextDto> {
    let conn = state.connect()?;
    load_evidence_context(&conn, &target_table, &target_id)
}

fn load_review_project(conn: &Connection, document_id: &str) -> AppResult<ReviewProjectDto> {
    let document = document_ingestion::load_document_summary(conn, document_id)?;
    let project = load_project_summary(conn, document_id)?;

    let Some(project) = project else {
        return Ok(ReviewProjectDto {
            document,
            project: None,
            overview_fields: Vec::new(),
            candidate_bundles: Vec::new(),
            requirements: Vec::new(),
            procurement_items: Vec::new(),
            staffing_requirements: Vec::new(),
            deliverables: Vec::new(),
            acceptance_criteria: Vec::new(),
            risk_clauses: Vec::new(),
            findings: Vec::new(),
            metrics: ReviewMetricsDto {
                requirement_count: 0,
                procurement_count: 0,
                staffing_count: 0,
                total_mm: None,
                high_risk_count: 0,
                blocker_count: 0,
                warning_count: 0,
            },
        });
    };

    let overview_fields = load_overview_fields(conn, &project.id)?;
    let candidate_bundles = load_candidate_bundles(conn, &project.id)?;
    let requirements = load_requirements(conn, &project.id)?;
    let procurement_items = load_procurement_items(conn, &project.id)?;
    let staffing_requirements = load_staffing_requirements(conn, &project.id)?;
    let deliverables = load_deliverables(conn, &project.id)?;
    let acceptance_criteria = load_acceptance_criteria(conn, &project.id)?;
    let risk_clauses = load_risk_clauses(conn, &project.id)?;
    let findings = load_findings(conn, &project.id)?;
    let metrics = build_metrics(
        &requirements,
        &procurement_items,
        &staffing_requirements,
        &risk_clauses,
        &findings,
    );

    Ok(ReviewProjectDto {
        document,
        project: Some(project),
        overview_fields,
        candidate_bundles,
        requirements,
        procurement_items,
        staffing_requirements,
        deliverables,
        acceptance_criteria,
        risk_clauses,
        findings,
        metrics,
    })
}

fn load_candidate_bundles(
    conn: &Connection,
    project_id: &str,
) -> AppResult<Vec<CandidateBundleSummaryDto>> {
    let mut statement = conn.prepare(
        "SELECT bundle_key, candidate_count
         FROM candidate_bundles
         WHERE rfp_project_id = ?
         ORDER BY CASE bundle_key
           WHEN 'project_info_candidates' THEN 1
           WHEN 'requirement_candidates' THEN 2
           WHEN 'procurement_candidates' THEN 3
           WHEN 'staffing_candidates' THEN 4
           WHEN 'deliverable_candidates' THEN 5
           WHEN 'acceptance_candidates' THEN 6
           WHEN 'risk_candidates' THEN 7
           ELSE 99
         END",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok(CandidateBundleSummaryDto {
                bundle_key: row.get(0)?,
                candidate_count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_project_summary(
    conn: &Connection,
    document_id: &str,
) -> AppResult<Option<ReviewProjectSummary>> {
    Ok(conn
        .query_row(
            "SELECT id, status, summary, analysis_version
             FROM rfp_projects
             WHERE document_id = ?",
            [document_id],
            |row| {
                Ok(ReviewProjectSummary {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    summary: row.get(2)?,
                    analysis_version: row.get(3)?,
                })
            },
        )
        .optional()?)
}

fn load_overview_fields(conn: &Connection, project_id: &str) -> AppResult<Vec<ReviewFieldDto>> {
    let mut statement = conn.prepare(
        "SELECT
            field.id,
            field.field_key,
            field.label,
            field.raw_value,
            field.normalized_value,
            field.confidence,
            field.source,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count
         FROM rfp_fields field
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'rfp_fields'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = field.id
         WHERE field.rfp_project_id = ?
         ORDER BY CASE field.field_key
           WHEN 'business_name' THEN 1
           WHEN 'client' THEN 2
           WHEN 'budget' THEN 3
           WHEN 'period' THEN 4
           WHEN 'contract_method' THEN 5
           WHEN 'deadline' THEN 6
           ELSE 99
         END",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok(ReviewFieldDto {
                id: row.get(0)?,
                field_key: row.get(1)?,
                label: row.get(2)?,
                raw_value: row.get(3)?,
                normalized_value: row.get(4)?,
                confidence: row.get(5)?,
                source: row.get(6)?,
                evidence_count: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_requirements(conn: &Connection, project_id: &str) -> AppResult<Vec<RequirementReviewRow>> {
    let mut statement = conn.prepare(
        "SELECT
            requirement.id,
            requirement.requirement_code,
            requirement.title,
            requirement.description,
            requirement.category,
            requirement.mandatory,
            requirement.confidence,
            requirement.source,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count,
            COALESCE(finding_counts.blocker_count, 0) AS blocker_count,
            COALESCE(finding_counts.warning_count, 0) AS warning_count
         FROM requirements requirement
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'requirements'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = requirement.id
         LEFT JOIN (
            SELECT target_id,
                   SUM(CASE WHEN severity = 'blocker' THEN 1 ELSE 0 END) AS blocker_count,
                   SUM(CASE WHEN severity = 'warning' THEN 1 ELSE 0 END) AS warning_count
            FROM validation_findings
            WHERE target_table = 'requirements'
            GROUP BY target_id
         ) finding_counts ON finding_counts.target_id = requirement.id
         WHERE requirement.rfp_project_id = ?
         ORDER BY requirement.requirement_code, requirement.title",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            let mandatory: i64 = row.get(5)?;
            Ok(RequirementReviewRow {
                id: row.get(0)?,
                requirement_code: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                category: row.get(4)?,
                mandatory: mandatory == 1,
                confidence: row.get(6)?,
                source: row.get(7)?,
                evidence_count: row.get(8)?,
                blocker_count: row.get(9)?,
                warning_count: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_procurement_items(
    conn: &Connection,
    project_id: &str,
) -> AppResult<Vec<ProcurementItemReviewRow>> {
    let mut statement = conn.prepare(
        "SELECT
            item.id,
            item.item_type,
            item.name,
            item.spec,
            item.quantity,
            item.unit,
            item.required,
            item.confidence,
            requirement.requirement_code,
            requirement.title,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count,
            COALESCE(finding_counts.warning_count, 0) AS warning_count
         FROM procurement_items item
         JOIN requirements requirement ON requirement.id = item.requirement_id
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'procurement_items'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = item.id
         LEFT JOIN (
            SELECT target_id,
                   SUM(CASE WHEN severity = 'warning' THEN 1 ELSE 0 END) AS warning_count
            FROM validation_findings
            WHERE target_table = 'procurement_items'
            GROUP BY target_id
         ) finding_counts ON finding_counts.target_id = item.id
         WHERE requirement.rfp_project_id = ?
         ORDER BY requirement.requirement_code, item.name",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            let required: i64 = row.get(6)?;
            Ok(ProcurementItemReviewRow {
                id: row.get(0)?,
                item_type: row.get(1)?,
                name: row.get(2)?,
                spec: row.get(3)?,
                quantity: row.get(4)?,
                unit: blank_to_none(row.get(5)?),
                required: required == 1,
                confidence: row.get(7)?,
                requirement_code: row.get(8)?,
                requirement_title: row.get(9)?,
                evidence_count: row.get(10)?,
                warning_count: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_staffing_requirements(
    conn: &Connection,
    project_id: &str,
) -> AppResult<Vec<StaffingReviewRow>> {
    let mut statement = conn.prepare(
        "SELECT
            staffing.id,
            staffing.role,
            staffing.grade,
            staffing.headcount,
            staffing.mm,
            staffing.onsite,
            staffing.period_text,
            requirement.requirement_code,
            requirement.title,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count
         FROM staffing_requirements staffing
         JOIN requirements requirement ON requirement.id = staffing.requirement_id
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'staffing_requirements'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = staffing.id
         WHERE requirement.rfp_project_id = ?
         ORDER BY requirement.requirement_code, staffing.role",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            let onsite: Option<i64> = row.get(5)?;
            Ok(StaffingReviewRow {
                id: row.get(0)?,
                role: row.get(1)?,
                grade: row.get(2)?,
                headcount: row.get(3)?,
                mm: row.get(4)?,
                onsite: onsite.map(|value| value == 1),
                period_text: row.get(6)?,
                requirement_code: row.get(7)?,
                requirement_title: row.get(8)?,
                evidence_count: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_risk_clauses(conn: &Connection, project_id: &str) -> AppResult<Vec<RiskReviewRow>> {
    let mut statement = conn.prepare(
        "SELECT
            risk.id,
            risk.risk_type,
            risk.severity,
            risk.description,
            risk.recommended_action,
            requirement.requirement_code,
            requirement.title,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count
         FROM risk_clauses risk
         JOIN requirements requirement ON requirement.id = risk.requirement_id
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'risk_clauses'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = risk.id
         WHERE requirement.rfp_project_id = ?
         ORDER BY CASE risk.severity
            WHEN 'blocker' THEN 1
            WHEN 'high' THEN 2
            WHEN 'medium' THEN 3
            ELSE 4
          END,
          requirement.requirement_code",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok(RiskReviewRow {
                id: row.get(0)?,
                risk_type: row.get(1)?,
                severity: row.get(2)?,
                description: row.get(3)?,
                recommended_action: row.get(4)?,
                requirement_code: row.get(5)?,
                requirement_title: row.get(6)?,
                evidence_count: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_deliverables(conn: &Connection, project_id: &str) -> AppResult<Vec<DeliverableReviewRow>> {
    let mut statement = conn.prepare(
        "SELECT
            deliverable.id,
            deliverable.name,
            deliverable.due_text,
            deliverable.format_text,
            deliverable.description,
            deliverable.confidence,
            requirement.requirement_code,
            requirement.title,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count
         FROM deliverables deliverable
         JOIN requirements requirement ON requirement.id = deliverable.requirement_id
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'deliverables'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = deliverable.id
         WHERE requirement.rfp_project_id = ?
         ORDER BY requirement.requirement_code, deliverable.name",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok(DeliverableReviewRow {
                id: row.get(0)?,
                name: row.get(1)?,
                due_text: row.get(2)?,
                format_text: row.get(3)?,
                description: row.get(4)?,
                confidence: row.get(5)?,
                requirement_code: row.get(6)?,
                requirement_title: row.get(7)?,
                evidence_count: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_acceptance_criteria(
    conn: &Connection,
    project_id: &str,
) -> AppResult<Vec<AcceptanceReviewRow>> {
    let mut statement = conn.prepare(
        "SELECT
            acceptance.id,
            acceptance.criterion_type,
            acceptance.description,
            acceptance.threshold,
            acceptance.due_text,
            acceptance.confidence,
            requirement.requirement_code,
            requirement.title,
            COALESCE(evidence_counts.evidence_count, 0) AS evidence_count
         FROM acceptance_criteria acceptance
         JOIN requirements requirement ON requirement.id = acceptance.requirement_id
         LEFT JOIN (
            SELECT target_id, COUNT(*) AS evidence_count
            FROM evidence_links
            WHERE target_table = 'acceptance_criteria'
            GROUP BY target_id
         ) evidence_counts ON evidence_counts.target_id = acceptance.id
         WHERE requirement.rfp_project_id = ?
         ORDER BY requirement.requirement_code, acceptance.criterion_type",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok(AcceptanceReviewRow {
                id: row.get(0)?,
                criterion_type: row.get(1)?,
                description: row.get(2)?,
                threshold: row.get(3)?,
                due_text: row.get(4)?,
                confidence: row.get(5)?,
                requirement_code: row.get(6)?,
                requirement_title: row.get(7)?,
                evidence_count: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_findings(conn: &Connection, project_id: &str) -> AppResult<Vec<ValidationFindingDto>> {
    let mut statement = conn.prepare(
        "SELECT id, severity, finding_type, message, target_table, target_id, created_at
         FROM validation_findings
         WHERE rfp_project_id = ?
         ORDER BY CASE severity
            WHEN 'blocker' THEN 1
            WHEN 'warning' THEN 2
            ELSE 3
          END,
          created_at,
          id",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok(ValidationFindingDto {
                id: row.get(0)?,
                severity: row.get(1)?,
                finding_type: row.get(2)?,
                message: row.get(3)?,
                target_table: row.get(4)?,
                target_id: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn build_metrics(
    requirements: &[RequirementReviewRow],
    procurement_items: &[ProcurementItemReviewRow],
    staffing_requirements: &[StaffingReviewRow],
    risk_clauses: &[RiskReviewRow],
    findings: &[ValidationFindingDto],
) -> ReviewMetricsDto {
    let total_mm = staffing_requirements
        .iter()
        .filter_map(|row| row.mm)
        .reduce(|acc, value| acc + value);
    ReviewMetricsDto {
        requirement_count: requirements.len() as i64,
        procurement_count: procurement_items.len() as i64,
        staffing_count: staffing_requirements.len() as i64,
        total_mm,
        high_risk_count: risk_clauses
            .iter()
            .filter(|row| row.severity == "high")
            .count() as i64,
        blocker_count: findings
            .iter()
            .filter(|finding| finding.severity == "blocker")
            .count() as i64,
        warning_count: findings
            .iter()
            .filter(|finding| finding.severity == "warning")
            .count() as i64,
    }
}

fn blank_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn load_evidence_context(
    conn: &Connection,
    target_table: &str,
    target_id: &str,
) -> AppResult<EvidenceContextDto> {
    validate_evidence_target(target_table)?;
    let evidence = load_direct_evidence(conn, target_table, target_id)?;
    if evidence.is_empty() {
        return Ok(EvidenceContextDto {
            target_table: target_table.to_string(),
            target_id: target_id.to_string(),
            evidence,
            blocks: Vec::new(),
        });
    }

    let mut statement = conn.prepare(
        "SELECT neighbor.id,
                neighbor.page_number,
                neighbor.block_index,
                neighbor.kind,
                neighbor.text,
                neighbor.bbox_json,
                CASE WHEN neighbor.id = direct.id THEN 1 ELSE 0 END AS is_direct_evidence
         FROM document_blocks direct
         JOIN document_blocks neighbor
           ON neighbor.document_id = direct.document_id
          AND neighbor.page_number = direct.page_number
          AND neighbor.block_index BETWEEN direct.block_index - 2 AND direct.block_index + 2
         WHERE direct.id IN (
           SELECT document_block_id
           FROM evidence_links
           WHERE target_table = ?1 AND target_id = ?2
         )
         ORDER BY neighbor.page_number, neighbor.block_index",
    )?;
    let mut blocks = Vec::new();
    let mut rows = statement.query([target_table, target_id])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let is_direct_evidence = row.get::<_, i64>(6)? == 1;
        if let Some(block) = blocks
            .iter_mut()
            .find(|block: &&mut SourceBlockDto| block.id == id)
        {
            block.is_direct_evidence = block.is_direct_evidence || is_direct_evidence;
        } else {
            blocks.push(SourceBlockDto {
                id,
                page_number: row.get(1)?,
                block_index: row.get(2)?,
                kind: row.get(3)?,
                text: row.get(4)?,
                bbox_json: row.get(5)?,
                is_direct_evidence,
            });
        }
    }

    Ok(EvidenceContextDto {
        target_table: target_table.to_string(),
        target_id: target_id.to_string(),
        evidence,
        blocks,
    })
}

fn validate_evidence_target(target_table: &str) -> AppResult<()> {
    if EVIDENCE_TARGET_TABLES.contains(&target_table) {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "지원하지 않는 근거 대상입니다.".to_string(),
        ))
    }
}

fn load_direct_evidence(
    conn: &Connection,
    target_table: &str,
    target_id: &str,
) -> AppResult<Vec<EvidenceLinkDto>> {
    let mut statement = conn.prepare(
        "SELECT id, document_block_id, quote, confidence
         FROM evidence_links
         WHERE target_table = ?1 AND target_id = ?2
         ORDER BY confidence DESC, id",
    )?;
    let evidence = statement
        .query_map([target_table, target_id], |row| {
            Ok(EvidenceLinkDto {
                id: Some(row.get(0)?),
                document_block_id: row.get(1)?,
                quote: row.get(2)?,
                confidence: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db;

    #[test]
    fn loads_review_project_with_domain_rows_and_metrics() {
        let conn = seed_review_database();

        let review = load_review_project(&conn, "doc-1").expect("load review");

        assert_eq!(review.document.id, "doc-1");
        assert_eq!(
            review.project.as_ref().expect("project").status,
            "review_needed"
        );
        assert_eq!(review.overview_fields.len(), 2);
        assert_eq!(review.candidate_bundles.len(), 2);
        assert_eq!(
            review.candidate_bundles[0].bundle_key,
            "project_info_candidates"
        );
        assert_eq!(review.candidate_bundles[0].candidate_count, 4);
        assert_eq!(review.requirements[0].requirement_code, "SFR-001");
        assert_eq!(review.procurement_items[0].name, "API Gateway");
        assert_eq!(review.staffing_requirements[0].mm, Some(3.0));
        assert_eq!(review.deliverables[0].name, "통합시험 결과서");
        assert_eq!(review.acceptance_criteria[0].criterion_type, "test");
        assert_eq!(review.risk_clauses[0].severity, "high");
        assert_eq!(review.metrics.requirement_count, 1);
        assert_eq!(review.metrics.procurement_count, 1);
        assert_eq!(review.metrics.staffing_count, 1);
        assert_eq!(review.metrics.total_mm, Some(3.0));
        assert_eq!(review.metrics.high_risk_count, 1);
        assert_eq!(review.metrics.blocker_count, 1);
        assert_eq!(review.metrics.warning_count, 1);
    }

    #[test]
    fn loads_evidence_context_with_neighbor_blocks() {
        let conn = seed_review_database();

        let context =
            load_evidence_context(&conn, "requirements", "req-1").expect("load evidence context");

        assert_eq!(context.target_table, "requirements");
        assert_eq!(context.target_id, "req-1");
        assert_eq!(context.evidence.len(), 1);
        assert!(context
            .blocks
            .iter()
            .any(|block| block.id == "block-2" && block.is_direct_evidence));
        assert!(context
            .blocks
            .iter()
            .any(|block| block.id == "block-1" && !block.is_direct_evidence));
        assert!(context
            .blocks
            .iter()
            .any(|block| block.id == "block-3" && !block.is_direct_evidence));
    }

    fn seed_review_database() -> Connection {
        let conn = Connection::open_in_memory().expect("open memory db");
        db::migrate(&conn).expect("migrate");
        conn.execute_batch(
            "
            INSERT INTO documents (id, title, created_at, updated_at, status)
            VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'review_needed');

            INSERT INTO source_files (id, document_id, path, file_name, mime_type, sha256, size_bytes, created_at)
            VALUES ('source-1', 'doc-1', '/tmp/sample.pdf', 'sample.pdf', 'application/pdf', 'abc', 12, '2026-05-02T00:00:00Z');

            INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at, finished_at)
            VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-02T00:00:00Z', '2026-05-02T00:00:01Z');

            INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
            ) VALUES
                ('block-1', 'run-1', 'doc-1', 'el-1', 3, 1, 'paragraph', NULL, '사업 개요 문장', NULL, '{}'),
                ('block-2', 'run-1', 'doc-1', 'el-2', 3, 2, 'table', NULL, 'SFR-001 API Gateway 구성', '[72,400,540,650]', '{}'),
                ('block-3', 'run-1', 'doc-1', 'el-3', 3, 3, 'paragraph', NULL, '연계 요구사항 설명', NULL, '{}');

            INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
            VALUES ('project-1', 'doc-1', 'rfp-v2-domain-test', 'review_needed', '검토용 분석 초안', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z');

            INSERT INTO rfp_fields (id, rfp_project_id, field_key, label, raw_value, normalized_value, confidence, source, created_at, updated_at)
            VALUES
                ('field-1', 'project-1', 'business_name', '사업명', 'API 고도화 사업', 'API 고도화 사업', 0.91, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'),
                ('field-2', 'project-1', 'client', '발주기관', '서울시', '서울시', 0.88, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z');

            INSERT INTO candidate_bundles (id, rfp_project_id, document_id, bundle_key, bundle_json, candidate_count, created_at)
            VALUES
                ('bundle-1', 'project-1', 'doc-1', 'project_info_candidates', '{}', 4, '2026-05-02T00:00:00Z'),
                ('bundle-2', 'project-1', 'doc-1', 'requirement_candidates', '{}', 7, '2026-05-02T00:00:00Z');

            INSERT INTO requirements (
                id, rfp_project_id, requirement_code, title, description, category, mandatory, confidence, source, created_at, updated_at
            ) VALUES (
                'req-1', 'project-1', 'SFR-001', 'API Gateway 구성', '통합 API Gateway를 구성한다.', 'technical', 1, 0.86, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'
            );

            INSERT INTO procurement_items (
                id, requirement_id, item_type, name, spec, quantity, unit, required, confidence, source, created_at, updated_at
            ) VALUES (
                'item-1', 'req-1', 'software', 'API Gateway', 'HA 구성', 1, '식', 1, 0.82, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'
            );

            INSERT INTO staffing_requirements (
                id, requirement_id, role, grade, headcount, mm, onsite, period_text, confidence, source, created_at, updated_at
            ) VALUES (
                'staff-1', 'req-1', 'API 개발자', '중급', 1, 3, 1, '착수 후 3개월', 0.84, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'
            );

            INSERT INTO risk_clauses (
                id, requirement_id, risk_type, severity, description, recommended_action, confidence, source, created_at, updated_at
            ) VALUES (
                'risk-1', 'req-1', 'short_schedule', 'high', '구축 기간이 짧다.', '일정 버퍼와 단계 검수를 질의한다.', 0.78, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'
            );

            INSERT INTO deliverables (
                id, requirement_id, name, due_text, format_text, description, confidence, source, created_at, updated_at
            ) VALUES (
                'deliverable-1', 'req-1', '통합시험 결과서', '검수 전', '문서', '통합시험 결과를 제출한다.', 0.81, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'
            );

            INSERT INTO acceptance_criteria (
                id, requirement_id, criterion_type, description, threshold, due_text, confidence, source, created_at, updated_at
            ) VALUES (
                'acceptance-1', 'req-1', 'test', '통합시험을 통과해야 한다.', '결함 0건', '검수 단계', 0.83, 'llm', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z'
            );

            INSERT INTO evidence_links (id, document_block_id, target_table, target_id, quote, confidence, created_at)
            VALUES
                ('ev-field-1', 'block-1', 'rfp_fields', 'field-1', 'API 고도화 사업', 0.9, '2026-05-02T00:00:00Z'),
                ('ev-req-1', 'block-2', 'requirements', 'req-1', 'SFR-001 API Gateway 구성', 0.92, '2026-05-02T00:00:00Z'),
                ('ev-item-1', 'block-2', 'procurement_items', 'item-1', 'API Gateway', 0.87, '2026-05-02T00:00:00Z'),
                ('ev-staff-1', 'block-3', 'staffing_requirements', 'staff-1', '3개월', 0.8, '2026-05-02T00:00:00Z'),
                ('ev-deliverable-1', 'block-3', 'deliverables', 'deliverable-1', '통합시험 결과서', 0.81, '2026-05-02T00:00:00Z'),
                ('ev-acceptance-1', 'block-3', 'acceptance_criteria', 'acceptance-1', '통합시험', 0.83, '2026-05-02T00:00:00Z'),
                ('ev-risk-1', 'block-3', 'risk_clauses', 'risk-1', '구축 기간', 0.76, '2026-05-02T00:00:00Z');

            INSERT INTO validation_findings (
                id, rfp_project_id, severity, finding_type, message, target_table, target_id, created_at
            ) VALUES
                ('finding-1', 'project-1', 'blocker', 'missing_budget', '사업예산이 추출되지 않았습니다.', 'rfp_projects', 'project-1', '2026-05-02T00:00:00Z'),
                ('finding-2', 'project-1', 'warning', 'low_confidence', '신뢰도가 낮은 항목이 있습니다.', 'requirements', 'req-1', '2026-05-02T00:00:00Z');
            ",
        )
        .expect("seed review data");
        conn
    }
}

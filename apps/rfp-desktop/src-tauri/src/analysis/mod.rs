use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::candidate_domain;
use crate::candidate_extractor;
use crate::domain_writer;
use crate::error::AppResult;
use crate::validation;

const ANALYSIS_VERSION: &str = "rfp-v2-baseline-2026-05-01";
const BASELINE_SUMMARY: &str = "LLM 없이 생성된 1차 분석 초안입니다.";
const CANDIDATE_ANALYSIS_VERSION: &str = "rfp-v2-candidates-2026-05-02";
const CANDIDATE_SUMMARY: &str = "규칙 기반 후보 추출로 생성된 분석 초안입니다.";
const DOMAIN_ANALYSIS_VERSION: &str = "rfp-v2-domain-writer-2026-05-02";
const DOMAIN_SUMMARY: &str = "구조화 domain draft로 생성된 분석 초안입니다.";

pub fn create_or_update_baseline_project(
    conn: &Connection,
    document_id: &str,
) -> AppResult<String> {
    let project_id =
        create_or_update_project_row(conn, document_id, ANALYSIS_VERSION, BASELINE_SUMMARY)?;
    domain_writer::clear_project_domain_rows(conn, &project_id)?;
    validation::evaluate_baseline_project(conn, &project_id)?;
    Ok(project_id)
}

pub fn create_or_update_candidate_project(
    conn: &Connection,
    document_id: &str,
) -> AppResult<String> {
    let project_id = create_or_update_project_row(
        conn,
        document_id,
        CANDIDATE_ANALYSIS_VERSION,
        CANDIDATE_SUMMARY,
    )?;
    domain_writer::clear_project_domain_rows(conn, &project_id)?;
    candidate_extractor::extract_and_store_candidates(conn, &project_id)?;
    let write_summary =
        if let Some(draft) = candidate_domain::build_rule_domain_draft(conn, &project_id)? {
            Some(domain_writer::write_domain_draft(conn, &project_id, draft)?)
        } else {
            None
        };
    validation::evaluate_candidate_project(conn, &project_id)?;
    if let Some(summary) = write_summary {
        validation::insert_domain_rejections(conn, &project_id, &summary.rejections)?;
        validation::refresh_project_status_from_findings(conn, &project_id)?;
    }
    Ok(project_id)
}

pub fn write_domain_analysis(
    conn: &mut Connection,
    document_id: &str,
    draft: domain_writer::DomainDraft,
) -> AppResult<domain_writer::DomainWriteSummary> {
    let project_id =
        create_or_update_project_row(conn, document_id, DOMAIN_ANALYSIS_VERSION, DOMAIN_SUMMARY)?;
    let summary = domain_writer::write_domain_draft(conn, &project_id, draft)?;
    validation::evaluate_project(conn, &project_id)?;
    validation::insert_domain_rejections(conn, &project_id, &summary.rejections)?;
    validation::refresh_project_status_from_findings(conn, &project_id)?;
    Ok(summary)
}

fn create_or_update_project_row(
    conn: &Connection,
    document_id: &str,
    analysis_version: &str,
    summary: &str,
) -> AppResult<String> {
    let now = Utc::now().to_rfc3339();
    let existing_project_id: Option<String> = conn
        .query_row(
            "SELECT id FROM rfp_projects WHERE document_id = ?",
            [document_id],
            |row| row.get(0),
        )
        .optional()?;

    let project_id = if let Some(project_id) = existing_project_id {
        conn.execute(
            "UPDATE rfp_projects
             SET analysis_version = ?, status = 'draft', summary = ?, updated_at = ?
             WHERE id = ?",
            params![analysis_version, summary, now, project_id],
        )?;
        project_id
    } else {
        let project_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO rfp_projects (
                id, document_id, analysis_version, status, summary, created_at, updated_at
             ) VALUES (?, ?, ?, 'draft', ?, ?, ?)",
            params![project_id, document_id, analysis_version, summary, now, now],
        )?;
        project_id
    };

    Ok(project_id)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::db;
    use crate::domain_writer::test_support::{full_domain_draft, seed_document_project_and_blocks};

    #[test]
    fn baseline_analysis_creates_review_needed_project_and_blockers() {
        let temp = tempdir().expect("temp dir");
        let conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'created')",
            [],
        )
        .expect("insert doc");

        let project_id = create_or_update_baseline_project(&conn, "doc-1").expect("analyze");

        let status: String = conn
            .query_row(
                "SELECT status FROM rfp_projects WHERE id = ?",
                [&project_id],
                |row| row.get(0),
            )
            .expect("project status");
        assert_eq!(status, "review_needed");

        let blocker_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'blocker'",
                [&project_id],
                |row| row.get(0),
            )
            .expect("blocker count");
        assert!(blocker_count >= 5);
    }

    #[test]
    fn candidate_analysis_removes_found_project_info_blockers() {
        let temp = tempdir().expect("temp dir");
        let conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'created')",
            [],
        )
        .expect("insert doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-02T00:00:00Z')",
            [],
        )
        .expect("insert run");
        for (id, index, text) in [
            ("block-1", 0, "사업명: 서울시 통합 유지관리 사업"),
            ("block-2", 1, "발주기관: 서울특별시"),
            ("block-3", 2, "사업예산: 1,200,000,000원"),
            ("block-4", 3, "사업기간: 계약일로부터 12개월"),
        ] {
            conn.execute(
                "INSERT INTO document_blocks (
                    id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                    kind, heading_level, text, bbox_json, raw_json
                 ) VALUES (?, 'run-1', 'doc-1', ?, 1, ?, 'paragraph', NULL, ?, NULL, '{}')",
                rusqlite::params![id, id, index, text],
            )
            .expect("insert block");
        }

        let project_id = create_or_update_candidate_project(&conn, "doc-1").expect("analyze");

        let missing_project_info_blockers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings
                 WHERE rfp_project_id = ?
                   AND finding_type IN (
                     'missing_business_name',
                     'missing_client',
                     'missing_budget',
                     'missing_period'
                   )",
                [&project_id],
                |row| row.get(0),
            )
            .expect("missing blockers");
        assert_eq!(missing_project_info_blockers, 0);

        let zero_requirements: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings
                 WHERE rfp_project_id = ? AND finding_type = 'zero_requirements'",
                [&project_id],
                |row| row.get(0),
            )
            .expect("zero requirements blocker");
        assert_eq!(zero_requirements, 1);
    }

    #[test]
    fn candidate_analysis_writes_rule_domain_rows_from_candidate_bundles() {
        let temp = tempdir().expect("temp dir");
        let conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'created')",
            [],
        )
        .expect("insert doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-02T00:00:00Z')",
            [],
        )
        .expect("insert run");
        for (id, page, index, kind, text) in [
            (
                "block-1",
                1,
                0,
                "paragraph",
                "사업명: 서울시 통합 유지관리 사업",
            ),
            ("block-2", 1, 1, "paragraph", "발주기관: 서울특별시"),
            ("block-3", 1, 2, "paragraph", "사업예산: 1,200,000,000원"),
            (
                "block-4",
                1,
                3,
                "paragraph",
                "사업기간: 계약일로부터 12개월",
            ),
            (
                "block-5",
                3,
                4,
                "table",
                "요구사항 고유번호 SFR-001 통합 로그인 기능을 제공한다.",
            ),
            (
                "block-6",
                4,
                5,
                "table",
                "서버 및 DB 소프트웨어 라이선스 2식을 납품한다.",
            ),
            (
                "block-7",
                5,
                6,
                "paragraph",
                "투입인력: PM 1명, 개발자 2명, 총 6MM 투입",
            ),
            (
                "block-8",
                6,
                7,
                "paragraph",
                "산출물: 설계서, 시험결과서, 완료보고서를 제출한다.",
            ),
            (
                "block-9",
                7,
                8,
                "paragraph",
                "검수 기준: 통합시험 통과 및 성능 시험 결과 승인",
            ),
            (
                "block-10",
                8,
                9,
                "paragraph",
                "지체상금 및 추가 비용 부담 책임은 사업자가 진다.",
            ),
        ] {
            conn.execute(
                "INSERT INTO document_blocks (
                    id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                    kind, heading_level, text, bbox_json, raw_json
                 ) VALUES (?, 'run-1', 'doc-1', ?, ?, ?, ?, NULL, ?, NULL, '{}')",
                rusqlite::params![id, id, page, index, kind, text],
            )
            .expect("insert block");
        }

        let project_id = create_or_update_candidate_project(&conn, "doc-1").expect("analyze");

        for (table, minimum) in [
            ("requirements", 1),
            ("procurement_items", 1),
            ("staffing_requirements", 1),
            ("deliverables", 1),
            ("acceptance_criteria", 1),
            ("risk_clauses", 1),
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("domain row count");
            assert!(
                count >= minimum,
                "expected {table} to have at least {minimum} row, got {count}"
            );
        }

        let source_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM requirements WHERE rfp_project_id = ? AND source = 'rule'",
                [&project_id],
                |row| row.get(0),
            )
            .expect("rule requirements");
        assert!(source_count >= 1);

        let zero_requirements: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings
                 WHERE rfp_project_id = ? AND finding_type = 'zero_requirements'",
                [&project_id],
                |row| row.get(0),
            )
            .expect("zero requirements");
        assert_eq!(zero_requirements, 0);
    }

    #[test]
    fn domain_analysis_writes_draft_and_marks_ready_when_gate_passes() {
        let temp = tempdir().expect("temp dir");
        let mut conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        let summary =
            write_domain_analysis(&mut conn, "doc-1", full_domain_draft()).expect("domain write");

        assert_eq!(summary.requirements_written, 1);
        let status: String = conn
            .query_row(
                "SELECT status FROM rfp_projects WHERE id = ?",
                [summary.rfp_project_id.as_str()],
                |row| row.get(0),
            )
            .expect("project status");
        assert_eq!(status, "ready");
    }

    #[test]
    fn candidate_analysis_after_domain_write_clears_stale_domain_rows() {
        let temp = tempdir().expect("temp dir");
        let mut conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_project_and_blocks(&conn);

        write_domain_analysis(&mut conn, "doc-1", full_domain_draft()).expect("domain write");
        let project_id = create_or_update_candidate_project(&conn, "doc-1").expect("candidate");

        let requirement_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM requirements WHERE rfp_project_id = ?",
                [project_id.as_str()],
                |row| row.get(0),
            )
            .expect("requirement count");
        assert_eq!(requirement_count, 0);

        let zero_requirements: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings
                 WHERE rfp_project_id = ? AND finding_type = 'zero_requirements'",
                [project_id.as_str()],
                |row| row.get(0),
            )
            .expect("zero requirements");
        assert_eq!(zero_requirements, 1);
    }
}

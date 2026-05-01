use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::AppResult;
use crate::validation;

const ANALYSIS_VERSION: &str = "rfp-v2-baseline-2026-05-01";
const BASELINE_SUMMARY: &str = "LLM 없이 생성된 1차 분석 초안입니다.";

pub fn create_or_update_baseline_project(
    conn: &Connection,
    document_id: &str,
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
            params![ANALYSIS_VERSION, BASELINE_SUMMARY, now, project_id],
        )?;
        project_id
    } else {
        let project_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO rfp_projects (
                id, document_id, analysis_version, status, summary, created_at, updated_at
             ) VALUES (?, ?, ?, 'draft', ?, ?, ?)",
            params![
                project_id,
                document_id,
                ANALYSIS_VERSION,
                BASELINE_SUMMARY,
                now,
                now
            ],
        )?;
        project_id
    };

    validation::evaluate_baseline_project(conn, &project_id)?;
    Ok(project_id)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::db;

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
}

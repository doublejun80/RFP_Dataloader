use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use rfp_desktop_lib::analysis;
use rfp_desktop_lib::block_normalizer;
use rfp_desktop_lib::db;
use rfp_desktop_lib::document_ingestion;
use rfp_desktop_lib::opendataloader_adapter;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("failed=true error={error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let pdf_path = env::args()
        .nth(1)
        .ok_or("usage: smoke_first_pdf /absolute/path/to/rfp.pdf")?;
    let root = env::current_dir()?;
    let smoke_dir = root.join(".smoke-rfp-v2");
    std::fs::create_dir_all(&smoke_dir)?;
    let db_path = smoke_dir.join("smoke.sqlite3");
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }

    let conn = db::open_database(&db_path)?;
    let document = document_ingestion::register_document(&conn, &PathBuf::from(pdf_path))?;
    let extraction =
        opendataloader_adapter::run_fast_extraction(&conn, &smoke_dir, &document.id, None)?;
    let json_path = extraction
        .json_path
        .as_ref()
        .ok_or("OpenDataLoader JSON path missing")?;
    let block_count = block_normalizer::normalize_extraction_json(
        &conn,
        &document.id,
        &extraction.id,
        &PathBuf::from(json_path),
    )?;
    let project_id = analysis::create_or_update_candidate_project(&conn, &document.id)?;
    let generated_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM rfp_projects", [], |row| row.get(0))?;
    let field_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM rfp_fields WHERE rfp_project_id = ?",
        [&project_id],
        |row| row.get(0),
    )?;
    let candidate_bundle_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM candidate_bundles WHERE rfp_project_id = ?",
        [&project_id],
        |row| row.get(0),
    )?;
    let field_evidence_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM evidence_links WHERE target_table = 'rfp_fields'",
        [],
        |row| row.get(0),
    )?;
    let requirement_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM requirements WHERE rfp_project_id = ?",
        [&project_id],
        |row| row.get(0),
    )?;
    let procurement_item_count: i64 = count_child_rows(&conn, &project_id, "procurement_items")?;
    let staffing_requirement_count: i64 =
        count_child_rows(&conn, &project_id, "staffing_requirements")?;
    let deliverable_count: i64 = count_child_rows(&conn, &project_id, "deliverables")?;
    let acceptance_criteria_count: i64 =
        count_child_rows(&conn, &project_id, "acceptance_criteria")?;
    let risk_clause_count: i64 = count_child_rows(&conn, &project_id, "risk_clauses")?;
    let domain_evidence_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM evidence_links
         WHERE target_table IN (
            'requirements',
            'procurement_items',
            'staffing_requirements',
            'deliverables',
            'acceptance_criteria',
            'risk_clauses'
         )",
        [],
        |row| row.get(0),
    )?;
    let (llm_enabled, llm_offline_mode, llm_provider): (i64, i64, String) = conn.query_row(
        "SELECT enabled, offline_mode, provider FROM llm_settings WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let llm_run_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM llm_runs", [], |row| row.get(0))?;
    let ready_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE status = 'ready'",
        [],
        |row| row.get(0),
    )?;
    let review_needed_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE status = 'review_needed'",
        [],
        |row| row.get(0),
    )?;
    let failed_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE status = 'failed'",
        [],
        |row| row.get(0),
    )?;
    let blocker_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'blocker'",
        [&project_id],
        |row| row.get(0),
    )?;
    let warning_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'warning'",
        [&project_id],
        |row| row.get(0),
    )?;

    println!("document_id={}", document.id);
    println!("extraction_status={}", extraction.status);
    println!("document_blocks={block_count}");
    println!("generated_count={generated_count}");
    println!("field_count={field_count}");
    println!("candidate_bundle_count={candidate_bundle_count}");
    println!("field_evidence_count={field_evidence_count}");
    println!("requirement_count={requirement_count}");
    println!("procurement_item_count={procurement_item_count}");
    println!("staffing_requirement_count={staffing_requirement_count}");
    println!("deliverable_count={deliverable_count}");
    println!("acceptance_criteria_count={acceptance_criteria_count}");
    println!("risk_clause_count={risk_clause_count}");
    println!("domain_evidence_count={domain_evidence_count}");
    println!("llm_enabled={llm_enabled}");
    println!("llm_offline_mode={llm_offline_mode}");
    println!("llm_provider={llm_provider}");
    println!("llm_run_count={llm_run_count}");
    println!("ready_count={ready_count}");
    println!("review_needed_count={review_needed_count}");
    println!("failed_count={failed_count}");
    println!("blocker_count={blocker_count}");
    println!("warning_count={warning_count}");

    if failed_count > 0 {
        Ok(ExitCode::from(1))
    } else if blocker_count > 0 {
        Ok(ExitCode::from(2))
    } else {
        Ok(ExitCode::from(0))
    }
}

fn count_child_rows(
    conn: &rusqlite::Connection,
    project_id: &str,
    table_name: &str,
) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        &format!(
            "SELECT COUNT(*)
             FROM {table_name} child
             JOIN requirements r ON r.id = child.requirement_id
             WHERE r.rfp_project_id = ?"
        ),
        [project_id],
        |row| row.get(0),
    )
}

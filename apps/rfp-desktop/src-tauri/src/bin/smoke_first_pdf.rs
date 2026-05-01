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
    let project_id = analysis::create_or_update_baseline_project(&conn, &document.id)?;
    let generated_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM rfp_projects", [], |row| row.get(0))?;
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


use std::fs;
use std::io::Read;
use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::DocumentSummary;
use crate::error::{AppError, AppResult};

pub fn register_document(conn: &Connection, path: &Path) -> AppResult<DocumentSummary> {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("pdf"))
        != Some(true)
    {
        return Err(AppError::InvalidInput(
            "PDF 파일만 등록할 수 있습니다.".to_string(),
        ));
    }

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(AppError::InvalidInput("파일 경로가 아닙니다.".to_string()));
    }

    let sha256 = calculate_sha256(path)?;
    if let Some(existing_id) = find_document_by_sha(conn, &sha256)? {
        return load_document_summary(conn, &existing_id);
    }

    let now = Utc::now().to_rfc3339();
    let document_id = Uuid::new_v4().to_string();
    let source_file_id = Uuid::new_v4().to_string();
    let audit_id = Uuid::new_v4().to_string();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::InvalidInput("파일명을 읽을 수 없습니다.".to_string()))?
        .to_string();

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO documents (id, title, created_at, updated_at, status) VALUES (?, ?, ?, ?, 'created')",
        params![document_id, file_name, now, now],
    )?;
    tx.execute(
        "INSERT INTO source_files (id, document_id, path, file_name, mime_type, sha256, size_bytes, created_at)
         VALUES (?, ?, ?, ?, 'application/pdf', ?, ?, ?)",
        params![
            source_file_id,
            document_id,
            path.to_string_lossy().to_string(),
            file_name,
            sha256,
            metadata.len() as i64,
            now
        ],
    )?;
    tx.execute(
        "INSERT INTO audit_events (id, document_id, event_type, payload_json, created_at)
         VALUES (?, ?, 'document_added', ?, ?)",
        params![
            audit_id,
            document_id,
            serde_json::json!({ "path": path.to_string_lossy().to_string() }).to_string(),
            now
        ],
    )?;
    tx.commit()?;

    load_document_summary(conn, &document_id)
}

fn calculate_sha256(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn find_document_by_sha(conn: &Connection, sha256: &str) -> AppResult<Option<String>> {
    let value = conn
        .query_row(
            "SELECT document_id FROM source_files WHERE sha256 = ?",
            [sha256],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value)
}

pub fn load_document_summary(conn: &Connection, document_id: &str) -> AppResult<DocumentSummary> {
    let summary = conn.query_row(
        "SELECT
            d.id,
            d.title,
            d.status,
            sf.file_name,
            COALESCE(SUM(CASE WHEN vf.severity = 'blocker' THEN 1 ELSE 0 END), 0) AS blocker_count,
            COALESCE(SUM(CASE WHEN vf.severity = 'warning' THEN 1 ELSE 0 END), 0) AS warning_count,
            (SELECT COUNT(*) FROM document_blocks db WHERE db.document_id = d.id) AS block_count
         FROM documents d
         LEFT JOIN source_files sf ON sf.document_id = d.id
         LEFT JOIN rfp_projects rp ON rp.document_id = d.id
         LEFT JOIN validation_findings vf ON vf.rfp_project_id = rp.id
         WHERE d.id = ?
         GROUP BY d.id, sf.file_name",
        [document_id],
        |row| {
            Ok(DocumentSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                file_name: row.get(3)?,
                blocker_count: row.get(4)?,
                warning_count: row.get(5)?,
                block_count: row.get(6)?,
            })
        },
    )?;
    Ok(summary)
}

pub fn list_documents(conn: &Connection) -> AppResult<Vec<DocumentSummary>> {
    let mut statement = conn.prepare("SELECT d.id FROM documents d ORDER BY d.created_at DESC")?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| load_document_summary(conn, &id))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::db;

    #[test]
    fn register_document_creates_source_file_and_audit_event() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let pdf_path = temp.path().join("sample.pdf");
        fs::write(&pdf_path, b"%PDF-1.7\nsample").expect("write pdf");
        let conn = db::open_database(&db_path).expect("open db");

        let document = register_document(&conn, &pdf_path).expect("register");

        assert_eq!(document.status, "created");
        assert_eq!(document.title, "sample.pdf");

        let source_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_files WHERE document_id = ?",
                [&document.id],
                |row| row.get(0),
            )
            .expect("source count");
        assert_eq!(source_count, 1);

        let audit_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE document_id = ?",
                [&document.id],
                |row| row.get(0),
            )
            .expect("audit count");
        assert_eq!(audit_count, 1);
    }
}

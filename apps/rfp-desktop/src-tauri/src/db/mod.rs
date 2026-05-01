use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_core.sql"),
    include_str!("../../migrations/0002_candidate_extractor.sql"),
];

pub fn open_database(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> AppResult<()> {
    for migration in MIGRATIONS {
        conn.execute_batch(migration)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_core_tables() {
        let conn = Connection::open_in_memory().expect("open memory db");

        migrate(&conn).expect("run migrations");

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'documents',
                    'source_files',
                    'extraction_runs',
                    'document_blocks',
                    'rfp_projects',
                    'validation_findings',
                    'audit_events'
                )",
                [],
                |row| row.get(0),
            )
            .expect("count tables");
        assert_eq!(table_count, 7);
    }

    #[test]
    fn migrates_candidate_extractor_tables() {
        let conn = Connection::open_in_memory().expect("open memory db");

        migrate(&conn).expect("run migrations");

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'rfp_fields',
                    'evidence_links',
                    'candidate_bundles'
                )",
                [],
                |row| row.get(0),
            )
            .expect("count candidate tables");
        assert_eq!(table_count, 3);

        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN (
                    'idx_rfp_fields_project_key',
                    'idx_evidence_links_target',
                    'idx_candidate_bundles_project_key'
                )",
                [],
                |row| row.get(0),
            )
            .expect("count candidate indexes");
        assert_eq!(index_count, 3);
    }
}

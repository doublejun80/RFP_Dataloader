use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

const CORE_MIGRATION: &str = include_str!("../../migrations/0001_core.sql");

pub fn open_database(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(CORE_MIGRATION)?;
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
}

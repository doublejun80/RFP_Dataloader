use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::error::AppResult;

const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_core.sql"),
    include_str!("../../migrations/0002_candidate_extractor.sql"),
    include_str!("../../migrations/0003_domain_writer.sql"),
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
    repair_shared_candidate_domain_schema(conn)?;
    Ok(())
}

fn repair_shared_candidate_domain_schema(conn: &Connection) -> AppResult<()> {
    if let Some(sql) = table_sql(conn, "rfp_fields")? {
        let needs_rebuild = !has_column(conn, "rfp_fields", "created_at")?
            || !has_column(conn, "rfp_fields", "updated_at")?
            || !sql.contains("requirement_count");
        if needs_rebuild {
            rebuild_rfp_fields(conn)?;
        }
    }

    if let Some(sql) = table_sql(conn, "evidence_links")? {
        let needs_rebuild =
            !has_column(conn, "evidence_links", "created_at")? || !sql.contains("risk_clauses");
        if needs_rebuild {
            rebuild_evidence_links(conn)?;
        }
    }

    Ok(())
}

fn table_sql(conn: &Connection, table_name: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
            [table_name],
            |row| row.get(0),
        )
        .optional()?)
}

fn has_column(conn: &Connection, table_name: &str, column_name: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table_name}') WHERE name = ?"),
        [column_name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn rebuild_rfp_fields(conn: &Connection) -> AppResult<()> {
    let created_expr = if has_column(conn, "rfp_fields", "created_at")? {
        "created_at"
    } else {
        "datetime('now')"
    };
    let updated_expr = if has_column(conn, "rfp_fields", "updated_at")? {
        "updated_at"
    } else {
        "datetime('now')"
    };

    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE IF EXISTS rfp_fields_rebuild;
         CREATE TABLE rfp_fields_rebuild (
           id TEXT PRIMARY KEY,
           rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
           field_key TEXT NOT NULL CHECK (
             field_key IN (
               'business_name',
               'client',
               'budget',
               'period',
               'contract_method',
               'deadline',
               'evaluation_ratio',
               'requirement_count'
             )
           ),
           label TEXT NOT NULL,
           raw_value TEXT NOT NULL,
           normalized_value TEXT NOT NULL,
           confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
           source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );",
    )?;
    conn.execute(
        &format!(
            "INSERT INTO rfp_fields_rebuild (
                id, rfp_project_id, field_key, label, raw_value, normalized_value,
                confidence, source, created_at, updated_at
             )
             SELECT id, rfp_project_id, field_key, label, raw_value, normalized_value,
                    confidence, source, {created_expr}, {updated_expr}
             FROM rfp_fields
             WHERE field_key IN (
                'business_name',
                'client',
                'budget',
                'period',
                'contract_method',
                'deadline',
                'evaluation_ratio',
                'requirement_count'
             )"
        ),
        [],
    )?;
    conn.execute_batch(
        "DROP TABLE rfp_fields;
         ALTER TABLE rfp_fields_rebuild RENAME TO rfp_fields;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_rfp_fields_project_key
           ON rfp_fields(rfp_project_id, field_key);
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

fn rebuild_evidence_links(conn: &Connection) -> AppResult<()> {
    let created_expr = if has_column(conn, "evidence_links", "created_at")? {
        "created_at"
    } else {
        "datetime('now')"
    };
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE IF EXISTS evidence_links_rebuild;
         CREATE TABLE evidence_links_rebuild (
           id TEXT PRIMARY KEY,
           document_block_id TEXT NOT NULL REFERENCES document_blocks(id) ON DELETE CASCADE,
           target_table TEXT NOT NULL CHECK (
             target_table IN (
               'rfp_fields',
               'requirements',
               'procurement_items',
               'staffing_requirements',
               'deliverables',
               'acceptance_criteria',
               'risk_clauses'
             )
           ),
           target_id TEXT NOT NULL,
           quote TEXT NOT NULL,
           confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
           created_at TEXT NOT NULL
         );",
    )?;
    conn.execute(
        &format!(
            "INSERT INTO evidence_links_rebuild (
                id, document_block_id, target_table, target_id, quote, confidence, created_at
             )
             SELECT id, document_block_id, target_table, target_id, quote, confidence, {created_expr}
             FROM evidence_links
             WHERE target_table IN (
               'rfp_fields',
               'requirements',
               'procurement_items',
               'staffing_requirements',
               'deliverables',
               'acceptance_criteria',
               'risk_clauses'
             )"
        ),
        [],
    )?;
    conn.execute_batch(
        "DROP TABLE evidence_links;
         ALTER TABLE evidence_links_rebuild RENAME TO evidence_links;
         CREATE INDEX IF NOT EXISTS idx_evidence_links_target
           ON evidence_links(target_table, target_id);
         CREATE INDEX IF NOT EXISTS idx_evidence_links_block_id
           ON evidence_links(document_block_id);
         PRAGMA foreign_keys = ON;",
    )?;
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

    #[test]
    fn migrates_domain_writer_tables() {
        let conn = Connection::open_in_memory().expect("open memory db");

        migrate(&conn).expect("run migrations");

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'rfp_fields',
                    'requirements',
                    'procurement_items',
                    'staffing_requirements',
                    'deliverables',
                    'acceptance_criteria',
                    'risk_clauses',
                    'evidence_links'
                )",
                [],
                |row| row.get(0),
            )
            .expect("count domain tables");
        assert_eq!(table_count, 8);
    }

    #[test]
    fn migrates_legacy_candidate_schema_to_shared_domain_schema() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(MIGRATIONS[0])
            .expect("run core migration");
        conn.execute_batch(
            "CREATE TABLE rfp_fields (
                id TEXT PRIMARY KEY,
                rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
                field_key TEXT NOT NULL CHECK (
                  field_key IN (
                    'business_name',
                    'client',
                    'budget',
                    'period',
                    'contract_method',
                    'deadline'
                  )
                ),
                label TEXT NOT NULL,
                raw_value TEXT NOT NULL,
                normalized_value TEXT NOT NULL,
                confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
                source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction'))
             );
             CREATE TABLE evidence_links (
                id TEXT PRIMARY KEY,
                document_block_id TEXT NOT NULL REFERENCES document_blocks(id) ON DELETE CASCADE,
                target_table TEXT NOT NULL,
                target_id TEXT NOT NULL,
                quote TEXT NOT NULL,
                confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0)
             );",
        )
        .expect("create legacy candidate tables");

        migrate(&conn).expect("run migrations");

        let rfp_field_columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('rfp_fields')
                 WHERE name IN ('created_at', 'updated_at')",
                [],
                |row| row.get(0),
            )
            .expect("count rfp_fields columns");
        assert_eq!(rfp_field_columns, 2);

        let evidence_created_at: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('evidence_links')
                 WHERE name = 'created_at'",
                [],
                |row| row.get(0),
            )
            .expect("count evidence column");
        assert_eq!(evidence_created_at, 1);

        let rfp_fields_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'rfp_fields'",
                [],
                |row| row.get(0),
            )
            .expect("table sql");
        assert!(rfp_fields_sql.contains("requirement_count"));
    }
}

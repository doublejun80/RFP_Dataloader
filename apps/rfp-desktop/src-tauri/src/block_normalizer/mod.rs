use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};
use serde_json::Value;
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone)]
struct NormalizedBlock {
    source_element_id: String,
    page_number: i64,
    kind: String,
    heading_level: Option<i64>,
    text: String,
    bbox_json: Option<String>,
    raw_json: String,
}

pub fn normalize_extraction_json(
    conn: &Connection,
    document_id: &str,
    extraction_run_id: &str,
    json_path: &Path,
) -> AppResult<usize> {
    let content = fs::read_to_string(json_path)?;
    let value: Value = serde_json::from_str(&content)?;
    let mut blocks = Vec::new();
    collect_blocks(&value, &mut blocks)?;

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM document_blocks WHERE extraction_run_id = ?",
        [extraction_run_id],
    )?;
    for (index, block) in blocks.iter().enumerate() {
        tx.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                Uuid::new_v4().to_string(),
                extraction_run_id,
                document_id,
                block.source_element_id,
                block.page_number,
                index as i64,
                block.kind,
                block.heading_level,
                block.text,
                block.bbox_json,
                block.raw_json,
            ],
        )?;
    }
    tx.commit()?;
    Ok(blocks.len())
}

fn collect_blocks(value: &Value, blocks: &mut Vec<NormalizedBlock>) -> AppResult<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_blocks(item, blocks)?;
            }
        }
        Value::Object(map) => {
            if let Some(children) = first_array(
                map,
                &[
                    "elements",
                    "items",
                    "blocks",
                    "kids",
                    "list items",
                    "rows",
                    "cells",
                ],
            ) {
                for child in children {
                    collect_blocks(child, blocks)?;
                }
            }

            if let Some(text) = first_string(map, &["text", "content", "markdown", "value"]) {
                blocks.push(NormalizedBlock {
                    source_element_id: first_string(map, &["id", "element_id", "element id"])
                        .unwrap_or_else(|| format!("generated-{}", blocks.len())),
                    page_number: first_i64(map, &["page", "page_number", "page number"])
                        .unwrap_or(1),
                    kind: normalize_kind(
                        &first_string(map, &["type", "kind", "role", "category"])
                            .unwrap_or_else(|| "unknown".to_string()),
                    ),
                    heading_level: first_i64(map, &["heading_level", "heading level", "level"]),
                    text,
                    bbox_json: first_value(map, &["bbox", "bounding_box", "bounding box"])
                        .map(Value::to_string),
                    raw_json: Value::Object(map.clone()).to_string(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn first_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key).and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
    })
}

fn first_i64(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        map.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.parse::<i64>().ok(),
            _ => None,
        })
    })
}

fn first_array<'a>(
    map: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<&'a Vec<Value>> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(Value::as_array))
}

fn first_value<'a>(map: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| map.get(*key))
}

fn normalize_kind(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "heading" | "title" => "heading".to_string(),
        "paragraph" | "text" => "paragraph".to_string(),
        "table" => "table".to_string(),
        "list" | "list_item" | "list item" => "list".to_string(),
        "image" => "image".to_string(),
        "caption" => "caption".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::db;

    #[test]
    fn normalizes_key_variants_and_nested_elements() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let json_path = temp.path().join("sample-output.json");
        fs::write(
            &json_path,
            include_str!("../../../../../fixtures/opendataloader/sample-output.json"),
        )
        .expect("write fixture");
        let conn = db::open_database(&db_path).expect("open db");
        seed_document_and_run(&conn, "doc-1", "run-1");

        let count =
            normalize_extraction_json(&conn, "doc-1", "run-1", &json_path).expect("normalize");

        assert_eq!(count, 3);
        let heading: (i64, String, Option<i64>, String, Option<String>, String) = conn
            .query_row(
                "SELECT page_number, kind, heading_level, text, bbox_json, raw_json
                 FROM document_blocks
                 WHERE source_element_id = '42'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("heading block");
        assert_eq!(heading.0, 1);
        assert_eq!(heading.1, "heading");
        assert_eq!(heading.2, Some(1));
        assert_eq!(heading.3, "사업 개요");
        assert!(heading.4.expect("heading bbox").contains("700"));
        assert!(heading.5.contains("\"content\":\"사업 개요\""));

        let table: (i64, String, String, Option<String>) = conn
            .query_row(
                "SELECT page_number, kind, text, bbox_json
                 FROM document_blocks
                 WHERE source_element_id = 'req-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("table block");
        assert_eq!(table.0, 2);
        assert_eq!(table.1, "table");
        assert!(table.2.contains("SFR-001"));
        assert!(table.3.expect("table bbox").contains("650"));

        let risk: (i64, String, String, String) = conn
            .query_row(
                "SELECT page_number, kind, text, raw_json
                 FROM document_blocks
                 WHERE source_element_id = 'risk-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("risk block");
        assert_eq!(risk.0, 3);
        assert_eq!(risk.1, "paragraph");
        assert!(risk.2.contains("무상"));
        assert!(risk
            .3
            .contains("\"value\":\"필요 시 추가 산출물을 무상으로 제출한다.\""));
    }

    fn seed_document_and_run(conn: &rusqlite::Connection, document_id: &str, run_id: &str) {
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES (?, 'sample.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'extracting')",
            [document_id],
        )
        .expect("insert doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES (?, ?, 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
            [run_id, document_id],
        )
        .expect("insert run");
    }
}

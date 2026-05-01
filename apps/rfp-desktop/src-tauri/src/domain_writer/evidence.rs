use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use uuid::Uuid;

use crate::domain_writer::EvidenceDraft;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct EvidenceBlock {
    pub id: String,
    pub kind: String,
    pub text: String,
}

pub fn load_valid_evidence_blocks(
    tx: &Transaction<'_>,
    document_id: &str,
    evidence: &[EvidenceDraft],
) -> AppResult<Vec<(EvidenceDraft, EvidenceBlock)>> {
    let mut valid = Vec::new();
    for item in evidence {
        if !(0.0..=1.0).contains(&item.confidence) {
            continue;
        }
        let block = tx
            .query_row(
                "SELECT id, kind, text
                 FROM document_blocks
                 WHERE id = ? AND document_id = ?",
                params![item.block_id, document_id],
                |row| {
                    Ok(EvidenceBlock {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        text: row.get(2)?,
                    })
                },
            )
            .optional()?;
        if let Some(block) = block {
            valid.push((item.clone(), block));
        }
    }
    Ok(valid)
}

pub fn insert_evidence_links(
    tx: &Transaction<'_>,
    target_table: &str,
    target_id: &str,
    evidence_blocks: &[(EvidenceDraft, EvidenceBlock)],
) -> AppResult<usize> {
    if evidence_blocks.is_empty() {
        return Err(AppError::InvalidInput(
            "근거 block이 없는 domain row는 저장할 수 없습니다.".to_string(),
        ));
    }

    for (draft, block) in evidence_blocks {
        tx.execute(
            "INSERT INTO evidence_links (
                id, document_block_id, target_table, target_id, quote, confidence, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                Uuid::new_v4().to_string(),
                block.id,
                target_table,
                target_id,
                build_quote(draft, block),
                draft.confidence,
                Utc::now().to_rfc3339(),
            ],
        )?;
    }
    Ok(evidence_blocks.len())
}

pub fn build_quote(draft: &EvidenceDraft, block: &EvidenceBlock) -> String {
    let candidate = draft
        .quote
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| block.text.trim());
    let quote = if candidate.is_empty() && (block.kind == "table" || block.kind == "image") {
        "[empty block]"
    } else {
        candidate
    };
    quote.chars().take(500).collect()
}

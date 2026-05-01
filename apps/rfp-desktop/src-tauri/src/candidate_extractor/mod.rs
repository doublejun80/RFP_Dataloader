use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;

const MAX_SNIPPETS_PER_BUNDLE: usize = 80;
const MAX_QUOTE_CHARS: usize = 600;
const CONTEXT_WINDOW: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateBundleKey {
    ProjectInfoCandidates,
    RequirementCandidates,
    ProcurementCandidates,
    StaffingCandidates,
    DeliverableCandidates,
    AcceptanceCandidates,
    RiskCandidates,
}

impl CandidateBundleKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectInfoCandidates => "project_info_candidates",
            Self::RequirementCandidates => "requirement_candidates",
            Self::ProcurementCandidates => "procurement_candidates",
            Self::StaffingCandidates => "staffing_candidates",
            Self::DeliverableCandidates => "deliverable_candidates",
            Self::AcceptanceCandidates => "acceptance_candidates",
            Self::RiskCandidates => "risk_candidates",
        }
    }

    pub fn all() -> &'static [CandidateBundleKey] {
        &[
            Self::ProjectInfoCandidates,
            Self::RequirementCandidates,
            Self::ProcurementCandidates,
            Self::StaffingCandidates,
            Self::DeliverableCandidates,
            Self::AcceptanceCandidates,
            Self::RiskCandidates,
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceBlock {
    pub id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub heading_level: Option<i64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSnippet {
    pub document_block_id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub quote: String,
    pub score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBundle {
    pub bundle_key: String,
    pub document_id: String,
    pub rfp_project_id: String,
    pub snippets: Vec<CandidateSnippet>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateExtractionResult {
    pub rfp_project_id: String,
    pub document_id: String,
    pub bundle_count: usize,
    pub field_count: usize,
}

pub fn build_candidate_bundles(
    document_id: &str,
    rfp_project_id: &str,
    blocks: &[SourceBlock],
) -> Vec<CandidateBundle> {
    CandidateBundleKey::all()
        .iter()
        .map(|key| CandidateBundle {
            bundle_key: key.as_str().to_string(),
            document_id: document_id.to_string(),
            rfp_project_id: rfp_project_id.to_string(),
            snippets: select_snippets(*key, blocks),
        })
        .collect()
}

fn select_snippets(key: CandidateBundleKey, blocks: &[SourceBlock]) -> Vec<CandidateSnippet> {
    let mut snippets = Vec::new();
    let mut selected_ids = std::collections::HashSet::new();

    for block in blocks {
        let (score, reasons) = score_block(key, block, blocks);
        if score >= 0.45 && selected_ids.insert(block.id.clone()) {
            add_with_context(&mut snippets, &mut selected_ids, key, block, blocks);
            snippets.push(CandidateSnippet {
                document_block_id: block.id.clone(),
                page_number: block.page_number,
                block_index: block.block_index,
                kind: block.kind.clone(),
                quote: quote(&block.text),
                score,
                reasons,
            });
        }
    }

    snippets.sort_by(|left, right| {
        left.page_number
            .cmp(&right.page_number)
            .then(left.block_index.cmp(&right.block_index))
            .then(left.document_block_id.cmp(&right.document_block_id))
    });
    snippets.truncate(MAX_SNIPPETS_PER_BUNDLE);
    snippets
}

fn add_with_context(
    snippets: &mut Vec<CandidateSnippet>,
    selected_ids: &mut std::collections::HashSet<String>,
    key: CandidateBundleKey,
    anchor: &SourceBlock,
    blocks: &[SourceBlock],
) {
    if key != CandidateBundleKey::ProjectInfoCandidates {
        return;
    }

    for block in blocks {
        let near_anchor = block.page_number == anchor.page_number
            && (block.block_index - anchor.block_index).abs() <= CONTEXT_WINDOW;
        if near_anchor && selected_ids.insert(block.id.clone()) {
            snippets.push(CandidateSnippet {
                document_block_id: block.id.clone(),
                page_number: block.page_number,
                block_index: block.block_index,
                kind: block.kind.clone(),
                quote: quote(&block.text),
                score: 0.45,
                reasons: vec!["context_window".to_string()],
            });
        }
    }
}

fn score_block(
    key: CandidateBundleKey,
    block: &SourceBlock,
    blocks: &[SourceBlock],
) -> (f64, Vec<String>) {
    let text = normalize_space(&block.text);
    if text.is_empty() {
        return (0.0, Vec::new());
    }

    let mut score: f64 = 0.0;
    let mut reasons = Vec::new();

    for (reason, terms) in keyword_groups(key) {
        if contains_any(&text, terms) {
            score += 0.55;
            reasons.push(reason.to_string());
            break;
        }
    }

    if block.kind == "table" {
        score += 0.15;
        reasons.push("kind:table".to_string());
    }

    if key == CandidateBundleKey::ProjectInfoCandidates && (1..=5).contains(&block.page_number) {
        score += 0.05;
        reasons.push("early_page".to_string());
    }

    if has_label_value_shape(&text) {
        score += 0.10;
        reasons.push("same_block_value".to_string());
    }

    if has_recent_heading_match(key, block, blocks) {
        score += 0.10;
        reasons.push("heading_context".to_string());
    }

    (score.min(0.95), reasons)
}

fn keyword_groups(key: CandidateBundleKey) -> &'static [(&'static str, &'static [&'static str])] {
    match key {
        CandidateBundleKey::ProjectInfoCandidates => &[
            (
                "label:business_name",
                &["사업명", "용역명", "과업명", "프로젝트명"],
            ),
            (
                "label:client",
                &["발주기관", "수요기관", "주관기관", "발주처", "기관명"],
            ),
            (
                "label:budget",
                &["사업예산", "예산", "추정가격", "기초금액", "사업비"],
            ),
            (
                "label:period",
                &["사업기간", "용역기간", "과업기간", "계약기간", "수행기간"],
            ),
            (
                "label:contract_method",
                &[
                    "계약방법",
                    "계약방식",
                    "입찰방식",
                    "낙찰자 결정",
                    "협상에 의한 계약",
                ],
            ),
            (
                "label:deadline",
                &["제출마감", "마감일", "접수마감", "입찰마감", "제안서 제출"],
            ),
        ],
        CandidateBundleKey::RequirementCandidates => &[(
            "keyword:requirements",
            &[
                "요구사항",
                "요구 기능",
                "기능 요구",
                "요구사항 ID",
                "고유번호",
                "SFR-",
                "REQ-",
                "요구사항 총괄표",
            ],
        )],
        CandidateBundleKey::ProcurementCandidates => &[(
            "keyword:procurement",
            &[
                "장비",
                "서버",
                "스토리지",
                "소프트웨어",
                "SW",
                "라이선스",
                "license",
                "클라우드",
                "DB",
                "데이터베이스",
                "네트워크",
                "보안솔루션",
            ],
        )],
        CandidateBundleKey::StaffingCandidates => &[(
            "keyword:staffing",
            &[
                "투입인력",
                "인력",
                "PM",
                "PL",
                "개발자",
                "상주",
                "MM",
                "M/M",
                "수행조직",
                "등급",
            ],
        )],
        CandidateBundleKey::DeliverableCandidates => &[(
            "keyword:deliverable",
            &[
                "산출물",
                "납품물",
                "보고서",
                "설계서",
                "매뉴얼",
                "교육자료",
                "완료보고",
                "제출물",
            ],
        )],
        CandidateBundleKey::AcceptanceCandidates => &[(
            "keyword:acceptance",
            &[
                "검수",
                "인수",
                "시험",
                "성능",
                "보안점검",
                "하자보수",
                "SLA",
                "검사",
                "승인",
            ],
        )],
        CandidateBundleKey::RiskCandidates => &[(
            "keyword:risk",
            &[
                "무상",
                "추가 요청",
                "협의",
                "필요 시",
                "지체상금",
                "책임",
                "비용 부담",
                "손해배상",
                "위약",
                "특정 업체",
            ],
        )],
    }
}

fn has_recent_heading_match(
    key: CandidateBundleKey,
    block: &SourceBlock,
    blocks: &[SourceBlock],
) -> bool {
    blocks.iter().rev().any(|candidate| {
        candidate.page_number == block.page_number
            && candidate.block_index < block.block_index
            && block.block_index - candidate.block_index <= 3
            && candidate.kind == "heading"
            && keyword_groups(key)
                .iter()
                .any(|(_, terms)| contains_any(&candidate.text, terms))
    })
}

fn contains_any(text: &str, terms: &[&str]) -> bool {
    let lower = text.to_lowercase();
    terms
        .iter()
        .any(|term| lower.contains(&term.to_lowercase()))
}

fn has_label_value_shape(text: &str) -> bool {
    [":", "：", "-"].iter().any(|separator| {
        let mut parts = text.splitn(2, separator);
        let left = parts.next().unwrap_or_default().trim();
        let right = parts.next().unwrap_or_default().trim();
        !left.is_empty() && !right.is_empty() && left.chars().count() <= 24
    })
}

fn quote(text: &str) -> String {
    normalize_space(text)
        .chars()
        .take(MAX_QUOTE_CHARS)
        .collect()
}

fn normalize_space(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn extract_and_store_candidates(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<CandidateExtractionResult> {
    let document_id = load_project_document_id(conn, rfp_project_id)?;
    let blocks = load_source_blocks(conn, &document_id)?;
    let bundles = build_candidate_bundles(&document_id, rfp_project_id, &blocks);

    let tx = conn.unchecked_transaction()?;
    clear_project_candidate_outputs(&tx, rfp_project_id)?;
    store_bundles(&tx, &bundles)?;
    let field_count = store_project_info_fields(&tx, rfp_project_id, &bundles)?;
    tx.commit()?;

    Ok(CandidateExtractionResult {
        rfp_project_id: rfp_project_id.to_string(),
        document_id,
        bundle_count: bundles.len(),
        field_count,
    })
}

fn load_project_document_id(conn: &Connection, rfp_project_id: &str) -> AppResult<String> {
    Ok(conn.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?)
}

fn load_source_blocks(conn: &Connection, document_id: &str) -> AppResult<Vec<SourceBlock>> {
    let mut statement = conn.prepare(
        "SELECT id, page_number, block_index, kind, heading_level, text
         FROM document_blocks
         WHERE document_id = ? AND TRIM(text) <> ''
         ORDER BY page_number, block_index",
    )?;
    let blocks = statement
        .query_map([document_id], |row| {
            Ok(SourceBlock {
                id: row.get(0)?,
                page_number: row.get(1)?,
                block_index: row.get(2)?,
                kind: row.get(3)?,
                heading_level: row.get(4)?,
                text: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(blocks)
}

fn clear_project_candidate_outputs(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM evidence_links
         WHERE target_table = 'rfp_fields'
           AND target_id IN (SELECT id FROM rfp_fields WHERE rfp_project_id = ?)",
        [rfp_project_id],
    )?;
    conn.execute(
        "DELETE FROM rfp_fields WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;
    conn.execute(
        "DELETE FROM candidate_bundles WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;
    Ok(())
}

fn store_bundles(conn: &Connection, bundles: &[CandidateBundle]) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    for bundle in bundles {
        conn.execute(
            "INSERT INTO candidate_bundles (
                id, rfp_project_id, document_id, bundle_key, bundle_json, candidate_count, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                Uuid::new_v4().to_string(),
                &bundle.rfp_project_id,
                &bundle.document_id,
                &bundle.bundle_key,
                serde_json::to_string(bundle)?,
                bundle.snippets.len() as i64,
                now,
            ],
        )?;
    }
    Ok(())
}

fn store_project_info_fields(
    conn: &Connection,
    rfp_project_id: &str,
    bundles: &[CandidateBundle],
) -> AppResult<usize> {
    let Some(project_info) = bundles
        .iter()
        .find(|bundle| bundle.bundle_key == CandidateBundleKey::ProjectInfoCandidates.as_str())
    else {
        return Ok(0);
    };

    let candidates = best_field_candidates(project_info);
    let mut inserted = 0;

    for candidate in candidates {
        if candidate.confidence < 0.55 {
            continue;
        }

        let field_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO rfp_fields (
                id, rfp_project_id, field_key, label, raw_value, normalized_value, confidence, source
             ) VALUES (?, ?, ?, ?, ?, ?, ?, 'rule')",
            params![
                field_id,
                rfp_project_id,
                candidate.spec.key,
                candidate.spec.label,
                candidate.raw_value,
                candidate.normalized_value,
                candidate.confidence,
            ],
        )?;
        conn.execute(
            "INSERT INTO evidence_links (
                id, document_block_id, target_table, target_id, quote, confidence
             ) VALUES (?, ?, 'rfp_fields', ?, ?, ?)",
            params![
                Uuid::new_v4().to_string(),
                candidate.snippet.document_block_id,
                field_id,
                candidate.snippet.quote,
                candidate.confidence,
            ],
        )?;
        inserted += 1;
    }

    Ok(inserted)
}

#[derive(Debug, Clone, Copy)]
struct FieldSpec {
    key: &'static str,
    label: &'static str,
    terms: &'static [&'static str],
}

const FIELD_SPECS: &[FieldSpec] = &[
    FieldSpec {
        key: "business_name",
        label: "사업명",
        terms: &["사업명", "용역명", "과업명", "프로젝트명"],
    },
    FieldSpec {
        key: "client",
        label: "발주기관",
        terms: &["발주기관", "수요기관", "주관기관", "발주처", "기관명"],
    },
    FieldSpec {
        key: "budget",
        label: "사업예산",
        terms: &["사업예산", "예산", "추정가격", "기초금액", "사업비"],
    },
    FieldSpec {
        key: "period",
        label: "사업기간",
        terms: &["사업기간", "용역기간", "과업기간", "계약기간", "수행기간"],
    },
    FieldSpec {
        key: "contract_method",
        label: "계약방식",
        terms: &[
            "계약방법",
            "계약방식",
            "입찰방식",
            "낙찰자 결정",
            "협상에 의한 계약",
        ],
    },
    FieldSpec {
        key: "deadline",
        label: "제출마감",
        terms: &["제출마감", "마감일", "접수마감", "입찰마감", "제안서 제출"],
    },
];

#[derive(Debug, Clone)]
struct FieldCandidate {
    spec: FieldSpec,
    raw_value: String,
    normalized_value: String,
    snippet: CandidateSnippet,
    confidence: f64,
}

fn best_field_candidates(bundle: &CandidateBundle) -> Vec<FieldCandidate> {
    FIELD_SPECS
        .iter()
        .filter_map(|spec| {
            bundle
                .snippets
                .iter()
                .filter_map(|snippet| field_candidate(*spec, snippet))
                .max_by(|left, right| {
                    left.confidence
                        .partial_cmp(&right.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| right.snippet.page_number.cmp(&left.snippet.page_number))
                        .then_with(|| right.snippet.block_index.cmp(&left.snippet.block_index))
                })
        })
        .collect()
}

fn field_candidate(spec: FieldSpec, snippet: &CandidateSnippet) -> Option<FieldCandidate> {
    let raw_value = extract_labeled_value(&snippet.quote, spec.terms)?;
    let normalized_value = normalize_field_value(spec.key, &raw_value);
    if normalized_value.is_empty() {
        return None;
    }

    let mut confidence = snippet.score + 0.10;
    if spec.key == "budget"
        && has_digit_and_any(&normalized_value, &["원", "천원", "백만원", "억원"])
    {
        confidence += 0.10;
    }
    if matches!(spec.key, "period" | "deadline")
        && has_digit_and_any(&normalized_value, &["년", "월", "일", "개월", "착수"])
    {
        confidence += 0.10;
    }

    Some(FieldCandidate {
        spec,
        raw_value,
        normalized_value,
        snippet: snippet.clone(),
        confidence: confidence.min(0.95),
    })
}

fn extract_labeled_value(text: &str, terms: &[&str]) -> Option<String> {
    let normalized = normalize_space(text);
    for term in terms {
        if let Some(index) = normalized.find(term) {
            let after_label = normalized[index + term.len()..]
                .trim_start_matches([' ', ':', '：', '-', '|'])
                .trim();
            if !after_label.is_empty() {
                return Some(after_label.to_string());
            }
        }
    }
    None
}

fn normalize_field_value(field_key: &str, raw_value: &str) -> String {
    let value = normalize_space(raw_value)
        .trim_matches([':', '：', '-', '|', ' '])
        .to_string();
    if field_key != "budget" {
        return value;
    }

    let mut normalized = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() || matches!(ch, '원' | '천' | '백' | '만' | '억') {
            normalized.push(ch);
        } else if ch == ',' || ch.is_whitespace() {
            continue;
        } else if !normalized.is_empty() {
            normalized.push(ch);
        }
    }
    if normalized.is_empty() {
        value
    } else {
        normalized
    }
}

fn has_digit_and_any(text: &str, terms: &[&str]) -> bool {
    text.chars().any(|ch| ch.is_ascii_digit()) && contains_any(text, terms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_project_info_and_context_deterministically() {
        let blocks = vec![
            SourceBlock {
                id: "heading-1".to_string(),
                page_number: 1,
                block_index: 0,
                kind: "heading".to_string(),
                heading_level: Some(1),
                text: "사업 개요".to_string(),
            },
            SourceBlock {
                id: "project-1".to_string(),
                page_number: 1,
                block_index: 1,
                kind: "paragraph".to_string(),
                heading_level: None,
                text: "사업명: 서울시 통합 유지관리 사업".to_string(),
            },
            SourceBlock {
                id: "risk-1".to_string(),
                page_number: 8,
                block_index: 9,
                kind: "paragraph".to_string(),
                heading_level: None,
                text: "필요 시 추가 산출물을 무상으로 제출한다.".to_string(),
            },
        ];

        let bundles = build_candidate_bundles("doc-1", "project-1", &blocks);
        let project_info = bundles
            .iter()
            .find(|bundle| bundle.bundle_key == "project_info_candidates")
            .expect("project info bundle");
        let risk = bundles
            .iter()
            .find(|bundle| bundle.bundle_key == "risk_candidates")
            .expect("risk bundle");

        assert_eq!(project_info.snippets[0].document_block_id, "heading-1");
        assert_eq!(project_info.snippets[1].document_block_id, "project-1");
        assert!(project_info.snippets[1].score >= 0.55);
        assert!(project_info.snippets[1]
            .reasons
            .contains(&"label:business_name".to_string()));
        assert_eq!(risk.snippets[0].document_block_id, "risk-1");
        assert!(risk.snippets[0].quote.contains("무상"));
    }

    #[test]
    fn stores_candidate_bundles_for_all_bundle_keys() {
        let temp = tempfile::tempdir().expect("temp dir");
        let conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_run_project_and_blocks(&conn);

        let result = extract_and_store_candidates(&conn, "project-1").expect("extract candidates");

        assert_eq!(result.bundle_count, 7);

        let bundle_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM candidate_bundles WHERE rfp_project_id = 'project-1'",
                [],
                |row| row.get(0),
            )
            .expect("bundle count");
        assert_eq!(bundle_count, 7);

        let project_info_json: String = conn
            .query_row(
                "SELECT bundle_json FROM candidate_bundles
                 WHERE rfp_project_id = 'project-1' AND bundle_key = 'project_info_candidates'",
                [],
                |row| row.get(0),
            )
            .expect("project info json");
        assert!(project_info_json.contains("사업명"));
        assert!(project_info_json.contains("documentBlockId"));
    }

    #[test]
    fn extracts_project_info_fields_with_evidence_links() {
        let temp = tempfile::tempdir().expect("temp dir");
        let conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        seed_document_run_project_and_blocks(&conn);
        conn.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (
                'block-5', 'run-1', 'doc-1', 'block-5', 1, 4, 'paragraph', NULL,
                '사업예산: 1,200,000,000원', NULL, '{}'
             )",
            [],
        )
        .expect("insert budget block");

        let result = extract_and_store_candidates(&conn, "project-1").expect("extract candidates");

        assert_eq!(result.field_count, 3);

        let fields = load_field_values_for_test(&conn);
        assert_eq!(
            fields.get("business_name").map(String::as_str),
            Some("서울시 통합 유지관리 사업")
        );
        assert_eq!(fields.get("client").map(String::as_str), Some("서울특별시"));
        assert_eq!(
            fields.get("budget").map(String::as_str),
            Some("1200000000원")
        );

        let evidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM evidence_links e
                 JOIN rfp_fields f ON f.id = e.target_id
                 WHERE f.rfp_project_id = 'project-1' AND e.target_table = 'rfp_fields'",
                [],
                |row| row.get(0),
            )
            .expect("evidence count");
        assert_eq!(evidence_count, 3);
    }

    fn seed_document_run_project_and_blocks(conn: &rusqlite::Connection) {
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
        conn.execute(
            "INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
             VALUES ('project-1', 'doc-1', 'test', 'draft', '', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z')",
            [],
        )
        .expect("insert project");

        for (id, page, index, kind, text) in [
            (
                "block-1",
                1,
                0,
                "paragraph",
                "사업명: 서울시 통합 유지관리 사업",
            ),
            ("block-2", 1, 1, "paragraph", "발주기관: 서울특별시"),
            (
                "block-3",
                3,
                2,
                "table",
                "요구사항 고유번호 SFR-001 통합 로그인 기능",
            ),
            (
                "block-4",
                5,
                3,
                "paragraph",
                "필요 시 추가 산출물을 무상으로 제출한다.",
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
    }

    fn load_field_values_for_test(
        conn: &rusqlite::Connection,
    ) -> std::collections::HashMap<String, String> {
        let mut statement = conn
            .prepare("SELECT field_key, normalized_value FROM rfp_fields ORDER BY field_key")
            .expect("prepare field query");
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("query fields")
            .collect::<Result<std::collections::HashMap<_, _>, _>>()
            .expect("collect fields")
    }
}

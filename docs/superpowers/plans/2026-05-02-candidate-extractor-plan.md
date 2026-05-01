# Candidate Extractor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Priority 2 rule-based candidate extractor that turns `document_blocks` into deterministic candidate bundles, stores first-pass `rfp_fields`, links each field to source evidence, and feeds the quality gate with real field coverage.

**Architecture:** Rust owns candidate extraction, persistence, validation, and Tauri DTOs; React only calls commands and renders summaries. The extractor is deterministic and local-first: it reads `document_blocks`, scores Korean RFP keywords and nearby context, writes candidate bundle JSON for later LLM use, writes only high-confidence `rfp_fields`, and never marks requirement extraction complete because requirement/domain writers are outside this slice.

**Tech Stack:** Tauri v2, React, TypeScript, Rust, SQLite via `rusqlite`, `serde_json`, Vitest, Rust unit tests, existing `scripts/verify.sh`. No new production dependency is required for this slice.

---

## Scope

Included:

- Add SQLite tables for `rfp_fields`, `evidence_links`, and persisted `candidate_bundles`.
- Add a Rust `candidate_extractor` module that builds these bundle keys from `document_blocks`:
  - `project_info_candidates`
  - `requirement_candidates`
  - `procurement_candidates`
  - `staffing_candidates`
  - `deliverable_candidates`
  - `acceptance_candidates`
  - `risk_candidates`
- Extract rule-based `rfp_fields` for:
  - `business_name`
  - `client`
  - `budget`
  - `period`
  - `contract_method`
  - `deadline`
- Link every inserted `rfp_fields` row to at least one `document_blocks` row through `evidence_links`.
- Update validation so missing business/client/budget/period blockers are based on stored `rfp_fields`, not unconditional baseline assumptions.
- Add Tauri API and UI touchpoints for project info fields and candidate bundle counts.
- Update smoke output to report candidate bundle and field counts.

Out of scope:

- LLM provider settings, API key storage, OpenAI/Gemini calls, and `llm_runs`.
- Durable requirement/item/staffing/deliverable/acceptance/risk domain tables.
- Correction dialog and export generation.
- Replacing OpenDataLoader or introducing v1 PySide6 extraction logic.

## Current Code Facts

- `apps/rfp-desktop/src-tauri/migrations/0001_core.sql` currently creates only `documents`, `source_files`, `extraction_runs`, `document_blocks`, `rfp_projects`, `validation_findings`, and `audit_events`.
- `apps/rfp-desktop/src-tauri/src/db/mod.rs` currently executes one embedded migration string.
- `apps/rfp-desktop/src-tauri/src/analysis/mod.rs` creates a baseline project and immediately calls `validation::evaluate_baseline_project`.
- `apps/rfp-desktop/src-tauri/src/validation/mod.rs` currently inserts missing field blockers unconditionally.
- `apps/rfp-desktop/src-tauri/src/block_normalizer/mod.rs` already preserves `page_number`, `block_index`, `kind`, `heading_level`, `text`, `bbox_json`, and `raw_json`.
- `apps/rfp-desktop/src/App.tsx` currently calls `runFastExtraction` and then `analyzeDocumentBaseline`; the backend candidate command must normalize the latest successful extraction if blocks are not yet present.

## File Structure

Create:

```text
apps/rfp-desktop/src-tauri/migrations/0002_candidate_extractor.sql
apps/rfp-desktop/src-tauri/src/candidate_extractor/mod.rs
apps/rfp-desktop/src/components/CandidateBundlePanel.tsx
apps/rfp-desktop/src/components/ProjectInfoPanel.tsx
```

Modify:

```text
apps/rfp-desktop/src-tauri/src/db/mod.rs
apps/rfp-desktop/src-tauri/src/domain.rs
apps/rfp-desktop/src-tauri/src/analysis/mod.rs
apps/rfp-desktop/src-tauri/src/validation/mod.rs
apps/rfp-desktop/src-tauri/src/commands/pipeline.rs
apps/rfp-desktop/src-tauri/src/commands/mod.rs
apps/rfp-desktop/src-tauri/src/lib.rs
apps/rfp-desktop/src-tauri/src/bin/smoke_first_pdf.rs
apps/rfp-desktop/src/lib/types.ts
apps/rfp-desktop/src/lib/api.ts
apps/rfp-desktop/src/App.tsx
apps/rfp-desktop/src/App.test.tsx
apps/rfp-desktop/src/App.css
```

## Data Contract

### `candidate_bundles.bundle_json`

Persist a compact, deterministic JSON shape:

```json
{
  "bundleKey": "project_info_candidates",
  "documentId": "doc-1",
  "rfpProjectId": "project-1",
  "snippets": [
    {
      "documentBlockId": "block-1",
      "pageNumber": 1,
      "blockIndex": 0,
      "kind": "paragraph",
      "quote": "사업명: 통합 유지관리 사업",
      "score": 0.9,
      "reasons": ["label:business_name", "same_block_value"]
    }
  ]
}
```

Rules:

- `quote` is collapsed whitespace and capped at 600 Unicode scalar values.
- `snippets` are de-duplicated by `document_block_id`.
- Candidate selection is deterministic: sort selected snippets by page, then block index, then id.
- Store at most 80 snippets per bundle.
- Store empty bundles too with `candidate_count = 0`; this keeps UI and smoke output reproducible.

### `rfp_fields`

Rules:

- One row per `(rfp_project_id, field_key)`.
- `source = 'rule'` for this slice.
- `raw_value` is the selected source text fragment.
- `normalized_value` is deterministic and conservative. For `budget`, remove spaces and commas inside numeric amounts when possible, but keep the Korean unit text. For other fields, use collapsed `raw_value`.
- Insert only if confidence is at least `0.55`.
- Cap confidence at `0.95`; rule extraction never reaches `1.0`.

### `evidence_links`

Rules:

- Every inserted `rfp_fields` row gets one `evidence_links` row.
- `target_table = 'rfp_fields'`.
- `quote` is the candidate quote from the winning source block.
- `confidence` equals the stored field confidence.

## Scoring And Evidence Rules

Use simple Rust string scanning; do not add `regex` unless a later task proves the standard-library approach is too fragile.

### Bundle Keywords

| Bundle | Positive terms |
|---|---|
| `project_info_candidates` | `사업명`, `용역명`, `과업명`, `프로젝트명`, `발주기관`, `수요기관`, `주관기관`, `사업예산`, `예산`, `추정가격`, `사업기간`, `용역기간`, `계약기간`, `계약방법`, `입찰방식`, `제출마감`, `입찰마감` |
| `requirement_candidates` | `요구사항`, `요구 기능`, `기능 요구`, `요구사항 ID`, `고유번호`, `SFR-`, `REQ-`, `요구사항 총괄표` |
| `procurement_candidates` | `장비`, `서버`, `스토리지`, `소프트웨어`, `SW`, `라이선스`, `license`, `클라우드`, `DB`, `데이터베이스`, `네트워크`, `보안솔루션` |
| `staffing_candidates` | `투입인력`, `인력`, `PM`, `PL`, `개발자`, `상주`, `MM`, `M/M`, `수행조직`, `등급` |
| `deliverable_candidates` | `산출물`, `납품물`, `보고서`, `설계서`, `매뉴얼`, `교육자료`, `완료보고`, `제출물` |
| `acceptance_candidates` | `검수`, `인수`, `시험`, `성능`, `보안점검`, `하자보수`, `SLA`, `검사`, `승인` |
| `risk_candidates` | `무상`, `추가 요청`, `협의`, `필요 시`, `지체상금`, `책임`, `비용 부담`, `손해배상`, `위약`, `특정 업체` |

### Field Labels

| Field key | Label | Terms |
|---|---|---|
| `business_name` | `사업명` | `사업명`, `용역명`, `과업명`, `프로젝트명` |
| `client` | `발주기관` | `발주기관`, `수요기관`, `주관기관`, `발주처`, `기관명` |
| `budget` | `사업예산` | `사업예산`, `예산`, `추정가격`, `기초금액`, `사업비` |
| `period` | `사업기간` | `사업기간`, `용역기간`, `과업기간`, `계약기간`, `수행기간` |
| `contract_method` | `계약방식` | `계약방법`, `계약방식`, `입찰방식`, `낙찰자 결정`, `협상에 의한 계약` |
| `deadline` | `제출마감` | `제출마감`, `마감일`, `접수마감`, `입찰마감`, `제안서 제출` |

### Scoring

For each block and bundle:

- `+0.55` if the block contains a field label or bundle keyword.
- `+0.15` if the block is `table`, because RFP summary tables often carry project info.
- `+0.10` if the nearest heading within the previous three blocks contains a bundle keyword.
- `+0.10` if the block has a concise label/value form using `:`, `：`, `-`, or a table-like whitespace split.
- `+0.05` if the block is on pages 1 through 5 for `project_info_candidates`.
- Cap at `0.95`; include the block if score is at least `0.45`.

For `rfp_fields`:

- Start from the project-info bundle snippet score.
- Add `+0.10` when `extract_labeled_value` finds a non-empty value after the label.
- Add `+0.10` for `budget` when the value contains a digit and `원`, `천원`, `백만원`, or `억원`.
- Add `+0.10` for `period` or `deadline` when the value contains a digit and one of `년`, `월`, `일`, `개월`, or `착수`.
- Cap at `0.95`; store the best candidate for each field if confidence is at least `0.55`.

## Task 1: Add Candidate Extractor Schema

**Files:**

- Create: `apps/rfp-desktop/src-tauri/migrations/0002_candidate_extractor.sql`
- Modify: `apps/rfp-desktop/src-tauri/src/db/mod.rs`

- [ ] **Step 1: Write the failing migration test**

Add this test to `apps/rfp-desktop/src-tauri/src/db/mod.rs`:

```rust
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
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_candidate_extractor_tables
```

Expected: FAIL because the three tables do not exist.

- [ ] **Step 3: Create the `0002` migration**

Create `apps/rfp-desktop/src-tauri/migrations/0002_candidate_extractor.sql`:

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS rfp_fields (
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_rfp_fields_project_key
  ON rfp_fields(rfp_project_id, field_key);

CREATE TABLE IF NOT EXISTS evidence_links (
  id TEXT PRIMARY KEY,
  document_block_id TEXT NOT NULL REFERENCES document_blocks(id) ON DELETE CASCADE,
  target_table TEXT NOT NULL,
  target_id TEXT NOT NULL,
  quote TEXT NOT NULL,
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0)
);

CREATE INDEX IF NOT EXISTS idx_evidence_links_target
  ON evidence_links(target_table, target_id);

CREATE TABLE IF NOT EXISTS candidate_bundles (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  bundle_key TEXT NOT NULL CHECK (
    bundle_key IN (
      'project_info_candidates',
      'requirement_candidates',
      'procurement_candidates',
      'staffing_candidates',
      'deliverable_candidates',
      'acceptance_candidates',
      'risk_candidates'
    )
  ),
  bundle_json TEXT NOT NULL,
  candidate_count INTEGER NOT NULL CHECK (candidate_count >= 0),
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_candidate_bundles_project_key
  ON candidate_bundles(rfp_project_id, bundle_key);

CREATE INDEX IF NOT EXISTS idx_candidate_bundles_document_id
  ON candidate_bundles(document_id);
```

- [ ] **Step 4: Execute both migrations from `db::migrate`**

Replace the migration constants and `migrate` function in `apps/rfp-desktop/src-tauri/src/db/mod.rs` with:

```rust
const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_core.sql"),
    include_str!("../../migrations/0002_candidate_extractor.sql"),
];

pub fn migrate(conn: &Connection) -> AppResult<()> {
    for migration in MIGRATIONS {
        conn.execute_batch(migration)?;
    }
    Ok(())
}
```

Keep `open_database` unchanged.

- [ ] **Step 5: Run the focused migration tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests
```

Expected: PASS.

## Task 2: Add Candidate Extractor Types And Pure Scoring

**Files:**

- Create: `apps/rfp-desktop/src-tauri/src/candidate_extractor/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Register the module**

Add this public module to `apps/rfp-desktop/src-tauri/src/lib.rs`:

```rust
pub mod candidate_extractor;
```

- [ ] **Step 2: Create extractor types and a failing pure test**

Create `apps/rfp-desktop/src-tauri/src/candidate_extractor/mod.rs` with the public DTOs, constants, and this test first:

```rust
use serde::{Deserialize, Serialize};

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
        assert!(project_info.snippets[1].reasons.contains(&"label:business_name".to_string()));
        assert_eq!(risk.snippets[0].document_block_id, "risk-1");
        assert!(risk.snippets[0].quote.contains("무상"));
    }
}
```

- [ ] **Step 3: Run the pure test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml candidate_extractor::tests::scores_project_info_and_context_deterministically
```

Expected: FAIL because `build_candidate_bundles` is not implemented.

- [ ] **Step 4: Implement scoring helpers**

Add these functions to the same file:

```rust
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

    let mut score = 0.0;
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
            ("label:business_name", &["사업명", "용역명", "과업명", "프로젝트명"]),
            ("label:client", &["발주기관", "수요기관", "주관기관", "발주처", "기관명"]),
            ("label:budget", &["사업예산", "예산", "추정가격", "기초금액", "사업비"]),
            ("label:period", &["사업기간", "용역기간", "과업기간", "계약기간", "수행기간"]),
            ("label:contract_method", &["계약방법", "계약방식", "입찰방식", "낙찰자 결정", "협상에 의한 계약"]),
            ("label:deadline", &["제출마감", "마감일", "접수마감", "입찰마감", "제안서 제출"]),
        ],
        CandidateBundleKey::RequirementCandidates => &[(
            "keyword:requirements",
            &["요구사항", "요구 기능", "기능 요구", "요구사항 ID", "고유번호", "SFR-", "REQ-", "요구사항 총괄표"],
        )],
        CandidateBundleKey::ProcurementCandidates => &[(
            "keyword:procurement",
            &["장비", "서버", "스토리지", "소프트웨어", "SW", "라이선스", "license", "클라우드", "DB", "데이터베이스", "네트워크", "보안솔루션"],
        )],
        CandidateBundleKey::StaffingCandidates => &[(
            "keyword:staffing",
            &["투입인력", "인력", "PM", "PL", "개발자", "상주", "MM", "M/M", "수행조직", "등급"],
        )],
        CandidateBundleKey::DeliverableCandidates => &[(
            "keyword:deliverable",
            &["산출물", "납품물", "보고서", "설계서", "매뉴얼", "교육자료", "완료보고", "제출물"],
        )],
        CandidateBundleKey::AcceptanceCandidates => &[(
            "keyword:acceptance",
            &["검수", "인수", "시험", "성능", "보안점검", "하자보수", "SLA", "검사", "승인"],
        )],
        CandidateBundleKey::RiskCandidates => &[(
            "keyword:risk",
            &["무상", "추가 요청", "협의", "필요 시", "지체상금", "책임", "비용 부담", "손해배상", "위약", "특정 업체"],
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
    terms.iter().any(|term| lower.contains(&term.to_lowercase()))
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
    normalize_space(text).chars().take(MAX_QUOTE_CHARS).collect()
}

fn normalize_space(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

- [ ] **Step 5: Run the pure scoring test**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml candidate_extractor::tests::scores_project_info_and_context_deterministically
```

Expected: PASS.

## Task 3: Persist Candidate Bundles From `document_blocks`

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/candidate_extractor/mod.rs`

- [ ] **Step 1: Add the failing persistence test**

Add this test:

```rust
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
```

Add this test helper inside the test module:

```rust
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
        ("block-1", 1, 0, "paragraph", "사업명: 서울시 통합 유지관리 사업"),
        ("block-2", 1, 1, "paragraph", "발주기관: 서울특별시"),
        ("block-3", 3, 2, "table", "요구사항 고유번호 SFR-001 통합 로그인 기능"),
        ("block-4", 5, 3, "paragraph", "필요 시 추가 산출물을 무상으로 제출한다."),
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
```

- [ ] **Step 2: Run the persistence test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml candidate_extractor::tests::stores_candidate_bundles_for_all_bundle_keys
```

Expected: FAIL because `extract_and_store_candidates` is not implemented.

- [ ] **Step 3: Add persistence structs and functions**

Add these public structs and functions:

```rust
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateExtractionResult {
    pub rfp_project_id: String,
    pub document_id: String,
    pub bundle_count: usize,
    pub field_count: usize,
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
    conn.execute("DELETE FROM rfp_fields WHERE rfp_project_id = ?", [rfp_project_id])?;
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
                bundle.rfp_project_id,
                bundle.document_id,
                bundle.bundle_key,
                serde_json::to_string(bundle)?,
                bundle.snippets.len() as i64,
                now,
            ],
        )?;
    }
    Ok(())
}
```

Temporarily implement `store_project_info_fields` as:

```rust
fn store_project_info_fields(
    _conn: &Connection,
    _rfp_project_id: &str,
    _bundles: &[CandidateBundle],
) -> AppResult<usize> {
    Ok(0)
}
```

- [ ] **Step 4: Run the persistence test**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml candidate_extractor::tests::stores_candidate_bundles_for_all_bundle_keys
```

Expected: PASS.

## Task 4: Extract `rfp_fields` And Evidence Links

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/candidate_extractor/mod.rs`

- [ ] **Step 1: Add the failing field extraction test**

Add this test:

```rust
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
    assert_eq!(fields.get("business_name").map(String::as_str), Some("서울시 통합 유지관리 사업"));
    assert_eq!(fields.get("client").map(String::as_str), Some("서울특별시"));
    assert_eq!(fields.get("budget").map(String::as_str), Some("1200000000원"));

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
```

Add this test helper:

```rust
fn load_field_values_for_test(
    conn: &rusqlite::Connection,
) -> std::collections::HashMap<String, String> {
    let mut statement = conn
        .prepare("SELECT field_key, normalized_value FROM rfp_fields ORDER BY field_key")
        .expect("prepare field query");
    statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .expect("query fields")
        .collect::<Result<std::collections::HashMap<_, _>, _>>()
        .expect("collect fields")
}
```

- [ ] **Step 2: Run the field test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml candidate_extractor::tests::extracts_project_info_fields_with_evidence_links
```

Expected: FAIL because `store_project_info_fields` still returns zero.

- [ ] **Step 3: Add field extraction helpers**

Add:

```rust
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
        terms: &["계약방법", "계약방식", "입찰방식", "낙찰자 결정", "협상에 의한 계약"],
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
```

Replace `store_project_info_fields` with:

```rust
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
    if spec.key == "budget" && has_digit_and_any(&normalized_value, &["원", "천원", "백만원", "억원"]) {
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
```

- [ ] **Step 4: Run candidate extractor tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml candidate_extractor::tests
```

Expected: PASS.

## Task 5: Integrate Candidate Extraction Into Analysis And Validation

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/validation/mod.rs`

- [ ] **Step 1: Add a failing analysis test**

In `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`, add:

```rust
#[test]
fn candidate_analysis_removes_found_project_info_blockers() {
    let temp = tempdir().expect("temp dir");
    let conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
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
    for (id, index, text) in [
        ("block-1", 0, "사업명: 서울시 통합 유지관리 사업"),
        ("block-2", 1, "발주기관: 서울특별시"),
        ("block-3", 2, "사업예산: 1,200,000,000원"),
        ("block-4", 3, "사업기간: 계약일로부터 12개월"),
    ] {
        conn.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (?, 'run-1', 'doc-1', ?, 1, ?, 'paragraph', NULL, ?, NULL, '{}')",
            rusqlite::params![id, id, index, text],
        )
        .expect("insert block");
    }

    let project_id = create_or_update_candidate_project(&conn, "doc-1").expect("analyze");

    let missing_project_info_blockers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM validation_findings
             WHERE rfp_project_id = ?
               AND finding_type IN (
                 'missing_business_name',
                 'missing_client',
                 'missing_budget',
                 'missing_period'
               )",
            [&project_id],
            |row| row.get(0),
        )
        .expect("missing blockers");
    assert_eq!(missing_project_info_blockers, 0);

    let zero_requirements: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM validation_findings
             WHERE rfp_project_id = ? AND finding_type = 'zero_requirements'",
            [&project_id],
            |row| row.get(0),
        )
        .expect("zero requirements blocker");
    assert_eq!(zero_requirements, 1);
}
```

- [ ] **Step 2: Run the analysis test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests::candidate_analysis_removes_found_project_info_blockers
```

Expected: FAIL because `create_or_update_candidate_project` is not implemented and validation is still baseline-only.

- [ ] **Step 3: Add candidate project analysis**

In `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`, import the extractor:

```rust
use crate::candidate_extractor;
```

Add constants:

```rust
const CANDIDATE_ANALYSIS_VERSION: &str = "rfp-v2-candidates-2026-05-02";
const CANDIDATE_SUMMARY: &str = "규칙 기반 후보 추출로 생성된 분석 초안입니다.";
```

Add:

```rust
pub fn create_or_update_candidate_project(
    conn: &Connection,
    document_id: &str,
) -> AppResult<String> {
    let project_id = create_or_update_project_row(
        conn,
        document_id,
        CANDIDATE_ANALYSIS_VERSION,
        CANDIDATE_SUMMARY,
    )?;
    candidate_extractor::extract_and_store_candidates(conn, &project_id)?;
    validation::evaluate_candidate_project(conn, &project_id)?;
    Ok(project_id)
}

fn create_or_update_project_row(
    conn: &Connection,
    document_id: &str,
    analysis_version: &str,
    summary: &str,
) -> AppResult<String> {
    let now = Utc::now().to_rfc3339();
    let existing_project_id: Option<String> = conn
        .query_row(
            "SELECT id FROM rfp_projects WHERE document_id = ?",
            [document_id],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(project_id) = existing_project_id {
        conn.execute(
            "UPDATE rfp_projects
             SET analysis_version = ?, status = 'draft', summary = ?, updated_at = ?
             WHERE id = ?",
            params![analysis_version, summary, now, project_id],
        )?;
        Ok(project_id)
    } else {
        let project_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO rfp_projects (
                id, document_id, analysis_version, status, summary, created_at, updated_at
             ) VALUES (?, ?, ?, 'draft', ?, ?, ?)",
            params![project_id, document_id, analysis_version, summary, now, now],
        )?;
        Ok(project_id)
    }
}
```

Refactor the existing `create_or_update_baseline_project` to call `create_or_update_project_row` and keep existing behavior for compatibility:

```rust
pub fn create_or_update_baseline_project(
    conn: &Connection,
    document_id: &str,
) -> AppResult<String> {
    let project_id = create_or_update_project_row(
        conn,
        document_id,
        ANALYSIS_VERSION,
        BASELINE_SUMMARY,
    )?;
    validation::evaluate_baseline_project(conn, &project_id)?;
    Ok(project_id)
}
```

- [ ] **Step 4: Update validation to use stored fields**

In `apps/rfp-desktop/src-tauri/src/validation/mod.rs`, keep `evaluate_baseline_project` for compatibility and add:

```rust
pub fn evaluate_candidate_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM validation_findings WHERE rfp_project_id = ?",
        [rfp_project_id],
    )?;

    let document_id: String = conn.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let block_count = count_document_blocks(conn, &document_id)?;
    let project_target = Some(rfp_project_id.to_string());
    let mut findings = Vec::new();

    for (field_key, finding_type, message) in [
        ("business_name", "missing_business_name", "사업명이 추출되지 않았습니다."),
        ("client", "missing_client", "발주기관이 추출되지 않았습니다."),
        ("budget", "missing_budget", "사업예산이 추출되지 않았습니다."),
        ("period", "missing_period", "사업기간이 추출되지 않았습니다."),
    ] {
        if !has_field(conn, rfp_project_id, field_key)? {
            findings.push(FindingInput {
                severity: "blocker",
                finding_type,
                message,
                target_table: Some("rfp_projects"),
                target_id: project_target.clone(),
            });
        }
    }

    findings.push(FindingInput {
        severity: "blocker",
        finding_type: "zero_requirements",
        message: "요구사항이 0건입니다.",
        target_table: Some("rfp_projects"),
        target_id: project_target.clone(),
    });

    if block_count == 0 || has_field_without_evidence(conn, rfp_project_id)? {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "missing_evidence",
            message: "원문 근거가 없는 항목이 있습니다.",
            target_table: Some("rfp_fields"),
            target_id: None,
        });
    }

    let low_confidence_field_ids = low_confidence_fields(conn, rfp_project_id)?;
    for field_id in low_confidence_field_ids {
        findings.push(FindingInput {
            severity: "warning",
            finding_type: "low_confidence",
            message: "신뢰도가 낮은 추출값이 있습니다.",
            target_table: Some("rfp_fields"),
            target_id: Some(field_id),
        });
    }

    findings.push(FindingInput {
        severity: "warning",
        finding_type: "llm_not_used",
        message: "LLM opt-in이 꺼져 구조화가 제한됩니다.",
        target_table: Some("rfp_projects"),
        target_id: project_target,
    });

    for finding in findings {
        insert_finding(conn, rfp_project_id, finding)?;
    }

    update_status_from_findings(conn, rfp_project_id, &document_id)
}
```

Add helpers:

```rust
fn count_document_blocks(conn: &Connection, document_id: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM document_blocks WHERE document_id = ?",
        [document_id],
        |row| row.get(0),
    )?)
}

fn has_field(conn: &Connection, rfp_project_id: &str, field_key: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM rfp_fields
         WHERE rfp_project_id = ? AND field_key = ? AND TRIM(normalized_value) <> ''",
        [rfp_project_id, field_key],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn has_field_without_evidence(conn: &Connection, rfp_project_id: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM rfp_fields f
         LEFT JOIN evidence_links e ON e.target_table = 'rfp_fields' AND e.target_id = f.id
         WHERE f.rfp_project_id = ? AND e.id IS NULL",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn low_confidence_fields(conn: &Connection, rfp_project_id: &str) -> AppResult<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT id FROM rfp_fields WHERE rfp_project_id = ? AND confidence < 0.6",
    )?;
    let ids = statement
        .query_map([rfp_project_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

fn update_status_from_findings(
    conn: &Connection,
    rfp_project_id: &str,
    document_id: &str,
) -> AppResult<()> {
    let blocker_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings
         WHERE rfp_project_id = ? AND severity = 'blocker'",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let status = if blocker_count > 0 { "review_needed" } else { "ready" };
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE rfp_projects SET status = ?, updated_at = ? WHERE id = ?",
        params![status, now, rfp_project_id],
    )?;
    conn.execute(
        "UPDATE documents SET status = ?, updated_at = ? WHERE id = ?",
        params![status, now, document_id],
    )?;
    Ok(())
}
```

The existing `evaluate_baseline_project` can keep its current unconditional findings; do not remove it until frontend and smoke are migrated.

- [ ] **Step 5: Run analysis and validation tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml validation
```

Expected: PASS, including the previous baseline test.

## Task 6: Add Tauri Commands And DTOs

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/domain.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add DTOs**

Add to `apps/rfp-desktop/src-tauri/src/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLinkDto {
    pub document_block_id: String,
    pub quote: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RfpFieldDto {
    pub id: String,
    pub field_key: String,
    pub label: String,
    pub raw_value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub source: String,
    pub evidence: Vec<EvidenceLinkDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBundleSummaryDto {
    pub bundle_key: String,
    pub candidate_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateExtractionSummary {
    pub document: DocumentSummary,
    pub project_id: String,
    pub fields: Vec<RfpFieldDto>,
    pub bundles: Vec<CandidateBundleSummaryDto>,
    pub ready_count: i64,
    pub review_needed_count: i64,
    pub failed_count: i64,
}
```

- [ ] **Step 2: Add a failing command-level test**

In `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`, add:

```rust
#[test]
fn candidate_pipeline_normalizes_blocks_and_returns_fields() {
    let temp = tempdir().expect("temp dir");
    let db_path = temp.path().join("test.sqlite3");
    let json_path = temp.path().join("sample-output.json");
    fs::write(
        &json_path,
        r#"[
          {"id":"b1","type":"paragraph","page_number":1,"text":"사업명: 서울시 통합 유지관리 사업"},
          {"id":"b2","type":"paragraph","page_number":1,"text":"발주기관: 서울특별시"},
          {"id":"b3","type":"paragraph","page_number":1,"text":"사업예산: 1,200,000,000원"},
          {"id":"b4","type":"paragraph","page_number":1,"text":"사업기간: 계약일로부터 12개월"}
        ]"#,
    )
    .expect("write json");
    let conn = db::open_database(&db_path).expect("open db");
    conn.execute(
        "INSERT INTO documents (id, title, created_at, updated_at, status)
         VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'created')",
        [],
    )
    .expect("insert doc");
    conn.execute(
        "INSERT INTO extraction_runs (
            id, document_id, provider, mode, status, json_path, started_at, finished_at
         ) VALUES (
            'run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', ?, '2026-05-02T00:00:00Z', '2026-05-02T00:00:01Z'
         )",
        [json_path.to_string_lossy().to_string()],
    )
    .expect("insert run");

    let summary = run_candidate_analysis_for_document(&conn, "doc-1").expect("candidate analysis");

    assert_eq!(summary.fields.len(), 4);
    assert_eq!(summary.bundles.len(), 7);
    assert_eq!(summary.document.status, "review_needed");
    assert_eq!(summary.review_needed_count, 1);
    assert!(summary.fields.iter().any(|field| field.field_key == "business_name"));
}
```

- [ ] **Step 3: Run the command test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::pipeline::tests::candidate_pipeline_normalizes_blocks_and_returns_fields
```

Expected: FAIL because `run_candidate_analysis_for_document` is not implemented.

- [ ] **Step 4: Implement command helpers**

In `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`, import:

```rust
use std::path::PathBuf;

use crate::block_normalizer;
use crate::domain::{
    CandidateBundleSummaryDto, CandidateExtractionSummary, EvidenceLinkDto, RfpFieldDto,
};
```

Add the Tauri command:

```rust
#[tauri::command]
pub fn analyze_document_candidates(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<CandidateExtractionSummary> {
    let conn = state.connect()?;
    run_candidate_analysis_for_document(&conn, &document_id)
}
```

Add:

```rust
pub fn run_candidate_analysis_for_document(
    conn: &Connection,
    document_id: &str,
) -> AppResult<CandidateExtractionSummary> {
    normalize_latest_successful_extraction_if_needed(conn, document_id)?;
    let project_id = analysis::create_or_update_candidate_project(conn, document_id)?;
    load_candidate_extraction_summary(conn, document_id, &project_id)
}

fn normalize_latest_successful_extraction_if_needed(
    conn: &Connection,
    document_id: &str,
) -> AppResult<()> {
    let run: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT id, json_path FROM extraction_runs
             WHERE document_id = ? AND status = 'succeeded'
             ORDER BY finished_at DESC, started_at DESC
             LIMIT 1",
            [document_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let Some((run_id, Some(json_path))) = run else {
        return Ok(());
    };

    let existing_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM document_blocks WHERE extraction_run_id = ?",
        [&run_id],
        |row| row.get(0),
    )?;
    if existing_count == 0 {
        block_normalizer::normalize_extraction_json(
            conn,
            document_id,
            &run_id,
            &PathBuf::from(json_path),
        )?;
    }

    Ok(())
}
```

Add summary loaders:

```rust
pub fn load_candidate_extraction_summary(
    conn: &Connection,
    document_id: &str,
    project_id: &str,
) -> AppResult<CandidateExtractionSummary> {
    Ok(CandidateExtractionSummary {
        document: document_ingestion::load_document_summary(conn, document_id)?,
        project_id: project_id.to_string(),
        fields: load_project_fields(conn, project_id)?,
        bundles: load_candidate_bundle_summaries(conn, project_id)?,
        ready_count: count_documents_by_status(conn, "ready")?,
        review_needed_count: count_documents_by_status(conn, "review_needed")?,
        failed_count: count_documents_by_status(conn, "failed")?,
    })
}

fn load_project_fields(conn: &Connection, project_id: &str) -> AppResult<Vec<RfpFieldDto>> {
    let mut statement = conn.prepare(
        "SELECT id, field_key, label, raw_value, normalized_value, confidence, source
         FROM rfp_fields
         WHERE rfp_project_id = ?
         ORDER BY CASE field_key
           WHEN 'business_name' THEN 1
           WHEN 'client' THEN 2
           WHEN 'budget' THEN 3
           WHEN 'period' THEN 4
           WHEN 'contract_method' THEN 5
           WHEN 'deadline' THEN 6
           ELSE 99
         END",
    )?;
    let fields = statement
        .query_map([project_id], |row| {
            Ok(RfpFieldDto {
                id: row.get(0)?,
                field_key: row.get(1)?,
                label: row.get(2)?,
                raw_value: row.get(3)?,
                normalized_value: row.get(4)?,
                confidence: row.get(5)?,
                source: row.get(6)?,
                evidence: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    fields
        .into_iter()
        .map(|mut field| {
            field.evidence = load_field_evidence(conn, &field.id)?;
            Ok(field)
        })
        .collect()
}

fn load_field_evidence(conn: &Connection, field_id: &str) -> AppResult<Vec<EvidenceLinkDto>> {
    let mut statement = conn.prepare(
        "SELECT document_block_id, quote, confidence
         FROM evidence_links
         WHERE target_table = 'rfp_fields' AND target_id = ?
         ORDER BY confidence DESC",
    )?;
    let evidence = statement
        .query_map([field_id], |row| {
            Ok(EvidenceLinkDto {
                document_block_id: row.get(0)?,
                quote: row.get(1)?,
                confidence: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(evidence)
}

fn load_candidate_bundle_summaries(
    conn: &Connection,
    project_id: &str,
) -> AppResult<Vec<CandidateBundleSummaryDto>> {
    let mut statement = conn.prepare(
        "SELECT bundle_key, candidate_count
         FROM candidate_bundles
         WHERE rfp_project_id = ?
         ORDER BY bundle_key",
    )?;
    let bundles = statement
        .query_map([project_id], |row| {
            Ok(CandidateBundleSummaryDto {
                bundle_key: row.get(0)?,
                candidate_count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(bundles)
}
```

Add this import because `.optional()` is used:

```rust
use rusqlite::OptionalExtension;
```

- [ ] **Step 5: Register the command**

In `apps/rfp-desktop/src-tauri/src/lib.rs`, add:

```rust
commands::pipeline::analyze_document_candidates
```

to the `tauri::generate_handler!` list.

- [ ] **Step 6: Run pipeline tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::pipeline::tests
```

Expected: PASS.

## Task 7: Add Frontend API And Review Touchpoints

**Files:**

- Modify: `apps/rfp-desktop/src/lib/types.ts`
- Modify: `apps/rfp-desktop/src/lib/api.ts`
- Create: `apps/rfp-desktop/src/components/ProjectInfoPanel.tsx`
- Create: `apps/rfp-desktop/src/components/CandidateBundlePanel.tsx`
- Modify: `apps/rfp-desktop/src/App.tsx`
- Modify: `apps/rfp-desktop/src/App.test.tsx`
- Modify: `apps/rfp-desktop/src/App.css`

- [ ] **Step 1: Add frontend types**

Add to `apps/rfp-desktop/src/lib/types.ts`:

```ts
export interface EvidenceLinkDto {
  documentBlockId: string;
  quote: string;
  confidence: number;
}

export interface RfpFieldDto {
  id: string;
  fieldKey: string;
  label: string;
  rawValue: string;
  normalizedValue: string;
  confidence: number;
  source: string;
  evidence: EvidenceLinkDto[];
}

export interface CandidateBundleSummaryDto {
  bundleKey: string;
  candidateCount: number;
}

export interface CandidateExtractionSummary {
  document: DocumentSummary;
  projectId: string;
  fields: RfpFieldDto[];
  bundles: CandidateBundleSummaryDto[];
  readyCount: number;
  reviewNeededCount: number;
  failedCount: number;
}
```

- [ ] **Step 2: Add API wrapper**

Add to `apps/rfp-desktop/src/lib/api.ts`:

```ts
import type { CandidateExtractionSummary } from "./types";

export function analyzeDocumentCandidates(
  documentId: string,
): Promise<CandidateExtractionSummary> {
  return invoke<CandidateExtractionSummary>("analyze_document_candidates", {
    documentId,
  });
}
```

Keep `analyzeDocumentBaseline` for compatibility until all UI and smoke paths use the candidate command.

- [ ] **Step 3: Add project info panel**

Create `apps/rfp-desktop/src/components/ProjectInfoPanel.tsx`:

```tsx
import type { RfpFieldDto } from "../lib/types";

interface ProjectInfoPanelProps {
  fields: RfpFieldDto[];
}

export function ProjectInfoPanel({ fields }: ProjectInfoPanelProps) {
  const byKey = new Map(fields.map((field) => [field.fieldKey, field]));
  const orderedKeys = [
    "business_name",
    "client",
    "budget",
    "period",
    "contract_method",
    "deadline",
  ];

  return (
    <section className="project-info" aria-label="사업 기본정보">
      <div>
        <span className="eyeline">사업 기본정보</span>
        <h3>추출 필드</h3>
      </div>
      <dl className="project-info-grid">
        {orderedKeys.map((key) => {
          const field = byKey.get(key);
          return (
            <div className="project-info-item" key={key}>
              <dt>{field?.label ?? fallbackLabel(key)}</dt>
              <dd>{field?.normalizedValue ?? "미추출"}</dd>
            </div>
          );
        })}
      </dl>
    </section>
  );
}

function fallbackLabel(key: string): string {
  const labels: Record<string, string> = {
    business_name: "사업명",
    client: "발주기관",
    budget: "사업예산",
    period: "사업기간",
    contract_method: "계약방식",
    deadline: "제출마감",
  };
  return labels[key] ?? key;
}
```

- [ ] **Step 4: Add bundle count panel**

Create `apps/rfp-desktop/src/components/CandidateBundlePanel.tsx`:

```tsx
import type { CandidateBundleSummaryDto } from "../lib/types";

interface CandidateBundlePanelProps {
  bundles: CandidateBundleSummaryDto[];
}

export function CandidateBundlePanel({ bundles }: CandidateBundlePanelProps) {
  if (bundles.length === 0) {
    return null;
  }

  return (
    <section className="candidate-bundles" aria-label="후보 묶음">
      {bundles.map((bundle) => (
        <div className="candidate-bundle" key={bundle.bundleKey}>
          <span>{bundleLabel(bundle.bundleKey)}</span>
          <strong>{bundle.candidateCount}</strong>
        </div>
      ))}
    </section>
  );
}

function bundleLabel(key: string): string {
  const labels: Record<string, string> = {
    project_info_candidates: "기본정보",
    requirement_candidates: "요구사항",
    procurement_candidates: "구매항목",
    staffing_candidates: "인력",
    deliverable_candidates: "산출물",
    acceptance_candidates: "검수",
    risk_candidates: "리스크",
  };
  return labels[key] ?? key;
}
```

- [ ] **Step 5: Wire candidate summary into `App.tsx`**

In `apps/rfp-desktop/src/App.tsx`:

- Import `analyzeDocumentCandidates`.
- Import `ProjectInfoPanel` and `CandidateBundlePanel`.
- Add state:

```ts
const [candidateSummary, setCandidateSummary] =
  useState<CandidateExtractionSummary | null>(null);
```

- In `handleAnalyze`, replace the baseline call with:

```ts
const summary = await analyzeDocumentCandidates(selectedDocument.id);
setCandidateSummary(summary);
await refreshDocuments();
```

- Clear candidate summary when the selected document changes:

```ts
useEffect(() => {
  setCandidateSummary(null);
}, [selectedDocument?.id]);
```

- Render below `QualityGate`:

```tsx
<ProjectInfoPanel fields={candidateSummary?.fields ?? []} />
<CandidateBundlePanel bundles={candidateSummary?.bundles ?? []} />
```

- [ ] **Step 6: Add UI test expectations**

In `apps/rfp-desktop/src/App.test.tsx`, update the mock so `analyze_document_candidates` returns:

```ts
import userEvent from "@testing-library/user-event";
```

```ts
if (command === "analyze_document_candidates") {
  return Promise.resolve({
    document: {
      id: "doc-1",
      title: "서울시 통합 유지관리 RFP",
      status: "review_needed",
      fileName: "seoul-rfp.pdf",
      blockerCount: 1,
      warningCount: 1,
      blockCount: 37,
    },
    projectId: "project-1",
    fields: [
      {
        id: "field-1",
        fieldKey: "business_name",
        label: "사업명",
        rawValue: "서울시 통합 유지관리 사업",
        normalizedValue: "서울시 통합 유지관리 사업",
        confidence: 0.9,
        source: "rule",
        evidence: [],
      },
    ],
    bundles: [
      { bundleKey: "project_info_candidates", candidateCount: 4 },
      { bundleKey: "risk_candidates", candidateCount: 1 },
    ],
    readyCount: 0,
    reviewNeededCount: 1,
    failedCount: 0,
  });
}
```

Add a test that clicks `추출/분석`:

```ts
it("runs candidate analysis and renders project info and bundle counts", async () => {
  render(<App />);

  const analyzeButton = await screen.findByRole("button", { name: /추출\/분석/ });
  await userEvent.click(analyzeButton);

  expect(invokeMock).toHaveBeenCalledWith("run_fast_extraction", {
    documentId: "doc-1",
    cliPath: null,
  });
  expect(invokeMock).toHaveBeenCalledWith("analyze_document_candidates", {
    documentId: "doc-1",
  });
  expect(await screen.findByText("서울시 통합 유지관리 사업")).toBeInTheDocument();
  expect(screen.getByText("기본정보")).toBeInTheDocument();
  expect(screen.getByText("리스크")).toBeInTheDocument();
});
```

Install `@testing-library/user-event` only if it is not already present:

```bash
npm install --prefix apps/rfp-desktop --save-dev @testing-library/user-event
```

Record the dev dependency addition in `IMPLEMENTATION_LOG.md` during implementation because repository rules require dependency reasons. The reason is: it is the testing-library standard user interaction helper for clicking the existing analyze button in Vitest.

- [ ] **Step 7: Run frontend tests**

Run:

```bash
npm run test --prefix apps/rfp-desktop
```

Expected: PASS.

## Task 8: Update Smoke And Verification

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/bin/smoke_first_pdf.rs`
- Modify: `tests/smoke/README.md`

- [ ] **Step 1: Update smoke to call candidate analysis**

In `apps/rfp-desktop/src-tauri/src/bin/smoke_first_pdf.rs`, after block normalization, replace:

```rust
let project_id = analysis::create_or_update_baseline_project(&conn, &document.id)?;
```

with:

```rust
let project_id = analysis::create_or_update_candidate_project(&conn, &document.id)?;
```

Add counts:

```rust
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
let evidence_count: i64 = conn.query_row(
    "SELECT COUNT(*) FROM evidence_links WHERE target_table = 'rfp_fields'",
    [],
    |row| row.get(0),
)?;
```

Print them:

```rust
println!("field_count={field_count}");
println!("candidate_bundle_count={candidate_bundle_count}");
println!("field_evidence_count={evidence_count}");
```

- [ ] **Step 2: Update smoke README expected output**

In `tests/smoke/README.md`, include these expected lines:

```text
field_count=<number of extracted project info fields>
candidate_bundle_count=7
field_evidence_count=<number of extracted fields with evidence>
```

Keep exit code semantics unchanged:

- `0` when all documents are ready or only allowed warnings exist.
- `1` when extraction or analysis execution failed.
- `2` when generation succeeded but blockers remain.

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
cargo build --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf
scripts/verify.sh
```

Expected: PASS.

- [ ] **Step 4: Run optional real PDF smoke when OpenDataLoader is available**

Run:

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- "rfp/rfp_bundle/05_AI/18_월드비전_AI서비스_플랫폼_구축_제안요청서.pdf"
```

Expected:

- `extraction_status=succeeded`
- `document_blocks` greater than `0`
- `candidate_bundle_count=7`
- `field_evidence_count` equals `field_count`
- exit code `2` is acceptable until requirement extraction is implemented, because `zero_requirements` remains a blocker.

## Risks And Mitigations

- **Keyword coverage may miss field labels in real RFPs.** Mitigation: keep bundle JSON persisted so reviewers can inspect what was selected; add terms through focused tests from real PDFs.
- **No `regex` dependency means value parsing is conservative.** Mitigation: store raw values and evidence links; only normalize budget lightly.
- **Candidate command may run before extraction.** Mitigation: `run_candidate_analysis_for_document` should proceed with zero blocks and validation should report `missing_evidence` and missing field blockers instead of failing the app.
- **The document can still be `review_needed` after project info extraction.** Mitigation: `zero_requirements` remains a blocker by design because durable requirement extraction is not in this slice.
- **Parallel workers may touch UI or validation files.** Mitigation: before implementation, read current files and preserve unrelated changes; do not revert another worker's edits.

## Done When

- `0002_candidate_extractor.sql` creates `rfp_fields`, `evidence_links`, and `candidate_bundles`.
- `db::migrate` applies both migrations idempotently.
- `candidate_extractor::build_candidate_bundles` returns all seven bundle keys deterministically.
- `candidate_extractor::extract_and_store_candidates` persists all bundle rows for a project.
- Rule extraction writes `rfp_fields` for detected project info and writes one evidence link per field.
- Candidate validation removes missing project-info blockers when fields are present and keeps `zero_requirements` until requirement writing exists.
- Tauri exposes `analyze_document_candidates`.
- The frontend can run candidate analysis and show business info fields plus candidate bundle counts.
- Smoke prints `field_count`, `candidate_bundle_count`, and `field_evidence_count`.
- The following commands pass:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
cargo build --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf
scripts/verify.sh
```

## Self-Review

- Spec coverage: covers PRD FR-004 project fields, FR-007 evidence links for fields, data pipeline step 4 candidate bundles, ERD `rfp_fields` and `evidence_links`, and quality gate missing-field behavior. Durable requirements/items/staffing/deliverables/acceptance/risk remain outside this slice and are represented only as candidate bundles.
- Type consistency: Rust DTO names use `RfpFieldDto`, `EvidenceLinkDto`, `CandidateBundleSummaryDto`, and `CandidateExtractionSummary`; TypeScript mirrors the same camelCase fields.
- Verification: every backend task has a focused Rust test, frontend touchpoints have Vitest coverage, and integrated verification ends with `scripts/verify.sh`.

# Domain Writer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist structured RFP domain records for requirements, procurement, staffing/MM, deliverables, acceptance criteria, risks, and source evidence links from deterministic candidates or accepted LLM output.

**Architecture:** Keep the domain writer as a Rust backend boundary that accepts typed draft records, validates evidence against `document_blocks`, writes SQLite rows in one transaction, and then runs the existing quality gate. Candidate extraction and LLM provider calls stay outside this module; this writer only normalizes, persists, links evidence, records rejected inputs, and updates `rfp_projects`/`documents` status.

**Tech Stack:** Tauri v2 backend, Rust, `rusqlite`, `serde`, `serde_json`, `chrono`, `uuid`, SQLite migrations, Rust unit tests, `scripts/verify.sh`.

---

## Source Specs

- `AGENTS.md`: continuous implementation rules, verification order, and parallel ownership policy.
- `TASKS.md`: Priority 2 Task 13 requires a plan for requirements, procurement, staffing, deliverables, acceptance, risks, and evidence links.
- `spec/02_prd.md`: FR-006 structured extraction and FR-007 evidence links; generated rows are not confirmed without evidence.
- `spec/04_erd.md`: target domain tables and indexes.
- `spec/05_data_pipeline.md`: structured output is saved only after schema and evidence validation; blockers produce `review_needed`.
- `spec/06_llm_contract.md`: LLM output schemas, opt-in boundary, evidence block IDs, confidence handling, and local normalizers.
- `spec/09_quality_gate.md`: blocker/warning taxonomy, status semantics, and generated vs ready/review_needed distinction.
- `spec/11_backlog_seed.md`: Epic 7 domain writer scope.
- Current code:
  - `apps/rfp-desktop/src-tauri/migrations/0001_core.sql`
  - `apps/rfp-desktop/src-tauri/src/db/mod.rs`
  - `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`
  - `apps/rfp-desktop/src-tauri/src/validation/mod.rs`
  - `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`

## Scope

Included:
- Add SQLite tables for `rfp_fields`, `requirements`, `procurement_items`, `staffing_requirements`, `deliverables`, `acceptance_criteria`, `risk_clauses`, and `evidence_links`.
- Add focused Rust DTOs for domain drafts accepted from deterministic candidate code or schema-validated LLM output.
- Add an internal `domain_writer` module that writes the full domain graph transactionally.
- Validate that every stored domain row has at least one evidence block belonging to the same document.
- Normalize numeric quantity, headcount, and MM from text without trusting LLM numeric interpretations.
- Resolve child rows to a requirement by `requirement_code`, creating deterministic generated requirements only when an evidenced child row has no resolvable parent requirement.
- Extend validation to inspect durable domain rows and set `ready` only when required fields, requirements, and evidence links pass.
- Add unit tests and focused verification commands.

Out of scope:
- Candidate bundle extraction from raw `document_blocks`. That is Priority 2 Task 11.
- OpenAI/Gemini network calls and provider settings. That is Priority 2 Task 12.
- Review UI tables and source evidence viewer. That is Priority 2 Task 14.
- Markdown/JSON/Docx export. That is Priority 2 Task 15.
- Mock-only Tauri commands. Tests should construct domain drafts directly in Rust.

## Current Baseline

The vertical slice already has:
- `documents`, `source_files`, `extraction_runs`, `document_blocks`, `rfp_projects`, `validation_findings`, and `audit_events`.
- `analysis::create_or_update_baseline_project`, which creates a baseline project and intentionally emits blockers.
- `validation::evaluate_baseline_project`, which currently assumes no domain rows exist.
- `db::migrate`, which executes only `0001_core.sql`.

The domain writer plan extends this foundation without reviving v1 PySide6 or adding production dependencies.

## File Structure

```text
apps/rfp-desktop/src-tauri/
├─ migrations/
│  ├─ 0001_core.sql
│  └─ 0002_domain_writer.sql
└─ src/
   ├─ analysis/
   │  └─ mod.rs
   ├─ db/
   │  └─ mod.rs
   ├─ domain.rs
   ├─ domain_writer/
   │  ├─ mod.rs
   │  ├─ numeric.rs
   │  └─ evidence.rs
   ├─ lib.rs
   └─ validation/
      └─ mod.rs
```

Responsibilities:
- `migrations/0002_domain_writer.sql`: durable schema for all domain records and evidence links.
- `db/mod.rs`: include and execute the second migration; update migration tests.
- `domain_writer/mod.rs`: public draft DTOs, transaction orchestration, row inserts, deterministic generated requirement handling, and write summary.
- `domain_writer/evidence.rs`: evidence block lookup, same-document enforcement, quote creation, and `evidence_links` inserts.
- `domain_writer/numeric.rs`: local parsing for quantity/headcount/MM from text.
- `validation/mod.rs`: domain-aware quality gate. It should keep baseline behavior for projects with no domain records but use domain rows once they exist.
- `analysis/mod.rs`: add a public helper that creates or resets a project, delegates to `domain_writer`, and evaluates the domain project.
- `domain.rs`: add serializable DTOs only if frontend or Tauri commands need the summary. Keep internal writer DTOs in `domain_writer` unless they must cross the Tauri boundary.
- `lib.rs`: expose `pub mod domain_writer;`.

## Schema Changes

Create `apps/rfp-desktop/src-tauri/migrations/0002_domain_writer.sql`:

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
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_rfp_fields_project_key
  ON rfp_fields(rfp_project_id, field_key);

CREATE TABLE IF NOT EXISTS requirements (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  requirement_code TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  category TEXT NOT NULL CHECK (
    category IN (
      'functional',
      'technical',
      'security',
      'data',
      'staffing',
      'management',
      'quality',
      'performance',
      'other'
    )
  ),
  mandatory INTEGER NOT NULL CHECK (mandatory IN (0, 1)),
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_requirements_project_code
  ON requirements(rfp_project_id, requirement_code);

CREATE TABLE IF NOT EXISTS procurement_items (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  item_type TEXT NOT NULL CHECK (
    item_type IN ('hardware', 'software', 'license', 'cloud', 'network', 'database', 'security', 'service', 'other')
  ),
  name TEXT NOT NULL,
  spec TEXT NOT NULL,
  quantity REAL,
  quantity_text TEXT NOT NULL DEFAULT '',
  unit TEXT NOT NULL DEFAULT '',
  required INTEGER NOT NULL CHECK (required IN (0, 1)),
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_procurement_items_requirement_id
  ON procurement_items(requirement_id);

CREATE TABLE IF NOT EXISTS staffing_requirements (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  grade TEXT NOT NULL DEFAULT '',
  headcount REAL,
  headcount_text TEXT NOT NULL DEFAULT '',
  mm REAL,
  mm_text TEXT NOT NULL DEFAULT '',
  onsite INTEGER,
  onsite_text TEXT NOT NULL DEFAULT '',
  period_text TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_staffing_requirements_requirement_id
  ON staffing_requirements(requirement_id);

CREATE TABLE IF NOT EXISTS deliverables (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  due_text TEXT NOT NULL DEFAULT '',
  format_text TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_deliverables_requirement_id
  ON deliverables(requirement_id);

CREATE TABLE IF NOT EXISTS acceptance_criteria (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  criterion_type TEXT NOT NULL CHECK (
    criterion_type IN ('test', 'performance', 'security', 'inspection', 'sla', 'warranty', 'other')
  ),
  description TEXT NOT NULL,
  threshold TEXT NOT NULL DEFAULT '',
  due_text TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_acceptance_criteria_requirement_id
  ON acceptance_criteria(requirement_id);

CREATE TABLE IF NOT EXISTS risk_clauses (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  risk_type TEXT NOT NULL CHECK (
    risk_type IN ('scope_creep', 'free_work', 'short_schedule', 'liability', 'ambiguous_spec', 'vendor_lock', 'payment', 'security', 'other')
  ),
  severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'blocker')),
  description TEXT NOT NULL,
  recommended_action TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_risk_clauses_requirement_id
  ON risk_clauses(requirement_id);

CREATE TABLE IF NOT EXISTS evidence_links (
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
);

CREATE INDEX IF NOT EXISTS idx_evidence_links_target
  ON evidence_links(target_table, target_id);

CREATE INDEX IF NOT EXISTS idx_evidence_links_block_id
  ON evidence_links(document_block_id);
```

Notes:
- `source` is added to all domain tables so deterministic rule rows and accepted LLM rows remain distinguishable.
- `confidence` is added to child tables that need to preserve LLM contract confidence.
- Polymorphic `evidence_links.target_table/target_id` cannot be fully enforced by SQLite FKs, so Rust validation must enforce target existence.
- `quantity_text`, `headcount_text`, and `mm_text` preserve source wording while numeric columns hold local parser output.

## Domain Draft Contract

The domain writer consumes a typed draft, not provider-specific JSON. Candidate extractor and LLM adapter plans should convert their outputs into this contract.

Add to `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftSource {
    Rule,
    Llm,
}

impl DraftSource {
    pub fn as_db_value(self) -> &'static str {
        match self {
            DraftSource::Rule => "rule",
            DraftSource::Llm => "llm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDraft {
    pub source: DraftSource,
    pub fields: Vec<FieldDraft>,
    pub requirements: Vec<RequirementDraft>,
    pub procurement_items: Vec<ProcurementItemDraft>,
    pub staffing_requirements: Vec<StaffingRequirementDraft>,
    pub deliverables: Vec<DeliverableDraft>,
    pub acceptance_criteria: Vec<AcceptanceCriterionDraft>,
    pub risk_clauses: Vec<RiskClauseDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDraft {
    pub block_id: String,
    pub quote: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDraft {
    pub field_key: String,
    pub label: String,
    pub raw_value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDraft {
    pub requirement_code: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub mandatory: bool,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcurementItemDraft {
    pub requirement_code: String,
    pub item_type: String,
    pub name: String,
    pub spec: String,
    pub quantity_text: String,
    pub unit: String,
    pub required: bool,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaffingRequirementDraft {
    pub requirement_code: String,
    pub role: String,
    pub grade: String,
    pub headcount_text: String,
    pub mm_text: String,
    pub onsite_text: String,
    pub period_text: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliverableDraft {
    pub requirement_code: String,
    pub name: String,
    pub due_text: String,
    pub format_text: String,
    pub description: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCriterionDraft {
    pub requirement_code: String,
    pub criterion_type: String,
    pub description: String,
    pub threshold: String,
    pub due_text: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskClauseDraft {
    pub requirement_code: String,
    pub risk_type: String,
    pub severity: String,
    pub description: String,
    pub recommended_action: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainRejection {
    pub severity: String,
    pub finding_type: String,
    pub message: String,
    pub target_table: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainWriteSummary {
    pub rfp_project_id: String,
    pub fields_written: usize,
    pub requirements_written: usize,
    pub procurement_items_written: usize,
    pub staffing_requirements_written: usize,
    pub deliverables_written: usize,
    pub acceptance_criteria_written: usize,
    pub risk_clauses_written: usize,
    pub evidence_links_written: usize,
    pub rejected_records: usize,
    pub rejections: Vec<DomainRejection>,
}
```

Rules:
- Field names mirror the LLM contract where possible, but evidence is normalized to `Vec<EvidenceDraft>`.
- Empty string values are allowed only when the source really lacks the optional detail. They must not be invented.
- `confidence` must be clamped by validation input rules: values outside `0.0..=1.0` reject that record.
- LLM JSON should never be parsed inside `domain_writer`; the LLM adapter should validate schema and convert into `DomainDraft`.

## Deterministic vs LLM-Generated Behavior

Deterministic behavior:
- Rule/candidate code may create `DraftSource::Rule` drafts.
- The writer must not infer missing facts from nearby prose.
- The writer may perform local normalization:
  - Parse `"3대"` into `quantity = 3.0`, `unit = "대"` if `unit` is empty.
  - Parse `"2명"` into `headcount = 2.0`.
  - Parse `"12MM"`, `"12 M/M"`, or `"12개월"` into `mm = 12.0` only when the text clearly expresses MM or person-month.
  - Parse onsite text into `Some(1)` for `상주`, `Some(0)` for `비상주` or `원격`, and `None` for ambiguous text.
- If a child row references an empty or unknown `requirement_code`, create a stable generated requirement only when the child has valid evidence. Use `GEN-001`, `GEN-002`, ... in deterministic sorted order of first evidence block `(page_number, block_index, block_id)` and entity type.
- Generated requirement rows:
  - `title`: `"프로젝트 공통 요구사항"`
  - `description`: first evidence quote
  - `category`: `staffing` for staffing rows, `management` for deliverables/acceptance, `other` for procurement/risk
  - `mandatory`: `true`
  - `confidence`: child confidence capped at `0.6`
  - `source`: same as draft source

LLM-generated behavior:
- LLM rows enter this module only after provider adapter schema validation.
- The writer must still validate evidence block IDs against SQLite. Schema-valid LLM output with missing, wrong-document, or empty evidence must be rejected.
- Numeric values from LLM text are not trusted as normalized numeric columns. Always reparse from `quantity_text`, `headcount_text`, and `mm_text`.
- LLM output that cites candidate blocks but contains values unsupported by those blocks should be flagged by the LLM adapter when possible. The writer must still store only rows with evidence and let validation create low-confidence or missing-detail findings.
- `llm_runs.response_json` remains the raw LLM audit source; domain tables are normalized accepted rows only.

## Source Evidence Link Rules

Every stored row in these tables must have at least one `evidence_links` row:
- `rfp_fields`
- `requirements`
- `procurement_items`
- `staffing_requirements`
- `deliverables`
- `acceptance_criteria`
- `risk_clauses`

Evidence validation rules:
- `EvidenceDraft.block_id` must exist in `document_blocks`.
- The block must belong to the same `documents.id` as the `rfp_project`.
- Empty evidence arrays reject the record.
- Evidence confidence outside `0.0..=1.0` rejects that evidence item.
- If all evidence items for a record are invalid, reject the record.
- `quote` is:
  - the provided quote trimmed to 500 characters when non-empty, or
  - the matching block text trimmed to 500 characters, or
  - `"[empty block]"` only for table/image blocks with preserved raw JSON and no text.

Rejected records:
- Do not insert partial domain rows.
- Increment `DomainWriteSummary.rejected_records`.
- Append `DomainRejection` details to `DomainWriteSummary.rejections`.
- Insert the rejection details into `validation_findings` after `validation::evaluate_project` has rebuilt durable-row findings, so rejection findings are not deleted by the evaluator.
- Use `blocker` severity for:
  - `missing_evidence`
  - `schema_invalid` for invalid enum/value shape reaching this boundary
  - `invalid_quantity` when numeric text exists but parses to zero or negative
- Use `warning` severity for:
  - `low_confidence`
  - `missing_quantity`
  - `unknown_requirement_reference` when a generated requirement was created

## Validation Rules

Extend `validation::evaluate_baseline_project` into a domain-aware evaluator while preserving the existing baseline behavior.

New public function:

```rust
pub fn evaluate_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()>;
```

Behavior:
- Delete existing findings for the project at the start, then rebuild findings from current durable rows.
- Required field blockers:
  - `missing_business_name` if no `rfp_fields.field_key = 'business_name'`.
  - `missing_client` if no `rfp_fields.field_key = 'client'`.
  - `missing_budget` if no `rfp_fields.field_key = 'budget'`.
  - `missing_period` if no `rfp_fields.field_key = 'period'`.
- Requirement blockers:
  - `zero_requirements` if no `requirements` rows exist.
  - `duplicate_requirement_code` if duplicates are detected before DB insertion or if the unique index would be violated.
  - `over_extraction` if `rfp_fields.field_key = 'requirement_count'` parses to `n` and actual requirements are greater than `max(n + 5, ceil(n * 1.5))`.
- Evidence blockers:
  - `missing_evidence` for any durable domain row without an evidence link.
- Procurement warnings:
  - `missing_quantity` if a procurement item has `name` but both `spec` and parsed `quantity` are missing.
  - `invalid_quantity` blocker if parsed `quantity <= 0.0`.
- Confidence warnings:
  - `low_confidence` for any domain row with `confidence < 0.6`.
- LLM warning:
  - Keep `llm_not_used` only when the project has no LLM-sourced rows.
- Risk blockers:
  - A `risk_clauses.severity = 'blocker'` row should create a `blocker` finding with `finding_type = 'risk_clause_blocker'`.

Status update:
- If any blocker exists: `rfp_projects.status = 'review_needed'` and `documents.status = 'review_needed'`.
- Else: `rfp_projects.status = 'ready'` and `documents.status = 'ready'`.
- If the write operation fails before validation completes: leave previous committed rows unchanged and return an error; do not mark `ready`.

## Implementation Tasks

### Task 1: Add Domain Schema Migration

**Files:**
- Create: `apps/rfp-desktop/src-tauri/migrations/0002_domain_writer.sql`
- Modify: `apps/rfp-desktop/src-tauri/src/db/mod.rs`

- [ ] **Step 1: Write the failing migration test**

Add a new test in `db::tests`:

```rust
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
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_domain_writer_tables
```

Expected: FAIL because `0002_domain_writer.sql` is not yet included.

- [ ] **Step 3: Add `0002_domain_writer.sql`**

Use the SQL from the "Schema Changes" section exactly.

- [ ] **Step 4: Include the second migration**

Modify `apps/rfp-desktop/src-tauri/src/db/mod.rs`:

```rust
const CORE_MIGRATION: &str = include_str!("../../migrations/0001_core.sql");
const DOMAIN_WRITER_MIGRATION: &str = include_str!("../../migrations/0002_domain_writer.sql");

pub fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(CORE_MIGRATION)?;
    conn.execute_batch(DOMAIN_WRITER_MIGRATION)?;
    Ok(())
}
```

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_domain_writer_tables
```

Expected: PASS.

### Task 2: Add Domain Writer Module and DTOs

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/domain_writer/evidence.rs`
- Create: `apps/rfp-desktop/src-tauri/src/domain_writer/numeric.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create module files**

Create:

```rust
// apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs
mod evidence;
mod numeric;

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
```

Add all DTOs from the "Domain Draft Contract" section below those imports.

- [ ] **Step 2: Expose the module**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs`:

```rust
pub mod domain_writer;
```

- [ ] **Step 3: Add initial compile test**

Add to `domain_writer/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_source_maps_to_db_value() {
        assert_eq!(DraftSource::Rule.as_db_value(), "rule");
        assert_eq!(DraftSource::Llm.as_db_value(), "llm");
    }
}
```

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::tests::draft_source_maps_to_db_value
```

Expected: PASS.

### Task 3: Implement Numeric Normalizers

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/numeric.rs`

- [ ] **Step 1: Write tests**

Add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quantity_and_unit_from_korean_text() {
        assert_eq!(parse_number("3대"), Some(3.0));
        assert_eq!(parse_number("1.5식"), Some(1.5));
        assert_eq!(parse_number("총 12 M/M"), Some(12.0));
    }

    #[test]
    fn parses_onsite_text() {
        assert_eq!(parse_onsite("상주"), Some(1));
        assert_eq!(parse_onsite("비상주"), Some(0));
        assert_eq!(parse_onsite("원격 수행"), Some(0));
        assert_eq!(parse_onsite("협의"), None);
    }
}
```

- [ ] **Step 2: Implement**

```rust
pub fn parse_number(text: &str) -> Option<f64> {
    let mut value = String::new();
    let mut started = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            value.push(ch);
            started = true;
        } else if started {
            break;
        }
    }
    if value.is_empty() {
        None
    } else {
        value.parse::<f64>().ok()
    }
}

pub fn parse_onsite(text: &str) -> Option<i64> {
    let normalized = text.trim();
    if normalized.contains("비상주") || normalized.contains("원격") {
        Some(0)
    } else if normalized.contains("상주") {
        Some(1)
    } else {
        None
    }
}
```

- [ ] **Step 3: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::numeric::tests
```

Expected: PASS.

### Task 4: Implement Evidence Validation and Inserts

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/evidence.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`

- [ ] **Step 1: Add evidence helper types**

In `evidence.rs`:

```rust
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use uuid::Uuid;

use crate::domain_writer::EvidenceDraft;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct EvidenceBlock {
    pub id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub text: String,
}
```

- [ ] **Step 2: Implement lookup and insert**

```rust
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
                "SELECT id, page_number, block_index, kind, text
                 FROM document_blocks
                 WHERE id = ? AND document_id = ?",
                params![item.block_id, document_id],
                |row| {
                    Ok(EvidenceBlock {
                        id: row.get(0)?,
                        page_number: row.get(1)?,
                        block_index: row.get(2)?,
                        kind: row.get(3)?,
                        text: row.get(4)?,
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
        return Err(AppError::InvalidInput("근거 block이 없는 domain row는 저장할 수 없습니다.".to_string()));
    }

    for (draft, block) in evidence_blocks {
        let quote = build_quote(draft.quote.as_deref(), &block.text, &block.kind);
        tx.execute(
            "INSERT INTO evidence_links (
                id, document_block_id, target_table, target_id, quote, confidence, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                Uuid::new_v4().to_string(),
                block.id,
                target_table,
                target_id,
                quote,
                draft.confidence,
                Utc::now().to_rfc3339(),
            ],
        )?;
    }
    Ok(evidence_blocks.len())
}

fn build_quote(provided: Option<&str>, block_text: &str, kind: &str) -> String {
    let candidate = provided
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| block_text.trim());
    let quote = if candidate.is_empty() && (kind == "table" || kind == "image") {
        "[empty block]"
    } else {
        candidate
    };
    quote.chars().take(500).collect()
}
```

- [ ] **Step 3: Verify compile**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer
```

Expected: PASS once Task 2 and Task 3 are complete.

### Task 5: Implement Transactional Domain Write

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`

- [ ] **Step 1: Add a failing integration-style unit test**

Add a test that creates a document, project, extraction run, blocks, and a full draft:

```rust
use super::test_support::{evidence, full_domain_draft, seed_document_project_and_blocks};

#[test]
fn writes_full_domain_graph_with_evidence_links() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);

    let draft = full_domain_draft();
    let summary = write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

    assert_eq!(summary.requirements_written, 1);
    assert_eq!(summary.procurement_items_written, 1);
    assert_eq!(summary.staffing_requirements_written, 1);
    assert_eq!(summary.deliverables_written, 1);
    assert_eq!(summary.acceptance_criteria_written, 1);
    assert_eq!(summary.risk_clauses_written, 1);
    assert_eq!(summary.rejected_records, 0);

    let evidence_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM evidence_links", [], |row| row.get(0))
        .expect("evidence count");
    assert!(evidence_count >= 7);
}
```

Use these helpers in a test support module so `validation::tests` can reuse the same realistic draft:

```rust
#[cfg(test)]
pub(crate) mod test_support {
use super::*;
use rusqlite::{params, Connection};

pub(crate) fn seed_document_project_and_blocks(conn: &Connection) {
    conn.execute(
        "INSERT INTO documents (id, title, created_at, updated_at, status)
         VALUES ('doc-1', 'sample.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'created')",
        [],
    )
    .expect("insert doc");
    conn.execute(
        "INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
         VALUES ('project-1', 'doc-1', 'test-version', 'draft', '', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z')",
        [],
    )
    .expect("insert project");
    conn.execute(
        "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
         VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
        [],
    )
    .expect("insert run");
    for index in 1..=8 {
        conn.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (?, 'run-1', 'doc-1', ?, 1, ?, 'paragraph', NULL, ?, NULL, '{}')",
            params![
                format!("block-{index}"),
                format!("el-{index}"),
                index,
                format!("테스트 근거 문장 {index}")
            ],
        )
        .expect("insert block");
    }
}

pub(crate) fn evidence(block_id: &str) -> Vec<EvidenceDraft> {
    vec![EvidenceDraft {
        block_id: block_id.to_string(),
        quote: None,
        confidence: 0.9,
    }]
}

pub(crate) fn full_domain_draft() -> DomainDraft {
    DomainDraft {
        source: DraftSource::Llm,
        fields: vec![
            FieldDraft {
                field_key: "business_name".to_string(),
                label: "사업명".to_string(),
                raw_value: "AI 서비스 플랫폼 구축".to_string(),
                normalized_value: "AI 서비스 플랫폼 구축".to_string(),
                confidence: 0.9,
                evidence: evidence("block-1"),
            },
            FieldDraft {
                field_key: "client".to_string(),
                label: "발주기관".to_string(),
                raw_value: "테스트 기관".to_string(),
                normalized_value: "테스트 기관".to_string(),
                confidence: 0.9,
                evidence: evidence("block-2"),
            },
            FieldDraft {
                field_key: "budget".to_string(),
                label: "예산".to_string(),
                raw_value: "100,000,000원".to_string(),
                normalized_value: "100000000 KRW".to_string(),
                confidence: 0.8,
                evidence: evidence("block-3"),
            },
            FieldDraft {
                field_key: "period".to_string(),
                label: "사업기간".to_string(),
                raw_value: "계약일로부터 6개월".to_string(),
                normalized_value: "6개월".to_string(),
                confidence: 0.8,
                evidence: evidence("block-4"),
            },
        ],
        requirements: vec![RequirementDraft {
            requirement_code: "SFR-001".to_string(),
            title: "통합 로그인".to_string(),
            description: "통합 로그인 기능을 제공한다.".to_string(),
            category: "functional".to_string(),
            mandatory: true,
            confidence: 0.9,
            evidence: evidence("block-5"),
        }],
        procurement_items: vec![ProcurementItemDraft {
            requirement_code: "SFR-001".to_string(),
            item_type: "software".to_string(),
            name: "인증 솔루션".to_string(),
            spec: "SSO 지원".to_string(),
            quantity_text: "1식".to_string(),
            unit: "식".to_string(),
            required: true,
            confidence: 0.8,
            evidence: evidence("block-6"),
        }],
        staffing_requirements: vec![StaffingRequirementDraft {
            requirement_code: "SFR-001".to_string(),
            role: "PM".to_string(),
            grade: "고급".to_string(),
            headcount_text: "1명".to_string(),
            mm_text: "6MM".to_string(),
            onsite_text: "상주".to_string(),
            period_text: "착수부터 종료까지".to_string(),
            confidence: 0.8,
            evidence: evidence("block-6"),
        }],
        deliverables: vec![DeliverableDraft {
            requirement_code: "SFR-001".to_string(),
            name: "설계서".to_string(),
            due_text: "설계 단계 종료 시".to_string(),
            format_text: "문서".to_string(),
            description: "시스템 설계서를 제출한다.".to_string(),
            confidence: 0.8,
            evidence: evidence("block-7"),
        }],
        acceptance_criteria: vec![AcceptanceCriterionDraft {
            requirement_code: "SFR-001".to_string(),
            criterion_type: "test".to_string(),
            description: "통합 로그인 시험 통과".to_string(),
            threshold: "성공률 100%".to_string(),
            due_text: "검수 시".to_string(),
            confidence: 0.8,
            evidence: evidence("block-7"),
        }],
        risk_clauses: vec![RiskClauseDraft {
            requirement_code: "SFR-001".to_string(),
            risk_type: "ambiguous_spec".to_string(),
            severity: "medium".to_string(),
            description: "세부 연동 범위가 모호하다.".to_string(),
            recommended_action: "질의서로 연동 대상 확정".to_string(),
            confidence: 0.7,
            evidence: evidence("block-8"),
        }],
    }
}
}
```

- [ ] **Step 2: Implement `write_domain_draft`**

Public signature:

```rust
pub fn write_domain_draft(
    conn: &mut Connection,
    rfp_project_id: &str,
    draft: DomainDraft,
) -> AppResult<DomainWriteSummary> {
    let tx = conn.transaction()?;
    let document_id: String = tx.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?;

    clear_existing_domain_rows(&tx, rfp_project_id)?;

    let mut writer = WriterState::new(rfp_project_id.to_string(), document_id, draft.source);
    writer.write_fields(&tx, &draft.fields)?;
    writer.write_requirements(&tx, &draft.requirements)?;
    writer.write_children(&tx, &draft)?;

    let summary = writer.summary();
    tx.commit()?;
    Ok(summary)
}
```

Implementation requirements:
- Use `conn.transaction()` instead of `unchecked_transaction()` because this writer owns the full write boundary.
- Clear rows in child-to-parent order:
  - `evidence_links` for current project targets
  - `risk_clauses`
  - `acceptance_criteria`
  - `deliverables`
  - `staffing_requirements`
  - `procurement_items`
  - `requirements`
  - `rfp_fields`
- Preserve `rfp_projects` row and `document_blocks`.
- Maintain a `BTreeMap<String, String>` from `requirement_code` to inserted `requirements.id`.
- Validate enums in Rust before insert so invalid LLM values become rejected records rather than SQLite errors.
- Return rejected record details in `DomainWriteSummary.rejections`; do not insert those findings in this low-level writer.

- [ ] **Step 3: Verify focused test**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::tests::writes_full_domain_graph_with_evidence_links
```

Expected: PASS.

### Task 6: Reject Missing or Wrong Evidence

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/evidence.rs`

- [ ] **Step 1: Add failing tests**

Add:

```rust
#[test]
fn rejects_domain_record_without_evidence() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);

    let mut draft = full_domain_draft();
    draft.requirements[0].evidence = vec![];

    let summary = write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

    assert_eq!(summary.requirements_written, 0);
    assert!(summary.rejected_records >= 1);
    assert!(summary
        .rejections
        .iter()
        .any(|rejection| rejection.finding_type == "missing_evidence"));
}

#[test]
fn rejects_evidence_from_another_document() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);
    conn.execute(
        "INSERT INTO documents (id, title, created_at, updated_at, status)
         VALUES ('doc-2', 'other.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'created')",
        [],
    )
    .expect("insert other doc");
    conn.execute(
        "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
         VALUES ('run-2', 'doc-2', 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
        [],
    )
    .expect("insert other run");
    conn.execute(
        "INSERT INTO document_blocks (
            id, extraction_run_id, document_id, source_element_id, page_number, block_index,
            kind, heading_level, text, bbox_json, raw_json
         ) VALUES ('foreign-block', 'run-2', 'doc-2', 'el-x', 1, 1, 'paragraph', NULL, '다른 문서 근거', NULL, '{}')",
        [],
    )
    .expect("insert foreign block");

    let mut draft = full_domain_draft();
    draft.requirements[0].evidence = evidence("foreign-block");

    let summary = write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

    assert_eq!(summary.requirements_written, 0);
    assert!(summary.rejected_records >= 1);
}
```

- [ ] **Step 2: Implement rejection handling**

For each insert method:
- Load valid evidence blocks before inserting the target row.
- If valid evidence is empty, call `record_rejection("blocker", "missing_evidence", "...", Some(target_table))` and skip the row.
- If the target row is inserted, insert evidence links immediately after insert.

- [ ] **Step 3: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::tests::rejects
```

Expected: PASS.

### Task 7: Resolve Child Rows to Requirements

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`

- [ ] **Step 1: Add generated requirement test**

Add:

```rust
#[test]
fn creates_generated_requirement_for_evidenced_orphan_child() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);

    let mut draft = full_domain_draft();
    draft.requirements = vec![];
    draft.procurement_items[0].requirement_code = "".to_string();

    let summary = write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

    assert_eq!(summary.requirements_written, 1);
    assert_eq!(summary.procurement_items_written, 1);
    let code: String = conn
        .query_row("SELECT requirement_code FROM requirements", [], |row| row.get(0))
        .expect("generated code");
    assert_eq!(code, "GEN-001");
}
```

- [ ] **Step 2: Add duplicate requirement test**

Add:

```rust
#[test]
fn rejects_duplicate_requirement_codes_before_sql_unique_failure() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);

    let mut draft = full_domain_draft();
    draft.requirements.push(RequirementDraft {
        requirement_code: "SFR-001".to_string(),
        title: "중복 요구사항".to_string(),
        description: "같은 코드".to_string(),
        category: "functional".to_string(),
        mandatory: true,
        confidence: 0.9,
        evidence: evidence("block-5"),
    });

    let summary = write_domain_draft(&mut conn, "project-1", draft).expect("write domain draft");

    assert_eq!(summary.requirements_written, 1);
    assert!(summary.rejected_records >= 1);
    assert!(summary
        .rejections
        .iter()
        .any(|rejection| rejection.finding_type == "duplicate_requirement_code"));
}
```

- [ ] **Step 3: Implement requirement resolver**

Implementation rules:
- Normalize codes with `trim()`.
- Keep original casing unless empty. Do not auto-uppercase Korean/English mixed IDs.
- Detect duplicates with `BTreeSet<String>`.
- For child rows:
  - If code exists in the map, use it.
  - If code is empty or unknown and evidence exists, create `GEN-###` requirement in stable insertion order.
  - Record warning `unknown_requirement_reference` for generated requirement.
  - If no evidence exists, reject child with `missing_evidence`.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::tests::creates_generated_requirement_for_evidenced_orphan_child
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::tests::rejects_duplicate_requirement_codes_before_sql_unique_failure
```

Expected: PASS.

### Task 8: Extend Quality Gate for Domain Rows

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/validation/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`

- [ ] **Step 1: Add validation tests**

Add tests in `validation::tests`:

```rust
use crate::domain_writer::test_support::{full_domain_draft, seed_document_project_and_blocks};

#[test]
fn domain_project_with_required_fields_requirement_and_evidence_becomes_ready() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);
    let draft = full_domain_draft();

    crate::domain_writer::write_domain_draft(&mut conn, "project-1", draft).expect("write");
    evaluate_project(&conn, "project-1").expect("evaluate");

    let status: String = conn
        .query_row("SELECT status FROM rfp_projects WHERE id = 'project-1'", [], |row| row.get(0))
        .expect("project status");
    assert_eq!(status, "ready");
}

#[test]
fn domain_project_missing_required_field_stays_review_needed() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);
    let mut draft = full_domain_draft();
    draft.fields.retain(|field| field.field_key != "budget");

    crate::domain_writer::write_domain_draft(&mut conn, "project-1", draft).expect("write");
    evaluate_project(&conn, "project-1").expect("evaluate");

    let blocker_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM validation_findings
             WHERE rfp_project_id = 'project-1' AND finding_type = 'missing_budget'",
            [],
            |row| row.get(0),
        )
        .expect("blocker count");
    assert_eq!(blocker_count, 1);
}
```

- [ ] **Step 2: Implement `evaluate_project`**

Refactor existing validation:
- Keep `FindingInput`.
- Add `insert_domain_rejections(conn, rfp_project_id, &summary.rejections)` that inserts `DomainRejection` values into `validation_findings`.
- Add `refresh_project_status_from_findings(conn, rfp_project_id)` so rejection blockers inserted after evaluation can still move the project/document back to `review_needed`.
- `evaluate_baseline_project` can call `evaluate_project` after ensuring baseline warnings are still present for no-domain projects.
- `evaluate_project` should query durable tables, insert findings, and update statuses as described in "Validation Rules".

- [ ] **Step 3: Update analysis helper**

Add:

```rust
pub fn write_domain_analysis(
    conn: &mut Connection,
    document_id: &str,
    draft: crate::domain_writer::DomainDraft,
) -> AppResult<crate::domain_writer::DomainWriteSummary> {
    let project_id = create_or_update_baseline_project(conn, document_id)?;
    let summary = crate::domain_writer::write_domain_draft(conn, &project_id, draft)?;
    validation::evaluate_project(conn, &project_id)?;
    validation::insert_domain_rejections(conn, &project_id, &summary.rejections)?;
    validation::refresh_project_status_from_findings(conn, &project_id)?;
    Ok(summary)
}
```

If `create_or_update_baseline_project` currently accepts `&Connection`, update signatures carefully so existing tests still pass. `rusqlite::Connection` can be passed as `&mut Connection` to functions requiring `&Connection`.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml validation::tests
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests
```

Expected: PASS.

### Task 9: Add Audit Event and Write Summary Coverage

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain_writer/mod.rs`

- [ ] **Step 1: Add audit test**

Add:

```rust
#[test]
fn domain_write_records_analysis_completed_audit_event() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_document_project_and_blocks(&conn);

    let summary = write_domain_draft(&mut conn, "project-1", full_domain_draft()).expect("write");

    let payload: String = conn
        .query_row(
            "SELECT payload_json FROM audit_events WHERE rfp_project_id = 'project-1' AND event_type = 'analysis_completed'",
            [],
            |row| row.get(0),
        )
        .expect("audit payload");
    assert!(payload.contains(&format!("\"requirementsWritten\":{}", summary.requirements_written)));
}
```

- [ ] **Step 2: Insert audit event**

At the end of successful transaction before `commit()`:

```rust
tx.execute(
    "INSERT INTO audit_events (id, rfp_project_id, document_id, event_type, payload_json, created_at)
     VALUES (?, ?, ?, 'analysis_completed', ?, ?)",
    params![
        Uuid::new_v4().to_string(),
        rfp_project_id,
        writer.document_id,
        serde_json::to_string(&summary)?,
        Utc::now().to_rfc3339(),
    ],
)?;
```

- [ ] **Step 3: Verify**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer::tests::domain_write_records_analysis_completed_audit_event
```

Expected: PASS.

### Task 10: Full Verification and Smoke Readiness

**Files:**
- No additional files unless tests reveal a required fix.

- [ ] **Step 1: Run focused domain writer suite**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer
```

Expected: PASS.

- [ ] **Step 2: Run full Rust tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run repo verification**

Run:

```bash
scripts/verify.sh
```

Expected: PASS.

- [ ] **Step 4: Optional real PDF smoke after candidate/LLM integration**

Run when Priority 2 candidate and LLM work can produce `DomainDraft`:

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- /absolute/path/to/rfp.pdf
```

Expected:
- The smoke still reports generated/ready/review_needed/failed separately.
- Domain row counts can be added later by the smoke worker without changing this writer boundary.

## Risks and Mitigations

- **Parallel edit collisions:** This plan touches `db`, `analysis`, `validation`, `domain_writer`, `lib.rs`, and migrations. Coordinate with candidate extractor, LLM adapter, and smoke workers before editing shared files.
- **Migration versioning is simple:** Current `db::migrate` executes static SQL files with `CREATE TABLE IF NOT EXISTS`. This is acceptable for the early local milestone, but future migrations that alter columns will need explicit migration tracking.
- **Polymorphic evidence target cannot use real FKs:** Keep target table enum checks in SQL and target existence checks in Rust tests.
- **LLM hallucination:** The writer only accepts schema-validated drafts and still requires block evidence. Unsupported semantic claims may need deeper quote checking in the LLM adapter or later reviewer UI.
- **Generated requirements can hide poor extraction:** Generated `GEN-###` rows are allowed to preserve evidenced child entities, but they must emit `unknown_requirement_reference` warnings so UI/export can show review risk.
- **Overwriting user corrections:** This plan clears generated domain rows on rewrite. Once corrections exist, the writer must preserve `corrections` and the display/export layer must apply them as overlays. Do not delete corrections in this plan.
- **Status optimism:** `ready` is allowed only when no blockers exist after domain validation. Generated rows alone are not success.

## Done When

- `0002_domain_writer.sql` creates all domain and evidence tables with indexes.
- `db::migrate` applies core and domain migrations.
- `domain_writer::write_domain_draft` transactionally writes fields, requirements, procurement, staffing/MM, deliverables, acceptance criteria, risks, and evidence links.
- Stored domain rows always have same-document evidence links.
- Rows with missing or invalid evidence are rejected and create findings instead of being stored.
- Quantity, headcount, MM, and onsite values are locally normalized from source text.
- Child entities resolve to existing requirements or deterministic `GEN-###` requirements with warnings.
- Domain-aware validation sets `review_needed` for blockers and `ready` only when required fields, requirements, and evidence pass.
- Focused Rust tests pass:
  - `db::tests::migrates_domain_writer_tables`
  - `domain_writer::tests`
  - `validation::tests`
  - `analysis::tests`
- Full verification passes:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
scripts/verify.sh
```

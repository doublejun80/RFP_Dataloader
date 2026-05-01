# RFP Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate deterministic Markdown, JSON, and Docx exports from an RFP project DB snapshot, preserving quality-gate status, corrections, and source evidence citations.

**Architecture:** Rust owns snapshot loading, quality confirmation, rendering, file writing, export history, and audit events behind Tauri commands. React only requests an export, confirms `검토 필요` exports when needed, and displays export history returned by typed DTOs.

**Tech Stack:** Tauri v2, React, TypeScript, Rust, SQLite via `rusqlite`, `serde_json`, `sha2`, `zip` for the Docx container, Vitest, Rust unit tests, repository `scripts/verify.sh`.

---

## Source Context

- `spec/02_prd.md`: FR-010 requires Markdown, JSON, Docx export and export history.
- `spec/04_erd.md`: `rfp_projects ||--o{ exports : produces`; `exports` stores type, path, status, and creation time.
- `spec/05_data_pipeline.md`: export is stage 8 and must use a DB snapshot; Markdown sections are fixed.
- `spec/08_ui_product_flow.md`: export UX shows quality state first; blocker exports require explicit `검토 필요 상태로 내보내기` confirmation.
- `spec/09_quality_gate.md`: export must report generated, ready, review_needed, failed, blocker, and warning semantics without treating blockers as success.
- Current code already has `documents`, `source_files`, `extraction_runs`, `document_blocks`, `rfp_projects`, `validation_findings`, and `audit_events`.
- Current code does not yet have `exports` or the full domain tables. This plan should be implemented after the Domain Writer plan has added `rfp_fields`, `requirements`, `procurement_items`, `staffing_requirements`, `deliverables`, `acceptance_criteria`, `risk_clauses`, `evidence_links`, and `corrections`.

## Sequencing and Parallel Safety

Start this plan after the domain writer migration has landed. The export worker owns the files listed in this plan and must not rewrite candidate extraction, LLM adapter, or review UI internals.

If another worker has already introduced a migration after `0001_core.sql`, keep the same `exports` schema below and place it in the next numeric migration file. Update `db::migrate` with migrations in filename order and adjust the migration test to assert the actual filename sequence.

## File Structure

Create:

```text
apps/rfp-desktop/src-tauri/migrations/0002_export_history.sql
apps/rfp-desktop/src-tauri/src/exporter/mod.rs
apps/rfp-desktop/src-tauri/src/exporter/snapshot.rs
apps/rfp-desktop/src-tauri/src/exporter/report_model.rs
apps/rfp-desktop/src-tauri/src/exporter/markdown.rs
apps/rfp-desktop/src-tauri/src/exporter/json.rs
apps/rfp-desktop/src-tauri/src/exporter/docx.rs
apps/rfp-desktop/src-tauri/src/exporter/file_naming.rs
apps/rfp-desktop/src-tauri/src/exporter/writer.rs
apps/rfp-desktop/src-tauri/src/commands/exports.rs
apps/rfp-desktop/src/components/ExportPanel.tsx
apps/rfp-desktop/src/components/ExportHistory.tsx
```

Modify:

```text
apps/rfp-desktop/src-tauri/Cargo.toml
apps/rfp-desktop/src-tauri/src/db/mod.rs
apps/rfp-desktop/src-tauri/src/domain.rs
apps/rfp-desktop/src-tauri/src/error.rs
apps/rfp-desktop/src-tauri/src/lib.rs
apps/rfp-desktop/src-tauri/src/commands/mod.rs
apps/rfp-desktop/src/lib/types.ts
apps/rfp-desktop/src/lib/api.ts
apps/rfp-desktop/src/App.tsx
apps/rfp-desktop/src/App.test.tsx
scripts/verify.sh
tests/smoke/README.md
```

## Snapshot Contract

Export reads one consistent snapshot from SQLite inside one transaction. It applies corrections for display/export, keeps original values in JSON, and assigns stable evidence labels.

Stable ordering:

- Project fields: `business_name`, `client`, `budget`, `period`, `contract_method`, `deadline`, `evaluation_ratio`, then other keys by `field_key`.
- Requirements: natural sort by `requirement_code`, then `title`, then `id`.
- Child rows: by linked `requirement_code`, then human label (`name`, `role`, `description`), then `id`.
- Evidence labels: first-use order across the rendered report, then `page_number`, `block_index`, `document_block_id`.
- Findings: blockers before warnings before info, then `finding_type`, `message`, `id`.

Citation format:

- Human reports use `[E001]`, `[E002]`, etc.
- JSON stores the same label plus `documentBlockId`, `pageNumber`, `blockIndex`, `bbox`, `quote`, and `target`.
- If an entity lacks evidence, export the entity and include the related `missing_evidence` blocker in the quality section instead of inventing a citation.

File naming:

```text
rfp_{safe-title}_{project-id-8}_{snapshot-sha-8}.md
rfp_{safe-title}_{project-id-8}_{snapshot-sha-8}.json
rfp_{safe-title}_{project-id-8}_{snapshot-sha-8}.docx
```

`safe-title` keeps Korean, ASCII letters, numbers, `_`, and `-`; collapses every other run to `_`; trims leading/trailing `_`; limits to 80 Unicode scalar values; and falls back to `rfp` when empty. Default output directory is `app_data_dir/exports/{rfp_project_id}/`. A caller-provided output directory is accepted only when it is absolute; Rust creates it if missing.

## Task 1: Add Export History Schema

**Files:**
- Create: `apps/rfp-desktop/src-tauri/migrations/0002_export_history.sql`
- Modify: `apps/rfp-desktop/src-tauri/src/db/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/error.rs`

- [ ] **Step 1: Write failing migration test**

Add this assertion to `db::tests::migrates_core_tables` or create `db::tests::migrates_export_history_table`:

```rust
#[test]
fn migrates_export_history_table() {
    let conn = Connection::open_in_memory().expect("open memory db");

    migrate(&conn).expect("run migrations");

    let table_sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'exports'",
            [],
            |row| row.get(0),
        )
        .expect("exports table exists");
    assert!(table_sql.contains("content_sha256"));
    assert!(table_sql.contains("snapshot_sha256"));
    assert!(table_sql.contains("error_message"));
}
```

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_export_history_table
```

Expected: FAIL because `exports` does not exist.

- [ ] **Step 2: Add the migration**

Create `apps/rfp-desktop/src-tauri/migrations/0002_export_history.sql`:

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS exports (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  export_type TEXT NOT NULL CHECK (export_type IN ('markdown', 'json', 'docx')),
  path TEXT,
  status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
  content_sha256 TEXT,
  snapshot_sha256 TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_exports_project_created
  ON exports(rfp_project_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_exports_snapshot_sha256
  ON exports(snapshot_sha256);
```

- [ ] **Step 3: Run migrations in deterministic order**

Replace `db::migrate` with a migration list:

```rust
const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_core.sql"),
    include_str!("../../migrations/0002_export_history.sql"),
];

pub fn migrate(conn: &Connection) -> AppResult<()> {
    for migration in MIGRATIONS {
        conn.execute_batch(migration)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Add export-specific errors**

Add these variants to `AppError`:

```rust
#[error("검토 필요 상태는 확인 후 내보낼 수 있습니다.")]
ReviewNeededConfirmationRequired,

#[error("지원하지 않는 export 형식입니다: {0}")]
InvalidExportFormat(String),

#[error("내보내기 경로는 절대 경로여야 합니다: {0}")]
InvalidExportPath(String),
```

- [ ] **Step 5: Verify migration**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_export_history_table
```

Expected: PASS.

## Task 2: Define Export DTOs

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/domain.rs`
- Modify: `apps/rfp-desktop/src/lib/types.ts`

- [ ] **Step 1: Write Rust DTOs**

Add DTOs using `#[serde(rename_all = "camelCase")]` to match current Tauri command responses:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Markdown,
    Json,
    Docx,
}

impl ExportFormat {
    pub fn as_db_value(&self) -> &'static str {
        match self {
            ExportFormat::Markdown => "markdown",
            ExportFormat::Json => "json",
            ExportFormat::Docx => "docx",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Markdown => "md",
            ExportFormat::Json => "json",
            ExportFormat::Docx => "docx",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub output_dir: Option<String>,
    pub allow_review_needed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub id: String,
    pub rfp_project_id: String,
    pub document_id: String,
    pub format: ExportFormat,
    pub path: String,
    pub status: String,
    pub snapshot_sha256: String,
    pub content_sha256: String,
    pub blocker_count: i64,
    pub warning_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryItem {
    pub id: String,
    pub rfp_project_id: String,
    pub export_type: String,
    pub path: Option<String>,
    pub status: String,
    pub content_sha256: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}
```

- [ ] **Step 2: Mirror TypeScript types**

Add to `apps/rfp-desktop/src/lib/types.ts`:

```ts
export type ExportFormat = "markdown" | "json" | "docx";

export interface ExportRequest {
  format: ExportFormat;
  outputDir?: string | null;
  allowReviewNeeded: boolean;
}

export interface ExportResult {
  id: string;
  rfpProjectId: string;
  documentId: string;
  format: ExportFormat;
  path: string;
  status: "succeeded" | "failed";
  snapshotSha256: string;
  contentSha256: string;
  blockerCount: number;
  warningCount: number;
  createdAt: string;
}

export interface ExportHistoryItem {
  id: string;
  rfpProjectId: string;
  exportType: ExportFormat;
  path?: string | null;
  status: "succeeded" | "failed";
  contentSha256?: string | null;
  snapshotSha256?: string | null;
  errorMessage?: string | null;
  createdAt: string;
  finishedAt?: string | null;
}
```

- [ ] **Step 3: Verify type checking**

Run:

```bash
npm run build --prefix apps/rfp-desktop
```

Expected: PASS.

## Task 3: Load a DB Snapshot With Evidence and Corrections

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/exporter/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/exporter/snapshot.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create the exporter module**

Add to `lib.rs`:

```rust
pub mod exporter;
```

Create `exporter/mod.rs`:

```rust
pub mod docx;
pub mod file_naming;
pub mod json;
pub mod markdown;
pub mod report_model;
pub mod snapshot;
pub mod writer;
```

- [ ] **Step 2: Define snapshot structs**

In `snapshot.rs`, define serializable structs with stable field order:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportSnapshot {
    pub schema_version: &'static str,
    pub document: SnapshotDocument,
    pub project: SnapshotProject,
    pub fields: Vec<SnapshotField>,
    pub requirements: Vec<SnapshotRequirement>,
    pub procurement_items: Vec<SnapshotProcurementItem>,
    pub staffing_requirements: Vec<SnapshotStaffingRequirement>,
    pub deliverables: Vec<SnapshotDeliverable>,
    pub acceptance_criteria: Vec<SnapshotAcceptanceCriterion>,
    pub risk_clauses: Vec<SnapshotRiskClause>,
    pub validation_findings: Vec<SnapshotFinding>,
    pub evidence: Vec<SnapshotEvidence>,
    pub corrections: Vec<SnapshotCorrection>,
}
```

Every domain row struct must include:

```rust
pub id: String,
pub evidence_labels: Vec<String>,
pub original_values: serde_json::Value,
pub corrected_values: serde_json::Value,
```

Use `original_values` for DB values before correction and `corrected_values` for export display values after correction. For rows with no correction, both values are identical.

- [ ] **Step 3: Write failing snapshot test**

Seed a minimal project with one field, one requirement, one procurement item, one evidence link, one correction, and one warning:

```rust
#[test]
fn snapshot_applies_corrections_and_preserves_evidence() {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_export_domain_snapshot(&conn);

    let snapshot = load_export_snapshot(&conn, "doc-1").expect("load snapshot");

    assert_eq!(snapshot.project.id, "project-1");
    assert_eq!(snapshot.fields[0].field_key, "business_name");
    assert_eq!(
        snapshot.fields[0].corrected_values["normalizedValue"],
        serde_json::json!("보정된 사업명")
    );
    assert_eq!(snapshot.fields[0].evidence_labels, vec!["E001"]);
    assert_eq!(snapshot.evidence[0].page_number, 3);
    assert_eq!(snapshot.validation_findings[0].finding_type, "correction_applied");
}
```

`seed_export_domain_snapshot` should insert real rows into the domain tables added by the Domain Writer plan. Do not mock the snapshot loader.

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::snapshot::tests::snapshot_applies_corrections_and_preserves_evidence
```

Expected: FAIL before the loader exists.

- [ ] **Step 4: Implement snapshot loading**

Implement `load_export_snapshot(conn: &Connection, document_id: &str) -> AppResult<ExportSnapshot>`:

```rust
pub fn load_export_snapshot(conn: &Connection, document_id: &str) -> AppResult<ExportSnapshot> {
    let tx = conn.unchecked_transaction()?;
    let document = load_document(&tx, document_id)?;
    let project = load_project(&tx, document_id)?;
    let corrections = load_corrections(&tx, &project.id)?;
    let correction_index = CorrectionIndex::new(&corrections);
    let fields = load_fields(&tx, &project.id, &correction_index)?;
    let requirements = load_requirements(&tx, &project.id, &correction_index)?;
    let procurement_items = load_procurement_items(&tx, &project.id, &correction_index)?;
    let staffing_requirements = load_staffing_requirements(&tx, &project.id, &correction_index)?;
    let deliverables = load_deliverables(&tx, &project.id, &correction_index)?;
    let acceptance_criteria = load_acceptance_criteria(&tx, &project.id, &correction_index)?;
    let risk_clauses = load_risk_clauses(&tx, &project.id, &correction_index)?;
    let validation_findings = load_validation_findings(&tx, &project.id)?;
    let mut snapshot = ExportSnapshot {
        schema_version: "rfp-export-v1",
        document,
        project,
        fields,
        requirements,
        procurement_items,
        staffing_requirements,
        deliverables,
        acceptance_criteria,
        risk_clauses,
        validation_findings,
        evidence: Vec::new(),
        corrections,
    };
    attach_evidence_labels(&tx, &mut snapshot)?;
    tx.commit()?;
    Ok(snapshot)
}
```

Queries must include explicit `ORDER BY` clauses from the Snapshot Contract. Evidence loading must join `evidence_links` to `document_blocks` and preserve `bbox_json`.

- [ ] **Step 5: Verify snapshot test**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::snapshot::tests::snapshot_applies_corrections_and_preserves_evidence
```

Expected: PASS.

## Task 4: Build a Shared Report Model

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/exporter/report_model.rs`

- [ ] **Step 1: Write report model tests**

Add tests that prove all required sections are present in order:

```rust
#[test]
fn report_model_sections_follow_pipeline_export_order() {
    let snapshot = fixture_snapshot();
    let report = ReportDocument::from_snapshot(&snapshot);

    let headings = report
        .blocks
        .iter()
        .filter_map(|block| match block {
            ReportBlock::Heading { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        headings,
        vec![
            "사업 기본정보",
            "구매 항목 BOM",
            "인력/MM",
            "업무 범위",
            "납품물",
            "검수/인수 조건",
            "리스크/독소 조항",
            "요구사항 Traceability",
            "품질 게이트 결과",
            "원문 근거"
        ]
    );
}
```

- [ ] **Step 2: Implement the model**

Create these types:

```rust
pub struct ReportDocument {
    pub title: String,
    pub status: String,
    pub snapshot_sha256: String,
    pub blocks: Vec<ReportBlock>,
}

pub enum ReportBlock {
    Heading { level: u8, text: String },
    Paragraph { text: String },
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
}
```

`ReportDocument::from_snapshot` must build the section order from `spec/05_data_pipeline.md` and include citations in the relevant table cells, such as `SFR-001 [E001]`.

- [ ] **Step 3: Verify report model**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::report_model::tests::report_model_sections_follow_pipeline_export_order
```

Expected: PASS.

## Task 5: Render Deterministic Markdown and JSON

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/exporter/markdown.rs`
- Create: `apps/rfp-desktop/src-tauri/src/exporter/json.rs`

- [ ] **Step 1: Write renderer tests**

Add tests:

```rust
#[test]
fn markdown_render_is_deterministic_and_cites_evidence() {
    let snapshot = fixture_snapshot();
    let report = ReportDocument::from_snapshot(&snapshot);

    let first = render_markdown(&report);
    let second = render_markdown(&report);

    assert_eq!(first, second);
    assert!(first.contains("# RFP 검토 보고서:"));
    assert!(first.contains("## 사업 기본정보"));
    assert!(first.contains("## 품질 게이트 결과"));
    assert!(first.contains("[E001]"));
    assert!(first.contains("검토 필요"));
}

#[test]
fn json_render_is_deterministic_and_contains_snapshot_schema() {
    let snapshot = fixture_snapshot();

    let first = render_json(&snapshot).expect("render first");
    let second = render_json(&snapshot).expect("render second");

    assert_eq!(first, second);
    assert!(first.contains("\"schemaVersion\": \"rfp-export-v1\""));
    assert!(first.contains("\"evidenceLabels\""));
}
```

- [ ] **Step 2: Implement Markdown rendering**

Use the shared report model. Escape Markdown table pipes and newlines:

```rust
fn escape_table_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "<br>")
        .trim()
        .to_string()
}
```

Markdown structure must be:

```markdown
# RFP 검토 보고서: 서울시 통합 유지관리 RFP

- 품질 상태: review_needed
- Snapshot SHA-256: abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789
- Blocker: 2
- Warning: 1

## 사업 기본정보

| 항목 | 값 | 근거 |
|---|---|---|
| 사업명 | 서울시 통합 유지관리 RFP | [E001] |

## 원문 근거

| ID | Page | Block | Quote |
|---|---:|---:|---|
| E001 | 3 | 12 | 사업명: 서울시 통합 유지관리 RFP |
```

Do not include the current wall-clock time in Markdown. The export row records created time; the content stays deterministic for the same DB snapshot.

- [ ] **Step 3: Implement JSON rendering**

Implement:

```rust
pub fn render_json(snapshot: &ExportSnapshot) -> AppResult<Vec<u8>> {
    let text = serde_json::to_string_pretty(snapshot)?;
    Ok(format!("{text}\n").into_bytes())
}
```

Stable arrays and struct field order come from the snapshot loader and DTO declarations.

- [ ] **Step 4: Verify Markdown and JSON tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::markdown::tests::markdown_render_is_deterministic_and_cites_evidence
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::json::tests::json_render_is_deterministic_and_contains_snapshot_schema
```

Expected: PASS.

## Task 6: Render Deterministic Docx

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/Cargo.toml`
- Create: `apps/rfp-desktop/src-tauri/src/exporter/docx.rs`

- [ ] **Step 1: Add the minimal Docx container dependency**

Add to `Cargo.toml`:

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

Record the dependency reason in `IMPLEMENTATION_LOG.md` during implementation: Docx is a zipped OpenXML package, and `zip` avoids shelling out to external tools or introducing a heavier document renderer.

- [ ] **Step 2: Write Docx tests**

Add tests that inspect the zipped XML:

```rust
#[test]
fn docx_render_is_deterministic_and_contains_evidence() {
    let snapshot = fixture_snapshot();
    let report = ReportDocument::from_snapshot(&snapshot);

    let first = render_docx(&report).expect("render first");
    let second = render_docx(&report).expect("render second");

    assert_eq!(first, second);

    let document_xml = read_docx_part(&first, "word/document.xml");
    assert!(document_xml.contains("RFP 검토 보고서"));
    assert!(document_xml.contains("품질 게이트 결과"));
    assert!(document_xml.contains("[E001]"));
}
```

- [ ] **Step 3: Implement minimal OOXML generation**

Create fixed zip entries in this exact order:

```text
[Content_Types].xml
_rels/.rels
docProps/core.xml
docProps/app.xml
word/_rels/document.xml.rels
word/document.xml
word/styles.xml
```

Set every zip entry timestamp to `2026-01-01T00:00:00` using `zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0)`. Render headings, paragraphs, and tables from `ReportDocument`. Escape XML using:

```rust
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
```

Do not call Pandoc, Word, LibreOffice, or a network service.

- [ ] **Step 4: Verify Docx tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::docx::tests::docx_render_is_deterministic_and_contains_evidence
```

Expected: PASS.

## Task 7: Write Export Files and History Rows

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/exporter/file_naming.rs`
- Create: `apps/rfp-desktop/src-tauri/src/exporter/writer.rs`

- [ ] **Step 1: Write file naming tests**

Add tests:

```rust
#[test]
fn safe_filename_keeps_korean_and_limits_length() {
    let title = "서울시 통합 유지관리 RFP / 2026: 최종본.pdf";

    let safe = safe_filename_component(title);

    assert!(safe.starts_with("서울시_통합_유지관리_RFP_2026_최종본"));
    assert!(safe.chars().count() <= 80);
}

#[test]
fn export_filename_uses_project_and_snapshot_hash() {
    let name = export_file_name(
        "서울시 통합 유지관리 RFP",
        "12345678-aaaa-bbbb-cccc-123456789abc",
        "abcdef0123456789",
        ExportFormat::Markdown,
    );

    assert_eq!(
        name,
        "rfp_서울시_통합_유지관리_RFP_12345678_abcdef01.md"
    );
}
```

- [ ] **Step 2: Implement writer behavior**

Implement:

```rust
pub fn export_document_snapshot(
    conn: &Connection,
    app_data_dir: &Path,
    document_id: &str,
    request: ExportRequest,
) -> AppResult<ExportResult>
```

Behavior:

1. Load snapshot.
2. Count blockers and warnings from `snapshot.validation_findings`.
3. If blockers exist and `request.allow_review_needed == false`, return `AppError::ReviewNeededConfirmationRequired` without writing a file and without inserting an export row.
4. Render requested bytes from the same snapshot.
5. Compute `snapshot_sha256` from canonical JSON bytes and `content_sha256` from rendered bytes.
6. Resolve output directory.
7. Write the file atomically by writing `{file}.part` first, flushing it, and renaming it to the final name.
8. Insert `exports.status = 'succeeded'` with path, content hash, snapshot hash, created and finished timestamps.
9. Insert `audit_events.event_type = 'export_created'` with `rfp_project_id`, `document_id`, `format`, `path`, and hash payload.

On render or write failure after export starts, insert `exports.status = 'failed'` with `error_message`, `created_at`, and `finished_at`, then return the error.

- [ ] **Step 3: Write integration test**

Add:

```rust
#[test]
fn export_writes_file_history_and_audit_event() {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = crate::db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
    seed_export_domain_snapshot(&conn);

    let result = export_document_snapshot(
        &conn,
        temp.path(),
        "doc-1",
        ExportRequest {
            format: ExportFormat::Markdown,
            output_dir: None,
            allow_review_needed: true,
        },
    )
    .expect("export");

    assert!(std::path::Path::new(&result.path).exists());
    assert_eq!(result.status, "succeeded");
    assert_eq!(result.blocker_count, 0);

    let export_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM exports WHERE rfp_project_id = 'project-1'", [], |row| row.get(0))
        .expect("export count");
    assert_eq!(export_count, 1);

    let audit_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_events WHERE event_type = 'export_created'", [], |row| row.get(0))
        .expect("audit count");
    assert_eq!(audit_count, 1);
}
```

- [ ] **Step 4: Verify writer tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::file_naming
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::writer::tests::export_writes_file_history_and_audit_event
```

Expected: PASS.

## Task 8: Add Tauri Export Commands

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/commands/exports.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`
- Modify: `apps/rfp-desktop/src/lib/api.ts`

- [ ] **Step 1: Add Rust commands**

Create `commands/exports.rs`:

```rust
use tauri::State;

use crate::domain::{ExportHistoryItem, ExportRequest, ExportResult};
use crate::error::AppResult;
use crate::exporter::writer;
use crate::state::AppState;

#[tauri::command]
pub fn export_document_snapshot(
    document_id: String,
    request: ExportRequest,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    let conn = state.connect()?;
    writer::export_document_snapshot(&conn, &state.app_data_dir, &document_id, request)
}

#[tauri::command]
pub fn list_document_exports(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ExportHistoryItem>> {
    let conn = state.connect()?;
    writer::list_document_exports(&conn, &document_id)
}
```

Register:

```rust
pub mod exports;
```

and add to `tauri::generate_handler!`:

```rust
commands::exports::export_document_snapshot,
commands::exports::list_document_exports,
```

- [ ] **Step 2: Add frontend API functions**

Add to `apps/rfp-desktop/src/lib/api.ts`:

```ts
export function exportDocumentSnapshot(
  documentId: string,
  request: ExportRequest,
): Promise<ExportResult> {
  return invoke<ExportResult>("export_document_snapshot", {
    documentId,
    request,
  });
}

export function listDocumentExports(
  documentId: string,
): Promise<ExportHistoryItem[]> {
  return invoke<ExportHistoryItem[]>("list_document_exports", { documentId });
}
```

- [ ] **Step 3: Verify command compilation**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::exports
npm run build --prefix apps/rfp-desktop
```

Expected: PASS.

## Task 9: Add Export UI

**Files:**
- Create: `apps/rfp-desktop/src/components/ExportPanel.tsx`
- Create: `apps/rfp-desktop/src/components/ExportHistory.tsx`
- Modify: `apps/rfp-desktop/src/App.tsx`
- Modify: `apps/rfp-desktop/src/App.test.tsx`

- [ ] **Step 1: Write UI test**

Add a Vitest case that mocks export commands:

```ts
it("shows export controls and requires review-needed confirmation", async () => {
  render(<App />);

  expect(await screen.findByRole("heading", { name: "RFP 분석 작업대" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Markdown/ })).toBeDisabled();
  expect(screen.getByLabelText("검토 필요 상태로 내보내기")).toBeInTheDocument();
});
```

The mock document in `App.test.tsx` has `status: "review_needed"`, so export buttons stay disabled until the confirmation checkbox is checked.

- [ ] **Step 2: Implement `ExportPanel`**

UI rules:

- Three icon buttons: Markdown, JSON, Docx.
- A checkbox labeled `검토 필요 상태로 내보내기` appears when selected document status is `review_needed` or blocker count is greater than zero.
- On success, show the returned file path.
- On `ReviewNeededConfirmationRequired`, keep the path empty and show the Korean error from Rust.

Use the existing `runAction` pattern in `App.tsx` so export errors share the existing error banner.

- [ ] **Step 3: Implement `ExportHistory`**

Show a compact list with:

- 형식
- 상태
- 생성 시각
- 파일 경로
- 오류 메시지 when status is `failed`

Refresh history after each export and when the selected document changes.

- [ ] **Step 4: Verify UI**

Run:

```bash
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
```

Expected: PASS.

## Task 10: Add Smoke Coverage and Full Verification

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/bin/smoke_export_snapshot.rs`
- Modify: `scripts/verify.sh`
- Modify: `tests/smoke/README.md`

- [ ] **Step 1: Add export smoke binary**

Create `smoke_export_snapshot.rs` that seeds a deterministic DB snapshot with domain rows, exports Markdown, JSON, and Docx to a temp export directory, then prints:

```text
export_markdown=succeeded
export_json=succeeded
export_docx=succeeded
snapshot_sha256=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789
markdown_path=/tmp/rfp-export/rfp_서울시_통합_유지관리_RFP_project1_abcdef01.md
json_path=/tmp/rfp-export/rfp_서울시_통합_유지관리_RFP_project1_abcdef01.json
docx_path=/tmp/rfp-export/rfp_서울시_통합_유지관리_RFP_project1_abcdef01.docx
```

Exit code:

- `0`: all three exports succeeded.
- `1`: any render/write command failed.
- `2`: export was blocked because `allow_review_needed` was false.

- [ ] **Step 2: Update `scripts/verify.sh`**

After the existing smoke binary build, add:

```bash
if [ -f "$TAURI_DIR/src/bin/smoke_export_snapshot.rs" ]; then
  echo "== Export smoke binary build =="
  cargo build --manifest-path "$TAURI_DIR/Cargo.toml" --bin smoke_export_snapshot
else
  echo "Skipping export smoke binary build: smoke_export_snapshot.rs not found yet."
fi
```

- [ ] **Step 3: Document manual verification**

Add to `tests/smoke/README.md`:

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_export_snapshot
```

Expected output includes `export_markdown=succeeded`, `export_json=succeeded`, and `export_docx=succeeded`.

- [ ] **Step 4: Run focused and full verification**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml exporter::
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::exports
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
scripts/verify.sh
```

Expected: all commands exit 0.

## Risks and Mitigations

- Domain tables may land with column names that differ from `spec/04_erd.md`. Mitigation: snapshot loader is the only place that maps table columns to export DTOs; update only that module and its seed helper.
- Manual OOXML is easy to make invalid. Mitigation: test the zipped parts, keep the Docx subset small, and render from the same `ReportDocument` used by Markdown.
- Large RFPs can produce large traceability sections. Mitigation: load only columns needed for export, keep raw JSON out of export files, and cite block/page/quote instead of embedding full `document_blocks.raw_json`.
- `검토 필요` exports can be mistaken for final output. Mitigation: Rust blocks export without `allow_review_needed`, and all formats place quality status and blocker counts near the top.
- File path permissions can fail. Mitigation: validate absolute output dirs, write atomically with `.part`, and store failed export rows with `error_message`.
- Determinism can be broken by timestamps. Mitigation: rendered content excludes wall-clock export time; Docx zip timestamps are fixed; export timestamps live only in SQLite history.

## Done When

- `exports` migration exists and migration tests pass.
- Rust can load one corrected, evidence-linked DB snapshot in deterministic order.
- Markdown, JSON, and Docx renderers produce stable output for the same snapshot.
- Every exported format includes quality gate status, blockers/warnings, traceability, and evidence citations.
- `review_needed` projects require explicit confirmation before file creation.
- Export history and `audit_events.export_created` are written on success; failed render/write attempts create failed export rows.
- Tauri commands `export_document_snapshot` and `list_document_exports` are registered and covered by tests.
- React UI exposes Markdown, JSON, and Docx export controls with Korean confirmation copy and export history.
- Focused Rust tests, frontend tests, frontend build, and `scripts/verify.sh` pass.
- `IMPLEMENTATION_LOG.md` records the `zip` dependency reason, verification commands, verification result, and remaining export risks during implementation.

## Self-Review

- Spec coverage: FR-010, ERD export history, DB snapshot export, Markdown section order, quality-gate export warning, and evidence preservation are covered by Tasks 1 through 10.
- Placeholder scan: this plan uses concrete file paths, command names, SQL, DTO names, test names, and output naming rules.
- Type consistency: Rust DTOs use camelCase serialization and TypeScript mirrors the same command payloads and response fields.

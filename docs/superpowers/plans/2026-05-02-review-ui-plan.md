# Review UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Priority 2 Korean review workbench for overview, BOM, staffing/MM, requirements, risks, and source evidence so users can inspect structured RFP analysis with 원문 근거 before export.

**Architecture:** Keep the current Tauri boundary: React renders state and interactions, while Rust owns SQLite reads and returns typed DTOs through commands. This plan is read-only for analysis data; corrections and export are separate Priority 2 plans. Evidence is shown from `document_blocks` plus `evidence_links`, not by adding a PDF renderer dependency.

**Tech Stack:** Tauri v2 commands, Rust, `rusqlite`, React 19, TypeScript, `@tauri-apps/api/core`, `lucide-react`, Vitest, Testing Library.

---

## Scope

Included:

- Review command/API DTOs for one selected document's current `rfp_project`.
- Analysis overview with business fields, quality gate, and extraction metrics.
- BOM table from `procurement_items`.
- Staffing/MM table from `staffing_requirements`.
- Requirements table from `requirements`.
- Risk table from `risk_clauses`.
- Source evidence viewer backed by `evidence_links` and neighboring `document_blocks`.
- Loading, empty, and error states consistent with the existing `RFP 분석 작업대`.
- Focused Rust command tests and frontend tests.

Out of scope:

- Writing or migrating the domain tables. This plan assumes the Priority 2 domain writer plan has added `rfp_fields`, `requirements`, `procurement_items`, `staffing_requirements`, `risk_clauses`, and `evidence_links` from `spec/04_erd.md`.
- Correction dialog and `corrections` writes.
- Markdown/JSON/Docx export.
- PDF canvas rendering, OCR overlays, or new frontend production dependencies.
- LLM settings UI.

## Source Specs

- `spec/02_prd.md`: review must separate `검토 필요` from `확정 가능` and let users click rows to inspect evidence.
- `spec/04_erd.md`: domain tables and `evidence_links` shapes.
- `spec/05_data_pipeline.md`: review is after validation and before export.
- `spec/08_ui_product_flow.md`: required screens, Korean labels, row click behavior, evidence viewer.
- `spec/09_quality_gate.md`: blocker/warning semantics and `validation_findings` source of truth.
- Current frontend files: `apps/rfp-desktop/src/App.tsx`, `apps/rfp-desktop/src/App.css`, `apps/rfp-desktop/src/lib/api.ts`, `apps/rfp-desktop/src/lib/types.ts`, `apps/rfp-desktop/src/components/*`.
- Current Rust command files: `apps/rfp-desktop/src-tauri/src/commands/*.rs`, `apps/rfp-desktop/src-tauri/src/domain.rs`, `apps/rfp-desktop/src-tauri/src/lib.rs`.

## Existing Constraints

- The app already has a dense workbench layout, Korean copy, `StatusBadge`, `QualityGate`, `DocumentList`, and a simple `BlockPreview`.
- Frontend calls Rust only through `invoke` wrappers in `apps/rfp-desktop/src/lib/api.ts`.
- Rust DTOs use `#[serde(rename_all = "camelCase")]`; TypeScript must mirror those names.
- Existing CSS uses restrained white panels, 6-8px radius, grid layout, and responsive collapse at small widths.
- Do not put SQL in the frontend. Do not introduce mock-only production behavior.

## File Structure

Create:

- `apps/rfp-desktop/src-tauri/src/commands/review.rs`: read-only review and evidence commands plus focused command tests.
- `apps/rfp-desktop/src/components/review/ReviewWorkbench.tsx`: selected-document review shell.
- `apps/rfp-desktop/src/components/review/ReviewTabs.tsx`: tab/segmented control for overview, BOM, staffing, requirements, risks.
- `apps/rfp-desktop/src/components/review/OverviewPanel.tsx`: project fields, status, counts, quality findings.
- `apps/rfp-desktop/src/components/review/ReviewDataTable.tsx`: reusable dense table wrapper.
- `apps/rfp-desktop/src/components/review/EvidenceButton.tsx`: compact evidence action for table rows.
- `apps/rfp-desktop/src/components/review/SourceEvidenceViewer.tsx`: side/bottom evidence panel with selected quote and neighboring blocks.
- `apps/rfp-desktop/src/components/review/reviewLabels.ts`: stable Korean labels and enum mappings.

Modify:

- `apps/rfp-desktop/src-tauri/src/domain.rs`: add review DTO structs.
- `apps/rfp-desktop/src-tauri/src/commands/mod.rs`: export `review`.
- `apps/rfp-desktop/src-tauri/src/lib.rs`: register `get_review_project` and `get_evidence_context`.
- `apps/rfp-desktop/src/lib/types.ts`: add mirrored review DTO types.
- `apps/rfp-desktop/src/lib/api.ts`: add `getReviewProject` and `getEvidenceContext`.
- `apps/rfp-desktop/src/App.tsx`: replace `BlockPreview` detail area with `ReviewWorkbench` while preserving document registration/extraction controls.
- `apps/rfp-desktop/src/App.css`: add review layout, tabs, tables, evidence viewer, and responsive constraints.
- `apps/rfp-desktop/src/App.test.tsx`: add review UI command mocks and interactions.

Do not modify during this plan unless the owning parent agent explicitly assigns it:

- `TASKS.md`
- `IMPLEMENTATION_LOG.md`

## Command And DTO Contract

Add two commands:

```rust
#[tauri::command]
pub fn get_review_project(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<ReviewProjectDto>
```

```rust
#[tauri::command]
pub fn get_evidence_context(
    target_table: String,
    target_id: String,
    state: State<'_, AppState>,
) -> AppResult<EvidenceContextDto>
```

Add Rust DTOs to `apps/rfp-desktop/src-tauri/src/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProjectDto {
    pub document: DocumentSummary,
    pub project: Option<ReviewProjectSummary>,
    pub overview_fields: Vec<ReviewFieldDto>,
    pub requirements: Vec<RequirementReviewRow>,
    pub procurement_items: Vec<ProcurementItemReviewRow>,
    pub staffing_requirements: Vec<StaffingReviewRow>,
    pub risk_clauses: Vec<RiskReviewRow>,
    pub findings: Vec<ValidationFindingDto>,
    pub metrics: ReviewMetricsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProjectSummary {
    pub id: String,
    pub status: String,
    pub summary: String,
    pub analysis_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFieldDto {
    pub id: String,
    pub field_key: String,
    pub label: String,
    pub raw_value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub source: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RequirementReviewRow {
    pub id: String,
    pub requirement_code: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub mandatory: bool,
    pub confidence: f64,
    pub source: String,
    pub evidence_count: i64,
    pub blocker_count: i64,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcurementItemReviewRow {
    pub id: String,
    pub item_type: String,
    pub name: String,
    pub spec: String,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub required: bool,
    pub confidence: f64,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StaffingReviewRow {
    pub id: String,
    pub role: String,
    pub grade: String,
    pub headcount: Option<f64>,
    pub mm: Option<f64>,
    pub onsite: Option<bool>,
    pub period_text: String,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RiskReviewRow {
    pub id: String,
    pub risk_type: String,
    pub severity: String,
    pub description: String,
    pub recommended_action: String,
    pub requirement_code: String,
    pub requirement_title: String,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFindingDto {
    pub id: String,
    pub severity: String,
    pub finding_type: String,
    pub message: String,
    pub target_table: Option<String>,
    pub target_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewMetricsDto {
    pub requirement_count: i64,
    pub procurement_count: i64,
    pub staffing_count: i64,
    pub total_mm: Option<f64>,
    pub high_risk_count: i64,
    pub blocker_count: i64,
    pub warning_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceContextDto {
    pub target_table: String,
    pub target_id: String,
    pub evidence: Vec<EvidenceLinkDto>,
    pub blocks: Vec<SourceBlockDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLinkDto {
    pub id: String,
    pub document_block_id: String,
    pub quote: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlockDto {
    pub id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub text: String,
    pub bbox_json: Option<String>,
    pub is_direct_evidence: bool,
}
```

Mirror the same names in `apps/rfp-desktop/src/lib/types.ts`; convert Rust `Option<T>` to `T | null`. Also add:

```ts
export type ReviewTab =
  | "overview"
  | "procurement"
  | "staffing"
  | "requirements"
  | "risks";

export interface EvidenceTarget {
  targetTable: string;
  targetId: string;
  title: string;
}
```

## SQL Query Rules

- `get_review_project` first loads `DocumentSummary` with `document_ingestion::load_document_summary`.
- If no `rfp_projects` row exists for the document, return `project: null` and empty arrays, not an error.
- Use only parameterized SQL. Do not interpolate user input into query strings.
- `get_evidence_context` must validate `target_table` against this allow-list before querying:

```rust
const EVIDENCE_TARGET_TABLES: &[&str] = &[
    "rfp_fields",
    "requirements",
    "procurement_items",
    "staffing_requirements",
    "risk_clauses",
];
```

Use this pattern for evidence counts:

```sql
LEFT JOIN (
  SELECT target_id, COUNT(*) AS evidence_count
  FROM evidence_links
  WHERE target_table = 'requirements'
  GROUP BY target_id
) evidence_counts ON evidence_counts.target_id = requirements.id
```

Use this pattern for row-specific finding counts:

```sql
LEFT JOIN (
  SELECT target_id,
         SUM(CASE WHEN severity = 'blocker' THEN 1 ELSE 0 END) AS blocker_count,
         SUM(CASE WHEN severity = 'warning' THEN 1 ELSE 0 END) AS warning_count
  FROM validation_findings
  WHERE target_table = 'requirements'
  GROUP BY target_id
) finding_counts ON finding_counts.target_id = requirements.id
```

For `get_evidence_context`, load all direct evidence rows, then load direct blocks plus two neighboring blocks on the same page:

```sql
SELECT neighbor.id,
       neighbor.page_number,
       neighbor.block_index,
       neighbor.kind,
       neighbor.text,
       neighbor.bbox_json,
       CASE WHEN neighbor.id = direct.id THEN 1 ELSE 0 END AS is_direct_evidence
FROM document_blocks direct
JOIN document_blocks neighbor
  ON neighbor.document_id = direct.document_id
 AND neighbor.page_number = direct.page_number
 AND neighbor.block_index BETWEEN direct.block_index - 2 AND direct.block_index + 2
WHERE direct.id IN (
  SELECT document_block_id
  FROM evidence_links
  WHERE target_table = ?1 AND target_id = ?2
)
ORDER BY neighbor.page_number, neighbor.block_index;
```

Deduplicate `SourceBlockDto` by `id` in Rust before returning.

## Frontend State Model

Add these state values near the existing document selection state in `App.tsx`:

```ts
const [review, setReview] = useState<ReviewProjectDto | null>(null);
const [reviewLoading, setReviewLoading] = useState(false);
const [reviewError, setReviewError] = useState<string | null>(null);
const [activeReviewTab, setActiveReviewTab] = useState<ReviewTab>("overview");
const [reviewRefreshKey, setReviewRefreshKey] = useState(0);
const [evidenceTarget, setEvidenceTarget] = useState<EvidenceTarget | null>(null);
const [evidenceContext, setEvidenceContext] = useState<EvidenceContextDto | null>(null);
const [evidenceLoading, setEvidenceLoading] = useState(false);
const [evidenceError, setEvidenceError] = useState<string | null>(null);
```

Use a request sequence guard so slow Tauri responses cannot overwrite newer selections:

```ts
const requestSeq = useRef(0);

useEffect(() => {
  if (!selectedDocument) {
    setReview(null);
    setReviewError(null);
    return;
  }

  const seq = requestSeq.current + 1;
  requestSeq.current = seq;
  setReviewLoading(true);
  setReviewError(null);

  getReviewProject(selectedDocument.id)
    .then((nextReview) => {
      if (requestSeq.current === seq) {
        setReview(nextReview);
      }
    })
    .catch((error) => {
      if (requestSeq.current === seq) {
        setReviewError(formatError(error));
      }
    })
    .finally(() => {
      if (requestSeq.current === seq) {
        setReviewLoading(false);
      }
    });
}, [selectedDocument, reviewRefreshKey]);
```

Behavior:

- Refresh/register/extract/analyze still use the existing global `pendingAction`.
- Review loading is scoped to the detail panel; document list remains usable.
- If `project` is null, show the selected document heading, quality counts, and an empty review panel with no table rows.
- If a tab has zero rows, keep the tab visible and render an empty table message:
  - BOM: `구매 항목 없음`
  - Staffing: `인력/MM 없음`
  - Requirements: `요구사항 없음`
  - Risks: `리스크 없음`
- Evidence button is disabled when `evidenceCount === 0`.
- Evidence errors are shown inside the evidence viewer, not as the global app error banner.

## Component Behavior

`ReviewWorkbench`:

- Props: `document: DocumentSummary | null`, `review: ReviewProjectDto | null`, loading/error props, and `onOpenEvidence`.
- Shows the selected document heading with `StatusBadge`.
- Reuses `QualityGate` with `{ blockerCount: review.metrics.blockerCount, warningCount: review.metrics.warningCount, blockCount: review.document.blockCount }` when review data exists, otherwise selected document counts.
- Owns tab rendering and passes rows to specific panels.

`ReviewTabs`:

- Use buttons with `aria-pressed` for the active tab.
- Labels:
  - `개요`
  - `구매 항목`
  - `인력/MM`
  - `요구사항`
  - `리스크`
- Include counts in compact text, for example `구매 항목 12`.

`OverviewPanel`:

- Show business fields in this order: `business_name`, `client`, `budget`, `period`, `contract_method`, `deadline`.
- Use `normalizedValue || rawValue || "-"`.
- Show confidence as an integer percent: `Math.round(confidence * 100) + "%"`.
- Show blocker/warning findings from `validation_findings`, ordered with blockers first.

`ReviewDataTable`:

- Render native `<table>` inside a `.review-table-scroll` wrapper.
- Use sticky header on desktop.
- Keep row height stable with `vertical-align: top`.
- Use `overflow-wrap: anywhere` for long Korean/English mixed strings.

`EvidenceButton`:

- Use `FileSearch` from `lucide-react`.
- Accessible label: `원문 근거 보기`.
- Calls `onOpenEvidence({ targetTable, targetId, title })`.

`SourceEvidenceViewer`:

- Desktop: right rail inside `.review-grid`, fixed width `360px`.
- Mobile: below the active tab, full width.
- Shows selected title, direct quote list, confidence, page/block metadata, bbox JSON if present, and neighboring block text.
- Mark direct evidence blocks with a left border and `원문` chip.
- It must still be useful when `bbox_json` is null.

## Visual And Responsive Constraints

- Keep the app as a workbench, not a landing page.
- Do not add a hero section, decorative gradients, or nested cards.
- Use the existing palette family but add restrained semantic colors only for blocker/warning/evidence highlights.
- Body `min-width` should be reduced from `920px` to `0`; use component-level min widths and horizontal table scrolling instead.
- Desktop layout:

```css
.review-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 360px;
  gap: 12px;
  min-height: 0;
}
```

- At `max-width: 980px`, collapse evidence below the table:

```css
@media (max-width: 980px) {
  .review-grid {
    grid-template-columns: 1fr;
  }

  .source-evidence-viewer {
    position: static;
  }
}
```

- Tables must support horizontal scroll without widening the whole app:

```css
.review-table-scroll {
  max-width: 100%;
  overflow-x: auto;
}

.review-table {
  border-collapse: collapse;
  min-width: 880px;
  table-layout: fixed;
  width: 100%;
}
```

## Tasks

### Task 1: Add Rust Review DTOs And Query Tests

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/domain.rs`
- Create: `apps/rfp-desktop/src-tauri/src/commands/review.rs`

- [ ] **Step 1: Add DTOs to `domain.rs`**

Add the DTO structs from the "Command And DTO Contract" section. Keep all structs `Serialize`, `Deserialize`, `Debug`, `Clone`, and `PartialEq`.

- [ ] **Step 2: Write a failing review project test**

Add this test to `commands/review.rs`:

```rust
#[test]
fn loads_review_project_with_domain_rows_and_metrics() {
    let conn = seed_review_database();

    let review = load_review_project(&conn, "doc-1").expect("load review");

    assert_eq!(review.document.id, "doc-1");
    assert_eq!(review.project.as_ref().expect("project").status, "review_needed");
    assert_eq!(review.overview_fields.len(), 2);
    assert_eq!(review.requirements[0].requirement_code, "SFR-001");
    assert_eq!(review.procurement_items[0].name, "API Gateway");
    assert_eq!(review.staffing_requirements[0].mm, Some(3.0));
    assert_eq!(review.risk_clauses[0].severity, "high");
    assert_eq!(review.metrics.requirement_count, 1);
    assert_eq!(review.metrics.procurement_count, 1);
    assert_eq!(review.metrics.staffing_count, 1);
    assert_eq!(review.metrics.total_mm, Some(3.0));
    assert_eq!(review.metrics.high_risk_count, 1);
    assert_eq!(review.metrics.blocker_count, 1);
    assert_eq!(review.metrics.warning_count, 1);
}
```

The helper `seed_review_database()` must create an in-memory database, run migrations, insert one document, one project, `rfp_fields`, one requirement, one procurement item, one staffing row, one high risk row, one evidence link per target, and blocker/warning findings.

Use this helper shape:

```rust
fn seed_review_database() -> Connection {
    let conn = Connection::open_in_memory().expect("open memory db");
    db::migrate(&conn).expect("migrate");
    conn.execute_batch(
        "
        INSERT INTO documents (id, title, created_at, updated_at, status)
        VALUES ('doc-1', 'sample.pdf', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z', 'review_needed');

        INSERT INTO source_files (id, document_id, path, file_name, mime_type, sha256, size_bytes, created_at)
        VALUES ('source-1', 'doc-1', '/tmp/sample.pdf', 'sample.pdf', 'application/pdf', 'abc', 12, '2026-05-02T00:00:00Z');

        INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at, finished_at)
        VALUES ('run-1', 'doc-1', 'opendataloader', 'fast', 'succeeded', '2026-05-02T00:00:00Z', '2026-05-02T00:00:01Z');

        INSERT INTO document_blocks (
            id, extraction_run_id, document_id, source_element_id, page_number, block_index,
            kind, heading_level, text, bbox_json, raw_json
        ) VALUES
            ('block-1', 'run-1', 'doc-1', 'el-1', 3, 1, 'paragraph', NULL, '사업 개요 문장', NULL, '{}'),
            ('block-2', 'run-1', 'doc-1', 'el-2', 3, 2, 'table', NULL, 'SFR-001 API Gateway 구성', '[72,400,540,650]', '{}'),
            ('block-3', 'run-1', 'doc-1', 'el-3', 3, 3, 'paragraph', NULL, '연계 요구사항 설명', NULL, '{}');

        INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
        VALUES ('project-1', 'doc-1', 'rfp-v2-domain-test', 'review_needed', '검토용 분석 초안', '2026-05-02T00:00:00Z', '2026-05-02T00:00:00Z');

        INSERT INTO rfp_fields (id, rfp_project_id, field_key, label, raw_value, normalized_value, confidence, source)
        VALUES
            ('field-1', 'project-1', 'business_name', '사업명', 'API 고도화 사업', 'API 고도화 사업', 0.91, 'llm'),
            ('field-2', 'project-1', 'client', '발주기관', '서울시', '서울시', 0.88, 'llm');

        INSERT INTO requirements (
            id, rfp_project_id, requirement_code, title, description, category, mandatory, confidence, source
        ) VALUES (
            'req-1', 'project-1', 'SFR-001', 'API Gateway 구성', '통합 API Gateway를 구성한다.', 'technical', 1, 0.86, 'llm'
        );

        INSERT INTO procurement_items (
            id, requirement_id, item_type, name, spec, quantity, unit, required, confidence
        ) VALUES (
            'item-1', 'req-1', 'software', 'API Gateway', 'HA 구성', 1, '식', 1, 0.82
        );

        INSERT INTO staffing_requirements (
            id, requirement_id, role, grade, headcount, mm, onsite, period_text
        ) VALUES (
            'staff-1', 'req-1', 'API 개발자', '중급', 1, 3, 1, '착수 후 3개월'
        );

        INSERT INTO risk_clauses (
            id, requirement_id, risk_type, severity, description, recommended_action
        ) VALUES (
            'risk-1', 'req-1', 'short_schedule', 'high', '구축 기간이 짧다.', '일정 버퍼와 단계 검수를 질의한다.'
        );

        INSERT INTO evidence_links (id, document_block_id, target_table, target_id, quote, confidence)
        VALUES
            ('ev-field-1', 'block-1', 'rfp_fields', 'field-1', 'API 고도화 사업', 0.9),
            ('ev-req-1', 'block-2', 'requirements', 'req-1', 'SFR-001 API Gateway 구성', 0.92),
            ('ev-item-1', 'block-2', 'procurement_items', 'item-1', 'API Gateway', 0.87),
            ('ev-staff-1', 'block-3', 'staffing_requirements', 'staff-1', '3개월', 0.8),
            ('ev-risk-1', 'block-3', 'risk_clauses', 'risk-1', '구축 기간', 0.76);

        INSERT INTO validation_findings (
            id, rfp_project_id, severity, finding_type, message, target_table, target_id, created_at
        ) VALUES
            ('finding-1', 'project-1', 'blocker', 'missing_budget', '사업예산이 추출되지 않았습니다.', 'rfp_projects', 'project-1', '2026-05-02T00:00:00Z'),
            ('finding-2', 'project-1', 'warning', 'low_confidence', '신뢰도가 낮은 항목이 있습니다.', 'requirements', 'req-1', '2026-05-02T00:00:00Z');
        ",
    )
    .expect("seed review data");
    conn
}
```

- [ ] **Step 3: Implement `load_review_project`**

Implement a private Rust function:

```rust
fn load_review_project(conn: &Connection, document_id: &str) -> AppResult<ReviewProjectDto>
```

It should return empty domain arrays with `project: None` if the document exists but has no project.

- [ ] **Step 4: Run focused test**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::review::tests::loads_review_project_with_domain_rows_and_metrics
```

Expected: PASS.

### Task 2: Add Evidence Context Command

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/commands/review.rs`

- [ ] **Step 1: Write a failing evidence context test**

Add:

```rust
#[test]
fn loads_evidence_context_with_neighbor_blocks() {
    let conn = seed_review_database();

    let context =
        load_evidence_context(&conn, "requirements", "req-1").expect("load evidence context");

    assert_eq!(context.target_table, "requirements");
    assert_eq!(context.target_id, "req-1");
    assert_eq!(context.evidence.len(), 1);
    assert!(context.blocks.iter().any(|block| block.id == "block-2" && block.is_direct_evidence));
    assert!(context.blocks.iter().any(|block| block.id == "block-1" && !block.is_direct_evidence));
    assert!(context.blocks.iter().any(|block| block.id == "block-3" && !block.is_direct_evidence));
}
```

- [ ] **Step 2: Implement target validation**

Add:

```rust
fn validate_evidence_target(target_table: &str) -> AppResult<()> {
    if EVIDENCE_TARGET_TABLES.contains(&target_table) {
        Ok(())
    } else {
        Err(AppError::InvalidInput("지원하지 않는 근거 대상입니다.".to_string()))
    }
}
```

- [ ] **Step 3: Implement `load_evidence_context`**

Implement:

```rust
fn load_evidence_context(
    conn: &Connection,
    target_table: &str,
    target_id: &str,
) -> AppResult<EvidenceContextDto>
```

Return `evidence: []` and `blocks: []` when the target has no evidence links.

- [ ] **Step 4: Run focused evidence test**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::review::tests::loads_evidence_context_with_neighbor_blocks
```

Expected: PASS.

### Task 3: Register Tauri Commands And TypeScript API

**Files:**

- Modify: `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`
- Modify: `apps/rfp-desktop/src/lib/types.ts`
- Modify: `apps/rfp-desktop/src/lib/api.ts`

- [ ] **Step 1: Export the Rust command module**

Add to `commands/mod.rs`:

```rust
pub mod review;
```

- [ ] **Step 2: Register command handlers**

Add to `tauri::generate_handler!` in `lib.rs`:

```rust
commands::review::get_review_project,
commands::review::get_evidence_context
```

- [ ] **Step 3: Add TypeScript DTOs**

Mirror the Rust DTOs in `types.ts` with `null` for optional Rust fields.

- [ ] **Step 4: Add API wrappers**

Add to `api.ts`:

```ts
export function getReviewProject(documentId: string): Promise<ReviewProjectDto> {
  return invoke<ReviewProjectDto>("get_review_project", { documentId });
}

export function getEvidenceContext(
  targetTable: string,
  targetId: string,
): Promise<EvidenceContextDto> {
  return invoke<EvidenceContextDto>("get_evidence_context", {
    targetTable,
    targetId,
  });
}
```

- [ ] **Step 5: Run Rust command tests**

Run:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::review
```

Expected: all `commands::review` tests pass.

### Task 4: Build Review Components

**Files:**

- Create: `apps/rfp-desktop/src/components/review/ReviewWorkbench.tsx`
- Create: `apps/rfp-desktop/src/components/review/ReviewTabs.tsx`
- Create: `apps/rfp-desktop/src/components/review/OverviewPanel.tsx`
- Create: `apps/rfp-desktop/src/components/review/ReviewDataTable.tsx`
- Create: `apps/rfp-desktop/src/components/review/EvidenceButton.tsx`
- Create: `apps/rfp-desktop/src/components/review/reviewLabels.ts`

- [ ] **Step 1: Add label maps**

Create `reviewLabels.ts`:

```ts
export const FIELD_LABEL_ORDER = [
  "business_name",
  "client",
  "budget",
  "period",
  "contract_method",
  "deadline",
] as const;

export const CATEGORY_LABELS: Record<string, string> = {
  functional: "기능",
  technical: "기술",
  security: "보안",
  data: "데이터",
  staffing: "인력",
  management: "관리",
  quality: "품질",
  performance: "성능",
  other: "기타",
};

export const RISK_TYPE_LABELS: Record<string, string> = {
  scope_creep: "범위 확장",
  free_work: "무상/비용 전가",
  short_schedule: "단기 일정",
  liability: "책임 과다",
  ambiguous_spec: "스펙 모호",
  vendor_lock: "특정 업체 유리",
  payment: "지급/검수 위험",
  security: "보안/개인정보 위험",
  other: "기타",
};
```

- [ ] **Step 2: Add `EvidenceButton`**

It should render a 32px icon button with `FileSearch` and disabled state when `evidenceCount === 0`.

- [ ] **Step 3: Add `ReviewDataTable`**

Implement a typed wrapper that receives `caption`, `emptyMessage`, `headers`, and `children`, and renders a scrollable table with an empty row when no children are provided.

- [ ] **Step 4: Add overview and tab components**

`OverviewPanel` renders ordered fields, `ReviewTabs` renders active-tab buttons with row counts.

- [ ] **Step 5: Add `ReviewWorkbench`**

Render:

- document heading and `StatusBadge`
- `QualityGate`
- tabs
- active tab's table
- `SourceEvidenceViewer` receives the current evidence target, context, loading state, and error state from `App.tsx`.

### Task 5: Build Source Evidence Viewer

**Files:**

- Create: `apps/rfp-desktop/src/components/review/SourceEvidenceViewer.tsx`
- Modify: `apps/rfp-desktop/src/components/review/ReviewWorkbench.tsx`

- [ ] **Step 1: Add evidence target type in `types.ts`**

```ts
export interface EvidenceTarget {
  targetTable: string;
  targetId: string;
  title: string;
}
```

- [ ] **Step 2: Implement viewer rendering**

`SourceEvidenceViewer` props:

```ts
interface SourceEvidenceViewerProps {
  target: EvidenceTarget | null;
  context: EvidenceContextDto | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}
```

Render `원문 근거`, selected target title, quote list, block list, page/block index, and bbox JSON.

- [ ] **Step 3: Wire evidence actions**

Every overview field and table row uses:

```ts
onOpenEvidence({
  targetTable: "requirements",
  targetId: row.id,
  title: `${row.requirementCode} ${row.title}`,
});
```

Use the matching table name for each row: `rfp_fields`, `requirements`, `procurement_items`, `staffing_requirements`, `risk_clauses`.

- [ ] **Step 4: Keep no-evidence rows clear**

Rows with `evidenceCount === 0` show disabled evidence buttons and do not call the command.

### Task 6: Integrate Review State Into `App.tsx`

**Files:**

- Modify: `apps/rfp-desktop/src/App.tsx`
- Modify: `apps/rfp-desktop/src/App.css`

- [ ] **Step 1: Import review API and component**

Replace `BlockPreview` usage with `ReviewWorkbench`.

- [ ] **Step 2: Add review/evidence state**

Use the state model from "Frontend State Model". Keep the existing registration, diagnostic, and analyze actions unchanged.

- [ ] **Step 3: Refresh review after analysis**

After `analyzeDocumentBaseline(selectedDocument.id)` and `refreshDocuments()`, call:

```ts
setReviewRefreshKey((value) => value + 1);
```

The selected-document effect refetches review data because `reviewRefreshKey` is in its dependency array.

- [ ] **Step 4: Preserve existing empty document behavior**

When no document is selected, the detail panel shows `문서 없음` and quality state `품질 상태 없음`.

- [ ] **Step 5: Add CSS**

Add classes for `.review-grid`, `.review-tabs`, `.review-table-scroll`, `.review-table`, `.source-evidence-viewer`, `.source-block--direct`, `.evidence-button`, `.review-empty`, and responsive collapse at `980px`.

### Task 7: Add Frontend Tests

**Files:**

- Modify: `apps/rfp-desktop/src/App.test.tsx`

- [ ] **Step 1: Extend Tauri mock**

Handle these commands:

```ts
if (command === "get_review_project") {
  return Promise.resolve(reviewFixture);
}

if (command === "get_evidence_context") {
  return Promise.resolve(evidenceFixture);
}
```

- [ ] **Step 2: Add review fixture**

Fixture must include:

- one `business_name` overview field
- one requirement `SFR-001`
- one procurement item
- one staffing row with `mm: 3`
- one high risk
- one blocker and one warning finding

- [ ] **Step 3: Test default overview**

Assert the first screen still shows `RFP 분석 작업대`, the selected document, `개요`, business name, blocker count, and warning count.

- [ ] **Step 4: Test tab navigation**

Click `구매 항목`, `인력/MM`, `요구사항`, and `리스크`; assert the expected row text is visible for each tab.

- [ ] **Step 5: Test evidence viewer**

Click an enabled evidence button and assert `원문 근거`, the direct quote, page number, and neighboring block text are visible.

- [ ] **Step 6: Run frontend tests**

Run:

```bash
npm run test --prefix apps/rfp-desktop
```

Expected: PASS.

### Task 8: Verification And Finish

**Files:**

- No new files beyond prior tasks.

- [ ] **Step 1: Run focused Rust tests**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::review
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

```bash
npm run test --prefix apps/rfp-desktop
```

Expected: PASS.

- [ ] **Step 3: Run frontend build**

```bash
npm run build --prefix apps/rfp-desktop
```

Expected: PASS.

- [ ] **Step 4: Run full repository verification**

```bash
scripts/verify.sh
```

Expected: PASS.

- [ ] **Step 5: Manual UI check**

Run:

```bash
npm run tauri dev --prefix apps/rfp-desktop
```

Check:

- Document list still loads.
- Selected document shows review tabs.
- Each tab can be selected.
- Evidence viewer opens from a row with evidence.
- Empty evidence does not call the evidence command.
- At narrow width, tables scroll horizontally and evidence moves below the table.

## Risks And Mitigations

- Domain schema may differ from the ERD when the domain writer plan lands. Mitigation: implement review commands after the domain writer branch is integrated, and keep the DTO contract stable in this plan.
- Dynamic evidence targets can become SQL injection if implemented with interpolated table names. Mitigation: use a hard allow-list and parameterized `target_table`/`target_id` values.
- Large RFPs can make tables slow. Mitigation: fetch the current project's review snapshot once, render native tables, and avoid per-row evidence fetch until the user opens a row.
- Text can overflow mixed Korean/English technical specs. Mitigation: fixed table layout, horizontal scroll, and `overflow-wrap: anywhere`.
- Bbox JSON may be absent or use variant coordinate shapes. Mitigation: display bbox as optional metadata and rely on page/block text as the primary source viewer.
- Parallel Priority 2 workers may touch the same API/type files. Mitigation: coordinate integration centrally; do not revert other workers' changes.

## Done When

- `get_review_project` returns document, project, overview fields, requirements, procurement items, staffing rows, risk clauses, findings, and metrics from SQLite.
- `get_evidence_context` returns direct evidence links and neighboring blocks for allowed target tables.
- React review tabs show overview, BOM, staffing/MM, requirements, and risk rows for the selected document.
- Evidence viewer opens from rows with evidence and remains usable without bbox data.
- Loading, empty, disabled, and error states are visible and scoped to the correct panel.
- Responsive layout works on desktop and narrow widths without overlapping text or forcing the whole body wider than the viewport.
- Focused Rust tests, frontend tests, frontend build, and `scripts/verify.sh` pass.

## Self-Review Checklist

- Spec coverage: overview, BOM, staffing/MM, requirements, risk, quality gate, and source evidence viewer are covered.
- Red-flag scan: each table name comes either from the existing app or from the ERD-backed domain writer dependency.
- Type consistency: Rust DTO names use `camelCase` in serde and matching TypeScript names.
- Scope control: no correction writes, export generation, PDF renderer, or LLM settings are included.

# TASKS.md

## Status Values

- `[ ]` Not started
- `[~]` In progress
- `[x]` Done
- `[!]` Blocked

## Operating Rule

Work continuously through Priority 1 until every task is `[x]` or a blocker from `AGENTS.md` occurs. Do not ask the user to say "continue" between tasks.

Use `docs/superpowers/plans/2026-05-01-tauri-rfp-v2-vertical-slice.md` as the detailed step-by-step source of truth.

## Priority 1: Tauri RFP v2 Vertical Slice

### [x] 0. Initialize repository tracking

Done when:

- Git tracking is initialized if missing.
- `.gitignore` exists and excludes generated app/build/database artifacts.
- The repository can report `git status --short`.

Primary files:

- `.gitignore`

Verification:

```bash
git status --short
```

### [x] 1. Scaffold Tauri React app

Done when:

- `apps/rfp-desktop/` exists.
- Tauri v2 + React + TypeScript scaffold exists.
- Frontend and Rust dependencies are installed.
- Initial frontend build passes.
- Initial Rust tests pass.

Primary files:

- `apps/rfp-desktop/package.json`
- `apps/rfp-desktop/src/`
- `apps/rfp-desktop/src-tauri/`

Verification:

```bash
npm run build --prefix apps/rfp-desktop
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
```

### [x] 2. Add SQLite schema and migration runner

Done when:

- Core SQLite migration exists.
- Migration runner opens the app database and enables foreign keys.
- Tables exist for documents, source files, extraction runs, document blocks, RFP projects, validation findings, and audit events.
- Focused migration test passes.

Primary files:

- `apps/rfp-desktop/src-tauri/migrations/0001_core.sql`
- `apps/rfp-desktop/src-tauri/src/db/mod.rs`
- `apps/rfp-desktop/src-tauri/src/error.rs`
- `apps/rfp-desktop/src-tauri/src/state.rs`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_core_tables
```

### [x] 3. Implement PDF document registration

Done when:

- A local PDF path can be registered.
- SHA-256, file name, MIME type, size, source path, and audit event are saved.
- Duplicate file hashes return the existing document summary.
- Tauri commands exist for registering and listing documents.
- Focused registration test passes.

Primary files:

- `apps/rfp-desktop/src-tauri/src/domain.rs`
- `apps/rfp-desktop/src-tauri/src/document_ingestion/mod.rs`
- `apps/rfp-desktop/src-tauri/src/commands/documents.rs`
- `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml document_ingestion::tests::register_document_creates_source_file_and_audit_event
```

### [x] 4. Implement OpenDataLoader diagnostics and fast extraction

Parallel-safe after Task 3.

Done when:

- OpenDataLoader CLI diagnostic reports CLI and Java availability.
- Fast mode runs `opendataloader-pdf` with bounded explicit args.
- Extraction run logs stdout/stderr, status, JSON path, and Markdown path.
- Failed extraction preserves run state and marks document failed.
- Focused adapter test passes.

Primary files:

- `apps/rfp-desktop/src-tauri/src/opendataloader_adapter/mod.rs`
- `apps/rfp-desktop/src-tauri/src/commands/extraction.rs`
- `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml opendataloader_adapter::tests::fast_mode_args_are_bounded_and_explicit
```

### [x] 5. Normalize OpenDataLoader JSON blocks

Parallel-safe after Task 3.

Done when:

- JSON key variants are accepted for text, kind, page, bbox, and nested elements.
- Normalized rows are inserted into `document_blocks`.
- Raw JSON is preserved.
- Fixture test passes.

Primary files:

- `fixtures/opendataloader/sample-output.json`
- `apps/rfp-desktop/src-tauri/src/block_normalizer/mod.rs`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml block_normalizer::tests::normalizes_key_variants_and_nested_elements
```

### [x] 6. Add baseline analysis and validation gate

Parallel-safe after Task 3.

Done when:

- Baseline `rfp_project` can be created without LLM.
- MVP blockers and warnings are inserted into `validation_findings`.
- Document and project status become `review_needed` when blockers exist.
- Focused baseline validation test passes.

Primary files:

- `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`
- `apps/rfp-desktop/src-tauri/src/validation/mod.rs`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests::baseline_analysis_creates_review_needed_project_and_blockers
```

### [x] 7. Orchestrate first analysis pipeline

Done when:

- A command can run baseline analysis for a registered document.
- The pipeline summary reports ready/review_needed/failed counts.
- Focused orchestration test passes.

Primary files:

- `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`
- `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::pipeline::tests::summarize_document_reports_review_needed_after_blocks_and_validation
```

### [x] 8. Build first-screen RFP workbench UI

Parallel-safe after Task 3 if API DTO names are stable.

Done when:

- The first screen is a Korean workbench, not a landing page.
- User can enter an absolute PDF path, register it, diagnose OpenDataLoader, and run extraction/analysis.
- Document list, status badge, blocker count, warning count, and block count are visible.
- UI test and frontend build pass.

Primary files:

- `apps/rfp-desktop/src/lib/types.ts`
- `apps/rfp-desktop/src/lib/api.ts`
- `apps/rfp-desktop/src/components/StatusBadge.tsx`
- `apps/rfp-desktop/src/components/QualityGate.tsx`
- `apps/rfp-desktop/src/components/DocumentList.tsx`
- `apps/rfp-desktop/src/components/BlockPreview.tsx`
- `apps/rfp-desktop/src/App.tsx`
- `apps/rfp-desktop/src/styles.css`
- `apps/rfp-desktop/src/App.test.tsx`
- `apps/rfp-desktop/vitest.config.ts`

Verification:

```bash
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
```

### [x] 9. Add real PDF smoke command

Done when:

- `smoke_first_pdf` binary registers one real PDF, runs extraction, normalizes blocks, creates baseline project, and reports quality counts.
- Smoke exit codes separate success, failed execution, and generated-with-blockers.
- Smoke binary compiles.

Primary files:

- `tests/smoke/README.md`
- `apps/rfp-desktop/src-tauri/src/bin/smoke_first_pdf.rs`
- `apps/rfp-desktop/src-tauri/Cargo.toml`
- `apps/rfp-desktop/src-tauri/src/lib.rs`

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
cargo build --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf
```

Optional real PDF verification:

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- /absolute/path/to/rfp.pdf
```

### [x] 10. Final verification checkpoint

Done when:

- Rust tests pass.
- Frontend tests pass.
- Frontend build passes.
- Tauri dev app opens to `RFP 분석 작업대`.
- Real PDF smoke creates document, extraction, block, project, and finding rows.
- Generated count and quality status are reported separately.

Primary files:

- No required new files.

Verification:

```bash
scripts/verify.sh
```

Manual verification:

```bash
npm run tauri dev --prefix apps/rfp-desktop
```

Real PDF smoke result:

- `opendataloader-pdf` is installed at `/Users/doublejun_air/.local/bin/opendataloader-pdf`.
- Verified with `rfp/rfp_bundle/05_AI/18_월드비전_AI서비스_플랫폼_구축_제안요청서.pdf`.
- The smoke command created document, extraction, block, project, and finding rows.
- The smoke command returned exit code 2 because validation blockers were intentionally reported separately from execution failure.

## Priority 2: Next Plans After Vertical Slice

### [x] 11. Candidate extractor plan

Done when:

- A new plan exists for `rfp_fields` and candidate bundles from `document_blocks`.

Plan:

- `docs/superpowers/plans/2026-05-02-candidate-extractor-plan.md`

### [x] 12. LLM adapter plan

Done when:

- A new plan exists for OpenAI/Gemini structured output, schema validation, and `llm_runs`.

Plan:

- `docs/superpowers/plans/2026-05-02-llm-adapter-plan.md`

### [x] 13. Domain writer plan

Done when:

- A new plan exists for requirements, procurement, staffing, deliverables, acceptance, risks, and evidence links.

Plan:

- `docs/superpowers/plans/2026-05-02-domain-writer-plan.md`

### [x] 14. Review UI plan

Done when:

- A new plan exists for overview, BOM, staffing/MM, requirements, risk, and source evidence viewer.

Plan:

- `docs/superpowers/plans/2026-05-02-review-ui-plan.md`

### [x] 15. Export plan

Done when:

- A new plan exists for Markdown, JSON, and Docx export from DB snapshots.

Plan:

- `docs/superpowers/plans/2026-05-02-export-plan.md`

## Priority 2: Candidate Extractor Implementation

### [x] 16. Implement candidate extractor vertical slice

Done when:

- SQLite migration exists for `rfp_fields`, `evidence_links`, and `candidate_bundles`.
- Candidate scoring creates seven deterministic bundle keys from `document_blocks`.
- Rule extraction writes project-info `rfp_fields` and one evidence link per field.
- Candidate validation removes found project-info blockers while keeping `zero_requirements`.
- Tauri exposes `analyze_document_candidates`.
- Frontend runs candidate analysis and displays 기본정보 plus candidate bundle counts.
- Smoke output reports `field_count`, `candidate_bundle_count`, and `field_evidence_count`.
- Repository verification and real PDF smoke complete.

Verification:

```bash
scripts/verify.sh
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- "rfp/rfp_bundle/05_AI/18_월드비전_AI서비스_플랫폼_구축_제안요청서.pdf"
```

## Priority 2: Domain Writer Implementation

### [x] 17. Implement domain writer vertical slice

Done when:

- SQLite migration exists for `requirements`, `procurement_items`, `staffing_requirements`, `deliverables`, `acceptance_criteria`, and `risk_clauses`.
- Existing `rfp_fields` and `evidence_links` schema is widened for domain writer use, with a legacy-schema repair path.
- `DomainDraft` DTOs and `domain_writer` boundary exist.
- `write_domain_draft` transactionally persists fields, requirements, procurement, staffing/MM, deliverables, acceptance criteria, risks, and evidence links.
- Same-document evidence validation rejects rows without usable block evidence.
- Numeric quantity, headcount, MM, and onsite values are normalized locally from source text.
- Orphan child rows can create deterministic generated requirements.
- Domain-aware validation can mark complete evidenced domain rows `ready` and missing required rows `review_needed`.
- Baseline/candidate re-analysis clears stale durable domain rows.
- Smoke output reports domain row and domain evidence counts.
- Focused Rust tests, repository verification, frontend tests/build, and real PDF smoke complete.

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml domain_writer
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml validation::tests
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests
scripts/verify.sh
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- "rfp/rfp_bundle/05_AI/18_월드비전_AI서비스_플랫폼_구축_제안요청서.pdf"
```

## Priority 2: LLM Adapter Implementation

### [x] 18. Implement LLM adapter vertical slice

Done when:

- SQLite migration exists for `llm_settings` and `llm_runs`, with default disabled/offline settings.
- Rust LLM contracts exist for candidate-only input envelopes, schema names, provider settings, and run summaries.
- JSON Schemas exist for project info, requirements, procurement/domain arrays, and risk classification.
- Schema validation and evidence validation reject malformed outputs, empty evidence arrays, and unknown candidate block IDs.
- API keys are stored through an OS keychain boundary with environment-variable fallback and are not persisted in SQLite plaintext.
- OpenAI and Gemini provider adapters build structured-output requests through fake transports without leaking API keys into request snapshots.
- Runner persists `succeeded`, `failed`, and `rejected` `llm_runs`, retries transient provider statuses, and records schema/evidence rejection findings.
- Candidate bundles can be converted into provider input envelopes without file paths or raw JSON.
- Validated provider outputs can be converted into `DomainDraft` and connected to a Tauri LLM domain analysis command.
- Frontend DTO/API wrappers compile for settings, single-schema LLM runs, and LLM domain analysis.
- Smoke output reports default LLM disabled/offline state and run count.
- Focused Rust tests, full Rust tests, repository verification, frontend tests/build, secret scan, and real PDF smoke complete.

Verification:

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_llm_tables_and_default_settings
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::llm::tests
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
scripts/verify.sh
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- "rfp/rfp_bundle/05_AI/18_월드비전_AI서비스_플랫폼_구축_제안요청서.pdf"
```

Secret scan:

- Real provider key prefixes were scanned across source, migrations, smoke docs, and task/log docs.
- Only fake test literals were present.

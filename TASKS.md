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

### [!] 10. Final verification checkpoint

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

Blocked on:

- No local real RFP PDF fixture was found under this repository.
- `opendataloader-pdf` is not currently available in `PATH`.

## Priority 2: Next Plans After Vertical Slice

### [ ] 11. Candidate extractor plan

Done when:

- A new plan exists for `rfp_fields` and candidate bundles from `document_blocks`.

### [ ] 12. LLM adapter plan

Done when:

- A new plan exists for OpenAI/Gemini structured output, schema validation, and `llm_runs`.

### [ ] 13. Domain writer plan

Done when:

- A new plan exists for requirements, procurement, staffing, deliverables, acceptance, risks, and evidence links.

### [ ] 14. Review UI plan

Done when:

- A new plan exists for overview, BOM, staffing/MM, requirements, risk, and source evidence viewer.

### [ ] 15. Export plan

Done when:

- A new plan exists for Markdown, JSON, and Docx export from DB snapshots.

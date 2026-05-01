# First RFP PDF Smoke

Use one real RFP PDF from the v1 bundle or another local fixture that can remain outside git.

Command:

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- /absolute/path/to/rfp.pdf
```

Expected report fields:

- document_id
- extraction_status
- document_blocks
- generated_count
- field_count
- candidate_bundle_count
- field_evidence_count
- ready_count
- review_needed_count
- failed_count
- blocker_count
- warning_count

Exit code:

- 0 when all documents are `ready`, or only allowed warnings remain.
- 1 when registration, extraction, normalization, or analysis execution fails.
- 2 when generation succeeds but blockers remain.

Candidate extractor expectations:

```text
field_count=<number of extracted project info fields>
candidate_bundle_count=7
field_evidence_count=<number of extracted fields with evidence>
```

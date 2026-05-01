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
- ready_count
- review_needed_count
- failed_count
- blocker_count
- warning_count

Exit code:

- 0 when the document reaches `ready`.
- 2 when rows are generated but blockers remain.
- 1 when registration, extraction, normalization, or analysis fails.


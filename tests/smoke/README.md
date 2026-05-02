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
- requirement_count
- procurement_item_count
- staffing_requirement_count
- deliverable_count
- acceptance_criteria_count
- risk_clause_count
- domain_evidence_count
- llm_enabled
- llm_offline_mode
- llm_provider
- llm_run_count
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

Domain writer expectations:

```text
requirement_count=<stored durable requirements>
procurement_item_count=<stored procurement rows>
staffing_requirement_count=<stored staffing/MM rows>
deliverable_count=<stored deliverable rows>
acceptance_criteria_count=<stored acceptance rows>
risk_clause_count=<stored risk rows>
domain_evidence_count=<evidence links for durable domain rows>
```

The current candidate-only smoke path may report zero for the domain writer counts until
a deterministic candidate-to-`DomainDraft` adapter or an opt-in LLM domain analysis command feeds
the writer.

LLM adapter expectations:

- Default `llm_settings` are disabled and offline.
- Normal `scripts/verify.sh` and this smoke command do not require OpenAI or Gemini keys.
- API keys must be supplied through the settings command/keychain or environment variables; they are not stored in SQLite plaintext.
- Live provider roundtrips are covered by ignored Rust tests and should only be run with explicit `OPENAI_API_KEY` or `GEMINI_API_KEY` environment variables.

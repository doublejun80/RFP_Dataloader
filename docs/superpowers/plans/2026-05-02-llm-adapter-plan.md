# LLM Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in OpenAI/Gemini structured-output adapter that sends only candidate block text and block IDs, validates schema/evidence, records `llm_runs`, and preserves the offline rule-baseline path.

**Architecture:** Rust owns settings, secret lookup, prompt/schema construction, provider HTTP calls, SQLite persistence, and validation. The adapter writes raw structured responses to `llm_runs` and returns validated draft DTOs; domain table writes stay in the separate domain-writer plan.

**Tech Stack:** Tauri v2 commands, Rust, SQLite via `rusqlite`, `reqwest` with rustls, `jsonschema`, `keyring`, `serde`, provider transport fakes for tests, React/TypeScript DTOs only.

---

## Scope

Included:
- SQLite migration support for `llm_runs` and non-secret `llm_settings`.
- Provider-neutral input envelope, schema names, prompt versions, request snapshots, response DTOs, and run summaries.
- OpenAI Responses API structured output adapter.
- Gemini API structured output adapter.
- JSON Schema validation and evidence block validation before anything can be handed to domain writing.
- Retry and status handling for transient failures, provider refusals, malformed output, and schema/evidence rejection.
- Secure API key strategy using OS keychain first and environment variables as a development fallback.
- Offline/default-disabled behavior that never opens the network and keeps baseline analysis usable.
- Focused Rust tests, TypeScript DTO/API build checks, and optional live-provider smoke commands.

Out of scope:
- Candidate selection heuristics. This plan consumes candidate bundles created by the Priority 2 candidate extractor plan.
- Writing `rfp_fields`, `requirements`, `procurement_items`, `staffing_requirements`, `deliverables`, `acceptance_criteria`, `risk_clauses`, and `evidence_links`. That belongs to the domain-writer plan.
- Full review UI. This plan only adds frontend API/type contracts needed by a later settings/review screen.
- Markdown/JSON/Docx export.

## Source Specs

- `spec/02_prd.md`: LLM is explicit opt-in, sends candidate text only, and must not store API keys in SQLite plaintext.
- `spec/03_architecture.md`: Rust command boundary owns LLM calls; schema failures preserve raw output but block domain writes.
- `spec/04_erd.md`: `llm_runs` belongs under `extraction_runs`.
- `spec/05_data_pipeline.md`: LLM consumes candidate bundles and stores responses before validation/domain writes.
- `spec/06_llm_contract.md`: provider list, input envelope, output schemas, refusal/schema mismatch rules.
- `spec/09_quality_gate.md`: `schema_invalid`, `missing_evidence`, and `llm_not_used` quality outcomes.
- `TASKS.md`: Priority 2 Task 12 requires a plan for OpenAI/Gemini structured output, schema validation, and `llm_runs`.

## External References Checked

- OpenAI Structured Outputs: <https://platform.openai.com/docs/guides/structured-outputs>
- Gemini structured output: <https://ai.google.dev/gemini-api/docs/structured-output>

Provider APIs can change. Keep live request shapes isolated in provider modules and covered by request-shape unit tests so drift is easy to spot.

## File Structure

Create or modify these files during implementation:

```text
apps/rfp-desktop/
├─ package.json
├─ src/
│  └─ lib/
│     ├─ api.ts
│     └─ types.ts
└─ src-tauri/
   ├─ Cargo.toml
   ├─ migrations/
   │  ├─ 0001_core.sql
   │  └─ 0002_llm.sql
   └─ src/
      ├─ commands/
      │  ├─ llm.rs
      │  └─ mod.rs
      ├─ db/
      │  └─ mod.rs
      ├─ llm_adapter/
      │  ├─ contracts.rs
      │  ├─ gemini.rs
      │  ├─ http.rs
      │  ├─ mod.rs
      │  ├─ openai.rs
      │  ├─ prompts.rs
      │  ├─ runner.rs
      │  ├─ schema_validation.rs
      │  ├─ schemas.rs
      │  └─ settings.rs
      ├─ domain.rs
      ├─ error.rs
      └─ lib.rs
```

Responsibilities:
- `migrations/0002_llm.sql`: `llm_runs`, `llm_settings`, and indexes.
- `db/mod.rs`: run ordered migrations and test new tables.
- `llm_adapter/contracts.rs`: provider-neutral DTOs for input envelopes, schema names, output structs, and run summaries.
- `llm_adapter/schemas.rs`: JSON Schema values matching `spec/06_llm_contract.md`.
- `llm_adapter/prompts.rs`: prompt version constants and deterministic system/user prompt builders.
- `llm_adapter/schema_validation.rs`: JSON Schema validation plus evidence block membership checks.
- `llm_adapter/settings.rs`: non-secret settings persistence, keychain writes, env fallback, offline/default-disabled policy.
- `llm_adapter/http.rs`: injectable transport trait and production `reqwest` transport.
- `llm_adapter/openai.rs`: OpenAI request/response mapping.
- `llm_adapter/gemini.rs`: Gemini request/response mapping.
- `llm_adapter/runner.rs`: orchestration, retries, `llm_runs` persistence, and command-facing summaries.
- `commands/llm.rs`: Tauri commands for settings and running structured extraction.
- `domain.rs`, `types.ts`, `api.ts`: command DTOs only; no provider logic.

## Data Contracts

### Candidate Input

The LLM adapter consumes this Rust DTO from the candidate extractor. Tests can build it directly until the candidate extractor plan is implemented.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmInputEnvelope {
    pub document_id: String,
    pub rfp_project_id: String,
    pub extraction_run_id: String,
    pub language: String,
    pub candidate_blocks: Vec<CandidateBlock>,
    pub instructions: LlmInstructions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBlock {
    pub block_id: String,
    pub page_number: i64,
    pub kind: String,
    pub text: String,
    pub bbox: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmInstructions {
    pub preserve_korean_terms: bool,
    pub do_not_invent_values: bool,
    pub require_evidence_block_ids: bool,
}
```

Rules:
- `candidate_blocks` must not include source PDF paths, local output paths, API keys, or full raw JSON.
- Empty `text` blocks are excluded before provider calls.
- `bbox` can be included because it helps evidence review and is already derived from OpenDataLoader blocks.
- If `candidate_blocks` is empty, skip provider calls and keep baseline validation.

### Schema Names

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmSchemaName {
    ProjectInfo,
    Requirements,
    Procurement,
    RiskClassification,
}

impl LlmSchemaName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectInfo => "project_info",
            Self::Requirements => "requirements",
            Self::Procurement => "procurement",
            Self::RiskClassification => "risk_classification",
        }
    }
}
```

`ProjectInfo`, `Requirements`, and `Procurement` map directly to the schemas in `spec/06_llm_contract.md`. `RiskClassification` uses the `risk_clauses` item shape from the procurement schema wrapped as:

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["risk_clauses"],
  "properties": {
    "risk_clauses": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "risk_type", "severity", "description", "recommended_action", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "risk_type": { "type": "string", "enum": ["scope_creep", "free_work", "short_schedule", "liability", "ambiguous_spec", "vendor_lock", "payment", "security", "other"] },
          "severity": { "type": "string", "enum": ["low", "medium", "high", "blocker"] },
          "description": { "type": "string" },
          "recommended_action": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    }
  }
}
```

### Run Summary DTO

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmRunSummary {
    pub id: String,
    pub extraction_run_id: String,
    pub provider: String,
    pub model: String,
    pub schema_name: String,
    pub prompt_version: String,
    pub status: String,
    pub input_token_count: i64,
    pub output_token_count: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}
```

## Task 1: Add LLM Migration and Ordered Migration Runner

**Files:**
- Create: `apps/rfp-desktop/src-tauri/migrations/0002_llm.sql`
- Modify: `apps/rfp-desktop/src-tauri/src/db/mod.rs`

- [ ] **Step 1: Add migration test first**

Add this test to `apps/rfp-desktop/src-tauri/src/db/mod.rs`:

```rust
#[test]
fn migrates_llm_tables_and_default_settings() {
    let conn = Connection::open_in_memory().expect("open memory db");

    migrate(&conn).expect("run migrations");

    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                'llm_runs',
                'llm_settings'
            )",
            [],
            |row| row.get(0),
        )
        .expect("count tables");
    assert_eq!(table_count, 2);

    let (enabled, offline_mode): (i64, i64) = conn
        .query_row(
            "SELECT enabled, offline_mode FROM llm_settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("default settings");
    assert_eq!(enabled, 0);
    assert_eq!(offline_mode, 1);
}
```

- [ ] **Step 2: Run the focused test and confirm failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_llm_tables_and_default_settings
```

Expected: fails because `0002_llm.sql` and ordered migration loading do not exist.

- [ ] **Step 3: Create `0002_llm.sql`**

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS llm_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  offline_mode INTEGER NOT NULL DEFAULT 1 CHECK (offline_mode IN (0, 1)),
  provider TEXT NOT NULL DEFAULT 'openai' CHECK (provider IN ('openai', 'gemini')),
  model TEXT NOT NULL DEFAULT '',
  api_key_ref TEXT,
  updated_at TEXT NOT NULL
);

INSERT INTO llm_settings (
  id, enabled, offline_mode, provider, model, api_key_ref, updated_at
) VALUES (
  1, 0, 1, 'openai', '', NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(id) DO NOTHING;

CREATE TABLE IF NOT EXISTS llm_runs (
  id TEXT PRIMARY KEY,
  extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('openai', 'gemini')),
  model TEXT NOT NULL,
  schema_name TEXT NOT NULL CHECK (schema_name IN ('project_info', 'requirements', 'procurement', 'risk_classification')),
  prompt_version TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'rejected')),
  input_token_count INTEGER NOT NULL DEFAULT 0,
  output_token_count INTEGER NOT NULL DEFAULT 0,
  request_json TEXT NOT NULL,
  response_json TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_llm_runs_extraction_schema
  ON llm_runs(extraction_run_id, schema_name, created_at);

CREATE INDEX IF NOT EXISTS idx_llm_runs_status
  ON llm_runs(status);
```

- [ ] **Step 4: Change migration runner to ordered migrations**

Replace the single `CORE_MIGRATION` constant with:

```rust
const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_core.sql"),
    include_str!("../../migrations/0002_llm.sql"),
];

pub fn migrate(conn: &Connection) -> AppResult<()> {
    for migration in MIGRATIONS {
        conn.execute_batch(migration)?;
    }
    Ok(())
}
```

- [ ] **Step 5: Verify migration tests**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_core_tables
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_llm_tables_and_default_settings
```

Expected: both tests pass.

## Task 2: Add LLM Dependencies and Error Variants

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/Cargo.toml`
- Modify: `apps/rfp-desktop/src-tauri/src/error.rs`

- [ ] **Step 1: Add focused dependencies**

Run:

```bash
cd apps/rfp-desktop/src-tauri
cargo add reqwest --features json,rustls-tls --no-default-features
cargo add jsonschema
cargo add keyring
```

Reason:
- `reqwest`: provider HTTPS calls.
- `jsonschema`: local schema validation independent of provider claims.
- `keyring`: OS keychain storage for API keys, avoiding SQLite plaintext secrets.

- [ ] **Step 2: Add LLM errors**

Extend `AppError`:

```rust
#[error("llm disabled: {0}")]
LlmDisabled(String),
#[error("llm provider error: {0}")]
LlmProvider(String),
#[error("llm schema rejected: {0}")]
LlmRejected(String),
#[error("secret storage error: {0}")]
Secret(String),
```

Extend `AppErrorDto` with matching variants:

```rust
LlmDisabled(String),
LlmProvider(String),
LlmRejected(String),
Secret(String),
```

Add match arms in `impl Serialize for AppError`:

```rust
AppError::LlmDisabled(message) => AppErrorDto::LlmDisabled(message.clone()),
AppError::LlmProvider(message) => AppErrorDto::LlmProvider(message.clone()),
AppError::LlmRejected(message) => AppErrorDto::LlmRejected(message.clone()),
AppError::Secret(message) => AppErrorDto::Secret(message.clone()),
```

- [ ] **Step 3: Verify crate compiles**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --no-run
```

Expected: compile succeeds.

## Task 3: Define Contracts, Schemas, and Prompts

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/contracts.rs`
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/schemas.rs`
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/prompts.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create module root**

```rust
pub mod contracts;
pub mod prompts;
pub mod schemas;
```

Add to `lib.rs`:

```rust
pub mod llm_adapter;
```

- [ ] **Step 2: Implement contracts**

Create the DTOs from the Data Contracts section plus these provider/result structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    #[serde(rename = "openai")]
    OpenAi,
    Gemini,
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStructuredResponse {
    pub output_json: serde_json::Value,
    pub raw_response_json: serde_json::Value,
    pub input_token_count: i64,
    pub output_token_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmRunSummary {
    pub id: String,
    pub extraction_run_id: String,
    pub provider: String,
    pub model: String,
    pub schema_name: String,
    pub prompt_version: String,
    pub status: String,
    pub input_token_count: i64,
    pub output_token_count: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}
```

- [ ] **Step 3: Implement schema builders**

Expose:

```rust
pub fn schema_for(schema_name: LlmSchemaName) -> serde_json::Value {
    match schema_name {
        LlmSchemaName::ProjectInfo => project_info_schema(),
        LlmSchemaName::Requirements => requirements_schema(),
        LlmSchemaName::Procurement => procurement_schema(),
        LlmSchemaName::RiskClassification => risk_classification_schema(),
    }
}
```

The schema contents must match `spec/06_llm_contract.md`:
- every object sets `"additionalProperties": false`;
- every schema uses the documented `required` arrays;
- confidence fields use `"minimum": 0` and `"maximum": 1`;
- enum values match the spec exactly.

- [ ] **Step 4: Implement prompts**

```rust
pub const PROMPT_VERSION: &str = "rfp-v2-llm-2026-05-02";

pub fn system_prompt(schema_name: LlmSchemaName) -> String {
    format!(
        "You are a structured RFP extraction component. Return only JSON for schema '{}'. Preserve Korean source terms. Do not invent values. Every extracted item must cite evidence_block_ids from the provided candidate_blocks. Empty arrays are allowed when evidence is insufficient.",
        schema_name.as_str()
    )
}

pub fn user_prompt(envelope: &LlmInputEnvelope) -> crate::error::AppResult<String> {
    Ok(serde_json::to_string_pretty(envelope)?)
}
```

- [ ] **Step 5: Add contract tests**

Add tests under `llm_adapter::schemas::tests`:

```rust
#[test]
fn project_info_schema_requires_fields_and_blocks_extra_properties() {
    let schema = schema_for(LlmSchemaName::ProjectInfo);

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"][0], "fields");
}

#[test]
fn procurement_schema_contains_all_domain_arrays() {
    let schema = schema_for(LlmSchemaName::Procurement);
    let required = schema["required"].as_array().expect("required array");

    for key in [
        "procurement_items",
        "staffing_requirements",
        "deliverables",
        "acceptance_criteria",
        "risk_clauses",
    ] {
        assert!(required.iter().any(|value| value == key));
        assert_eq!(schema["properties"][key]["type"], "array");
    }
}
```

Add a prompt test under `llm_adapter::prompts::tests`:

```rust
#[test]
fn user_prompt_contains_candidate_blocks_but_no_file_paths() {
    let envelope = LlmInputEnvelope {
        document_id: "doc-1".into(),
        rfp_project_id: "project-1".into(),
        extraction_run_id: "run-1".into(),
        language: "ko".into(),
        candidate_blocks: vec![CandidateBlock {
            block_id: "block-1".into(),
            page_number: 12,
            kind: "table".into(),
            text: "요구사항 고유번호 SFR-001".into(),
            bbox: Some(vec![72.0, 400.0, 540.0, 650.0]),
        }],
        instructions: LlmInstructions {
            preserve_korean_terms: true,
            do_not_invent_values: true,
            require_evidence_block_ids: true,
        },
    };

    let prompt = user_prompt(&envelope).expect("prompt");

    assert!(prompt.contains("SFR-001"));
    assert!(prompt.contains("block-1"));
    assert!(!prompt.contains(".pdf"));
    assert!(!prompt.contains("OPENAI_API_KEY"));
    assert!(!prompt.contains("GEMINI_API_KEY"));
}
```

- [ ] **Step 6: Verify contracts**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::schemas::tests
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::prompts::tests
```

Expected: all contract tests pass.

## Task 4: Implement Schema and Evidence Validation

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/schema_validation.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/mod.rs`

- [ ] **Step 1: Export module**

```rust
pub mod schema_validation;
```

- [ ] **Step 2: Implement validator API**

```rust
use std::collections::HashSet;

use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::contracts::{CandidateBlock, LlmSchemaName};
use super::schemas::schema_for;

pub fn validate_structured_output(
    schema_name: LlmSchemaName,
    output: &Value,
    candidate_blocks: &[CandidateBlock],
) -> AppResult<()> {
    let schema = schema_for(schema_name);
    let compiled = jsonschema::validator_for(&schema)
        .map_err(|error| AppError::LlmRejected(format!("invalid local schema: {error}")))?;

    if !compiled.is_valid(output) {
        let messages = compiled
            .iter_errors(output)
            .map(|error| format!("{} at {}", error, error.instance_path()))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(AppError::LlmRejected(format!("schema_invalid: {messages}")));
    }

    let allowed_block_ids = candidate_blocks
        .iter()
        .map(|block| block.block_id.as_str())
        .collect::<HashSet<_>>();
    validate_evidence_ids(output, &allowed_block_ids)
}

fn validate_evidence_ids(value: &Value, allowed_block_ids: &HashSet<&str>) -> AppResult<()> {
    match value {
        Value::Object(map) => {
            if let Some(ids) = map.get("evidence_block_ids") {
                let array = ids.as_array().ok_or_else(|| {
                    AppError::LlmRejected("evidence_block_ids must be an array".into())
                })?;
                if array.is_empty() {
                    return Err(AppError::LlmRejected(
                        "missing_evidence: evidence_block_ids is empty".into(),
                    ));
                }
                for id in array {
                    let id = id.as_str().ok_or_else(|| {
                        AppError::LlmRejected("evidence_block_ids entries must be strings".into())
                    })?;
                    if !allowed_block_ids.contains(id) {
                        return Err(AppError::LlmRejected(format!(
                            "missing_evidence: unknown evidence block id {id}"
                        )));
                    }
                }
            }

            for child in map.values() {
                validate_evidence_ids(child, allowed_block_ids)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for child in values {
                validate_evidence_ids(child, allowed_block_ids)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
```

- [ ] **Step 3: Add validator tests**

```rust
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::llm_adapter::contracts::CandidateBlock;

    fn blocks() -> Vec<CandidateBlock> {
        vec![CandidateBlock {
            block_id: "block-1".into(),
            page_number: 1,
            kind: "paragraph".into(),
            text: "사업명: RFP 분석 시스템".into(),
            bbox: None,
        }]
    }

    #[test]
    fn accepts_valid_project_info_with_known_evidence() {
        let output = json!({
            "fields": [{
                "field_key": "business_name",
                "raw_value": "RFP 분석 시스템",
                "normalized_value": "RFP 분석 시스템",
                "confidence": 0.91,
                "evidence_block_ids": ["block-1"]
            }]
        });

        validate_structured_output(LlmSchemaName::ProjectInfo, &output, &blocks())
            .expect("valid output");
    }

    #[test]
    fn rejects_schema_mismatch() {
        let output = json!({
            "fields": [{
                "field_key": "business_name",
                "raw_value": "RFP 분석 시스템",
                "confidence": 0.91,
                "evidence_block_ids": ["block-1"]
            }]
        });

        let error = validate_structured_output(LlmSchemaName::ProjectInfo, &output, &blocks())
            .expect_err("schema rejection");
        assert!(error.to_string().contains("schema_invalid"));
    }

    #[test]
    fn rejects_unknown_evidence_block_id() {
        let output = json!({
            "fields": [{
                "field_key": "business_name",
                "raw_value": "RFP 분석 시스템",
                "normalized_value": "RFP 분석 시스템",
                "confidence": 0.91,
                "evidence_block_ids": ["block-missing"]
            }]
        });

        let error = validate_structured_output(LlmSchemaName::ProjectInfo, &output, &blocks())
            .expect_err("evidence rejection");
        assert!(error.to_string().contains("unknown evidence block id"));
    }
}
```

- [ ] **Step 4: Verify validation**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::schema_validation::tests
```

Expected: all validator tests pass.

## Task 5: Implement Settings, Secrets, and Offline Mode

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/settings.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/mod.rs`

- [ ] **Step 1: Export module**

```rust
pub mod settings;
```

- [ ] **Step 2: Implement settings DTOs**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettings {
    pub enabled: bool,
    pub offline_mode: bool,
    pub provider: LlmProvider,
    pub model: String,
    pub api_key_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveLlmSettingsRequest {
    pub enabled: bool,
    pub offline_mode: bool,
    pub provider: LlmProvider,
    pub model: String,
    pub api_key: Option<String>,
}
```

- [ ] **Step 3: Implement key references and env fallback**

Use:
- keychain service: `rfp-desktop`;
- keychain usernames: `llm:openai` and `llm:gemini`;
- env fallback: `OPENAI_API_KEY` and `GEMINI_API_KEY`;
- optional env override: `RFP_LLM_OFFLINE=1` forces offline mode.

```rust
const KEYCHAIN_SERVICE: &str = "rfp-desktop";

fn keychain_user(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => "llm:openai",
        LlmProvider::Gemini => "llm:gemini",
    }
}

fn env_key_name(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => "OPENAI_API_KEY",
        LlmProvider::Gemini => "GEMINI_API_KEY",
    }
}
```

`load_api_key(provider)` order:
1. If `RFP_LLM_OFFLINE=1`, return `None`.
2. Try OS keychain.
3. Try provider env var.
4. Return `None`.

`save_llm_settings(conn, request)`:
- trim `model`;
- reject `enabled=true` with empty `model`;
- store `api_key` in keychain only when provided and non-empty;
- store only `api_key_ref` in SQLite;
- never write the API key into `llm_settings`, `llm_runs`, logs, or errors.

- [ ] **Step 4: Add settings tests without touching the real keychain**

Define a small `SecretStore` trait and an in-memory implementation for tests:

```rust
pub trait SecretStore {
    fn set_password(&self, provider: &LlmProvider, value: &str) -> AppResult<()>;
    fn get_password(&self, provider: &LlmProvider) -> AppResult<Option<String>>;
    fn delete_password(&self, provider: &LlmProvider) -> AppResult<()>;
}
```

Test cases:

```rust
#[test]
fn default_settings_are_disabled_and_offline() {
    let conn = Connection::open_in_memory().expect("db");
    crate::db::migrate(&conn).expect("migrate");

    let settings = load_llm_settings(&conn, &InMemorySecretStore::default())
        .expect("settings");

    assert!(!settings.enabled);
    assert!(settings.offline_mode);
    assert!(!settings.api_key_configured);
}

#[test]
fn save_settings_stores_only_key_reference_in_sqlite() {
    let conn = Connection::open_in_memory().expect("db");
    crate::db::migrate(&conn).expect("migrate");
    let store = InMemorySecretStore::default();

    save_llm_settings(
        &conn,
        &store,
        SaveLlmSettingsRequest {
            enabled: true,
            offline_mode: false,
            provider: LlmProvider::OpenAi,
            model: "gpt-4o-mini".into(),
            api_key: Some("sk-test-secret".into()),
        },
    )
    .expect("save");

    let stored_json: String = conn
        .query_row(
            "SELECT provider || ':' || model || ':' || COALESCE(api_key_ref, '') FROM llm_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .expect("stored settings");

    assert!(stored_json.contains("openai:gpt-4o-mini:keychain:"));
    assert!(!stored_json.contains("sk-test-secret"));
}
```

- [ ] **Step 5: Verify settings**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::settings::tests
```

Expected: all settings tests pass without requiring OpenAI, Gemini, or OS keychain access.

## Task 6: Implement Provider HTTP Transport and OpenAI Adapter

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/http.rs`
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/openai.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/mod.rs`

- [ ] **Step 1: Export modules**

```rust
pub mod http;
pub mod openai;
```

- [ ] **Step 2: Implement transport trait**

```rust
#[derive(Debug, Clone)]
pub struct HttpJsonResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

pub trait LlmHttpTransport: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        headers: Vec<(String, String)>,
        body: serde_json::Value,
    ) -> AppResult<HttpJsonResponse>;
}
```

Production transport:
- use `reqwest::blocking::Client`;
- set a 60-second request timeout;
- send JSON body;
- parse JSON response when possible;
- return an `AppError::LlmProvider` containing only status code and sanitized provider message.

- [ ] **Step 3: Implement OpenAI request builder**

OpenAI endpoint:

```text
https://api.openai.com/v1/responses
```

Request shape:

```json
{
  "model": "configured-model",
  "input": [
    { "role": "system", "content": "system prompt" },
    { "role": "user", "content": "serialized input envelope" }
  ],
  "text": {
    "format": {
      "type": "json_schema",
      "name": "project_info",
      "strict": true,
      "schema": { "type": "object" }
    }
  }
}
```

Headers:
- `Authorization: Bearer <api key>`;
- `Content-Type: application/json`.

Request snapshot for `llm_runs.request_json` must omit headers and API key:

```json
{
  "endpoint": "openai.responses",
  "model": "configured-model",
  "schema_name": "project_info",
  "prompt_version": "rfp-v2-llm-2026-05-02",
  "input": { "document_id": "doc-1", "candidate_blocks": [] }
}
```

- [ ] **Step 4: Implement OpenAI response parser**

Parsing rules:
- If any content item has type `refusal`, return `AppError::LlmRejected("provider_refusal: ...")`.
- Otherwise find the first content item with `type = "output_text"` and parse its `text` as JSON.
- If the provider returns a parsed JSON object under `output_parsed`, accept it.
- Usage maps to `input_token_count` from `usage.input_tokens` and `output_token_count` from `usage.output_tokens`; missing counts become `0`.
- Non-2xx status codes are provider errors. Retry handling is in the runner.

- [ ] **Step 5: Add OpenAI tests with fake transport**

Tests:

```rust
#[test]
fn openai_request_uses_json_schema_and_excludes_api_key_from_snapshot() {
    let transport = RecordingTransport::new(json!({
        "output": [{
            "type": "message",
            "content": [{
                "type": "output_text",
                "text": "{\"fields\":[]}"
            }]
        }],
        "usage": { "input_tokens": 10, "output_tokens": 4 }
    }));

    let result = call_openai_structured_output(
        &transport,
        "test-key",
        "gpt-4o-mini",
        LlmSchemaName::ProjectInfo,
        &sample_envelope(),
    )
    .expect("openai call");

    let request = transport.last_body().expect("request body");
    assert_eq!(request["text"]["format"]["type"], "json_schema");
    assert_eq!(request["text"]["format"]["strict"], true);
    assert_eq!(result.output_json, json!({ "fields": [] }));
    assert!(!serde_json::to_string(&request).unwrap().contains("test-key"));
}

#[test]
fn openai_refusal_becomes_rejected() {
    let transport = RecordingTransport::new(json!({
        "output": [{
            "type": "message",
            "content": [{
                "type": "refusal",
                "refusal": "Cannot process this request"
            }]
        }]
    }));

    let error = call_openai_structured_output(
        &transport,
        "test-key",
        "gpt-4o-mini",
        LlmSchemaName::ProjectInfo,
        &sample_envelope(),
    )
    .expect_err("refusal");

    assert!(error.to_string().contains("provider_refusal"));
}
```

- [ ] **Step 6: Verify OpenAI adapter**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::openai::tests
```

Expected: all OpenAI adapter tests pass without network access.

## Task 7: Implement Gemini Adapter

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/gemini.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/mod.rs`

- [ ] **Step 1: Export module**

```rust
pub mod gemini;
```

- [ ] **Step 2: Implement Gemini request builder**

Endpoint:

```text
https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent
```

Headers:
- `x-goog-api-key: <api key>`;
- `Content-Type: application/json`.

Request shape:

```json
{
  "contents": [{
    "role": "user",
    "parts": [{
      "text": "system prompt\n\nserialized input envelope"
    }]
  }],
  "generationConfig": {
    "responseMimeType": "application/json",
    "responseJsonSchema": { "type": "object" }
  }
}
```

Request snapshot for `llm_runs.request_json` must omit headers and API key.

- [ ] **Step 3: Implement Gemini response parser**

Parsing rules:
- If `promptFeedback.blockReason` exists, return `AppError::LlmRejected("provider_refusal: ...")`.
- If the first candidate has `finishReason` of `SAFETY`, `RECITATION`, or `PROHIBITED_CONTENT`, return `AppError::LlmRejected("provider_refusal: ...")`.
- Parse the first candidate content part `text` as JSON.
- Usage maps to `usageMetadata.promptTokenCount` and `usageMetadata.candidatesTokenCount`; missing counts become `0`.
- Non-2xx status codes are provider errors.

- [ ] **Step 4: Add Gemini tests with fake transport**

```rust
#[test]
fn gemini_request_uses_response_json_schema_and_excludes_api_key() {
    let transport = RecordingTransport::new(json!({
        "candidates": [{
            "content": {
                "parts": [{ "text": "{\"fields\":[]}" }]
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 8,
            "candidatesTokenCount": 3
        }
    }));

    let result = call_gemini_structured_output(
        &transport,
        "test-key",
        "gemini-2.5-flash",
        LlmSchemaName::ProjectInfo,
        &sample_envelope(),
    )
    .expect("gemini call");

    let request = transport.last_body().expect("request body");
    assert_eq!(request["generationConfig"]["responseMimeType"], "application/json");
    assert!(request["generationConfig"]["responseJsonSchema"].is_object());
    assert_eq!(result.output_json, json!({ "fields": [] }));
    assert!(!serde_json::to_string(&request).unwrap().contains("test-key"));
}

#[test]
fn gemini_safety_finish_reason_becomes_rejected() {
    let transport = RecordingTransport::new(json!({
        "candidates": [{ "finishReason": "SAFETY" }]
    }));

    let error = call_gemini_structured_output(
        &transport,
        "test-key",
        "gemini-2.5-flash",
        LlmSchemaName::ProjectInfo,
        &sample_envelope(),
    )
    .expect_err("safety refusal");

    assert!(error.to_string().contains("provider_refusal"));
}
```

- [ ] **Step 5: Verify Gemini adapter**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::gemini::tests
```

Expected: all Gemini adapter tests pass without network access.

## Task 8: Implement Runner, Persistence, Retry, and Rejection Handling

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/llm_adapter/runner.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/validation/mod.rs`

- [ ] **Step 1: Export module**

```rust
pub mod runner;
```

- [ ] **Step 2: Implement runner request**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunLlmRequest {
    pub schema_name: LlmSchemaName,
    pub input: LlmInputEnvelope,
}
```

- [ ] **Step 3: Implement run flow**

`run_structured_extraction(conn, secret_store, transport, request)`:
1. Load settings.
2. If `enabled=false`, `offline_mode=true`, missing model, missing API key, or empty candidate blocks, return `AppError::LlmDisabled` and do not insert `llm_runs`.
3. Insert `llm_runs` with `status='running'`, token counts `0`, redacted `request_json`, and current `created_at`.
4. Call provider through a `call_with_retry` helper.
5. Validate structured output with `validate_structured_output`.
6. On success, update run to `status='succeeded'`, save `response_json`, token counts, and `finished_at`.
7. On schema/evidence/provider refusal rejection, update run to `status='rejected'`, save raw response when available, save sanitized `error_message`, add `schema_invalid` blocker when the rejection contains `schema_invalid` or `missing_evidence`.
8. On exhausted provider/network errors, update run to `status='failed'`, save sanitized `error_message`, and leave baseline project status unchanged.

- [ ] **Step 4: Implement retry policy**

Retry only these cases:
- HTTP 408, 409, 429;
- HTTP 500, 502, 503, 504;
- timeout or connection reset.

Do not retry:
- HTTP 400/401/403;
- provider refusal;
- JSON parse failure;
- local schema validation failure;
- evidence validation failure.

Use at most 3 total attempts with backoff delays `500ms`, then `1500ms`. In tests, inject a no-sleep implementation.

- [ ] **Step 5: Add persistence tests**

```rust
#[test]
fn successful_run_persists_succeeded_llm_run() {
    let conn = seeded_conn_with_project_and_extraction();
    let store = InMemorySecretStore::with_key(LlmProvider::OpenAi, "test-key");
    save_enabled_settings(&conn, LlmProvider::OpenAi, "gpt-4o-mini");
    let transport = SequenceTransport::single_success(project_info_success_response());

    let summary = run_structured_extraction(
        &conn,
        &store,
        &transport,
        RunLlmRequest {
            schema_name: LlmSchemaName::ProjectInfo,
            input: sample_envelope(),
        },
    )
    .expect("run");

    assert_eq!(summary.status, "succeeded");

    let stored_status: String = conn
        .query_row("SELECT status FROM llm_runs WHERE id = ?", [&summary.id], |row| row.get(0))
        .expect("stored run");
    assert_eq!(stored_status, "succeeded");
}

#[test]
fn offline_mode_skips_without_persisting_run() {
    let conn = seeded_conn_with_project_and_extraction();
    let store = InMemorySecretStore::default();
    save_offline_settings(&conn);
    let transport = SequenceTransport::unused();

    let error = run_structured_extraction(
        &conn,
        &store,
        &transport,
        RunLlmRequest {
            schema_name: LlmSchemaName::ProjectInfo,
            input: sample_envelope(),
        },
    )
    .expect_err("offline");

    assert!(error.to_string().contains("llm disabled"));
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM llm_runs", [], |row| row.get(0))
        .expect("run count");
    assert_eq!(count, 0);
}

#[test]
fn schema_rejection_persists_rejected_run_and_schema_invalid_finding() {
    let conn = seeded_conn_with_project_and_extraction();
    let store = InMemorySecretStore::with_key(LlmProvider::OpenAi, "test-key");
    save_enabled_settings(&conn, LlmProvider::OpenAi, "gpt-4o-mini");
    let transport = SequenceTransport::single_success(project_info_missing_required_response());

    let error = run_structured_extraction(
        &conn,
        &store,
        &transport,
        RunLlmRequest {
            schema_name: LlmSchemaName::ProjectInfo,
            input: sample_envelope(),
        },
    )
    .expect_err("schema rejection");

    assert!(error.to_string().contains("schema_invalid"));
    let rejected_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM llm_runs WHERE status = 'rejected'", [], |row| row.get(0))
        .expect("rejected count");
    assert_eq!(rejected_count, 1);
    let finding_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM validation_findings WHERE finding_type = 'schema_invalid'",
            [],
            |row| row.get(0),
        )
        .expect("finding count");
    assert_eq!(finding_count, 1);
}

#[test]
fn retryable_provider_error_retries_then_succeeds() {
    let conn = seeded_conn_with_project_and_extraction();
    let store = InMemorySecretStore::with_key(LlmProvider::OpenAi, "test-key");
    save_enabled_settings(&conn, LlmProvider::OpenAi, "gpt-4o-mini");
    let transport = SequenceTransport::new(vec![
        retryable_status_response(429),
        project_info_success_response(),
    ]);

    let summary = run_structured_extraction(
        &conn,
        &store,
        &transport,
        RunLlmRequest {
            schema_name: LlmSchemaName::ProjectInfo,
            input: sample_envelope(),
        },
    )
    .expect("retry success");

    assert_eq!(summary.status, "succeeded");
    assert_eq!(transport.call_count(), 2);
}
```

- [ ] **Step 6: Verify runner**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::runner::tests
```

Expected: all runner tests pass without network access.

## Task 9: Add Tauri Commands and Frontend DTOs

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/commands/llm.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/domain.rs`
- Modify: `apps/rfp-desktop/src/lib/types.ts`
- Modify: `apps/rfp-desktop/src/lib/api.ts`

- [ ] **Step 1: Add command module**

In `commands/mod.rs`:

```rust
pub mod llm;
```

In `lib.rs` handler:

```rust
commands::llm::get_llm_settings,
commands::llm::save_llm_settings,
commands::llm::clear_llm_api_key,
commands::llm::run_llm_structuring,
```

- [ ] **Step 2: Implement settings commands**

```rust
#[tauri::command]
pub fn get_llm_settings(state: State<'_, AppState>) -> AppResult<LlmSettings> {
    let conn = state.connect()?;
    settings::load_llm_settings(&conn, &settings::KeyringSecretStore)
}

#[tauri::command]
pub fn save_llm_settings(
    request: SaveLlmSettingsRequest,
    state: State<'_, AppState>,
) -> AppResult<LlmSettings> {
    let conn = state.connect()?;
    settings::save_llm_settings(&conn, &settings::KeyringSecretStore, request)?;
    settings::load_llm_settings(&conn, &settings::KeyringSecretStore)
}

#[tauri::command]
pub fn clear_llm_api_key(provider: LlmProvider, state: State<'_, AppState>) -> AppResult<LlmSettings> {
    let conn = state.connect()?;
    settings::clear_api_key(&conn, &settings::KeyringSecretStore, provider)?;
    settings::load_llm_settings(&conn, &settings::KeyringSecretStore)
}
```

- [ ] **Step 3: Implement run command**

Signature:

```rust
#[tauri::command]
pub fn run_llm_structuring(
    document_id: String,
    schema_name: LlmSchemaName,
    state: State<'_, AppState>,
) -> AppResult<LlmRunSummary> {
    let conn = state.connect()?;
    let envelope = load_candidate_envelope_for_document(&conn, &document_id, schema_name)?;
    runner::run_structured_extraction(
        &conn,
        &settings::KeyringSecretStore,
        &http::ReqwestTransport::new()?,
        runner::RunLlmRequest { schema_name, input: envelope },
    )
}
```

`load_candidate_envelope_for_document` is the integration boundary with the candidate extractor plan. If the candidate extractor module is not implemented yet, keep this command behind a small function that returns `AppError::LlmDisabled("candidate bundles are unavailable")` and cover provider/runner behavior through module tests. When the candidate extractor lands, replace the function body with a call to its public bundle loader.

- [ ] **Step 4: Add TypeScript DTOs**

```ts
export type LlmProvider = "openai" | "gemini";
export type LlmSchemaName =
  | "project_info"
  | "requirements"
  | "procurement"
  | "risk_classification";

export interface LlmSettings {
  enabled: boolean;
  offlineMode: boolean;
  provider: LlmProvider;
  model: string;
  apiKeyConfigured: boolean;
}

export interface SaveLlmSettingsRequest {
  enabled: boolean;
  offlineMode: boolean;
  provider: LlmProvider;
  model: string;
  apiKey?: string | null;
}

export interface LlmRunSummary {
  id: string;
  extractionRunId: string;
  provider: string;
  model: string;
  schemaName: string;
  promptVersion: string;
  status: "queued" | "running" | "succeeded" | "failed" | "rejected";
  inputTokenCount: number;
  outputTokenCount: number;
  errorMessage?: string | null;
  createdAt: string;
  finishedAt?: string | null;
}
```

- [ ] **Step 5: Add API wrappers**

```ts
export function getLlmSettings(): Promise<LlmSettings> {
  return invoke<LlmSettings>("get_llm_settings");
}

export function saveLlmSettings(
  request: SaveLlmSettingsRequest,
): Promise<LlmSettings> {
  return invoke<LlmSettings>("save_llm_settings", { request });
}

export function clearLlmApiKey(provider: LlmProvider): Promise<LlmSettings> {
  return invoke<LlmSettings>("clear_llm_api_key", { provider });
}

export function runLlmStructuring(
  documentId: string,
  schemaName: LlmSchemaName,
): Promise<LlmRunSummary> {
  return invoke<LlmRunSummary>("run_llm_structuring", {
    documentId,
    schemaName,
  });
}
```

- [ ] **Step 6: Verify command wiring and frontend types**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --no-run
npm run build --prefix apps/rfp-desktop
```

Expected: Rust and TypeScript compile.

## Task 10: Add Optional Live Provider Smoke Tests

**Files:**
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/openai.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/llm_adapter/gemini.rs`

- [ ] **Step 1: Add ignored OpenAI live test**

```rust
#[test]
#[ignore]
fn openai_live_structured_output_roundtrip() {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY");
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
    let transport = crate::llm_adapter::http::ReqwestTransport::new().expect("transport");

    let response = call_openai_structured_output(
        &transport,
        &api_key,
        &model,
        LlmSchemaName::ProjectInfo,
        &sample_envelope(),
    )
    .expect("live openai");

    validate_structured_output(
        LlmSchemaName::ProjectInfo,
        &response.output_json,
        &sample_envelope().candidate_blocks,
    )
    .expect("schema valid");
}
```

- [ ] **Step 2: Add ignored Gemini live test**

```rust
#[test]
#[ignore]
fn gemini_live_structured_output_roundtrip() {
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY");
    let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".into());
    let transport = crate::llm_adapter::http::ReqwestTransport::new().expect("transport");

    let response = call_gemini_structured_output(
        &transport,
        &api_key,
        &model,
        LlmSchemaName::ProjectInfo,
        &sample_envelope(),
    )
    .expect("live gemini");

    validate_structured_output(
        LlmSchemaName::ProjectInfo,
        &response.output_json,
        &sample_envelope().candidate_blocks,
    )
    .expect("schema valid");
}
```

- [ ] **Step 3: Verify ignored tests are opt-in**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::openai::tests::openai_live_structured_output_roundtrip
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::gemini::tests::gemini_live_structured_output_roundtrip
```

Expected: tests are listed as ignored unless run with `-- --ignored`.

Optional live verification when a human has opted in and provided keys:

```bash
OPENAI_API_KEY=... OPENAI_MODEL=gpt-4o-mini cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::openai::tests::openai_live_structured_output_roundtrip -- --ignored
GEMINI_API_KEY=... GEMINI_MODEL=gemini-2.5-flash cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::gemini::tests::gemini_live_structured_output_roundtrip -- --ignored
```

Expected: each enabled provider returns schema-valid project info JSON for the small fixture envelope.

## Task 11: Integrated Verification

**Files:**
- No new files.

- [ ] **Step 1: Run all focused Rust tests**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_llm_tables_and_default_settings
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::
```

Expected: all LLM and migration tests pass without network access.

- [ ] **Step 2: Run full Rust tests**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
```

Expected: all Rust tests pass; ignored live-provider tests remain ignored.

- [ ] **Step 3: Run frontend checks**

```bash
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
```

Expected: frontend tests and build pass.

- [ ] **Step 4: Run repository verification**

```bash
scripts/verify.sh
```

Expected: existing vertical-slice verification still passes and does not require API keys.

- [ ] **Step 5: Confirm no secret leakage**

With fake test key `sk-test-secret` used only in tests:

```bash
rg -n "sk-test-secret|AIza" apps/rfp-desktop/src-tauri/migrations apps/rfp-desktop/src-tauri/src apps/rfp-desktop/src
```

Expected: only test literals appear; no migration, request snapshot, or production code path stores a real secret value.

## Retry and Error Handling Matrix

| Condition | `llm_runs.status` | Retry | Validation finding | User-visible result |
|---|---|---:|---|---|
| LLM disabled | no row | no | `llm_not_used` warning via baseline validation | `검토 필요` until rule/domain data clears blockers |
| Offline mode | no row | no | `llm_not_used` warning via baseline validation | Local-only baseline remains available |
| Missing API key | no row | no | `llm_not_used` warning via baseline validation | Settings screen says key not configured |
| Empty candidate bundle | no row | no | existing candidate/validation blocker | No network call |
| HTTP 408/409/429/5xx | `running` then final `succeeded` or `failed` | yes | none for final success | Retry is invisible except run audit |
| HTTP 400/401/403 | `failed` | no | none | Settings/provider error shown |
| Provider refusal/safety block | `rejected` | no | none; rejected run audit is the record | `검토 필요` through existing baseline/domain blockers |
| Malformed JSON | `rejected` | no | `schema_invalid` blocker | `검토 필요` |
| JSON Schema mismatch | `rejected` | no | `schema_invalid` blocker | `검토 필요` |
| Unknown or empty evidence IDs | `rejected` | no | `missing_evidence` blocker | `검토 필요` |
| Valid schema and evidence | `succeeded` | no | domain writer/validation decides | Draft DTO is safe to hand off |

## Security and Privacy Rules

- Default state is `enabled=false` and `offline_mode=true`.
- API keys are accepted only through settings commands or provider env vars.
- SQLite stores `api_key_ref`, never the secret.
- `llm_runs.request_json` stores prompts, schema name, model, and candidate text, but never auth headers or key values.
- Raw PDFs, source paths, and OpenDataLoader output paths are never sent to providers.
- Candidate text sent to providers must be visible in the request snapshot for auditability.
- Error messages must be sanitized before persistence; provider messages are allowed only after stripping headers, keys, and URLs with query secrets.
- Live tests are ignored by default and require explicit environment variables.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| OpenAI or Gemini request shape changes | Keep provider-specific code isolated and covered by request-shape tests; consult official docs before live verification. |
| Gemini and OpenAI support different JSON Schema subsets | Build schema values through provider adapter functions so provider-specific normalization can be contained. |
| OS keychain behavior differs by platform | Keep env-var fallback for development; test secret handling through `SecretStore` trait. |
| Candidate bundles are too large | Candidate extractor should cap block counts; runner rejects empty bundles and can add a max serialized prompt size before provider calls. |
| LLM invents values despite instructions | Evidence validator rejects unknown/empty `evidence_block_ids`; local domain writer and quality gate remain final arbiters. |
| Network failure blocks analysis | Offline/default-disabled mode keeps rule baseline available; failed LLM calls preserve existing extraction/project rows. |
| Other workers edit shared command or domain files | Implement in small module-owned steps, read before editing shared files, and avoid reverting unrelated changes. |

## Done When

- `0002_llm.sql` creates `llm_settings`, `llm_runs`, and indexes.
- Migration tests prove default LLM settings are disabled and offline.
- OpenAI and Gemini adapters build structured-output requests without including API keys in request snapshots.
- Schema validation rejects missing required fields, extra fields, invalid enum values, empty evidence arrays, and evidence block IDs outside the candidate envelope.
- Runner persists `succeeded`, `failed`, and `rejected` `llm_runs` with sanitized request/response/error fields.
- Disabled/offline/missing-key modes make no network calls and do not require secrets for normal verification.
- Rust command DTOs and TypeScript API wrappers compile.
- `cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml llm_adapter::` passes.
- `cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml` passes.
- `npm run test --prefix apps/rfp-desktop` passes.
- `npm run build --prefix apps/rfp-desktop` passes.
- `scripts/verify.sh` passes without OpenAI or Gemini credentials.
- Optional live tests are documented, ignored by default, and pass only when a human intentionally provides provider API keys.

## Execution Notes

- Implement Task 1 first because every later task depends on `llm_runs`.
- Implement Tasks 3 through 8 with fake transports and fixture envelopes before wiring Tauri commands.
- Keep live-provider tests opt-in; missing secrets are not blockers.
- Do not change `TASKS.md` or `IMPLEMENTATION_LOG.md` from worker-scoped plan execution unless acting as the integrating agent with ownership.

## Self-Review

Spec coverage:
- FR-005 is covered by settings, opt-in/offline behavior, and secure key handling.
- FR-006 is covered through structured output adapters and schema-specific DTOs.
- FR-007 is covered by evidence block validation before domain handoff.
- FR-008 is covered by `schema_invalid` and `missing_evidence` rejection behavior.
- Nonfunctional local-first and security requirements are covered by candidate-only inputs and keychain/env secret handling.

Placeholder scan:
- No placeholder markers or unspecified implementation steps remain.
- Every file path is concrete.
- Every verification command has an expected result.

Type consistency:
- Rust DTOs use `serde(rename_all = "camelCase")` for Tauri responses.
- Schema names use `snake_case` for DB and TypeScript union values.
- Provider names are stored as `openai` and `gemini` in SQLite.

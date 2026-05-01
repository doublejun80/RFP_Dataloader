# Tauri RFP v2 Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 실제 RFP PDF 1건을 등록하고 OpenDataLoader로 추출한 뒤 SQLite에 `documents`, `source_files`, `extraction_runs`, `document_blocks`, `rfp_projects`, `validation_findings`를 저장하고, Tauri UI에서 `검토 필요` 또는 `확정 가능` 상태를 확인한다.

**Architecture:** React/Vite UI는 Tauri command만 호출하고, Rust backend가 파일 접근, OpenDataLoader 실행, SQLite transaction, validation을 소유한다. 첫 vertical slice는 LLM 없이 rule baseline과 품질 게이트를 먼저 완성해서 `20/20 generated` 착시를 막는다.

**Tech Stack:** Tauri v2, React, TypeScript, Rust, SQLite via `rusqlite`, OpenDataLoader CLI, Vitest, Rust unit/integration tests.

---

## Scope

이 계획은 v2 전체 제품이 아니라 첫 성공 가능한 vertical slice다.

Included:
- Tauri v2 + React/TypeScript app scaffold.
- Rust-owned SQLite schema and migration runner.
- PDF path registration with SHA-256 and duplicate detection.
- OpenDataLoader CLI diagnostic and fast mode extraction.
- OpenDataLoader JSON to `document_blocks` normalization.
- Minimal `rfp_projects` creation.
- MVP validation blockers/warnings stored in `validation_findings`.
- First-screen UI for document list, analysis status, blocker/warning counts, and block preview.
- One real PDF smoke command that reports generated/ready/review_needed/failed counts separately.

Out of scope for this plan:
- OpenAI/Gemini adapters.
- Full candidate extractor.
- Domain writer for procurement/staffing/deliverables/acceptance/risk tables.
- Correction dialog.
- Markdown/JSON/Docx export.
- App bundling and notarization.

Those are separate plans after this vertical slice passes on one real PDF.

## Source Specs

- `spec/00_readme.md`: v2 principles and source links.
- `spec/01_failure_review_and_reuse.md`: failure causes and assets to reuse.
- `spec/02_prd.md`: MVP requirements and success criteria.
- `spec/03_architecture.md`: Rust command, SQLite, OpenDataLoader, validation boundaries.
- `spec/04_erd.md`: tables and indexes.
- `spec/05_data_pipeline.md`: registration through validation flow.
- `spec/07_opendataloader_integration.md`: CLI diagnostics and fast/hybrid modes.
- `spec/08_ui_product_flow.md`: first-screen workspace and Korean labels.
- `spec/09_quality_gate.md`: blocker/warning rules and smoke exit codes.
- `spec/10_migration_cutover.md`: v1 to v2 replacement map.
- `spec/11_backlog_seed.md`: recommended first implementation order.

## External References Checked

- Tauri project creation: `npm create tauri-app@latest`, then `npm install` and `npm run tauri dev`.
- Tauri command model: frontend calls `invoke` from `@tauri-apps/api/core`; Rust commands are registered with `tauri::generate_handler`.
- OpenDataLoader CLI: `opendataloader-pdf document.pdf -o ./output -f json,markdown`.

## File Structure

Create this tree:

```text
.
├─ apps/
│  └─ rfp-desktop/
│     ├─ package.json
│     ├─ vitest.config.ts
│     ├─ src/
│     │  ├─ App.tsx
│     │  ├─ App.test.tsx
│     │  ├─ main.tsx
│     │  ├─ styles.css
│     │  ├─ components/
│     │  │  ├─ BlockPreview.tsx
│     │  │  ├─ DocumentList.tsx
│     │  │  ├─ QualityGate.tsx
│     │  │  └─ StatusBadge.tsx
│     │  └─ lib/
│     │     ├─ api.ts
│     │     └─ types.ts
│     └─ src-tauri/
│        ├─ Cargo.toml
│        ├─ migrations/
│        │  └─ 0001_core.sql
│        └─ src/
│           ├─ analysis/
│           │  └─ mod.rs
│           ├─ block_normalizer/
│           │  └─ mod.rs
│           ├─ commands/
│           │  ├─ documents.rs
│           │  ├─ extraction.rs
│           │  ├─ mod.rs
│           │  └─ pipeline.rs
│           ├─ db/
│           │  └─ mod.rs
│           ├─ document_ingestion/
│           │  └─ mod.rs
│           ├─ opendataloader_adapter/
│           │  └─ mod.rs
│           ├─ validation/
│           │  └─ mod.rs
│           ├─ domain.rs
│           ├─ error.rs
│           ├─ lib.rs
│           ├─ main.rs
│           └─ state.rs
├─ fixtures/
│  └─ opendataloader/
│     └─ sample-output.json
└─ tests/
   └─ smoke/
      └─ README.md
```

## Task 0: Initialize Git Tracking

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Start git if this directory is not already a repository**

Run from repository root:

```bash
git init
```

Expected: `.git/` exists and `git status --short` succeeds.

- [ ] **Step 2: Create `.gitignore`**

Create `.gitignore` with:

```gitignore
.DS_Store
node_modules/
target/
dist/
*.sqlite3
*.db
*.log
apps/rfp-desktop/src-tauri/target/
apps/rfp-desktop/dist/
apps/rfp-desktop/node_modules/
fixtures/rfp_bundle/**/*.pdf
```

- [ ] **Step 3: Commit**

```bash
git add .gitignore
git commit -m "chore: initialize rfp v2 repository"
```

Expected: commit succeeds.

## Task 1: Scaffold Tauri React App

**Files:**
- Create: `apps/rfp-desktop/**`
- Modify: `apps/rfp-desktop/package.json`
- Modify: `apps/rfp-desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Scaffold app**

Run from repository root:

```bash
npm create tauri-app@latest apps/rfp-desktop -- --template react-ts --manager npm
```

Expected: `apps/rfp-desktop/src-tauri` and `apps/rfp-desktop/src` exist.

- [ ] **Step 2: Install frontend dependencies**

```bash
npm install --prefix apps/rfp-desktop
npm install --prefix apps/rfp-desktop @tauri-apps/api lucide-react clsx
npm install --prefix apps/rfp-desktop --save-dev vitest jsdom @testing-library/react @testing-library/jest-dom
```

Expected: `apps/rfp-desktop/package-lock.json` is updated.

- [ ] **Step 3: Install Rust dependencies**

```bash
cd apps/rfp-desktop/src-tauri
cargo add rusqlite --features bundled
cargo add serde --features derive
cargo add serde_json
cargo add uuid --features v4,serde
cargo add chrono --features serde
cargo add sha2
cargo add thiserror
cargo add tempfile --dev
```

Expected: `Cargo.toml` contains those crates.

- [ ] **Step 4: Add frontend test script**

Modify `apps/rfp-desktop/package.json` scripts to include:

```json
{
  "test": "vitest run",
  "test:watch": "vitest"
}
```

Keep existing `dev`, `build`, and `tauri` scripts from the scaffold.

- [ ] **Step 5: Verify scaffold**

```bash
npm run build --prefix apps/rfp-desktop
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
```

Expected: both commands exit 0.

- [ ] **Step 6: Commit**

```bash
git add apps/rfp-desktop
git commit -m "chore: scaffold tauri rfp desktop app"
```

Expected: commit succeeds.

## Task 2: Add SQLite Schema and Migration Runner

**Files:**
- Create: `apps/rfp-desktop/src-tauri/migrations/0001_core.sql`
- Create: `apps/rfp-desktop/src-tauri/src/error.rs`
- Create: `apps/rfp-desktop/src-tauri/src/db/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/state.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing migration test**

Add this test to `apps/rfp-desktop/src-tauri/src/db/mod.rs` before implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_core_tables() {
        let conn = Connection::open_in_memory().expect("open memory db");

        migrate(&conn).expect("run migrations");

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                    'documents',
                    'source_files',
                    'extraction_runs',
                    'document_blocks',
                    'rfp_projects',
                    'validation_findings',
                    'audit_events'
                )",
                [],
                |row| row.get(0),
            )
            .expect("count tables");
        assert_eq!(table_count, 7);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_core_tables
```

Expected: FAIL because `migrate` is not implemented.

- [ ] **Step 3: Create migration SQL**

Create `apps/rfp-desktop/src-tauri/migrations/0001_core.sql`:

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS documents (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('created', 'extracting', 'analyzing', 'review_needed', 'ready', 'failed'))
);

CREATE TABLE IF NOT EXISTS source_files (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  file_name TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  sha256 TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_source_files_sha256 ON source_files(sha256);

CREATE TABLE IF NOT EXISTS extraction_runs (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  mode TEXT NOT NULL CHECK (mode IN ('fast', 'hybrid_auto', 'hybrid_full')),
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
  json_path TEXT,
  markdown_path TEXT,
  stdout TEXT NOT NULL DEFAULT '',
  stderr TEXT NOT NULL DEFAULT '',
  started_at TEXT NOT NULL,
  finished_at TEXT,
  error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_extraction_runs_document_id ON extraction_runs(document_id);

CREATE TABLE IF NOT EXISTS document_blocks (
  id TEXT PRIMARY KEY,
  extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id) ON DELETE CASCADE,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  source_element_id TEXT NOT NULL,
  page_number INTEGER NOT NULL,
  block_index INTEGER NOT NULL,
  kind TEXT NOT NULL,
  heading_level INTEGER,
  text TEXT NOT NULL,
  bbox_json TEXT,
  raw_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_document_blocks_document_page ON document_blocks(document_id, page_number, block_index);

CREATE TABLE IF NOT EXISTS rfp_projects (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL UNIQUE REFERENCES documents(id) ON DELETE CASCADE,
  analysis_version TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('draft', 'review_needed', 'ready', 'failed')),
  summary TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS validation_findings (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'blocker')),
  finding_type TEXT NOT NULL,
  message TEXT NOT NULL,
  target_table TEXT,
  target_id TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_validation_findings_project_severity ON validation_findings(rfp_project_id, severity);

CREATE TABLE IF NOT EXISTS audit_events (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT,
  document_id TEXT,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_events_document_id ON audit_events(document_id);
```

- [ ] **Step 4: Implement app error type**

Create `apps/rfp-desktop/src-tauri/src/error.rs`:

```rust
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("external command failed: {0}")]
    ExternalCommand(String),
    #[error("application path unavailable: {0}")]
    Path(String),
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
enum AppErrorDto {
    Database(String),
    Io(String),
    Json(String),
    InvalidInput(String),
    ExternalCommand(String),
    Path(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let dto = match self {
            AppError::Database(message) => AppErrorDto::Database(message.to_string()),
            AppError::Io(message) => AppErrorDto::Io(message.to_string()),
            AppError::Json(message) => AppErrorDto::Json(message.to_string()),
            AppError::InvalidInput(message) => AppErrorDto::InvalidInput(message.clone()),
            AppError::ExternalCommand(message) => AppErrorDto::ExternalCommand(message.clone()),
            AppError::Path(message) => AppErrorDto::Path(message.clone()),
        };
        dto.serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 5: Implement migration runner**

Create `apps/rfp-desktop/src-tauri/src/db/mod.rs`:

```rust
use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

const CORE_MIGRATION: &str = include_str!("../../migrations/0001_core.sql");

pub fn open_database(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(CORE_MIGRATION)?;
    Ok(())
}
```

- [ ] **Step 6: Implement Tauri state**

Create `apps/rfp-desktop/src-tauri/src/state.rs`:

```rust
use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use tauri::Manager;

use crate::db;
use crate::error::{AppError, AppResult};

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn from_app_handle(app: &tauri::AppHandle) -> AppResult<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Path(error.to_string()))?;
        fs::create_dir_all(&app_data_dir)?;
        let db_path = app_data_dir.join("rfp_v2.sqlite3");
        let _conn = db::open_database(&db_path)?;
        Ok(Self { app_data_dir, db_path })
    }

    pub fn connect(&self) -> AppResult<Connection> {
        db::open_database(&self.db_path)
    }
}
```

- [ ] **Step 7: Wire modules in `lib.rs`**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs`:

```rust
mod db;
mod error;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = state::AppState::from_app_handle(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 8: Run test to verify pass**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml db::tests::migrates_core_tables
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add apps/rfp-desktop/src-tauri
git commit -m "feat: add rfp sqlite core schema"
```

Expected: commit succeeds.

## Task 3: Implement Document Registration

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/domain.rs`
- Create: `apps/rfp-desktop/src-tauri/src/document_ingestion/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/commands/documents.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing registration test**

Add this test to `apps/rfp-desktop/src-tauri/src/document_ingestion/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::db;

    #[test]
    fn register_document_creates_source_file_and_audit_event() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let pdf_path = temp.path().join("sample.pdf");
        fs::write(&pdf_path, b"%PDF-1.7\nsample").expect("write pdf");
        let conn = db::open_database(&db_path).expect("open db");

        let document = register_document(&conn, &pdf_path).expect("register");

        assert_eq!(document.status, "created");
        assert_eq!(document.title, "sample.pdf");

        let source_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_files WHERE document_id = ?", [&document.id], |row| row.get(0))
            .expect("source count");
        assert_eq!(source_count, 1);

        let audit_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_events WHERE document_id = ?", [&document.id], |row| row.get(0))
            .expect("audit count");
        assert_eq!(audit_count, 1);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml document_ingestion::tests::register_document_creates_source_file_and_audit_event
```

Expected: FAIL because `register_document` is not implemented.

- [ ] **Step 3: Add DTOs**

Create `apps/rfp-desktop/src-tauri/src/domain.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub file_name: Option<String>,
    pub blocker_count: i64,
    pub warning_count: i64,
    pub block_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockPreview {
    pub id: String,
    pub page_number: i64,
    pub block_index: i64,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionRunSummary {
    pub id: String,
    pub document_id: String,
    pub status: String,
    pub mode: String,
    pub json_path: Option<String>,
    pub markdown_path: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSummary {
    pub document: DocumentSummary,
    pub extraction: Option<ExtractionRunSummary>,
    pub ready_count: i64,
    pub review_needed_count: i64,
    pub failed_count: i64,
}
```

- [ ] **Step 4: Implement registration**

Create `apps/rfp-desktop/src-tauri/src/document_ingestion/mod.rs`:

```rust
use std::fs;
use std::io::Read;
use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::DocumentSummary;
use crate::error::{AppError, AppResult};

pub fn register_document(conn: &Connection, path: &Path) -> AppResult<DocumentSummary> {
    if path.extension().and_then(|value| value.to_str()).map(|value| value.eq_ignore_ascii_case("pdf")) != Some(true) {
        return Err(AppError::InvalidInput("PDF 파일만 등록할 수 있습니다.".to_string()));
    }

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(AppError::InvalidInput("파일 경로가 아닙니다.".to_string()));
    }

    let sha256 = calculate_sha256(path)?;
    if let Some(existing_id) = find_document_by_sha(conn, &sha256)? {
        return load_document_summary(conn, &existing_id);
    }

    let now = Utc::now().to_rfc3339();
    let document_id = Uuid::new_v4().to_string();
    let source_file_id = Uuid::new_v4().to_string();
    let audit_id = Uuid::new_v4().to_string();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::InvalidInput("파일명을 읽을 수 없습니다.".to_string()))?
        .to_string();

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO documents (id, title, created_at, updated_at, status) VALUES (?, ?, ?, ?, 'created')",
        params![document_id, file_name, now, now],
    )?;
    tx.execute(
        "INSERT INTO source_files (id, document_id, path, file_name, mime_type, sha256, size_bytes, created_at)
         VALUES (?, ?, ?, ?, 'application/pdf', ?, ?, ?)",
        params![
            source_file_id,
            document_id,
            path.to_string_lossy().to_string(),
            file_name,
            sha256,
            metadata.len() as i64,
            now
        ],
    )?;
    tx.execute(
        "INSERT INTO audit_events (id, document_id, event_type, payload_json, created_at)
         VALUES (?, ?, 'document_added', ?, ?)",
        params![
            audit_id,
            document_id,
            serde_json::json!({ "path": path.to_string_lossy().to_string() }).to_string(),
            now
        ],
    )?;
    tx.commit()?;

    load_document_summary(conn, &document_id)
}

fn calculate_sha256(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn find_document_by_sha(conn: &Connection, sha256: &str) -> AppResult<Option<String>> {
    let value = conn
        .query_row(
            "SELECT document_id FROM source_files WHERE sha256 = ?",
            [sha256],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value)
}

pub fn load_document_summary(conn: &Connection, document_id: &str) -> AppResult<DocumentSummary> {
    let summary = conn.query_row(
        "SELECT
            d.id,
            d.title,
            d.status,
            sf.file_name,
            COALESCE(SUM(CASE WHEN vf.severity = 'blocker' THEN 1 ELSE 0 END), 0) AS blocker_count,
            COALESCE(SUM(CASE WHEN vf.severity = 'warning' THEN 1 ELSE 0 END), 0) AS warning_count,
            (SELECT COUNT(*) FROM document_blocks db WHERE db.document_id = d.id) AS block_count
         FROM documents d
         LEFT JOIN source_files sf ON sf.document_id = d.id
         LEFT JOIN rfp_projects rp ON rp.document_id = d.id
         LEFT JOIN validation_findings vf ON vf.rfp_project_id = rp.id
         WHERE d.id = ?
         GROUP BY d.id, sf.file_name",
        [document_id],
        |row| {
            Ok(DocumentSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                file_name: row.get(3)?,
                blocker_count: row.get(4)?,
                warning_count: row.get(5)?,
                block_count: row.get(6)?,
            })
        },
    )?;
    Ok(summary)
}

pub fn list_documents(conn: &Connection) -> AppResult<Vec<DocumentSummary>> {
    let mut statement = conn.prepare(
        "SELECT d.id FROM documents d ORDER BY d.created_at DESC",
    )?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|id| load_document_summary(conn, &id)).collect()
}
```

- [ ] **Step 5: Add document commands**

Create `apps/rfp-desktop/src-tauri/src/commands/documents.rs`:

```rust
use std::path::PathBuf;

use tauri::State;

use crate::document_ingestion;
use crate::domain::DocumentSummary;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn register_document_by_path(path: String, state: State<'_, AppState>) -> AppResult<DocumentSummary> {
    let conn = state.connect()?;
    document_ingestion::register_document(&conn, &PathBuf::from(path))
}

#[tauri::command]
pub fn list_documents(state: State<'_, AppState>) -> AppResult<Vec<DocumentSummary>> {
    let conn = state.connect()?;
    document_ingestion::list_documents(&conn)
}
```

Create `apps/rfp-desktop/src-tauri/src/commands/mod.rs`:

```rust
pub mod documents;
```

- [ ] **Step 6: Register commands**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs`:

```rust
mod commands;
mod db;
mod document_ingestion;
mod domain;
mod error;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = state::AppState::from_app_handle(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::documents::register_document_by_path,
            commands::documents::list_documents
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: Run test to verify pass**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml document_ingestion::tests::register_document_creates_source_file_and_audit_event
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/rfp-desktop/src-tauri
git commit -m "feat: register local rfp pdf documents"
```

Expected: commit succeeds.

## Task 4: Implement OpenDataLoader Diagnostics and Fast Extraction

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/opendataloader_adapter/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/commands/extraction.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing args test**

Add this test to `apps/rfp-desktop/src-tauri/src/opendataloader_adapter/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn fast_mode_args_are_bounded_and_explicit() {
        let input = PathBuf::from("/tmp/rfp.pdf");
        let output = PathBuf::from("/tmp/out");

        let args = build_fast_mode_args(&input, &output);

        assert_eq!(
            args,
            vec![
                "/tmp/rfp.pdf".to_string(),
                "-o".to_string(),
                "/tmp/out".to_string(),
                "-f".to_string(),
                "json,markdown".to_string(),
                "--quiet".to_string()
            ]
        );
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml opendataloader_adapter::tests::fast_mode_args_are_bounded_and_explicit
```

Expected: FAIL because `build_fast_mode_args` is not implemented.

- [ ] **Step 3: Implement adapter**

Create `apps/rfp-desktop/src-tauri/src/opendataloader_adapter/mod.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::domain::ExtractionRunSummary;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDataLoaderDiagnostic {
    pub cli_found: bool,
    pub java_found: bool,
    pub cli_message: String,
    pub java_message: String,
}

pub fn build_fast_mode_args(input: &Path, output_dir: &Path) -> Vec<String> {
    vec![
        input.to_string_lossy().to_string(),
        "-o".to_string(),
        output_dir.to_string_lossy().to_string(),
        "-f".to_string(),
        "json,markdown".to_string(),
        "--quiet".to_string(),
    ]
}

pub fn diagnose(cli_path: Option<PathBuf>) -> OpenDataLoaderDiagnostic {
    let cli = cli_path.unwrap_or_else(|| PathBuf::from("opendataloader-pdf"));
    let cli_output = Command::new(&cli).arg("--version").output();
    let java_output = Command::new("java").arg("-version").output();

    let cli_found = cli_output.as_ref().map(|output| output.status.success()).unwrap_or(false);
    let java_found = java_output.as_ref().map(|output| output.status.success()).unwrap_or(false);

    OpenDataLoaderDiagnostic {
        cli_found,
        java_found,
        cli_message: cli_output
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|error| error.to_string()),
        java_message: java_output
            .map(|output| String::from_utf8_lossy(&output.stderr).trim().to_string())
            .unwrap_or_else(|error| error.to_string()),
    }
}

pub fn run_fast_extraction(
    conn: &Connection,
    app_data_dir: &Path,
    document_id: &str,
    cli_path: Option<PathBuf>,
) -> AppResult<ExtractionRunSummary> {
    let source_path: String = conn.query_row(
        "SELECT path FROM source_files WHERE document_id = ? ORDER BY created_at DESC LIMIT 1",
        [document_id],
        |row| row.get(0),
    )?;
    let input_path = PathBuf::from(source_path);
    let run_id = Uuid::new_v4().to_string();
    let output_dir = app_data_dir.join("extractions").join(document_id).join(&run_id);
    fs::create_dir_all(&output_dir)?;

    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
         VALUES (?, ?, 'opendataloader', 'fast', 'running', ?)",
        params![run_id, document_id, now],
    )?;
    conn.execute(
        "UPDATE documents SET status = 'extracting', updated_at = ? WHERE id = ?",
        params![now, document_id],
    )?;

    let cli = cli_path.unwrap_or_else(|| PathBuf::from("opendataloader-pdf"));
    let output = Command::new(cli)
        .args(build_fast_mode_args(&input_path, &output_dir))
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let finished_at = Utc::now().to_rfc3339();

    if !output.status.success() {
        conn.execute(
            "UPDATE extraction_runs
             SET status = 'failed', stdout = ?, stderr = ?, finished_at = ?, error_message = ?
             WHERE id = ?",
            params![stdout, stderr, finished_at, "OpenDataLoader 실행에 실패했습니다.", run_id],
        )?;
        conn.execute(
            "UPDATE documents SET status = 'failed', updated_at = ? WHERE id = ?",
            params![finished_at, document_id],
        )?;
        return Err(AppError::ExternalCommand(stderr));
    }

    let json_path = find_first_extension(&output_dir, "json")?;
    let markdown_path = find_first_extension(&output_dir, "md")
        .or_else(|_| find_first_extension(&output_dir, "markdown"))?;

    conn.execute(
        "UPDATE extraction_runs
         SET status = 'succeeded', json_path = ?, markdown_path = ?, stdout = ?, stderr = ?, finished_at = ?
         WHERE id = ?",
        params![
            json_path.to_string_lossy().to_string(),
            markdown_path.to_string_lossy().to_string(),
            stdout,
            stderr,
            finished_at,
            run_id
        ],
    )?;

    load_extraction_summary(conn, &run_id)
}

fn find_first_extension(dir: &Path, extension: &str) -> AppResult<PathBuf> {
    let mut matches = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some(extension))
        .collect::<Vec<_>>();
    matches.sort();
    matches
        .into_iter()
        .next()
        .ok_or_else(|| AppError::ExternalCommand(format!("OpenDataLoader {extension} 결과가 없습니다.")))
}

pub fn load_extraction_summary(conn: &Connection, run_id: &str) -> AppResult<ExtractionRunSummary> {
    let summary = conn.query_row(
        "SELECT id, document_id, status, mode, json_path, markdown_path, error_message
         FROM extraction_runs WHERE id = ?",
        [run_id],
        |row| {
            Ok(ExtractionRunSummary {
                id: row.get(0)?,
                document_id: row.get(1)?,
                status: row.get(2)?,
                mode: row.get(3)?,
                json_path: row.get(4)?,
                markdown_path: row.get(5)?,
                error_message: row.get(6)?,
            })
        },
    )?;
    Ok(summary)
}
```

- [ ] **Step 4: Add extraction commands**

Create `apps/rfp-desktop/src-tauri/src/commands/extraction.rs`:

```rust
use std::path::PathBuf;

use tauri::State;

use crate::domain::ExtractionRunSummary;
use crate::error::AppResult;
use crate::opendataloader_adapter::{self, OpenDataLoaderDiagnostic};
use crate::state::AppState;

#[tauri::command]
pub fn diagnose_opendataloader(cli_path: Option<String>) -> OpenDataLoaderDiagnostic {
    opendataloader_adapter::diagnose(cli_path.map(PathBuf::from))
}

#[tauri::command]
pub fn run_fast_extraction(
    document_id: String,
    cli_path: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<ExtractionRunSummary> {
    let conn = state.connect()?;
    opendataloader_adapter::run_fast_extraction(
        &conn,
        &state.app_data_dir,
        &document_id,
        cli_path.map(PathBuf::from),
    )
}
```

Modify `apps/rfp-desktop/src-tauri/src/commands/mod.rs`:

```rust
pub mod documents;
pub mod extraction;
```

- [ ] **Step 5: Register commands and module**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs` to include:

```rust
mod opendataloader_adapter;
```

Add commands to `tauri::generate_handler!`:

```rust
commands::extraction::diagnose_opendataloader,
commands::extraction::run_fast_extraction
```

- [ ] **Step 6: Run test to verify pass**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml opendataloader_adapter::tests::fast_mode_args_are_bounded_and_explicit
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/rfp-desktop/src-tauri
git commit -m "feat: run opendataloader fast extraction"
```

Expected: commit succeeds.

## Task 5: Normalize OpenDataLoader JSON Blocks

**Files:**
- Create: `fixtures/opendataloader/sample-output.json`
- Create: `apps/rfp-desktop/src-tauri/src/block_normalizer/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create JSON fixture**

Create `fixtures/opendataloader/sample-output.json`:

```json
[
  {
    "id": 42,
    "type": "heading",
    "page number": 1,
    "bounding box": [72.0, 700.0, 540.0, 730.0],
    "heading level": 1,
    "content": "사업 개요"
  },
  {
    "id": "req-1",
    "kind": "table",
    "page_number": 2,
    "bbox": [72.0, 400.0, 540.0, 650.0],
    "text": "요구사항 고유번호 SFR-001 통합 로그인 기능"
  },
  {
    "elements": [
      {
        "id": "risk-1",
        "role": "paragraph",
        "page": 3,
        "bounding box": [72.0, 300.0, 540.0, 350.0],
        "value": "필요 시 추가 산출물을 무상으로 제출한다."
      }
    ]
  }
]
```

- [ ] **Step 2: Write failing normalizer test**

Add this test to `apps/rfp-desktop/src-tauri/src/block_normalizer/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::db;

    #[test]
    fn normalizes_key_variants_and_nested_elements() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let json_path = temp.path().join("sample-output.json");
        fs::write(&json_path, include_str!("../../../../../fixtures/opendataloader/sample-output.json"))
            .expect("write fixture");
        let conn = db::open_database(&db_path).expect("open db");
        seed_document_and_run(&conn, "doc-1", "run-1");

        let count = normalize_extraction_json(&conn, "doc-1", "run-1", &json_path).expect("normalize");

        assert_eq!(count, 3);
        let risk_text: String = conn
            .query_row(
                "SELECT text FROM document_blocks WHERE source_element_id = 'risk-1'",
                [],
                |row| row.get(0),
            )
            .expect("risk text");
        assert!(risk_text.contains("무상"));
    }

    fn seed_document_and_run(conn: &rusqlite::Connection, document_id: &str, run_id: &str) {
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES (?, 'sample.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'extracting')",
            [document_id],
        )
        .expect("insert doc");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES (?, ?, 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
            [run_id, document_id],
        )
        .expect("insert run");
    }
}
```

- [ ] **Step 3: Run test to verify failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml block_normalizer::tests::normalizes_key_variants_and_nested_elements
```

Expected: FAIL because `normalize_extraction_json` is not implemented.

- [ ] **Step 4: Implement normalizer**

Create `apps/rfp-desktop/src-tauri/src/block_normalizer/mod.rs`:

```rust
use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};
use serde_json::Value;
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone)]
struct NormalizedBlock {
    source_element_id: String,
    page_number: i64,
    kind: String,
    heading_level: Option<i64>,
    text: String,
    bbox_json: Option<String>,
    raw_json: String,
}

pub fn normalize_extraction_json(
    conn: &Connection,
    document_id: &str,
    extraction_run_id: &str,
    json_path: &Path,
) -> AppResult<usize> {
    let content = fs::read_to_string(json_path)?;
    let value: Value = serde_json::from_str(&content)?;
    let mut blocks = Vec::new();
    collect_blocks(&value, &mut blocks)?;

    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM document_blocks WHERE extraction_run_id = ?", [extraction_run_id])?;
    for (index, block) in blocks.iter().enumerate() {
        tx.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                Uuid::new_v4().to_string(),
                extraction_run_id,
                document_id,
                block.source_element_id,
                block.page_number,
                index as i64,
                block.kind,
                block.heading_level,
                block.text,
                block.bbox_json,
                block.raw_json,
            ],
        )?;
    }
    tx.commit()?;
    Ok(blocks.len())
}

fn collect_blocks(value: &Value, blocks: &mut Vec<NormalizedBlock>) -> AppResult<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_blocks(item, blocks)?;
            }
        }
        Value::Object(map) => {
            if let Some(children) = first_array(map, &["elements", "items", "blocks", "kids", "rows", "cells"]) {
                for child in children {
                    collect_blocks(child, blocks)?;
                }
            }
            if let Some(text) = first_string(map, &["text", "content", "markdown", "value"]) {
                blocks.push(NormalizedBlock {
                    source_element_id: first_string(map, &["id", "element_id"])
                        .unwrap_or_else(|| format!("generated-{}", blocks.len())),
                    page_number: first_i64(map, &["page_number", "page", "page number"]).unwrap_or(1),
                    kind: normalize_kind(&first_string(map, &["type", "kind", "role", "category"]).unwrap_or_else(|| "unknown".to_string())),
                    heading_level: first_i64(map, &["heading_level", "heading level", "level"]),
                    text,
                    bbox_json: first_value(map, &["bbox", "bounding_box", "bounding box"]).map(Value::to_string),
                    raw_json: Value::Object(map.clone()).to_string(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn first_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key).and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
    })
}

fn first_i64(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| map.get(*key).and_then(Value::as_i64))
}

fn first_array<'a>(map: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Vec<Value>> {
    keys.iter().find_map(|key| map.get(*key).and_then(Value::as_array))
}

fn first_value<'a>(map: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| map.get(*key))
}

fn normalize_kind(value: &str) -> String {
    match value.to_lowercase().as_str() {
        "heading" | "title" => "heading".to_string(),
        "paragraph" | "text" => "paragraph".to_string(),
        "table" => "table".to_string(),
        "list" | "list_item" => "list".to_string(),
        "image" => "image".to_string(),
        "caption" => "caption".to_string(),
        other if !other.trim().is_empty() => other.to_string(),
        _ => "unknown".to_string(),
    }
}
```

- [ ] **Step 5: Register module**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs` to include:

```rust
mod block_normalizer;
```

- [ ] **Step 6: Run test to verify pass**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml block_normalizer::tests::normalizes_key_variants_and_nested_elements
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/rfp-desktop/src-tauri fixtures/opendataloader
git commit -m "feat: normalize opendataloader document blocks"
```

Expected: commit succeeds.

## Task 6: Add Baseline Analysis and Validation Gate

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`
- Create: `apps/rfp-desktop/src-tauri/src/validation/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing validation test**

Add this test to `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::db;

    #[test]
    fn baseline_analysis_creates_review_needed_project_and_blockers() {
        let temp = tempdir().expect("temp dir");
        let conn = db::open_database(&temp.path().join("test.sqlite3")).expect("open db");
        conn.execute(
            "INSERT INTO documents (id, title, created_at, updated_at, status)
             VALUES ('doc-1', 'sample.pdf', '2026-05-01T00:00:00Z', '2026-05-01T00:00:00Z', 'created')",
            [],
        )
        .expect("insert doc");

        let project_id = create_or_update_baseline_project(&conn, "doc-1").expect("analyze");

        let status: String = conn
            .query_row("SELECT status FROM rfp_projects WHERE id = ?", [&project_id], |row| row.get(0))
            .expect("project status");
        assert_eq!(status, "review_needed");

        let blocker_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'blocker'",
                [&project_id],
                |row| row.get(0),
            )
            .expect("blocker count");
        assert!(blocker_count >= 5);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests::baseline_analysis_creates_review_needed_project_and_blockers
```

Expected: FAIL because baseline analysis is not implemented.

- [ ] **Step 3: Implement validation**

Create `apps/rfp-desktop/src-tauri/src/validation/mod.rs`:

```rust
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct FindingInput {
    pub severity: &'static str,
    pub finding_type: &'static str,
    pub message: &'static str,
    pub target_table: Option<&'static str>,
    pub target_id: Option<String>,
}

pub fn evaluate_baseline_project(conn: &Connection, rfp_project_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM validation_findings WHERE rfp_project_id = ?", [rfp_project_id])?;
    let document_id: String = conn.query_row(
        "SELECT document_id FROM rfp_projects WHERE id = ?",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let block_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM document_blocks WHERE document_id = ?",
        [&document_id],
        |row| row.get(0),
    )?;

    let mut findings = vec![
        FindingInput {
            severity: "blocker",
            finding_type: "missing_business_name",
            message: "사업명이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: Some(rfp_project_id.to_string()),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "missing_client",
            message: "발주기관이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: Some(rfp_project_id.to_string()),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "missing_budget",
            message: "사업예산이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: Some(rfp_project_id.to_string()),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "missing_period",
            message: "사업기간이 추출되지 않았습니다.",
            target_table: Some("rfp_projects"),
            target_id: Some(rfp_project_id.to_string()),
        },
        FindingInput {
            severity: "blocker",
            finding_type: "zero_requirements",
            message: "요구사항이 0건입니다.",
            target_table: Some("rfp_projects"),
            target_id: Some(rfp_project_id.to_string()),
        },
        FindingInput {
            severity: "warning",
            finding_type: "llm_not_used",
            message: "LLM opt-in이 꺼져 구조화가 제한됩니다.",
            target_table: Some("rfp_projects"),
            target_id: Some(rfp_project_id.to_string()),
        },
    ];

    if block_count == 0 {
        findings.push(FindingInput {
            severity: "blocker",
            finding_type: "missing_evidence",
            message: "원문 근거 block이 없습니다.",
            target_table: Some("document_blocks"),
            target_id: None,
        });
    }

    for finding in findings {
        insert_finding(conn, rfp_project_id, finding)?;
    }

    let blocker_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'blocker'",
        [rfp_project_id],
        |row| row.get(0),
    )?;
    let project_status = if blocker_count > 0 { "review_needed" } else { "ready" };
    let document_status = if blocker_count > 0 { "review_needed" } else { "ready" };
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE rfp_projects SET status = ?, updated_at = ? WHERE id = ?",
        params![project_status, now, rfp_project_id],
    )?;
    conn.execute(
        "UPDATE documents SET status = ?, updated_at = ? WHERE id = ?",
        params![document_status, now, document_id],
    )?;
    Ok(())
}

fn insert_finding(conn: &Connection, rfp_project_id: &str, finding: FindingInput) -> AppResult<()> {
    conn.execute(
        "INSERT INTO validation_findings (
            id, rfp_project_id, severity, finding_type, message, target_table, target_id, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            Uuid::new_v4().to_string(),
            rfp_project_id,
            finding.severity,
            finding.finding_type,
            finding.message,
            finding.target_table,
            finding.target_id,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Implement baseline analysis**

Create `apps/rfp-desktop/src-tauri/src/analysis/mod.rs`:

```rust
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::AppResult;
use crate::validation;

const ANALYSIS_VERSION: &str = "rfp-v2-baseline-2026-05-01";

pub fn create_or_update_baseline_project(conn: &Connection, document_id: &str) -> AppResult<String> {
    let now = Utc::now().to_rfc3339();
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM rfp_projects WHERE document_id = ?",
            [document_id],
            |row| row.get(0),
        )
        .optional()?;

    let project_id = if let Some(project_id) = existing {
        conn.execute(
            "UPDATE rfp_projects
             SET analysis_version = ?, summary = ?, updated_at = ?, status = 'draft'
             WHERE id = ?",
            params![ANALYSIS_VERSION, "LLM 없이 생성된 1차 분석 초안입니다.", now, project_id],
        )?;
        project_id
    } else {
        let project_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO rfp_projects (id, document_id, analysis_version, status, summary, created_at, updated_at)
             VALUES (?, ?, ?, 'draft', ?, ?, ?)",
            params![
                project_id,
                document_id,
                ANALYSIS_VERSION,
                "LLM 없이 생성된 1차 분석 초안입니다.",
                now,
                now
            ],
        )?;
        project_id
    };

    validation::evaluate_baseline_project(conn, &project_id)?;
    Ok(project_id)
}
```

- [ ] **Step 5: Register modules**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs` to include:

```rust
mod analysis;
mod validation;
```

- [ ] **Step 6: Run test to verify pass**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml analysis::tests::baseline_analysis_creates_review_needed_project_and_blockers
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/rfp-desktop/src-tauri
git commit -m "feat: add baseline validation gate"
```

Expected: commit succeeds.

## Task 7: Orchestrate First Analysis Pipeline

**Files:**
- Create: `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/rfp-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing orchestration test**

Add this test to `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::db;
    use crate::document_ingestion;

    #[test]
    fn summarize_document_reports_review_needed_after_blocks_and_validation() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("test.sqlite3");
        let pdf_path = temp.path().join("sample.pdf");
        fs::write(&pdf_path, b"%PDF-1.7\nsample").expect("write pdf");
        let conn = db::open_database(&db_path).expect("open db");
        let doc = document_ingestion::register_document(&conn, &pdf_path).expect("register");
        conn.execute(
            "INSERT INTO extraction_runs (id, document_id, provider, mode, status, started_at)
             VALUES ('run-1', ?, 'opendataloader', 'fast', 'succeeded', '2026-05-01T00:00:00Z')",
            [&doc.id],
        )
        .expect("insert run");
        conn.execute(
            "INSERT INTO document_blocks (
                id, extraction_run_id, document_id, source_element_id, page_number, block_index,
                kind, heading_level, text, bbox_json, raw_json
             ) VALUES (
                'block-1', 'run-1', ?, 'el-1', 1, 0, 'paragraph', NULL,
                '요구사항 SFR-001 통합 로그인 기능', NULL, '{}'
             )",
            [&doc.id],
        )
        .expect("insert block");

        let summary = run_baseline_analysis_for_document(&conn, &doc.id).expect("baseline");

        assert_eq!(summary.document.status, "review_needed");
        assert_eq!(summary.review_needed_count, 1);
        assert_eq!(summary.failed_count, 0);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::pipeline::tests::summarize_document_reports_review_needed_after_blocks_and_validation
```

Expected: FAIL because `run_baseline_analysis_for_document` is not implemented.

- [ ] **Step 3: Implement pipeline command**

Create `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`:

```rust
use rusqlite::Connection;
use tauri::State;

use crate::analysis;
use crate::document_ingestion;
use crate::domain::PipelineSummary;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn analyze_document_baseline(
    document_id: String,
    state: State<'_, AppState>,
) -> AppResult<PipelineSummary> {
    let conn = state.connect()?;
    run_baseline_analysis_for_document(&conn, &document_id)
}

pub fn run_baseline_analysis_for_document(conn: &Connection, document_id: &str) -> AppResult<PipelineSummary> {
    analysis::create_or_update_baseline_project(conn, document_id)?;
    let document = document_ingestion::load_document_summary(conn, document_id)?;
    let ready_count = count_documents_by_status(conn, "ready")?;
    let review_needed_count = count_documents_by_status(conn, "review_needed")?;
    let failed_count = count_documents_by_status(conn, "failed")?;
    Ok(PipelineSummary {
        document,
        extraction: None,
        ready_count,
        review_needed_count,
        failed_count,
    })
}

fn count_documents_by_status(conn: &Connection, status: &str) -> AppResult<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE status = ?",
        [status],
        |row| row.get(0),
    )?;
    Ok(count)
}
```

Modify `apps/rfp-desktop/src-tauri/src/commands/mod.rs`:

```rust
pub mod documents;
pub mod extraction;
pub mod pipeline;
```

Add command to `tauri::generate_handler!`:

```rust
commands::pipeline::analyze_document_baseline
```

- [ ] **Step 4: Run test to verify pass**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml commands::pipeline::tests::summarize_document_reports_review_needed_after_blocks_and_validation
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/rfp-desktop/src-tauri
git commit -m "feat: orchestrate baseline rfp analysis"
```

Expected: commit succeeds.

## Task 8: Build First-Screen UI

**Files:**
- Create: `apps/rfp-desktop/src/lib/types.ts`
- Create: `apps/rfp-desktop/src/lib/api.ts`
- Create: `apps/rfp-desktop/src/components/StatusBadge.tsx`
- Create: `apps/rfp-desktop/src/components/QualityGate.tsx`
- Create: `apps/rfp-desktop/src/components/DocumentList.tsx`
- Create: `apps/rfp-desktop/src/components/BlockPreview.tsx`
- Create: `apps/rfp-desktop/vitest.config.ts`
- Modify: `apps/rfp-desktop/src/App.tsx`
- Modify: `apps/rfp-desktop/src/styles.css`
- Create: `apps/rfp-desktop/src/App.test.tsx`

- [ ] **Step 1: Write failing UI test**

Create `apps/rfp-desktop/src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it } from "vitest";
import { StatusBadge } from "./components/StatusBadge";

describe("StatusBadge", () => {
  it("renders Korean review-needed label", () => {
    render(<StatusBadge status="review_needed" />);
    expect(screen.getByText("검토 필요")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify failure**

```bash
npm run test --prefix apps/rfp-desktop
```

Expected: FAIL because `StatusBadge` is not implemented.

- [ ] **Step 3: Add Vitest config**

Create `apps/rfp-desktop/vitest.config.ts`:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
  },
});
```

- [ ] **Step 4: Add frontend types**

Create `apps/rfp-desktop/src/lib/types.ts`:

```ts
export type DocumentStatus =
  | "created"
  | "extracting"
  | "analyzing"
  | "review_needed"
  | "ready"
  | "failed";

export type DocumentSummary = {
  id: string;
  title: string;
  status: DocumentStatus;
  fileName?: string | null;
  blockerCount: number;
  warningCount: number;
  blockCount: number;
};

export type OpenDataLoaderDiagnostic = {
  cliFound: boolean;
  javaFound: boolean;
  cliMessage: string;
  javaMessage: string;
};
```

- [ ] **Step 5: Add Tauri API wrapper**

Create `apps/rfp-desktop/src/lib/api.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { DocumentSummary, OpenDataLoaderDiagnostic } from "./types";

export function listDocuments(): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents");
}

export function registerDocumentByPath(path: string): Promise<DocumentSummary> {
  return invoke<DocumentSummary>("register_document_by_path", { path });
}

export function diagnoseOpenDataLoader(): Promise<OpenDataLoaderDiagnostic> {
  return invoke<OpenDataLoaderDiagnostic>("diagnose_opendataloader");
}

export function runFastExtraction(documentId: string): Promise<void> {
  return invoke<void>("run_fast_extraction", { documentId });
}

export function analyzeDocumentBaseline(documentId: string): Promise<void> {
  return invoke<void>("analyze_document_baseline", { documentId });
}
```

- [ ] **Step 6: Add status badge**

Create `apps/rfp-desktop/src/components/StatusBadge.tsx`:

```tsx
import type { DocumentStatus } from "../lib/types";

const labels: Record<DocumentStatus, string> = {
  created: "문서 대기",
  extracting: "문서 구조 추출 중",
  analyzing: "요구사항 분석 중",
  review_needed: "검토 필요",
  ready: "확정 가능",
  failed: "실패",
};

export function StatusBadge({ status }: { status: DocumentStatus }) {
  return <span className={`status status-${status}`}>{labels[status]}</span>;
}
```

- [ ] **Step 7: Add quality summary**

Create `apps/rfp-desktop/src/components/QualityGate.tsx`:

```tsx
import type { DocumentSummary } from "../lib/types";

export function QualityGate({ document }: { document: DocumentSummary | null }) {
  if (!document) {
    return <section className="quality-empty">문서를 추가하면 품질 상태가 표시됩니다.</section>;
  }

  return (
    <section className="quality-panel">
      <div>
        <span className="metric-label">Blocker</span>
        <strong>{document.blockerCount}</strong>
      </div>
      <div>
        <span className="metric-label">Warning</span>
        <strong>{document.warningCount}</strong>
      </div>
      <div>
        <span className="metric-label">Blocks</span>
        <strong>{document.blockCount}</strong>
      </div>
    </section>
  );
}
```

- [ ] **Step 8: Add document list**

Create `apps/rfp-desktop/src/components/DocumentList.tsx`:

```tsx
import type { DocumentSummary } from "../lib/types";
import { StatusBadge } from "./StatusBadge";

type Props = {
  documents: DocumentSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
};

export function DocumentList({ documents, selectedId, onSelect }: Props) {
  return (
    <aside className="document-list">
      {documents.map((document) => (
        <button
          className={document.id === selectedId ? "document-row selected" : "document-row"}
          key={document.id}
          onClick={() => onSelect(document.id)}
          type="button"
        >
          <span className="document-title">{document.title}</span>
          <StatusBadge status={document.status} />
        </button>
      ))}
    </aside>
  );
}
```

- [ ] **Step 9: Add block preview empty-state component**

Create `apps/rfp-desktop/src/components/BlockPreview.tsx`:

```tsx
import type { DocumentSummary } from "../lib/types";

export function BlockPreview({ document }: { document: DocumentSummary | null }) {
  if (!document) {
    return <section className="block-preview">원문 block 미리보기가 여기에 표시됩니다.</section>;
  }

  return (
    <section className="block-preview">
      <h2>{document.title}</h2>
      <p>{document.blockCount}개 원문 block이 저장되어 있습니다.</p>
    </section>
  );
}
```

- [ ] **Step 10: Replace `App.tsx` with workspace UI**

Modify `apps/rfp-desktop/src/App.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { FilePlus, Play, RefreshCw } from "lucide-react";
import "./styles.css";
import { BlockPreview } from "./components/BlockPreview";
import { DocumentList } from "./components/DocumentList";
import { QualityGate } from "./components/QualityGate";
import { StatusBadge } from "./components/StatusBadge";
import {
  analyzeDocumentBaseline,
  diagnoseOpenDataLoader,
  listDocuments,
  registerDocumentByPath,
  runFastExtraction,
} from "./lib/api";
import type { DocumentSummary, OpenDataLoaderDiagnostic } from "./lib/types";

export default function App() {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pathInput, setPathInput] = useState("");
  const [diagnostic, setDiagnostic] = useState<OpenDataLoaderDiagnostic | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selected = useMemo(
    () => documents.find((document) => document.id === selectedId) ?? documents[0] ?? null,
    [documents, selectedId],
  );

  async function refresh() {
    const next = await listDocuments();
    setDocuments(next);
    setSelectedId((current) => current ?? next[0]?.id ?? null);
  }

  useEffect(() => {
    refresh().catch((nextError) => setError(String(nextError)));
  }, []);

  async function handleRegister() {
    setError(null);
    const document = await registerDocumentByPath(pathInput);
    await refresh();
    setSelectedId(document.id);
  }

  async function handleDiagnose() {
    setError(null);
    setDiagnostic(await diagnoseOpenDataLoader());
  }

  async function handleAnalyze() {
    if (!selected) return;
    setError(null);
    await runFastExtraction(selected.id);
    await analyzeDocumentBaseline(selected.id);
    await refresh();
  }

  return (
    <main className="workspace">
      <header className="topbar">
        <div>
          <h1>RFP 분석 작업대</h1>
          <p>OpenDataLoader 기반 v2 검증 흐름</p>
        </div>
        <button type="button" onClick={() => refresh().catch((nextError) => setError(String(nextError)))}>
          <RefreshCw size={16} />
          새로고침
        </button>
      </header>

      <section className="toolbar">
        <input
          aria-label="PDF 경로"
          onChange={(event) => setPathInput(event.target.value)}
          placeholder="/absolute/path/to/rfp.pdf"
          value={pathInput}
        />
        <button disabled={!pathInput.trim()} onClick={() => handleRegister().catch((nextError) => setError(String(nextError)))} type="button">
          <FilePlus size={16} />
          문서 추가
        </button>
        <button onClick={() => handleDiagnose().catch((nextError) => setError(String(nextError)))} type="button">
          진단
        </button>
        <button disabled={!selected} onClick={() => handleAnalyze().catch((nextError) => setError(String(nextError)))} type="button">
          <Play size={16} />
          추출/분석
        </button>
      </section>

      {error ? <div className="error">{error}</div> : null}
      {diagnostic ? (
        <section className="diagnostic">
          <span>CLI {diagnostic.cliFound ? "확인됨" : "없음"}</span>
          <span>Java {diagnostic.javaFound ? "확인됨" : "없음"}</span>
        </section>
      ) : null}

      <section className="content">
        <DocumentList documents={documents} selectedId={selected?.id ?? null} onSelect={setSelectedId} />
        <section className="detail">
          {selected ? (
            <div className="detail-heading">
              <div>
                <h2>{selected.title}</h2>
                <StatusBadge status={selected.status} />
              </div>
            </div>
          ) : null}
          <QualityGate document={selected} />
          <BlockPreview document={selected} />
        </section>
      </section>
    </main>
  );
}
```

- [ ] **Step 11: Add restrained workbench CSS**

Modify `apps/rfp-desktop/src/styles.css`:

```css
:root {
  color: #202124;
  background: #f7f8fa;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-width: 920px;
}

button,
input {
  font: inherit;
}

button {
  align-items: center;
  background: #ffffff;
  border: 1px solid #c8ced8;
  border-radius: 6px;
  color: #202124;
  display: inline-flex;
  gap: 6px;
  min-height: 36px;
  padding: 0 12px;
}

button:disabled {
  color: #8b94a3;
}

.workspace {
  min-height: 100vh;
}

.topbar {
  align-items: center;
  background: #ffffff;
  border-bottom: 1px solid #d9dee7;
  display: flex;
  justify-content: space-between;
  padding: 16px 20px;
}

.topbar h1,
.detail h2 {
  font-size: 20px;
  line-height: 1.2;
  margin: 0;
}

.topbar p {
  color: #5c6575;
  margin: 4px 0 0;
}

.toolbar {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(360px, 1fr) auto auto auto;
  padding: 12px 20px;
}

.toolbar input {
  border: 1px solid #c8ced8;
  border-radius: 6px;
  min-height: 36px;
  padding: 0 10px;
}

.content {
  display: grid;
  grid-template-columns: 320px 1fr;
  min-height: calc(100vh - 121px);
}

.document-list {
  border-right: 1px solid #d9dee7;
  padding: 12px;
}

.document-row {
  align-items: flex-start;
  border: 1px solid transparent;
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 76px;
  justify-content: center;
  margin-bottom: 8px;
  width: 100%;
}

.document-row.selected {
  border-color: #4877b8;
}

.document-title {
  overflow: hidden;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: 100%;
}

.detail {
  display: grid;
  gap: 12px;
  grid-template-rows: auto auto 1fr;
  padding: 16px;
}

.detail-heading {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.status {
  border-radius: 999px;
  display: inline-flex;
  font-size: 12px;
  line-height: 20px;
  padding: 0 8px;
  width: fit-content;
}

.status-created,
.status-extracting,
.status-analyzing {
  background: #e9eef7;
  color: #24466f;
}

.status-review_needed {
  background: #fff2d7;
  color: #714600;
}

.status-ready {
  background: #dff3e8;
  color: #145332;
}

.status-failed {
  background: #ffe1df;
  color: #84211b;
}

.quality-panel {
  background: #ffffff;
  border: 1px solid #d9dee7;
  border-radius: 8px;
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(3, minmax(120px, 1fr));
  padding: 14px;
}

.quality-panel div {
  display: grid;
  gap: 4px;
}

.metric-label {
  color: #5c6575;
  font-size: 12px;
}

.block-preview,
.quality-empty,
.diagnostic,
.error {
  background: #ffffff;
  border: 1px solid #d9dee7;
  border-radius: 8px;
  padding: 14px;
}

.error {
  border-color: #d55c55;
  color: #84211b;
  margin: 0 20px 12px;
}

.diagnostic {
  display: flex;
  gap: 12px;
  margin: 0 20px 12px;
}
```

- [ ] **Step 12: Run UI test and build**

```bash
npm run test --prefix apps/rfp-desktop
npm run build --prefix apps/rfp-desktop
```

Expected: both commands exit 0.

- [ ] **Step 13: Commit**

```bash
git add apps/rfp-desktop/src apps/rfp-desktop/vitest.config.ts apps/rfp-desktop/package.json apps/rfp-desktop/package-lock.json
git commit -m "feat: add rfp analysis workbench ui"
```

Expected: commit succeeds.

## Task 9: Add Real PDF Smoke Procedure

**Files:**
- Create: `tests/smoke/README.md`
- Create: `apps/rfp-desktop/src-tauri/src/bin/smoke_first_pdf.rs`
- Modify: `apps/rfp-desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Add smoke README**

Create `tests/smoke/README.md`:

```markdown
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
```

- [ ] **Step 2: Write smoke binary**

Create `apps/rfp-desktop/src-tauri/src/bin/smoke_first_pdf.rs`:

```rust
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use rfp_desktop_lib::analysis;
use rfp_desktop_lib::block_normalizer;
use rfp_desktop_lib::db;
use rfp_desktop_lib::document_ingestion;
use rfp_desktop_lib::opendataloader_adapter;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("failed=true error={error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let pdf_path = env::args()
        .nth(1)
        .ok_or("usage: smoke_first_pdf /absolute/path/to/rfp.pdf")?;
    let root = env::current_dir()?;
    let smoke_dir = root.join(".smoke-rfp-v2");
    std::fs::create_dir_all(&smoke_dir)?;
    let db_path = smoke_dir.join("smoke.sqlite3");
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }

    let conn = db::open_database(&db_path)?;
    let document = document_ingestion::register_document(&conn, &PathBuf::from(pdf_path))?;
    let extraction = opendataloader_adapter::run_fast_extraction(&conn, &smoke_dir, &document.id, None)?;
    let json_path = extraction
        .json_path
        .as_ref()
        .ok_or("OpenDataLoader JSON path missing")?;
    let block_count = block_normalizer::normalize_extraction_json(
        &conn,
        &document.id,
        &extraction.id,
        &PathBuf::from(json_path),
    )?;
    let project_id = analysis::create_or_update_baseline_project(&conn, &document.id)?;
    let generated_count: i64 = conn.query_row("SELECT COUNT(*) FROM rfp_projects", [], |row| row.get(0))?;
    let ready_count: i64 = conn.query_row("SELECT COUNT(*) FROM documents WHERE status = 'ready'", [], |row| row.get(0))?;
    let review_needed_count: i64 = conn.query_row("SELECT COUNT(*) FROM documents WHERE status = 'review_needed'", [], |row| row.get(0))?;
    let failed_count: i64 = conn.query_row("SELECT COUNT(*) FROM documents WHERE status = 'failed'", [], |row| row.get(0))?;
    let blocker_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'blocker'",
        [&project_id],
        |row| row.get(0),
    )?;
    let warning_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM validation_findings WHERE rfp_project_id = ? AND severity = 'warning'",
        [&project_id],
        |row| row.get(0),
    )?;

    println!("document_id={}", document.id);
    println!("extraction_status={}", extraction.status);
    println!("document_blocks={block_count}");
    println!("generated_count={generated_count}");
    println!("ready_count={ready_count}");
    println!("review_needed_count={review_needed_count}");
    println!("failed_count={failed_count}");
    println!("blocker_count={blocker_count}");
    println!("warning_count={warning_count}");

    if failed_count > 0 {
        Ok(ExitCode::from(1))
    } else if blocker_count > 0 {
        Ok(ExitCode::from(2))
    } else {
        Ok(ExitCode::from(0))
    }
}
```

- [ ] **Step 3: Export Rust modules for smoke binary**

Modify `apps/rfp-desktop/src-tauri/src/lib.rs` module declarations so these modules are public:

```rust
pub mod analysis;
pub mod block_normalizer;
pub mod db;
pub mod domain;
pub mod document_ingestion;
pub mod error;
pub mod opendataloader_adapter;
```

Keep command-only modules private:

```rust
mod commands;
mod state;
mod validation;
```

- [ ] **Step 4: Confirm crate name**

Open `apps/rfp-desktop/src-tauri/Cargo.toml` and confirm the library crate name. If the scaffold uses:

```toml
[lib]
name = "rfp_desktop_lib"
```

the smoke binary import path in Step 2 is correct. If the scaffold created a different `[lib].name`, rename it to:

```toml
[lib]
name = "rfp_desktop_lib"
crate-type = ["staticlib", "cdylib", "rlib"]
```

- [ ] **Step 5: Verify smoke binary compiles**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
cargo build --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf
```

Expected: both commands exit 0.

- [ ] **Step 6: Run real PDF smoke**

Use a real local RFP PDF path:

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- /absolute/path/to/rfp.pdf
```

Expected for this first vertical slice:

```text
extraction_status=succeeded
document_blocks=<number greater than 0>
generated_count=1
review_needed_count=1
failed_count=0
blocker_count=<number greater than 0>
```

Exit code should be 2 because baseline analysis intentionally creates blockers until candidate/LLM/domain extraction is added.

- [ ] **Step 7: Commit**

```bash
git add apps/rfp-desktop/src-tauri tests/smoke
git commit -m "test: add first real rfp smoke"
```

Expected: commit succeeds.

## Task 10: Final Verification Checkpoint

**Files:**
- No new files.

- [ ] **Step 1: Run Rust tests**

```bash
cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml
```

Expected: all tests pass.

- [ ] **Step 2: Run frontend tests**

```bash
npm run test --prefix apps/rfp-desktop
```

Expected: all tests pass.

- [ ] **Step 3: Build frontend**

```bash
npm run build --prefix apps/rfp-desktop
```

Expected: build exits 0.

- [ ] **Step 4: Run Tauri development app**

```bash
npm run tauri dev --prefix apps/rfp-desktop
```

Expected:
- Desktop window opens to `RFP 분석 작업대`.
- A PDF absolute path can be entered.
- `문서 추가` creates a row.
- `진단` reports CLI and Java status.
- `추출/분석` moves the row to `검토 필요` or `실패`.
- If extraction succeeds, block count is greater than 0.

- [ ] **Step 5: Run real PDF smoke**

```bash
cargo run --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml --bin smoke_first_pdf -- /absolute/path/to/rfp.pdf
```

Expected:
- `extraction_status=succeeded`
- `document_blocks` is greater than 0
- `generated_count=1`
- `ready_count=0`
- `review_needed_count=1`
- `failed_count=0`
- `blocker_count` is greater than 0
- exit code is 2

- [ ] **Step 6: Commit verification notes**

Create a commit only if files changed during verification:

```bash
git status --short
git add apps/rfp-desktop tests/smoke fixtures/opendataloader
git commit -m "chore: stabilize first rfp vertical slice"
```

Expected: either a commit succeeds or `git status --short` shows no changes.

## Completion Criteria

This plan is complete when all are true:

- `cargo test --manifest-path apps/rfp-desktop/src-tauri/Cargo.toml` passes.
- `npm run test --prefix apps/rfp-desktop` passes.
- `npm run build --prefix apps/rfp-desktop` passes.
- `npm run tauri dev --prefix apps/rfp-desktop` opens the Korean workbench UI.
- A real PDF smoke creates document, extraction, block, project, and finding rows.
- Smoke output reports generated and quality status separately.
- A blocker state is treated as `검토 필요`, not as a failed generation.

## Next Plan Queue

After this vertical slice passes, create these separate implementation plans:

1. Candidate extractor plan: `rfp_fields` and requirement candidate bundles from `document_blocks`.
2. LLM contract plan: OpenAI/Gemini structured output adapters, schema validation, and `llm_runs`.
3. Domain writer plan: requirements, procurement items, staffing, deliverables, acceptance criteria, risk clauses, evidence links.
4. Review UI plan: overview, BOM, staffing/MM, requirements, risk, source evidence viewer.
5. Export plan: DB snapshot to Markdown, JSON, and Docx with quality findings.

## Self-Review

Spec coverage:
- FR-001 is covered by Task 3.
- FR-002 is covered by Task 4.
- FR-003 is covered by Task 5.
- FR-004 is covered by Task 6.
- FR-008 is covered by Task 6 and Task 8.
- Real RFP smoke and generated/ready/review_needed separation are covered by Task 9 and Task 10.

Intentional gaps:
- FR-005, FR-006, FR-007, FR-009, and FR-010 are not in this first vertical slice. They need separate plans because they touch independent subsystems and should not block the first smoke.

Placeholder scan:
- No empty implementation steps remain.
- Every file path in the first vertical slice is concrete.
- Every verification step has an expected result.

Type consistency:
- Rust DTO fields use `serde(rename_all = "camelCase")`.
- TypeScript fields match Rust command output names.
- Document statuses match `spec/08_ui_product_flow.md`.
- Validation blocker names match `spec/09_quality_gate.md`.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-01-tauri-rfp-v2-vertical-slice.md`. Two execution options:

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, and keep each commit small.

**2. Inline Execution** - Execute tasks in this session using superpowers:executing-plans, with checkpoints after Task 3, Task 6, Task 8, and Task 10.

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

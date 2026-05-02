PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS llm_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  offline_mode INTEGER NOT NULL DEFAULT 1 CHECK (offline_mode IN (0, 1)),
  provider TEXT NOT NULL DEFAULT 'openai' CHECK (provider IN ('openai', 'gemini')),
  model TEXT NOT NULL DEFAULT 'gpt-5.5',
  api_key_ref TEXT,
  updated_at TEXT NOT NULL
);

INSERT INTO llm_settings (
  id, enabled, offline_mode, provider, model, api_key_ref, updated_at
) VALUES (
  1, 0, 1, 'openai', 'gpt-5.5', NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(id) DO NOTHING;

CREATE TABLE IF NOT EXISTS llm_runs (
  id TEXT PRIMARY KEY,
  extraction_run_id TEXT NOT NULL REFERENCES extraction_runs(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('openai', 'gemini')),
  model TEXT NOT NULL,
  schema_name TEXT NOT NULL CHECK (
    schema_name IN (
      'project_info',
      'requirements',
      'procurement',
      'risk_classification'
    )
  ),
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

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS rfp_fields (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  field_key TEXT NOT NULL CHECK (
    field_key IN (
      'business_name',
      'client',
      'budget',
      'period',
      'contract_method',
      'deadline',
      'evaluation_ratio',
      'requirement_count'
    )
  ),
  label TEXT NOT NULL,
  raw_value TEXT NOT NULL,
  normalized_value TEXT NOT NULL,
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_rfp_fields_project_key
  ON rfp_fields(rfp_project_id, field_key);

CREATE TABLE IF NOT EXISTS evidence_links (
  id TEXT PRIMARY KEY,
  document_block_id TEXT NOT NULL REFERENCES document_blocks(id) ON DELETE CASCADE,
  target_table TEXT NOT NULL CHECK (
    target_table IN (
      'rfp_fields',
      'requirements',
      'procurement_items',
      'staffing_requirements',
      'deliverables',
      'acceptance_criteria',
      'risk_clauses'
    )
  ),
  target_id TEXT NOT NULL,
  quote TEXT NOT NULL,
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_evidence_links_target
  ON evidence_links(target_table, target_id);

CREATE INDEX IF NOT EXISTS idx_evidence_links_block_id
  ON evidence_links(document_block_id);

CREATE TABLE IF NOT EXISTS candidate_bundles (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  bundle_key TEXT NOT NULL CHECK (
    bundle_key IN (
      'project_info_candidates',
      'requirement_candidates',
      'procurement_candidates',
      'staffing_candidates',
      'deliverable_candidates',
      'acceptance_candidates',
      'risk_candidates'
    )
  ),
  bundle_json TEXT NOT NULL,
  candidate_count INTEGER NOT NULL CHECK (candidate_count >= 0),
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_candidate_bundles_project_key
  ON candidate_bundles(rfp_project_id, bundle_key);

CREATE INDEX IF NOT EXISTS idx_candidate_bundles_document_id
  ON candidate_bundles(document_id);

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS requirements (
  id TEXT PRIMARY KEY,
  rfp_project_id TEXT NOT NULL REFERENCES rfp_projects(id) ON DELETE CASCADE,
  requirement_code TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  category TEXT NOT NULL CHECK (
    category IN (
      'functional',
      'technical',
      'security',
      'data',
      'staffing',
      'management',
      'quality',
      'performance',
      'other'
    )
  ),
  mandatory INTEGER NOT NULL CHECK (mandatory IN (0, 1)),
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_requirements_project_code
  ON requirements(rfp_project_id, requirement_code);

CREATE TABLE IF NOT EXISTS procurement_items (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  item_type TEXT NOT NULL CHECK (
    item_type IN ('hardware', 'software', 'license', 'cloud', 'network', 'database', 'security', 'service', 'other')
  ),
  name TEXT NOT NULL,
  spec TEXT NOT NULL,
  quantity REAL,
  quantity_text TEXT NOT NULL DEFAULT '',
  unit TEXT NOT NULL DEFAULT '',
  required INTEGER NOT NULL CHECK (required IN (0, 1)),
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_procurement_items_requirement_id
  ON procurement_items(requirement_id);

CREATE TABLE IF NOT EXISTS staffing_requirements (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  grade TEXT NOT NULL DEFAULT '',
  headcount REAL,
  headcount_text TEXT NOT NULL DEFAULT '',
  mm REAL,
  mm_text TEXT NOT NULL DEFAULT '',
  onsite INTEGER,
  onsite_text TEXT NOT NULL DEFAULT '',
  period_text TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_staffing_requirements_requirement_id
  ON staffing_requirements(requirement_id);

CREATE TABLE IF NOT EXISTS deliverables (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  due_text TEXT NOT NULL DEFAULT '',
  format_text TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_deliverables_requirement_id
  ON deliverables(requirement_id);

CREATE TABLE IF NOT EXISTS acceptance_criteria (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  criterion_type TEXT NOT NULL CHECK (
    criterion_type IN ('test', 'performance', 'security', 'inspection', 'sla', 'warranty', 'other')
  ),
  description TEXT NOT NULL,
  threshold TEXT NOT NULL DEFAULT '',
  due_text TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_acceptance_criteria_requirement_id
  ON acceptance_criteria(requirement_id);

CREATE TABLE IF NOT EXISTS risk_clauses (
  id TEXT PRIMARY KEY,
  requirement_id TEXT NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
  risk_type TEXT NOT NULL CHECK (
    risk_type IN ('scope_creep', 'free_work', 'short_schedule', 'liability', 'ambiguous_spec', 'vendor_lock', 'payment', 'security', 'other')
  ),
  severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'blocker')),
  description TEXT NOT NULL,
  recommended_action TEXT NOT NULL DEFAULT '',
  confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  source TEXT NOT NULL CHECK (source IN ('rule', 'llm', 'correction')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_risk_clauses_requirement_id
  ON risk_clauses(requirement_id);

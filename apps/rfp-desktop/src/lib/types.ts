export type DocumentStatus =
  | "created"
  | "extracting"
  | "analyzing"
  | "review_needed"
  | "ready"
  | "failed";

export type FindingSeverity = "blocker" | "warning";

export interface DocumentSummary {
  id: string;
  title: string;
  status: DocumentStatus;
  fileName?: string | null;
  blockerCount: number;
  warningCount: number;
  blockCount: number;
}

export interface BlockPreview {
  id: string;
  pageNumber: number;
  blockIndex: number;
  kind: string;
  text: string;
}

export interface ExtractionRunSummary {
  id: string;
  documentId: string;
  status: string;
  mode: string;
  jsonPath?: string | null;
  markdownPath?: string | null;
  errorMessage?: string | null;
}

export interface PipelineSummary {
  document: DocumentSummary;
  extraction?: ExtractionRunSummary | null;
  readyCount: number;
  reviewNeededCount: number;
  failedCount: number;
}

export interface EvidenceLinkDto {
  id?: string | null;
  documentBlockId: string;
  quote: string;
  confidence: number;
}

export interface RfpFieldDto {
  id: string;
  fieldKey: string;
  label: string;
  rawValue: string;
  normalizedValue: string;
  confidence: number;
  source: string;
  evidence: EvidenceLinkDto[];
}

export interface CandidateBundleSummaryDto {
  bundleKey: string;
  candidateCount: number;
}

export interface CandidateExtractionSummary {
  document: DocumentSummary;
  projectId: string;
  fields: RfpFieldDto[];
  bundles: CandidateBundleSummaryDto[];
  readyCount: number;
  reviewNeededCount: number;
  failedCount: number;
}

export interface OpenDataLoaderDiagnostic {
  cliFound: boolean;
  javaFound: boolean;
  cliMessage: string;
  javaMessage: string;
}

export interface QualityGateSummary {
  blockerCount: number;
  warningCount: number;
  blockCount: number;
}

export type ReviewTab =
  | "overview"
  | "procurement"
  | "staffing"
  | "requirements"
  | "deliverables"
  | "acceptance"
  | "risks";

export interface ReviewProjectDto {
  document: DocumentSummary;
  project?: ReviewProjectSummary | null;
  overviewFields: ReviewFieldDto[];
  candidateBundles: CandidateBundleSummaryDto[];
  requirements: RequirementReviewRow[];
  procurementItems: ProcurementItemReviewRow[];
  staffingRequirements: StaffingReviewRow[];
  deliverables: DeliverableReviewRow[];
  acceptanceCriteria: AcceptanceReviewRow[];
  riskClauses: RiskReviewRow[];
  findings: ValidationFindingDto[];
  metrics: ReviewMetricsDto;
}

export interface ReviewProjectSummary {
  id: string;
  status: string;
  summary: string;
  analysisVersion: string;
}

export interface ReviewFieldDto {
  id: string;
  fieldKey: string;
  label: string;
  rawValue: string;
  normalizedValue: string;
  confidence: number;
  source: string;
  evidenceCount: number;
}

export interface RequirementReviewRow {
  id: string;
  requirementCode: string;
  title: string;
  description: string;
  category: string;
  mandatory: boolean;
  confidence: number;
  source: string;
  evidenceCount: number;
  blockerCount: number;
  warningCount: number;
}

export interface ProcurementItemReviewRow {
  id: string;
  itemType: string;
  name: string;
  spec: string;
  quantity?: number | null;
  unit?: string | null;
  required: boolean;
  confidence: number;
  requirementCode: string;
  requirementTitle: string;
  evidenceCount: number;
  warningCount: number;
}

export interface StaffingReviewRow {
  id: string;
  role: string;
  grade: string;
  headcount?: number | null;
  mm?: number | null;
  onsite?: boolean | null;
  periodText: string;
  requirementCode: string;
  requirementTitle: string;
  evidenceCount: number;
}

export interface DeliverableReviewRow {
  id: string;
  name: string;
  dueText: string;
  formatText: string;
  description: string;
  confidence: number;
  requirementCode: string;
  requirementTitle: string;
  evidenceCount: number;
}

export interface AcceptanceReviewRow {
  id: string;
  criterionType: string;
  description: string;
  threshold: string;
  dueText: string;
  confidence: number;
  requirementCode: string;
  requirementTitle: string;
  evidenceCount: number;
}

export interface RiskReviewRow {
  id: string;
  riskType: string;
  severity: string;
  description: string;
  recommendedAction: string;
  requirementCode: string;
  requirementTitle: string;
  evidenceCount: number;
}

export interface ValidationFindingDto {
  id: string;
  severity: string;
  findingType: string;
  message: string;
  targetTable?: string | null;
  targetId?: string | null;
  createdAt: string;
}

export interface ReviewMetricsDto {
  requirementCount: number;
  procurementCount: number;
  staffingCount: number;
  totalMm?: number | null;
  highRiskCount: number;
  blockerCount: number;
  warningCount: number;
}

export interface EvidenceContextDto {
  targetTable: string;
  targetId: string;
  evidence: EvidenceLinkDto[];
  blocks: SourceBlockDto[];
}

export interface SourceBlockDto {
  id: string;
  pageNumber: number;
  blockIndex: number;
  kind: string;
  text: string;
  bboxJson?: string | null;
  isDirectEvidence: boolean;
}

export interface EvidenceTarget {
  targetTable: string;
  targetId: string;
  title: string;
}

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

export interface DomainWriteSummary {
  rfpProjectId: string;
  fieldsWritten: number;
  requirementsWritten: number;
  procurementItemsWritten: number;
  staffingRequirementsWritten: number;
  deliverablesWritten: number;
  acceptanceCriteriaWritten: number;
  riskClausesWritten: number;
  evidenceLinksWritten: number;
  rejectedRecords: number;
  rejections: Array<{
    severity: string;
    findingType: string;
    message: string;
    targetTable?: string | null;
  }>;
}

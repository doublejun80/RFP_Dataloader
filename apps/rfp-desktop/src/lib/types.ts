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

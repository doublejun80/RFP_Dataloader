import { invoke } from "@tauri-apps/api/core";

import type {
  CandidateExtractionSummary,
  DocumentSummary,
  DomainWriteSummary,
  EvidenceContextDto,
  ExtractionRunSummary,
  LlmProvider,
  LlmRunSummary,
  LlmSchemaName,
  LlmSettings,
  OpenDataLoaderDiagnostic,
  PipelineSummary,
  ReviewProjectDto,
  SaveLlmSettingsRequest,
} from "./types";

export function listDocuments(): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents");
}

export function registerDocumentByPath(path: string): Promise<DocumentSummary> {
  return invoke<DocumentSummary>("register_document_by_path", { path });
}

export function diagnoseOpenDataLoader(): Promise<OpenDataLoaderDiagnostic> {
  return invoke<OpenDataLoaderDiagnostic>("diagnose_opendataloader", {
    cliPath: null,
  });
}

export function runFastExtraction(
  documentId: string,
): Promise<ExtractionRunSummary> {
  return invoke<ExtractionRunSummary>("run_fast_extraction", {
    documentId,
    cliPath: null,
  });
}

export function analyzeDocumentBaseline(
  documentId: string,
): Promise<PipelineSummary> {
  return invoke<PipelineSummary>("analyze_document_baseline", { documentId });
}

export function analyzeDocumentCandidates(
  documentId: string,
): Promise<CandidateExtractionSummary> {
  return invoke<CandidateExtractionSummary>("analyze_document_candidates", {
    documentId,
  });
}

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

export function runLlmDomainAnalysis(
  documentId: string,
): Promise<DomainWriteSummary> {
  return invoke<DomainWriteSummary>("run_llm_domain_analysis", { documentId });
}

export function getReviewProject(documentId: string): Promise<ReviewProjectDto> {
  return invoke<ReviewProjectDto>("get_review_project", { documentId });
}

export function getEvidenceContext(
  targetTable: string,
  targetId: string,
): Promise<EvidenceContextDto> {
  return invoke<EvidenceContextDto>("get_evidence_context", {
    targetTable,
    targetId,
  });
}

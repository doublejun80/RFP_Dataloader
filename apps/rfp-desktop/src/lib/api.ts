import { invoke } from "@tauri-apps/api/core";

import type {
  DocumentSummary,
  ExtractionRunSummary,
  OpenDataLoaderDiagnostic,
  PipelineSummary,
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

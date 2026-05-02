import { useEffect, useMemo, useRef, useState } from "react";
import { FilePlus, Play, RefreshCw, SearchCheck } from "lucide-react";

import "./App.css";
import { CandidateBundlePanel } from "./components/CandidateBundlePanel";
import { DocumentList } from "./components/DocumentList";
import { ProjectInfoPanel } from "./components/ProjectInfoPanel";
import { ReviewWorkbench } from "./components/review/ReviewWorkbench";
import {
  analyzeDocumentCandidates,
  diagnoseOpenDataLoader,
  getEvidenceContext,
  getReviewProject,
  listDocuments,
  registerDocumentByPath,
  runFastExtraction,
} from "./lib/api";
import type {
  CandidateExtractionSummary,
  DocumentSummary,
  EvidenceContextDto,
  EvidenceTarget,
  OpenDataLoaderDiagnostic,
  ReviewProjectDto,
  ReviewTab,
} from "./lib/types";

type ActionName = "refresh" | "register" | "diagnose" | "analyze";

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "작업을 완료하지 못했습니다.";
}

function App() {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pathInput, setPathInput] = useState("");
  const [diagnostic, setDiagnostic] =
    useState<OpenDataLoaderDiagnostic | null>(null);
  const [candidateSummary, setCandidateSummary] =
    useState<CandidateExtractionSummary | null>(null);
  const [review, setReview] = useState<ReviewProjectDto | null>(null);
  const [reviewLoading, setReviewLoading] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [activeReviewTab, setActiveReviewTab] =
    useState<ReviewTab>("overview");
  const [reviewRefreshKey, setReviewRefreshKey] = useState(0);
  const [evidenceTarget, setEvidenceTarget] =
    useState<EvidenceTarget | null>(null);
  const [evidenceContext, setEvidenceContext] =
    useState<EvidenceContextDto | null>(null);
  const [evidenceLoading, setEvidenceLoading] = useState(false);
  const [evidenceError, setEvidenceError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<ActionName | null>(null);
  const requestSeq = useRef(0);
  const evidenceSeq = useRef(0);

  const selectedDocument = useMemo(
    () =>
      documents.find((document) => document.id === selectedId) ??
      documents[0] ??
      null,
    [documents, selectedId],
  );

  async function runAction<T>(
    action: ActionName,
    task: () => Promise<T>,
  ): Promise<T | null> {
    setPendingAction(action);
    setError(null);

    try {
      return await task();
    } catch (nextError) {
      setError(formatError(nextError));
      return null;
    } finally {
      setPendingAction(null);
    }
  }

  async function refreshDocuments() {
    const nextDocuments = await listDocuments();
    setDocuments(nextDocuments);
    setSelectedId((currentId) => {
      if (currentId && nextDocuments.some((document) => document.id === currentId)) {
        return currentId;
      }

      return nextDocuments[0]?.id ?? null;
    });
  }

  useEffect(() => {
    void runAction("refresh", refreshDocuments);
  }, []);

  useEffect(() => {
    setCandidateSummary(null);
    setActiveReviewTab("overview");
    setEvidenceTarget(null);
    setEvidenceContext(null);
    setEvidenceError(null);
  }, [selectedDocument?.id]);

  useEffect(() => {
    if (!selectedDocument) {
      setReview(null);
      setReviewError(null);
      setReviewLoading(false);
      return;
    }

    const seq = requestSeq.current + 1;
    requestSeq.current = seq;
    setReviewLoading(true);
    setReviewError(null);

    getReviewProject(selectedDocument.id)
      .then((nextReview) => {
        if (requestSeq.current === seq) {
          setReview(nextReview);
        }
      })
      .catch((nextError) => {
        if (requestSeq.current === seq) {
          setReviewError(formatError(nextError));
        }
      })
      .finally(() => {
        if (requestSeq.current === seq) {
          setReviewLoading(false);
        }
      });
  }, [selectedDocument, reviewRefreshKey]);

  useEffect(() => {
    if (!evidenceTarget) {
      setEvidenceContext(null);
      setEvidenceError(null);
      setEvidenceLoading(false);
      return;
    }

    const seq = evidenceSeq.current + 1;
    evidenceSeq.current = seq;
    setEvidenceLoading(true);
    setEvidenceError(null);

    getEvidenceContext(evidenceTarget.targetTable, evidenceTarget.targetId)
      .then((nextContext) => {
        if (evidenceSeq.current === seq) {
          setEvidenceContext(nextContext);
        }
      })
      .catch((nextError) => {
        if (evidenceSeq.current === seq) {
          setEvidenceError(formatError(nextError));
        }
      })
      .finally(() => {
        if (evidenceSeq.current === seq) {
          setEvidenceLoading(false);
        }
      });
  }, [evidenceTarget]);

  async function handleRegister() {
    const path = pathInput.trim();

    if (!path) {
      return;
    }

    await runAction("register", async () => {
      const document = await registerDocumentByPath(path);
      await refreshDocuments();
      setSelectedId(document.id);
      setPathInput("");
    });
  }

  async function handleDiagnose() {
    await runAction("diagnose", async () => {
      setDiagnostic(await diagnoseOpenDataLoader());
    });
  }

  async function handleAnalyze() {
    if (!selectedDocument) {
      return;
    }

    await runAction("analyze", async () => {
      await runFastExtraction(selectedDocument.id);
      setCandidateSummary(await analyzeDocumentCandidates(selectedDocument.id));
      await refreshDocuments();
      setReviewRefreshKey((value) => value + 1);
    });
  }

  const isBusy = pendingAction !== null;

  return (
    <main className="workspace">
      <header className="topbar">
        <div>
          <h1>RFP 분석 작업대</h1>
          <p>OpenDataLoader 기반 v2 검증 흐름</p>
        </div>
        <button
          disabled={isBusy}
          onClick={() => void runAction("refresh", refreshDocuments)}
          type="button"
        >
          <RefreshCw aria-hidden="true" size={16} />
          새로고침
        </button>
      </header>

      <form
        className="toolbar"
        onSubmit={(event) => {
          event.preventDefault();
          void handleRegister();
        }}
      >
        <input
          aria-label="PDF 경로"
          onChange={(event) => setPathInput(event.target.value)}
          placeholder="/absolute/path/to/rfp.pdf"
          value={pathInput}
        />
        <button disabled={!pathInput.trim() || isBusy} type="submit">
          <FilePlus aria-hidden="true" size={16} />
          문서 추가
        </button>
        <button
          disabled={isBusy}
          onClick={() => void handleDiagnose()}
          type="button"
        >
          <SearchCheck aria-hidden="true" size={16} />
          진단
        </button>
        <button
          disabled={!selectedDocument || isBusy}
          onClick={() => void handleAnalyze()}
          type="button"
        >
          <Play aria-hidden="true" size={16} />
          추출/분석
        </button>
      </form>

      {error ? (
        <section className="error-banner" role="alert">
          {error}
        </section>
      ) : null}

      {diagnostic ? (
        <section className="diagnostic" aria-label="OpenDataLoader 진단">
          <span>CLI {diagnostic.cliFound ? "확인됨" : "없음"}</span>
          <span>Java {diagnostic.javaFound ? "확인됨" : "없음"}</span>
          <span>{diagnostic.cliMessage}</span>
          <span>{diagnostic.javaMessage}</span>
        </section>
      ) : null}

      <section className="content">
        <DocumentList
          documents={documents}
          onSelect={setSelectedId}
          selectedId={selectedDocument?.id ?? null}
        />
        <section className="detail">
          <ReviewWorkbench
            activeTab={activeReviewTab}
            document={selectedDocument}
            error={reviewError}
            evidenceContext={evidenceContext}
            evidenceError={evidenceError}
            evidenceLoading={evidenceLoading}
            evidenceTarget={evidenceTarget}
            loading={reviewLoading}
            onCloseEvidence={() => setEvidenceTarget(null)}
            onOpenEvidence={setEvidenceTarget}
            onTabChange={setActiveReviewTab}
            review={review}
          />
          {selectedDocument ? (
            <>
              <ProjectInfoPanel fields={candidateSummary?.fields ?? []} />
              <CandidateBundlePanel bundles={candidateSummary?.bundles ?? []} />
            </>
          ) : null}
        </section>
      </section>
    </main>
  );
}

export default App;

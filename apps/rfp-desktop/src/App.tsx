import { useEffect, useMemo, useState } from "react";
import { FilePlus, Play, RefreshCw, SearchCheck } from "lucide-react";

import "./App.css";
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
  const [error, setError] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<ActionName | null>(null);

  const selectedDocument = useMemo(
    () =>
      documents.find((document) => document.id === selectedId) ??
      documents[0] ??
      null,
    [documents, selectedId],
  );

  const selectedQuality = selectedDocument
    ? {
        blockerCount: selectedDocument.blockerCount,
        warningCount: selectedDocument.warningCount,
        blockCount: selectedDocument.blockCount,
      }
    : null;

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
      await analyzeDocumentBaseline(selectedDocument.id);
      await refreshDocuments();
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
          {selectedDocument ? (
            <div className="detail-heading">
              <div>
                <span className="eyeline">선택 문서</span>
                <h2>{selectedDocument.title}</h2>
              </div>
              <StatusBadge status={selectedDocument.status} />
            </div>
          ) : (
            <div className="detail-heading detail-heading--empty">
              <div>
                <span className="eyeline">선택 문서</span>
                <h2>문서 없음</h2>
              </div>
            </div>
          )}

          <QualityGate summary={selectedQuality} />
          <BlockPreview document={selectedDocument} />
        </section>
      </section>
    </main>
  );
}

export default App;

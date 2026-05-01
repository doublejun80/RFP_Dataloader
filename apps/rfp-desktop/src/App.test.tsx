import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import { StatusBadge } from "./components/StatusBadge";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invokeMock(command, args),
}));

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "list_documents") {
      return Promise.resolve([
        {
          id: "doc-1",
          title: "서울시 통합 유지관리 RFP",
          status: "review_needed",
          fileName: "seoul-rfp.pdf",
          blockerCount: 2,
          warningCount: 1,
          blockCount: 37,
        },
      ]);
    }

    if (command === "run_fast_extraction") {
      return Promise.resolve({
        id: "run-1",
        documentId: "doc-1",
        status: "succeeded",
        mode: "fast",
        jsonPath: "/tmp/sample.json",
        markdownPath: "/tmp/sample.md",
        errorMessage: null,
      });
    }

    if (command === "analyze_document_candidates") {
      return Promise.resolve({
        document: {
          id: "doc-1",
          title: "서울시 통합 유지관리 RFP",
          status: "review_needed",
          fileName: "seoul-rfp.pdf",
          blockerCount: 1,
          warningCount: 1,
          blockCount: 37,
        },
        projectId: "project-1",
        fields: [
          {
            id: "field-1",
            fieldKey: "business_name",
            label: "사업명",
            rawValue: "서울시 통합 유지관리 사업",
            normalizedValue: "서울시 통합 유지관리 사업",
            confidence: 0.85,
            source: "rule",
            evidence: [
              {
                documentBlockId: "block-1",
                quote: "사업명: 서울시 통합 유지관리 사업",
                confidence: 0.85,
              },
            ],
          },
        ],
        bundles: [
          { bundleKey: "project_info_candidates", candidateCount: 4 },
          { bundleKey: "risk_candidates", candidateCount: 1 },
        ],
        readyCount: 0,
        reviewNeededCount: 1,
        failedCount: 0,
      });
    }

    return Promise.resolve(null);
  });
});

afterEach(() => {
  cleanup();
});

describe("StatusBadge", () => {
  it("shows review_needed as the Korean review-needed state", () => {
    render(<StatusBadge status="review_needed" />);

    expect(screen.getByText("검토 필요")).toBeInTheDocument();
    expect(screen.getByLabelText("상태: 검토 필요")).toBeInTheDocument();
  });
});

describe("App", () => {
  it("renders the first-screen Korean RFP workbench with document quality counts", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "RFP 분석 작업대" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("PDF 경로")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /문서 추가/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /진단/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /추출\/분석/ })).toBeInTheDocument();

    expect(screen.getAllByText("서울시 통합 유지관리 RFP")).toHaveLength(3);
    expect(screen.getAllByLabelText("상태: 검토 필요")).toHaveLength(2);
    expect(screen.getAllByText("검토 필요")).toHaveLength(3);
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("37")).toBeInTheDocument();
    expect(screen.getByText(/37개 원문 block/)).toBeInTheDocument();
  });

  it("runs candidate analysis and renders project info and bundle counts", async () => {
    render(<App />);

    await screen.findByRole("heading", { name: "RFP 분석 작업대" });

    fireEvent.click(screen.getByRole("button", { name: /추출\/분석/ }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("run_fast_extraction", {
        documentId: "doc-1",
        cliPath: null,
      });
      expect(invokeMock).toHaveBeenCalledWith("analyze_document_candidates", {
        documentId: "doc-1",
      });
    });

    expect(
      await screen.findByText("서울시 통합 유지관리 사업"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("기본정보").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("리스크")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
  });
});

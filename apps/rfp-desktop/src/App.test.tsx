import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import { StatusBadge } from "./components/StatusBadge";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invokeMock(command, args),
}));

const reviewFixture = {
  document: {
    id: "doc-1",
    title: "서울시 통합 유지관리 RFP",
    status: "review_needed",
    fileName: "seoul-rfp.pdf",
    blockerCount: 1,
    warningCount: 1,
    blockCount: 37,
  },
  project: {
    id: "project-1",
    status: "review_needed",
    summary: "검토용 분석 초안",
    analysisVersion: "rfp-v2-domain-test",
  },
  overviewFields: [
    {
      id: "field-1",
      fieldKey: "business_name",
      label: "사업명",
      rawValue: "서울시 통합 유지관리 사업",
      normalizedValue: "서울시 통합 유지관리 사업",
      confidence: 0.91,
      source: "llm",
      evidenceCount: 1,
    },
  ],
  requirements: [
    {
      id: "req-1",
      requirementCode: "SFR-001",
      title: "API Gateway 구성",
      description: "통합 API Gateway를 구성한다.",
      category: "technical",
      mandatory: true,
      confidence: 0.86,
      source: "llm",
      evidenceCount: 1,
      blockerCount: 0,
      warningCount: 1,
    },
  ],
  procurementItems: [
    {
      id: "item-1",
      itemType: "software",
      name: "API Gateway",
      spec: "HA 구성",
      quantity: 1,
      unit: "식",
      required: true,
      confidence: 0.82,
      requirementCode: "SFR-001",
      requirementTitle: "API Gateway 구성",
      evidenceCount: 1,
      warningCount: 0,
    },
  ],
  staffingRequirements: [
    {
      id: "staff-1",
      role: "API 개발자",
      grade: "중급",
      headcount: 1,
      mm: 3,
      onsite: true,
      periodText: "착수 후 3개월",
      requirementCode: "SFR-001",
      requirementTitle: "API Gateway 구성",
      evidenceCount: 1,
    },
  ],
  deliverables: [
    {
      id: "deliverable-1",
      name: "통합시험 결과서",
      dueText: "검수 전",
      formatText: "문서",
      description: "통합시험 결과를 제출한다.",
      confidence: 0.81,
      requirementCode: "SFR-001",
      requirementTitle: "API Gateway 구성",
      evidenceCount: 1,
    },
  ],
  acceptanceCriteria: [
    {
      id: "acceptance-1",
      criterionType: "test",
      description: "통합시험을 통과해야 한다.",
      threshold: "결함 0건",
      dueText: "검수 단계",
      confidence: 0.83,
      requirementCode: "SFR-001",
      requirementTitle: "API Gateway 구성",
      evidenceCount: 1,
    },
  ],
  riskClauses: [
    {
      id: "risk-1",
      riskType: "short_schedule",
      severity: "high",
      description: "구축 기간이 짧다.",
      recommendedAction: "일정 버퍼와 단계 검수를 질의한다.",
      requirementCode: "SFR-001",
      requirementTitle: "API Gateway 구성",
      evidenceCount: 1,
    },
  ],
  findings: [
    {
      id: "finding-1",
      severity: "blocker",
      findingType: "missing_budget",
      message: "사업예산이 추출되지 않았습니다.",
      targetTable: "rfp_projects",
      targetId: "project-1",
      createdAt: "2026-05-02T00:00:00Z",
    },
    {
      id: "finding-2",
      severity: "warning",
      findingType: "low_confidence",
      message: "신뢰도가 낮은 항목이 있습니다.",
      targetTable: "requirements",
      targetId: "req-1",
      createdAt: "2026-05-02T00:00:00Z",
    },
  ],
  metrics: {
    requirementCount: 1,
    procurementCount: 1,
    staffingCount: 1,
    totalMm: 3,
    highRiskCount: 1,
    blockerCount: 1,
    warningCount: 1,
  },
};

const evidenceFixture = {
  targetTable: "requirements",
  targetId: "req-1",
  evidence: [
    {
      id: "ev-req-1",
      documentBlockId: "block-2",
      quote: "SFR-001 API Gateway 구성",
      confidence: 0.92,
    },
  ],
  blocks: [
    {
      id: "block-1",
      pageNumber: 3,
      blockIndex: 1,
      kind: "paragraph",
      text: "사업 개요 문장",
      bboxJson: null,
      isDirectEvidence: false,
    },
    {
      id: "block-2",
      pageNumber: 3,
      blockIndex: 2,
      kind: "table",
      text: "SFR-001 API Gateway 구성",
      bboxJson: "[72,400,540,650]",
      isDirectEvidence: true,
    },
    {
      id: "block-3",
      pageNumber: 3,
      blockIndex: 3,
      kind: "paragraph",
      text: "연계 요구사항 설명",
      bboxJson: null,
      isDirectEvidence: false,
    },
  ],
};

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

    if (command === "get_review_project") {
      return Promise.resolve(reviewFixture);
    }

    if (command === "get_evidence_context") {
      return Promise.resolve(evidenceFixture);
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

    expect(screen.getAllByText("서울시 통합 유지관리 RFP")).toHaveLength(2);
    expect(screen.getAllByLabelText("상태: 검토 필요")).toHaveLength(2);
    expect(screen.getAllByText("검토 필요")).toHaveLength(2);
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("37")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: /개요/ })).toBeInTheDocument();
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
      await screen.findAllByText("서울시 통합 유지관리 사업"),
    ).not.toHaveLength(0);
    expect(screen.getAllByText("기본정보").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("리스크").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("4")).toBeInTheDocument();
  });

  it("renders review overview for the selected document", async () => {
    render(<App />);

    expect(await screen.findByRole("button", { name: /개요/ })).toBeInTheDocument();
    expect(screen.getByText("서울시 통합 유지관리 사업")).toBeInTheDocument();
    expect(screen.getByText("사업예산이 추출되지 않았습니다.")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("get_review_project", {
      documentId: "doc-1",
    });
  });

  it("navigates review tabs and shows domain rows", async () => {
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /구매 항목/ }));
    expect(screen.getByText("API Gateway")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /인력\/MM/ }));
    expect(screen.getByText("API 개발자")).toBeInTheDocument();
    expect(screen.getByText("3 MM")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /요구사항/ }));
    expect(screen.getByText("SFR-001")).toBeInTheDocument();
    expect(screen.getByText("통합 API Gateway를 구성한다.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /산출물/ }));
    expect(screen.getByText("통합시험 결과서")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /검수/ }));
    expect(screen.getByText("결함 0건")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /리스크/ }));
    expect(screen.getByText("단기 일정")).toBeInTheDocument();
    expect(screen.getByText("일정 버퍼와 단계 검수를 질의한다.")).toBeInTheDocument();
  });

  it("opens source evidence from a review row", async () => {
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: /요구사항/ }));
    fireEvent.click(screen.getByRole("button", { name: "원문 근거 보기" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_evidence_context", {
        targetTable: "requirements",
        targetId: "req-1",
      });
    });

    expect(
      await screen.findAllByText("SFR-001 API Gateway 구성"),
    ).not.toHaveLength(0);
    expect(screen.getByText("3쪽 / block 2")).toBeInTheDocument();
    expect(screen.getByText("사업 개요 문장")).toBeInTheDocument();
    expect(screen.getByText("[72,400,540,650]")).toBeInTheDocument();
  });
});

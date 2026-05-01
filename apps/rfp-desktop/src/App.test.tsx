import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
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
});

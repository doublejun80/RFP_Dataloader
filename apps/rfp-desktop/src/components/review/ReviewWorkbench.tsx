import type {
  DocumentSummary,
  EvidenceContextDto,
  EvidenceTarget,
  ProcurementItemReviewRow,
  ReviewProjectDto,
  ReviewTab,
  RiskReviewRow,
  StaffingReviewRow,
} from "../../lib/types";
import { QualityGate } from "../QualityGate";
import { StatusBadge } from "../StatusBadge";
import { EvidenceButton } from "./EvidenceButton";
import { OverviewPanel } from "./OverviewPanel";
import { ReviewDataTable } from "./ReviewDataTable";
import { ReviewTabs } from "./ReviewTabs";
import { SourceEvidenceViewer } from "./SourceEvidenceViewer";
import {
  ACCEPTANCE_TYPE_LABELS,
  CATEGORY_LABELS,
  RISK_TYPE_LABELS,
} from "./reviewLabels";

interface ReviewWorkbenchProps {
  activeTab: ReviewTab;
  document: DocumentSummary | null;
  error: string | null;
  evidenceContext: EvidenceContextDto | null;
  evidenceError: string | null;
  evidenceLoading: boolean;
  evidenceTarget: EvidenceTarget | null;
  loading: boolean;
  onCloseEvidence: () => void;
  onOpenEvidence: (target: EvidenceTarget) => void;
  onTabChange: (tab: ReviewTab) => void;
  review: ReviewProjectDto | null;
}

export function ReviewWorkbench({
  activeTab,
  document,
  error,
  evidenceContext,
  evidenceError,
  evidenceLoading,
  evidenceTarget,
  loading,
  onCloseEvidence,
  onOpenEvidence,
  onTabChange,
  review,
}: ReviewWorkbenchProps) {
  const qualitySummary = review
    ? {
        blockerCount: review.metrics.blockerCount,
        warningCount: review.metrics.warningCount,
        blockCount: review.document.blockCount,
      }
    : document
      ? {
          blockerCount: document.blockerCount,
          warningCount: document.warningCount,
          blockCount: document.blockCount,
        }
      : null;

  if (!document) {
    return (
      <section className="review-workbench">
        <div className="detail-heading detail-heading--empty">
          <div>
            <span className="eyeline">선택 문서</span>
            <h2>문서 없음</h2>
          </div>
        </div>
        <QualityGate summary={null} />
      </section>
    );
  }

  return (
    <section className="review-workbench">
      <div className="detail-heading">
        <div>
          <span className="eyeline">선택 문서</span>
          <h2>{document.title}</h2>
        </div>
        <StatusBadge status={document.status} />
      </div>

      <QualityGate summary={qualitySummary} />

      {loading ? (
        <section className="review-empty" aria-live="polite">
          검토 데이터를 불러오는 중
        </section>
      ) : null}

      {error ? (
        <section className="review-error" role="alert">
          {error}
        </section>
      ) : null}

      {review ? (
        <div className="review-grid">
          <div className="review-main">
            <ReviewTabs
              activeTab={activeTab}
              onTabChange={onTabChange}
              review={review}
            />
            {renderActivePanel(review, activeTab, onOpenEvidence)}
          </div>
          <SourceEvidenceViewer
            context={evidenceContext}
            error={evidenceError}
            loading={evidenceLoading}
            onClose={onCloseEvidence}
            target={evidenceTarget}
          />
        </div>
      ) : null}
    </section>
  );
}

function renderActivePanel(
  review: ReviewProjectDto,
  activeTab: ReviewTab,
  onOpenEvidence: (target: EvidenceTarget) => void,
) {
  if (activeTab === "overview") {
    return <OverviewPanel onOpenEvidence={onOpenEvidence} review={review} />;
  }

  if (activeTab === "procurement") {
    return (
      <ReviewDataTable
        caption="구매 항목"
        emptyMessage="구매 항목 없음"
        headers={["구분", "항목명", "스펙", "수량", "연결 요구사항", "근거", "신뢰도"]}
        rowCount={review.procurementItems.length}
      >
        {review.procurementItems.map((row) => (
          <tr key={row.id}>
            <td>{row.itemType}</td>
            <td>{row.name}</td>
            <td>{row.spec || "-"}</td>
            <td>{formatQuantity(row)}</td>
            <td>
              <strong>{row.requirementCode}</strong>
              <span>{row.requirementTitle}</span>
            </td>
            <td>
              <EvidenceButton
                evidenceCount={row.evidenceCount}
                onClick={() =>
                  onOpenEvidence({
                    targetTable: "procurement_items",
                    targetId: row.id,
                    title: row.name,
                  })
                }
              />
            </td>
            <td>{formatPercent(row.confidence)}</td>
          </tr>
        ))}
      </ReviewDataTable>
    );
  }

  if (activeTab === "staffing") {
    return (
      <ReviewDataTable
        caption="인력/MM"
        emptyMessage="인력/MM 없음"
        headers={["역할", "등급", "인원", "MM", "상주", "기간", "연결 요구사항", "근거"]}
        rowCount={review.staffingRequirements.length}
      >
        {review.staffingRequirements.map((row) => (
          <tr key={row.id}>
            <td>{row.role}</td>
            <td>{row.grade || "-"}</td>
            <td>{formatNullableNumber(row.headcount)}</td>
            <td>{row.mm == null ? "-" : `${row.mm} MM`}</td>
            <td>{formatOnsite(row)}</td>
            <td>{row.periodText || "-"}</td>
            <td>
              <strong>{row.requirementCode}</strong>
              <span>{row.requirementTitle}</span>
            </td>
            <td>
              <EvidenceButton
                evidenceCount={row.evidenceCount}
                onClick={() =>
                  onOpenEvidence({
                    targetTable: "staffing_requirements",
                    targetId: row.id,
                    title: row.role,
                  })
                }
              />
            </td>
          </tr>
        ))}
      </ReviewDataTable>
    );
  }

  if (activeTab === "requirements") {
    return (
      <ReviewDataTable
        caption="요구사항"
        emptyMessage="요구사항 없음"
        headers={["ID", "제목", "분류", "필수", "설명", "근거", "신뢰도"]}
        rowCount={review.requirements.length}
      >
        {review.requirements.map((row) => (
          <tr key={row.id}>
            <td>{row.requirementCode}</td>
            <td>{row.title}</td>
            <td>{CATEGORY_LABELS[row.category] ?? row.category}</td>
            <td>{row.mandatory ? "필수" : "선택"}</td>
            <td>{row.description}</td>
            <td>
              <EvidenceButton
                evidenceCount={row.evidenceCount}
                onClick={() =>
                  onOpenEvidence({
                    targetTable: "requirements",
                    targetId: row.id,
                    title: `${row.requirementCode} ${row.title}`,
                  })
                }
              />
            </td>
            <td>{formatPercent(row.confidence)}</td>
          </tr>
        ))}
      </ReviewDataTable>
    );
  }

  if (activeTab === "deliverables") {
    return (
      <ReviewDataTable
        caption="산출물"
        emptyMessage="산출물 없음"
        headers={["산출물", "제출 시점", "형식", "설명", "연결 요구사항", "근거", "신뢰도"]}
        rowCount={review.deliverables.length}
      >
        {review.deliverables.map((row) => (
          <tr key={row.id}>
            <td>{row.name}</td>
            <td>{row.dueText || "-"}</td>
            <td>{row.formatText || "-"}</td>
            <td>{row.description || "-"}</td>
            <td>
              <strong>{row.requirementCode}</strong>
              <span>{row.requirementTitle}</span>
            </td>
            <td>
              <EvidenceButton
                evidenceCount={row.evidenceCount}
                onClick={() =>
                  onOpenEvidence({
                    targetTable: "deliverables",
                    targetId: row.id,
                    title: row.name,
                  })
                }
              />
            </td>
            <td>{formatPercent(row.confidence)}</td>
          </tr>
        ))}
      </ReviewDataTable>
    );
  }

  if (activeTab === "acceptance") {
    return (
      <ReviewDataTable
        caption="검수"
        emptyMessage="검수 조건 없음"
        headers={["유형", "조건", "기준", "시점", "연결 요구사항", "근거", "신뢰도"]}
        rowCount={review.acceptanceCriteria.length}
      >
        {review.acceptanceCriteria.map((row) => (
          <tr key={row.id}>
            <td>{ACCEPTANCE_TYPE_LABELS[row.criterionType] ?? row.criterionType}</td>
            <td>{row.description}</td>
            <td>{row.threshold || "-"}</td>
            <td>{row.dueText || "-"}</td>
            <td>
              <strong>{row.requirementCode}</strong>
              <span>{row.requirementTitle}</span>
            </td>
            <td>
              <EvidenceButton
                evidenceCount={row.evidenceCount}
                onClick={() =>
                  onOpenEvidence({
                    targetTable: "acceptance_criteria",
                    targetId: row.id,
                    title: row.description,
                  })
                }
              />
            </td>
            <td>{formatPercent(row.confidence)}</td>
          </tr>
        ))}
      </ReviewDataTable>
    );
  }

  return (
    <ReviewDataTable
      caption="리스크"
      emptyMessage="리스크 없음"
      headers={["심각도", "유형", "설명", "권장 조치", "연결 요구사항", "근거"]}
      rowCount={review.riskClauses.length}
    >
      {review.riskClauses.map((row) => (
        <tr key={row.id}>
          <td>{row.severity}</td>
          <td>{RISK_TYPE_LABELS[row.riskType] ?? row.riskType}</td>
          <td>{row.description}</td>
          <td>{row.recommendedAction}</td>
          <td>
            <strong>{row.requirementCode}</strong>
            <span>{row.requirementTitle}</span>
          </td>
          <td>
            <EvidenceButton
              evidenceCount={row.evidenceCount}
              onClick={() =>
                onOpenEvidence({
                  targetTable: "risk_clauses",
                  targetId: row.id,
                  title: `${riskTitle(row)} ${row.description}`,
                })
              }
            />
          </td>
        </tr>
      ))}
    </ReviewDataTable>
  );
}

function formatPercent(confidence: number): string {
  return `${Math.round(confidence * 100)}%`;
}

function formatNullableNumber(value: number | null | undefined): string {
  return value == null ? "-" : String(value);
}

function formatOnsite(row: StaffingReviewRow): string {
  if (row.onsite == null) {
    return "-";
  }

  return row.onsite ? "상주" : "비상주";
}

function formatQuantity(row: ProcurementItemReviewRow): string {
  if (row.quantity == null) {
    return row.unit ?? "-";
  }

  return `${row.quantity}${row.unit ? ` ${row.unit}` : ""}`;
}

function riskTitle(row: RiskReviewRow): string {
  return RISK_TYPE_LABELS[row.riskType] ?? row.riskType;
}

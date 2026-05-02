import type { EvidenceTarget, ReviewProjectDto } from "../../lib/types";
import { EvidenceButton } from "./EvidenceButton";
import { FIELD_FALLBACK_LABELS, FIELD_LABEL_ORDER } from "./reviewLabels";

interface OverviewPanelProps {
  onOpenEvidence: (target: EvidenceTarget) => void;
  review: ReviewProjectDto;
}

function formatConfidence(confidence: number): string {
  return `${Math.round(confidence * 100)}%`;
}

export function OverviewPanel({ onOpenEvidence, review }: OverviewPanelProps) {
  const fieldMap = new Map(
    review.overviewFields.map((field) => [field.fieldKey, field]),
  );

  return (
    <section className="review-overview" aria-label="분석 개요">
      <div className="review-field-grid">
        {FIELD_LABEL_ORDER.map((fieldKey) => {
          const field = fieldMap.get(fieldKey);
          const label = field?.label ?? FIELD_FALLBACK_LABELS[fieldKey];
          const value = field?.normalizedValue || field?.rawValue || "-";

          return (
            <div className="review-field" key={fieldKey}>
              <span>{label}</span>
              <strong>{value}</strong>
              <div className="review-field-meta">
                <small>
                  {field ? `신뢰도 ${formatConfidence(field.confidence)}` : "미추출"}
                </small>
                {field ? (
                  <EvidenceButton
                    evidenceCount={field.evidenceCount}
                    onClick={() =>
                      onOpenEvidence({
                        targetTable: "rfp_fields",
                        targetId: field.id,
                        title: `${label} ${value}`,
                      })
                    }
                  />
                ) : null}
              </div>
            </div>
          );
        })}
      </div>

      <section className="review-findings" aria-label="검토 필요 항목">
        <h3>검토 필요 항목</h3>
        {review.findings.length > 0 ? (
          <ul>
            {review.findings.map((finding) => (
              <li className={`review-finding review-finding--${finding.severity}`} key={finding.id}>
                <strong>{finding.severity === "blocker" ? "Blocker" : "Warning"}</strong>
                <span>{finding.message}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="review-empty">검토 필요 항목 없음</p>
        )}
      </section>
    </section>
  );
}

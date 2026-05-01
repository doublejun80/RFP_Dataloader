import type { CandidateBundleSummaryDto } from "../lib/types";

const BUNDLE_LABELS: Record<string, string> = {
  project_info_candidates: "기본정보",
  requirement_candidates: "요구사항",
  procurement_candidates: "구매항목",
  staffing_candidates: "인력",
  deliverable_candidates: "산출물",
  acceptance_candidates: "검수",
  risk_candidates: "리스크",
};

interface CandidateBundlePanelProps {
  bundles: CandidateBundleSummaryDto[];
}

export function CandidateBundlePanel({ bundles }: CandidateBundlePanelProps) {
  if (bundles.length === 0) {
    return null;
  }

  return (
    <section className="candidate-bundles" aria-label="후보 묶음">
      <h3>후보 묶음</h3>
      <div className="candidate-bundle-grid">
        {bundles.map((bundle) => (
          <div className="candidate-bundle" key={bundle.bundleKey}>
            <span>{BUNDLE_LABELS[bundle.bundleKey] ?? bundle.bundleKey}</span>
            <strong>{bundle.candidateCount}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}

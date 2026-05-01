import type { QualityGateSummary } from "../lib/types";

interface QualityGateProps {
  summary: QualityGateSummary | null;
}

export function QualityGate({ summary }: QualityGateProps) {
  if (!summary) {
    return (
      <section className="quality-empty" aria-label="품질 게이트">
        품질 상태 없음
      </section>
    );
  }

  return (
    <section className="quality-panel" aria-label="품질 게이트">
      <div className="quality-metric quality-metric--blocker">
        <span className="metric-label">Blocker</span>
        <strong>{summary.blockerCount}</strong>
      </div>
      <div className="quality-metric quality-metric--warning">
        <span className="metric-label">Warning</span>
        <strong>{summary.warningCount}</strong>
      </div>
      <div className="quality-metric">
        <span className="metric-label">Blocks</span>
        <strong>{summary.blockCount}</strong>
      </div>
    </section>
  );
}

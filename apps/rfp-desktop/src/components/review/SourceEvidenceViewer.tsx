import type { EvidenceContextDto, EvidenceTarget } from "../../lib/types";

interface SourceEvidenceViewerProps {
  target: EvidenceTarget | null;
  context: EvidenceContextDto | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}

function formatConfidence(confidence: number): string {
  return `${Math.round(confidence * 100)}%`;
}

export function SourceEvidenceViewer({
  context,
  error,
  loading,
  onClose,
  target,
}: SourceEvidenceViewerProps) {
  if (!target) {
    return (
      <aside className="source-evidence-viewer" aria-label="원문 근거">
        원문 근거를 선택하세요.
      </aside>
    );
  }

  return (
    <aside className="source-evidence-viewer" aria-label="원문 근거">
      <div className="source-evidence-heading">
        <div>
          <span className="eyeline">원문 근거</span>
          <h3>{target.title}</h3>
        </div>
        <button onClick={onClose} type="button">
          닫기
        </button>
      </div>

      {loading ? <p className="review-empty">근거를 불러오는 중</p> : null}
      {error ? (
        <p className="review-error" role="alert">
          {error}
        </p>
      ) : null}

      {context ? (
        <>
          <section className="source-evidence-section">
            <h4>인용 문장</h4>
            {context.evidence.length > 0 ? (
              <ul className="source-quote-list">
                {context.evidence.map((evidence) => (
                  <li key={evidence.id ?? evidence.documentBlockId}>
                    <blockquote>{evidence.quote}</blockquote>
                    <span>신뢰도 {formatConfidence(evidence.confidence)}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="review-empty">연결된 근거 없음</p>
            )}
          </section>

          <section className="source-evidence-section">
            <h4>주변 원문</h4>
            {context.blocks.length > 0 ? (
              <div className="source-block-list">
                {context.blocks.map((block) => (
                  <article
                    className={
                      block.isDirectEvidence
                        ? "source-block source-block--direct"
                        : "source-block"
                    }
                    key={block.id}
                  >
                    <div className="source-block-meta">
                      <span>
                        {block.pageNumber}쪽 / block {block.blockIndex}
                      </span>
                      <span>{block.kind}</span>
                      {block.isDirectEvidence ? <strong>원문</strong> : null}
                    </div>
                    <p>{block.text}</p>
                    {block.bboxJson ? (
                      <code className="source-bbox">{block.bboxJson}</code>
                    ) : null}
                  </article>
                ))}
              </div>
            ) : (
              <p className="review-empty">주변 block 없음</p>
            )}
          </section>
        </>
      ) : null}
    </aside>
  );
}

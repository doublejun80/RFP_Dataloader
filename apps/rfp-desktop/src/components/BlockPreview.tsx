import type { DocumentSummary } from "../lib/types";
import { getStatusLabel } from "./StatusBadge";

interface BlockPreviewProps {
  document: DocumentSummary | null;
}

export function BlockPreview({ document }: BlockPreviewProps) {
  if (!document) {
    return (
      <section className="block-preview block-preview--empty">
        원문 block 없음
      </section>
    );
  }

  return (
    <section className="block-preview" aria-label="원문 block 미리보기">
      <div>
        <span className="eyeline">원문 근거</span>
        <h3>{document.title}</h3>
      </div>
      <p>{document.blockCount}개 원문 block이 저장되어 있습니다.</p>
      <dl>
        <div>
          <dt>파일</dt>
          <dd>{document.fileName ?? "등록된 PDF"}</dd>
        </div>
        <div>
          <dt>상태</dt>
          <dd>{getStatusLabel(document.status)}</dd>
        </div>
      </dl>
    </section>
  );
}

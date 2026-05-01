import type { DocumentSummary } from "../lib/types";
import { StatusBadge } from "./StatusBadge";

interface DocumentListProps {
  documents: DocumentSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

export function DocumentList({
  documents,
  selectedId,
  onSelect,
}: DocumentListProps) {
  if (documents.length === 0) {
    return (
      <aside className="document-list document-list--empty">
        <p>등록된 문서가 없습니다.</p>
      </aside>
    );
  }

  return (
    <aside className="document-list" aria-label="RFP 문서 목록">
      {documents.map((document) => (
        <button
          aria-pressed={document.id === selectedId}
          className={
            document.id === selectedId
              ? "document-row document-row--selected"
              : "document-row"
          }
          key={document.id}
          onClick={() => onSelect(document.id)}
          type="button"
        >
          <span className="document-title">{document.title}</span>
          <span className="document-meta">{document.fileName ?? "PDF"}</span>
          <StatusBadge status={document.status} />
        </button>
      ))}
    </aside>
  );
}

import { FileSearch } from "lucide-react";

interface EvidenceButtonProps {
  evidenceCount: number;
  onClick: () => void;
}

export function EvidenceButton({ evidenceCount, onClick }: EvidenceButtonProps) {
  return (
    <button
      aria-label="원문 근거 보기"
      className="evidence-button"
      disabled={evidenceCount === 0}
      onClick={onClick}
      title="원문 근거 보기"
      type="button"
    >
      <FileSearch aria-hidden="true" size={15} />
      <span>{evidenceCount}</span>
    </button>
  );
}

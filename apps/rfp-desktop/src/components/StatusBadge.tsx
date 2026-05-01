import clsx from "clsx";

import type { DocumentStatus } from "../lib/types";

const STATUS_LABELS: Record<DocumentStatus, string> = {
  created: "문서 대기",
  extracting: "구조 추출 중",
  analyzing: "요구사항 분석 중",
  review_needed: "검토 필요",
  ready: "확정 가능",
  failed: "실패",
};

interface StatusBadgeProps {
  status: DocumentStatus;
  className?: string;
}

export function getStatusLabel(status: DocumentStatus): string {
  return STATUS_LABELS[status];
}

export function StatusBadge({ status, className }: StatusBadgeProps) {
  const label = getStatusLabel(status);

  return (
    <span
      className={clsx("status-badge", `status-badge--${status}`, className)}
      aria-label={`상태: ${label}`}
    >
      {label}
    </span>
  );
}

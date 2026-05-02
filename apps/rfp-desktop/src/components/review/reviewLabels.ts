export const FIELD_LABEL_ORDER = [
  "business_name",
  "client",
  "budget",
  "period",
  "contract_method",
  "deadline",
] as const;

export const FIELD_FALLBACK_LABELS: Record<string, string> = {
  business_name: "사업명",
  client: "발주기관",
  budget: "사업예산",
  period: "사업기간",
  contract_method: "계약방식",
  deadline: "제출마감",
};

export const CATEGORY_LABELS: Record<string, string> = {
  functional: "기능",
  technical: "기술",
  security: "보안",
  data: "데이터",
  staffing: "인력",
  management: "관리",
  quality: "품질",
  performance: "성능",
  other: "기타",
};

export const RISK_TYPE_LABELS: Record<string, string> = {
  scope_creep: "범위 확장",
  free_work: "무상/비용 전가",
  short_schedule: "단기 일정",
  liability: "책임 과다",
  ambiguous_spec: "스펙 모호",
  vendor_lock: "특정 업체 유리",
  payment: "지급/검수 위험",
  security: "보안/개인정보 위험",
  other: "기타",
};

export const ACCEPTANCE_TYPE_LABELS: Record<string, string> = {
  test: "테스트",
  performance: "성능",
  security: "보안",
  inspection: "검수",
  sla: "SLA",
  warranty: "하자보수",
  other: "기타",
};

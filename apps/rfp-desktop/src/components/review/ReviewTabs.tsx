import type { ReviewProjectDto, ReviewTab } from "../../lib/types";

interface ReviewTabsProps {
  activeTab: ReviewTab;
  onTabChange: (tab: ReviewTab) => void;
  review: ReviewProjectDto;
}

const TABS: Array<{
  key: ReviewTab;
  label: string;
  count: (review: ReviewProjectDto) => number;
}> = [
  {
    key: "overview",
    label: "개요",
    count: (review) => review.overviewFields.length,
  },
  {
    key: "procurement",
    label: "구매 항목",
    count: (review) => review.procurementItems.length,
  },
  {
    key: "staffing",
    label: "인력/MM",
    count: (review) => review.staffingRequirements.length,
  },
  {
    key: "requirements",
    label: "요구사항",
    count: (review) => review.requirements.length,
  },
  {
    key: "deliverables",
    label: "산출물",
    count: (review) => review.deliverables.length,
  },
  {
    key: "acceptance",
    label: "검수",
    count: (review) => review.acceptanceCriteria.length,
  },
  {
    key: "risks",
    label: "리스크",
    count: (review) => review.riskClauses.length,
  },
];

export function ReviewTabs({ activeTab, onTabChange, review }: ReviewTabsProps) {
  return (
    <div className="review-tabs" aria-label="분석 검토 탭">
      {TABS.map((tab) => (
        <button
          aria-pressed={activeTab === tab.key}
          className="review-tab"
          key={tab.key}
          onClick={() => onTabChange(tab.key)}
          type="button"
        >
          <span>{tab.label}</span>
          <strong>{tab.count(review)}</strong>
        </button>
      ))}
    </div>
  );
}

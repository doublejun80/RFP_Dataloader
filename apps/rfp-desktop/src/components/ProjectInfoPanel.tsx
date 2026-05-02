import type { ReviewFieldDto, RfpFieldDto } from "../lib/types";

const FIELD_ORDER = [
  ["business_name", "사업명"],
  ["client", "발주기관"],
  ["budget", "사업예산"],
  ["period", "사업기간"],
  ["contract_method", "계약방식"],
  ["deadline", "제출마감"],
] as const;

interface ProjectInfoPanelProps {
  fields: Array<RfpFieldDto | ReviewFieldDto>;
}

export function ProjectInfoPanel({ fields }: ProjectInfoPanelProps) {
  const fieldMap = new Map(fields.map((field) => [field.fieldKey, field]));

  return (
    <section className="project-info" aria-label="기본정보">
      <h3>기본정보</h3>
      <dl className="project-info-grid">
        {FIELD_ORDER.map(([fieldKey, label]) => {
          const field = fieldMap.get(fieldKey);
          const value = field?.normalizedValue || field?.rawValue || "미추출";

          return (
            <div className="project-info-item" key={fieldKey}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          );
        })}
      </dl>
    </section>
  );
}

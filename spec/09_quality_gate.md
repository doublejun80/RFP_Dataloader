# 품질 게이트

## 원칙

파일 생성은 성공이 아니다. 분석 결과를 구매/제안 판단에 써도 되는지가 성공 기준이다.

## 상태 구분

| 상태 | 의미 |
|---|---|
| `generated` | 산출물/DB row가 생성됨 |
| `review_needed` | blocker 또는 중요 warning이 있음 |
| `ready` | 필수 blocker가 없고 근거가 연결됨 |
| `failed` | 추출/분석 작업 자체가 실패 |

## Blocker

다음은 MVP blocker다.

| 코드 | 조건 | 메시지 |
|---|---|---|
| `missing_business_name` | 사업명 없음 | 사업명이 추출되지 않았습니다. |
| `missing_client` | 발주기관 없음 | 발주기관이 추출되지 않았습니다. |
| `missing_budget` | 예산 없음 | 사업예산이 추출되지 않았습니다. |
| `missing_period` | 기간 없음 | 사업기간이 추출되지 않았습니다. |
| `zero_requirements` | 요구사항 0건 | 요구사항이 0건입니다. |
| `missing_evidence` | 핵심 entity에 근거 없음 | 원문 근거가 없는 항목이 있습니다. |
| `schema_invalid` | LLM output schema 불일치 | LLM 구조화 결과가 schema를 통과하지 못했습니다. |
| `duplicate_requirement_code` | 같은 code 충돌 | 중복 요구사항 ID가 있습니다. |
| `over_extraction` | 요구사항 수가 비정상적으로 많음 | 요구사항 과다 추출 가능성이 있습니다. |

## Warning

| 코드 | 조건 |
|---|---|
| `low_confidence` | confidence < 0.6 |
| `missing_quantity` | 구매 항목 이름은 있으나 수량/스펙 없음 |
| `ambiguous_period` | 기간 표현이 모호함 |
| `llm_not_used` | LLM opt-in이 꺼져 구조화가 제한됨 |
| `hybrid_recommended` | 표/스캔 문서로 보이나 fast mode 사용 |
| `correction_applied` | 사용자 보정값이 export에 반영됨 |

## Smoke 기준

실제 RFP bundle smoke는 다음을 보고해야 한다.

- total documents
- extraction succeeded count
- analysis generated count
- ready count
- review_needed count
- failed count
- blocker count by type
- warning count by type
- per-document summary

exit code:

| exit code | 의미 |
|---:|---|
| 0 | 모든 문서가 ready 또는 허용된 warning만 있음 |
| 1 | 추출/분석 실패가 있음 |
| 2 | 생성은 됐지만 blocker가 있음 |

## 회귀 기준

v1의 `20/20 generated` 착시를 금지한다.

완료 보고에는 항상 다음을 포함한다.

- generated count
- ready count
- review_needed count
- failed count
- blocker count
- 대표 blocker 문서
- 실행한 extraction mode
- LLM 사용 여부와 provider

## 품질 게이트 저장

모든 finding은 `validation_findings`에 저장한다.

필수 컬럼:

- severity
- finding_type
- message
- target_table
- target_id
- created_at

UI와 export는 이 테이블을 기준으로 상태를 보여준다.


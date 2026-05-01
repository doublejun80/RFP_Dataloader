# 데이터 파이프라인

## 단계 요약

| 단계 | 입력 | 출력 | 실패 시 |
|---|---|---|---|
| 1. 등록 | PDF 파일 | `documents`, `source_files` | 파일 접근 오류 표시 |
| 2. 추출 | source file | OpenDataLoader JSON/Markdown | `extraction_runs.failed` |
| 3. 블록 정규화 | OpenDataLoader JSON | `document_blocks` | raw JSON 보존 후 실패 표시 |
| 4. 후보 선별 | document blocks | candidate bundle | blocker/warning 생성 |
| 5. 구조화 | candidate bundle | field/requirement/item draft | LLM 실패 시 rule baseline 유지 |
| 6. 검증 | draft records | `validation_findings` | blocker면 `review_needed` |
| 7. 검토/보정 | UI 수정 | `corrections` | 이전 값 보존 |
| 8. 내보내기 | DB snapshot | Markdown/JSON/Docx | `exports.failed` |

## 1. 문서 등록

등록 시점에 원본 파일은 복사하지 않는 것을 기본으로 한다. 다만 사용자가 "프로젝트 내부 복사" 옵션을 켜면 앱 데이터 폴더 아래에 안전한 파일명으로 복사한다.

저장 값:

- 원본 경로
- 파일명
- 파일 크기
- SHA-256
- 등록 시각

## 2. OpenDataLoader 추출

기본 모드는 digital PDF용 fast mode다. 이미지 기반 스캔 문서나 표 추출 품질이 낮은 문서는 hybrid mode를 사용한다.

필수 output:

- JSON
- Markdown
- stdout/stderr log
- 실패 page 목록이 있으면 run log에 저장

## 3. 블록 정규화

OpenDataLoader JSON element를 `document_blocks`로 변환한다.

필수 보존 필드:

- page number
- element id
- element type
- heading level
- text/content
- bounding box
- raw JSON

텍스트가 비어 있는 image/table element도 raw JSON은 보존할 수 있다. LLM context에는 빈 텍스트 block을 넣지 않는다.

## 4. 후보 선별

LLM에는 문서 전체를 보내지 않는다. 후보 선별기는 다음 묶음을 만든다.

| candidate bundle | 포함 기준 |
|---|---|
| `project_info_candidates` | 사업명, 기관, 기간, 예산, 계약방식 주변 block |
| `requirement_candidates` | 요구사항 ID, 요구사항 명칭, 상세설명, 요구사항 총괄표 주변 |
| `procurement_candidates` | 장비, 서버, SW, license, cloud, DB, network, 보안 솔루션 |
| `staffing_candidates` | 투입인력, PM, PL, MM, 상주, 수행조직 |
| `deliverable_candidates` | 산출물, 보고서, 설계서, 매뉴얼, 교육자료 |
| `acceptance_candidates` | 검수, 시험, 성능, 보안점검, 하자보수, SLA |
| `risk_candidates` | 무상, 추가 요청, 협의, 필요 시, 지체상금, 책임, 비용 부담 |

후보는 block id 목록과 짧은 text quote를 함께 가진다.

## 5. 구조화

LLM opt-in이 꺼져 있으면 rule baseline만 생성한다. 이 경우 UI는 "자동 구조화 제한" 상태를 보여준다.

LLM opt-in이 켜져 있으면 provider adapter가 schema별로 호출한다.

- project info schema
- requirements schema
- procurement/staffing/deliverable/acceptance/risk schema

LLM output은 바로 확정하지 않는다. `llm_runs.response_json`에 저장한 뒤 schema validation과 evidence validation을 통과한 항목만 domain table로 쓴다.

## 6. 검증

검증기는 다음 blocker를 만든다.

- 사업명 없음
- 발주기관 없음
- 예산 없음
- 기간 없음
- 요구사항 0건
- 구매 항목에 수량/스펙이 모두 없음
- 핵심 entity에 evidence link 없음
- LLM output schema validation 실패
- 같은 requirement code가 충돌
- 요구사항 수가 summary count와 크게 다름

## 7. 검토/보정

사용자 보정은 원본 row를 직접 덮어쓰지 않는다.

흐름:

1. 사용자가 target row와 field를 선택한다.
2. old value와 new value를 저장한다.
3. 화면에서는 correction이 있는 값을 우선 표시한다.
4. export는 correction 적용 snapshot을 사용한다.

## 8. 내보내기

내보내기는 DB snapshot 기반이다.

산출물:

- `rfp_project.json`
- `rfp_review.md`
- `rfp_review.docx`

Markdown 구성:

1. 사업 기본정보
2. 구매 항목 BOM
3. 인력/MM
4. 업무 범위
5. 납품물
6. 검수/인수 조건
7. 리스크/독소 조항
8. 요구사항 traceability
9. 품질 게이트 결과


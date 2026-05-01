# Backlog Seed

이 문서는 다음 단계에서 implementation plan으로 바꿀 작업 후보이다.

## Epic 1. Tauri v2 workspace

- Tauri v2 + React/Vite workspace 생성.
- Rust command 기본 wiring.
- 앱 데이터 디렉터리 확인.
- SQLite 연결과 migration runner 추가.
- `documents`, `source_files`, `audit_events` migration 작성.

## Epic 2. Document ingestion

- PDF 파일 등록 command.
- SHA-256 계산.
- 중복 파일 감지.
- 등록 실패 오류 메시지.
- 문서 목록 UI.

## Epic 3. OpenDataLoader adapter

- CLI 진단 command.
- Java runtime 진단.
- fast mode 실행.
- output JSON/Markdown 수집.
- extraction run log 저장.
- 실패 run 보존.

## Epic 4. Block normalizer

- OpenDataLoader JSON parser.
- nested elements flatten.
- page/block/bbox 저장.
- raw JSON 저장.
- block viewer UI.

## Epic 5. Candidate extractor

- project info candidate.
- requirement candidate.
- procurement candidate.
- staffing candidate.
- deliverable/acceptance/risk candidate.
- candidate bundle JSON debug export.

## Epic 6. LLM adapter

- provider settings UI.
- API key secure storage strategy.
- OpenAI structured output adapter.
- Gemini structured output adapter.
- schema validation.
- LLM run audit 저장.

## Epic 7. Domain writer

- `rfp_projects` 생성.
- `rfp_fields` 저장.
- `requirements` 저장.
- `procurement_items` 저장.
- `staffing_requirements` 저장.
- `deliverables`, `acceptance_criteria`, `risk_clauses` 저장.
- evidence link 저장.

## Epic 8. Validation engine

- blocker/warning rule 구현.
- finding table 저장.
- status evaluator.
- smoke exit code.
- 실제 RFP bundle smoke.

## Epic 9. Review UI

- 분석 개요.
- 구매 항목 table.
- 인력/MM table.
- 요구사항 table.
- 리스크 table.
- 원문 근거 viewer.
- 보정 dialog.

## Epic 10. Export

- Markdown export.
- JSON export.
- Docx export.
- export history.
- blocker 포함 export warning.

## 첫 구현 계획 권장 순서

1. SQLite schema와 migration.
2. 문서 등록.
3. OpenDataLoader fast extraction.
4. document_blocks 저장.
5. validation finding 최소 구현.
6. 실제 RFP 1건 smoke.
7. UI 목록/상태.
8. 후보 추출.
9. LLM schema adapter.
10. 구매 검토 화면.

## 첫 smoke 성공 기준

1개 실제 PDF에 대해:

- document row 생성.
- extraction run succeeded.
- document_blocks 1개 이상.
- rfp_project row 생성.
- validation_findings 생성.
- UI에서 `검토 필요` 또는 `확정 가능`이 명확히 표시.


# Tauri RFP v2 문서 팩

이 폴더는 기존 PySide6 기반 RFP 분석 구현을 폐기하고, Tauri + Rust + SQLite + OpenDataLoader + LLM 구조로 새로 만들기 위한 제품/기술 기준 문서다.

목표는 "문서가 생성된다"가 아니라 "구매/제안 담당자가 범위, 비용, 인력, 납품, 검수, 리스크를 근거와 함께 판단할 수 있다"이다.

## 문서 목록

| 문서 | 용도 |
|---|---|
| `01_failure_review_and_reuse.md` | 기존 구현에서 차용할 것과 버릴 것 |
| `02_prd.md` | v2 제품 요구사항 |
| `03_architecture.md` | Tauri/Rust/OpenDataLoader/LLM 아키텍처 |
| `04_erd.md` | SQLite ERD와 테이블 정의 |
| `05_data_pipeline.md` | 문서 입력부터 검증까지 데이터 흐름 |
| `06_llm_contract.md` | OpenAI/Gemini 구조화 출력 계약 |
| `07_opendataloader_integration.md` | OpenDataLoader 연동 방식 |
| `08_ui_product_flow.md` | 구매팀용 화면 흐름 |
| `09_quality_gate.md` | 품질 게이트와 smoke 기준 |
| `10_migration_cutover.md` | v1 폐기와 v2 대체 매핑 |
| `11_backlog_seed.md` | 구현 계획으로 넘길 작업 후보 |

## v2 원칙

- 기존 PySide6 앱은 새 제품의 기반으로 쓰지 않는다.
- 기존 실제 RFP bundle과 quality gate 개념은 검증 자산으로 차용한다.
- 분석 결과는 SQLite에 구조화 데이터로 저장한다.
- 원문 근거는 page/block/bounding box 단위로 추적한다.
- LLM은 선택형 구조화 판독기로만 사용하고, 최종 검증은 로컬 규칙으로 수행한다.
- OpenDataLoader는 PDF 구조 추출의 기본 경로로 둔다.
- `20/20 generated`는 성공 기준이 아니다.

## 주요 근거 자료

- Tauri Rust command: <https://v2.tauri.app/develop/calling-rust/>
- Tauri SQLite SQL plugin: <https://v2.tauri.app/plugin/sql/>
- Tauri shell/sidecar: <https://v2.tauri.app/ko/plugin/shell/>
- OpenDataLoader PDF: <https://opendataloader.org/docs>
- OpenAI Structured Outputs: <https://platform.openai.com/docs/guides/structured-outputs>
- Gemini structured output: <https://ai.google.dev/gemini-api/docs/structured-output>


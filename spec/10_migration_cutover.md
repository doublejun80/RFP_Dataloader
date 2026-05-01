# v1 폐기와 v2 대체 매핑

## 폐기 결정

v2는 기존 PySide6 앱 위에 덧붙이지 않는다. 새 앱은 Tauri + Rust + SQLite 기준으로 만든다.

기존 코드는 다음 용도로만 남긴다.

- 실제 RFP fixture 위치 확인
- 실패 사례 재현
- 일부 테스트 문구 참고
- 품질 gate 반면교사

## 대체 매핑

| v1 | v2 대체 |
|---|---|
| PySide6 `main_window.py` | Tauri frontend + Rust commands |
| `services/rfp_analysis_job.py` | Rust `analysis_orchestrator` |
| `understanding/opendataloader_provider.py` | Rust `opendataloader_adapter` |
| `rfp/requirements.py` | `candidate_extractor` + LLM schema + validation |
| `rfp/field_extractor.py` | project info candidate + structured extraction |
| `rfp/procurement.py` | `procurement_items`, `staffing_requirements`, `deliverables`, `acceptance_criteria`, `risk_clauses` |
| `RfpQualityReport` | `validation_findings` table + gate evaluator |
| `rfp_analysis.json` | normalized SQLite snapshot + export JSON |
| `rfp_summary.md` | DB 기반 Markdown export |
| FastAPI archive MVP | 후속 sync/export feature |

## 차용 파일

| 파일/폴더 | 용도 |
|---|---|
| `rfp/rfp_bundle` | smoke fixture |
| `docs/superpowers/plans/*rfp*` | 실패/결정 이력 |
| `docs/03_user_flows_and_ui.md` | 사용자 흐름 중 구매팀 관점 |
| `docs/14_opendataloader_troubleshooting.md` | OpenDataLoader 진단 경험 |
| `tests/test_rfp_real_data_smoke_script.py` | smoke exit code 철학 참고 |

## 새 repo 구조 초안

```text
.
├─ apps/
│  └─ rfp-desktop/
│     ├─ src/
│     │  ├─ app/
│     │  ├─ components/
│     │  ├─ features/
│     │  └─ lib/
│     ├─ src-tauri/
│     │  ├─ src/
│     │  │  ├─ commands/
│     │  │  ├─ db/
│     │  │  ├─ document_ingestion/
│     │  │  ├─ opendataloader_adapter/
│     │  │  ├─ llm_adapter/
│     │  │  ├─ analysis/
│     │  │  ├─ validation/
│     │  │  └─ export/
│     │  ├─ migrations/
│     │  └─ capabilities/
│     └─ package.json
├─ fixtures/
│  └─ rfp_bundle/
├─ docs/
│  └─ product/
└─ tests/
   ├─ smoke/
   └─ fixtures/
```

## Cutover 단계

1. v2 문서 기준 확정.
2. 새 Tauri workspace 생성.
3. SQLite migration부터 작성.
4. OpenDataLoader smoke fixture 연결.
5. document block 저장 검증.
6. rule baseline candidate extractor 작성.
7. LLM adapter와 schema validation 작성.
8. UI는 DB 조회 기반으로 붙임.
9. 기존 PySide6 코드는 더 이상 수정하지 않음.

## 폐기 완료 기준

- v2 smoke가 실제 RFP bundle을 읽는다.
- v2 DB에 document/extraction/block/project/requirement/finding이 저장된다.
- v1 smoke보다 더 정확하다는 주장은 blocker/ready count로만 한다.
- PySide6 화면을 열어 확인하는 수동 검증이 더 이상 필요하지 않다.


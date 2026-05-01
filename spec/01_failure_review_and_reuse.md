# 실패 리뷰와 차용 기준

## 실패의 핵심

기존 구현은 산출물 생성과 분석 품질을 분리하지 못했다. `rfp_analysis.json`과 `rfp_summary.md`가 만들어져도, 구매팀이 믿고 쓸 수 있는 사업 정보, 요구사항, 자원, 수량, 인력, 검수, 독소조항이 충분히 구조화되지 않았다.

가장 큰 구조적 문제는 세 가지다.

| 문제 | 현상 | v2 대응 |
|---|---|---|
| 추출 기반이 약함 | 최신 real-data smoke는 OpenDataLoader가 아니라 `pypdf` fallback 중심 | OpenDataLoader JSON/Markdown을 기본 extraction source로 사용 |
| 도메인 모델이 얕음 | 요구사항 row를 키워드로만 HW/SW/인력 분류 | procurement/staffing/deliverable/acceptance/risk를 별도 엔티티로 저장 |
| 품질 판정이 늦음 | 산출물 생성 후에야 blocker를 확인 | pipeline 단계마다 validation finding 생성 |

## 차용할 것

| v1 자산 | 차용 방식 |
|---|---|
| 실제 RFP 20개 bundle | v2 smoke fixture와 회귀 기준으로 사용 |
| quality blocker 개념 | `validation_findings` 테이블과 release gate로 승격 |
| source evidence 개념 | `evidence_links`로 page/block/bounding box 근거 저장 |
| 구매팀 관점 UI | v2 첫 화면을 단일 RFP 상세 분석 workspace로 설계 |
| Markdown/Word export 요구 | DB 기반 export generator로 재구현 |
| 보정 기록 아이디어 | `corrections` 테이블로 추출값 override 기록 |
| 아카이브 이벤트 아이디어 | `audit_events`로 로컬 분석/수정/내보내기 기록 |
| OpenDataLoader provider 경계 | Rust command가 Python/OpenDataLoader sidecar를 호출하는 명확한 adapter로 재정의 |

## 버릴 것

| v1 요소 | 폐기 이유 |
|---|---|
| PySide6 UI | 새 제품 방향과 맞지 않고 UI/분석 상태가 강하게 얽힘 |
| `pypdf` fallback 중심 RFP 분석 | 줄 구조가 거칠고 표/요구사항 근거 품질이 낮음 |
| 정규식 누적 요구사항 추출 | 문서별 예외가 계속 늘어나며 품질이 안정되지 않음 |
| 얕은 procurement matrix | 실제 구매 질문에 답하지 못함 |
| FastAPI 필수 MVP | v2는 로컬 데스크톱 SQLite가 1차 제품이며 서버는 후속 동기화 기능 |
| 단일 "분석 완료" 상태 | 생성/검토필요/차단/확정 상태를 분리해야 함 |

## v2에서 반드시 복구할 사용자 가치

- 사업 기본정보: 기관, 사업명, 예산, 기간, 계약방식, 제출 마감.
- 구매 항목: HW, SW, license, cloud, network, DB, security, third-party service.
- 수량/스펙: 단위와 원문 표현을 분리해 저장.
- 인력: 역할, 등급, 상주 여부, MM, 기간.
- 업무 범위: 구축, 전환, 마이그레이션, 개발, 운영, 교육.
- 납품물: 문서, 소스, 교육자료, 산출물, 보고서.
- 검수/인수 조건: 테스트, 성능, 보안, 하자보수, SLA.
- 리스크: 무상/추가 요청/일정/책임/특정 스펙/범위 모호성.
- 근거: 모든 핵심 판단은 source page/block/bounding box로 역추적 가능해야 함.


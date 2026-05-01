# OpenDataLoader 연동

## 역할

OpenDataLoader는 v2의 기본 문서 구조 추출기다. Rust가 PDF layout parsing을 직접 구현하지 않는다.

OpenDataLoader가 제공해야 하는 값:

- Markdown output
- JSON output
- page number
- element kind
- heading level
- table/list structure
- bounding box
- raw element JSON

## 실행 방식

MVP는 다음 우선순위를 따른다.

1. 사용자가 설치한 `opendataloader-pdf` CLI를 진단하고 사용.
2. 앱 번들에 포함 가능한 sidecar wrapper를 후속 검토.
3. hybrid OCR server는 사용자가 명시적으로 켠 경우에만 사용.

Tauri shell/sidecar 기능은 제한된 command만 허용한다. 임의 shell 문자열 실행은 금지한다.

## 모드

| 모드 | 용도 | 기본 여부 |
|---|---|---|
| `fast` | 텍스트 레이어가 있는 일반 PDF | 기본 |
| `hybrid_auto` | 표/복잡 layout 품질 개선 | 옵션 |
| `hybrid_full` | 이미지 기반 스캔/OCR 필요 문서 | 옵션 |

## 진단

앱은 분석 시작 전 다음을 확인한다.

| 항목 | 실패 메시지 |
|---|---|
| CLI 존재 | OpenDataLoader CLI를 찾을 수 없습니다. |
| Java runtime | Java 런타임이 필요합니다. |
| output writable | 추출 결과 폴더에 쓸 수 없습니다. |
| hybrid health | 로컬 hybrid 서버가 실행 중이 아닙니다. |
| JSON output | OpenDataLoader JSON 결과가 없습니다. |
| Markdown output | OpenDataLoader Markdown 결과가 없습니다. |

## Output 정규화

OpenDataLoader JSON은 version별 key 차이를 adapter에서 흡수한다.

허용 key 후보:

- text/content/markdown/value
- type/kind/role/category
- page/page_number/page number
- bbox/bounding_box/bounding box
- elements/items/blocks/kids/list items/rows/cells

정규화 결과는 `document_blocks`에 저장한다.

## 실패 대응

- 추출 실패는 분석 실패가 아니라 extraction run 실패다.
- 실패해도 source file row와 run log는 유지한다.
- pypdf fallback은 MVP 기본 경로로 쓰지 않는다.
- fallback이 필요하면 "비상 텍스트만 추출" 상태를 별도로 표시하고, 구매 분석 완료로 인정하지 않는다.

## 참고 자료

- OpenDataLoader PDF docs: <https://opendataloader.org/docs>
- OpenDataLoader PyPI: <https://pypi.org/project/opendataloader-pdf/>
- Tauri shell plugin: <https://v2.tauri.app/ko/plugin/shell/>


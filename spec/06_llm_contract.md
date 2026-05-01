# LLM 계약

## 원칙

LLM은 선택형 구조화 판독기다. 최종 진실 공급원이 아니다.

- 사용자가 명시적으로 켠 경우에만 사용한다.
- 원본 파일은 전송하지 않는다.
- OpenDataLoader에서 추출한 후보 text와 block id만 보낸다.
- LLM output은 schema validation과 evidence validation을 통과해야 저장한다.
- API key는 SQLite plain text로 저장하지 않는다.

## Provider

MVP에서 고려할 provider:

| Provider | 장점 | 주의점 |
|---|---|---|
| OpenAI | Structured Outputs가 JSON Schema 준수를 강하게 지원 | 비용/네트워크/데이터 전송 동의 필요 |
| Gemini | structured output과 JSON schema 기반 추출 가능 | schema 호환성 차이를 adapter에서 흡수해야 함 |

## 호출 단위

문서 전체를 한 번에 보내지 않는다.

| 호출 | 입력 | 출력 |
|---|---|---|
| `extract_project_info` | 기본정보 후보 block | project fields |
| `extract_requirements` | 요구사항 후보 block chunk | requirements |
| `extract_procurement` | 요구사항 + 구매 후보 | procurement/staffing/deliverable/acceptance/risk |
| `classify_risks` | risk 후보 | risk clauses |

## 공통 입력 envelope

```json
{
  "document_id": "uuid",
  "rfp_project_id": "uuid",
  "language": "ko",
  "candidate_blocks": [
    {
      "block_id": "uuid",
      "page_number": 12,
      "kind": "table",
      "text": "요구사항 고유번호 SFR-001 ...",
      "bbox": [72.0, 400.0, 540.0, 650.0]
    }
  ],
  "instructions": {
    "preserve_korean_terms": true,
    "do_not_invent_values": true,
    "require_evidence_block_ids": true
  }
}
```

## Project info output schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["fields"],
  "properties": {
    "fields": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["field_key", "raw_value", "normalized_value", "confidence", "evidence_block_ids"],
        "properties": {
          "field_key": {
            "type": "string",
            "enum": ["business_name", "client", "budget", "period", "contract_method", "deadline", "evaluation_ratio"]
          },
          "raw_value": { "type": "string" },
          "normalized_value": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": {
            "type": "array",
            "items": { "type": "string" }
          }
        }
      }
    }
  }
}
```

## Requirement output schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["requirements"],
  "properties": {
    "requirements": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "title", "description", "category", "mandatory", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "title": { "type": "string" },
          "description": { "type": "string" },
          "category": {
            "type": "string",
            "enum": ["functional", "technical", "security", "data", "staffing", "management", "quality", "performance", "other"]
          },
          "mandatory": { "type": "boolean" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": {
            "type": "array",
            "items": { "type": "string" }
          }
        }
      }
    }
  }
}
```

## Procurement output schema

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["procurement_items", "staffing_requirements", "deliverables", "acceptance_criteria", "risk_clauses"],
  "properties": {
    "procurement_items": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "item_type", "name", "spec", "quantity_text", "unit", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "item_type": {
            "type": "string",
            "enum": ["hardware", "software", "license", "cloud", "network", "database", "security", "service", "other"]
          },
          "name": { "type": "string" },
          "spec": { "type": "string" },
          "quantity_text": { "type": "string" },
          "unit": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "staffing_requirements": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "role", "grade", "headcount_text", "mm_text", "onsite_text", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "role": { "type": "string" },
          "grade": { "type": "string" },
          "headcount_text": { "type": "string" },
          "mm_text": { "type": "string" },
          "onsite_text": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "deliverables": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "name", "due_text", "format_text", "description", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "name": { "type": "string" },
          "due_text": { "type": "string" },
          "format_text": { "type": "string" },
          "description": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "acceptance_criteria": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "criterion_type", "description", "threshold", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "criterion_type": {
            "type": "string",
            "enum": ["test", "performance", "security", "inspection", "sla", "warranty", "other"]
          },
          "description": { "type": "string" },
          "threshold": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "risk_clauses": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["requirement_code", "risk_type", "severity", "description", "recommended_action", "confidence", "evidence_block_ids"],
        "properties": {
          "requirement_code": { "type": "string" },
          "risk_type": {
            "type": "string",
            "enum": ["scope_creep", "free_work", "short_schedule", "liability", "ambiguous_spec", "vendor_lock", "payment", "security", "other"]
          },
          "severity": {
            "type": "string",
            "enum": ["low", "medium", "high", "blocker"]
          },
          "description": { "type": "string" },
          "recommended_action": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_block_ids": { "type": "array", "items": { "type": "string" } }
        }
      }
    }
  }
}
```

## 검증 규칙

- `evidence_block_ids`가 비면 domain table 저장 금지.
- 후보 block에 없는 값을 새로 만들면 warning 또는 blocker.
- 수량이 있는 경우 로컬 parser가 `quantity`와 `unit`으로 재분해한다.
- 금액/날짜/MM은 LLM 값을 그대로 믿지 않고 로컬 normalizer가 다시 검증한다.
- provider refusal 또는 schema mismatch는 `llm_runs.status = rejected`로 저장한다.

## 참고 자료

- OpenAI Structured Outputs: <https://platform.openai.com/docs/guides/structured-outputs>
- Gemini structured output: <https://ai.google.dev/gemini-api/docs/structured-output>


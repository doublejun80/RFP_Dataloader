use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection};

use crate::candidate_extractor::{CandidateBundle, CandidateBundleKey, CandidateSnippet};
use crate::domain_writer::{
    AcceptanceCriterionDraft, DeliverableDraft, DomainDraft, DraftSource, EvidenceDraft,
    FieldDraft, ProcurementItemDraft, RequirementDraft, RiskClauseDraft, StaffingRequirementDraft,
};
use crate::error::AppResult;

const MAX_REQUIREMENTS: usize = 12;
const MAX_CHILD_ROWS: usize = 8;
const DEFAULT_REQUIREMENT_CODE: &str = "REQ-001";

pub fn build_rule_domain_draft(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<Option<DomainDraft>> {
    let fields = load_field_drafts(conn, rfp_project_id)?;
    let bundles = load_candidate_bundles(conn, rfp_project_id)?;

    let requirement_snippets = best_snippets(
        bundles.get(CandidateBundleKey::RequirementCandidates.as_str()),
        MAX_REQUIREMENTS,
    );
    let mut requirements = requirement_snippets
        .iter()
        .enumerate()
        .map(|(index, snippet)| requirement_from_snippet(snippet, index + 1))
        .collect::<Vec<_>>();

    if requirements.is_empty() {
        if let Some(snippet) = first_domain_snippet(&bundles) {
            requirements.push(fallback_requirement_from_snippet(snippet));
        }
    }

    let primary_requirement_code = requirements
        .first()
        .map(|requirement| requirement.requirement_code.clone())
        .unwrap_or_else(|| DEFAULT_REQUIREMENT_CODE.to_string());

    let procurement_items = best_snippets(
        bundles.get(CandidateBundleKey::ProcurementCandidates.as_str()),
        MAX_CHILD_ROWS,
    )
    .iter()
    .map(|snippet| procurement_from_snippet(snippet, &primary_requirement_code))
    .collect::<Vec<_>>();

    let staffing_requirements = best_snippets(
        bundles.get(CandidateBundleKey::StaffingCandidates.as_str()),
        MAX_CHILD_ROWS,
    )
    .iter()
    .map(|snippet| staffing_from_snippet(snippet, &primary_requirement_code))
    .collect::<Vec<_>>();

    let deliverables = best_snippets(
        bundles.get(CandidateBundleKey::DeliverableCandidates.as_str()),
        MAX_CHILD_ROWS,
    )
    .iter()
    .map(|snippet| deliverable_from_snippet(snippet, &primary_requirement_code))
    .collect::<Vec<_>>();

    let acceptance_criteria = best_snippets(
        bundles.get(CandidateBundleKey::AcceptanceCandidates.as_str()),
        MAX_CHILD_ROWS,
    )
    .iter()
    .map(|snippet| acceptance_from_snippet(snippet, &primary_requirement_code))
    .collect::<Vec<_>>();

    let risk_clauses = best_snippets(
        bundles.get(CandidateBundleKey::RiskCandidates.as_str()),
        MAX_CHILD_ROWS,
    )
    .iter()
    .map(|snippet| risk_from_snippet(snippet, &primary_requirement_code))
    .collect::<Vec<_>>();

    let has_domain_rows = !requirements.is_empty()
        || !procurement_items.is_empty()
        || !staffing_requirements.is_empty()
        || !deliverables.is_empty()
        || !acceptance_criteria.is_empty()
        || !risk_clauses.is_empty();
    if fields.is_empty() && !has_domain_rows {
        return Ok(None);
    }

    Ok(Some(DomainDraft {
        source: DraftSource::Rule,
        fields,
        requirements,
        procurement_items,
        staffing_requirements,
        deliverables,
        acceptance_criteria,
        risk_clauses,
    }))
}

fn load_field_drafts(conn: &Connection, rfp_project_id: &str) -> AppResult<Vec<FieldDraft>> {
    let mut statement = conn.prepare(
        "SELECT id, field_key, label, raw_value, normalized_value, confidence
         FROM rfp_fields
         WHERE rfp_project_id = ?
         ORDER BY CASE field_key
           WHEN 'business_name' THEN 1
           WHEN 'client' THEN 2
           WHEN 'budget' THEN 3
           WHEN 'period' THEN 4
           WHEN 'contract_method' THEN 5
           WHEN 'deadline' THEN 6
           ELSE 99
         END",
    )?;
    let rows = statement
        .query_map([rfp_project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    rows.into_iter()
        .map(
            |(id, field_key, label, raw_value, normalized_value, confidence)| {
                Ok(FieldDraft {
                    field_key,
                    label,
                    raw_value,
                    normalized_value,
                    confidence,
                    evidence: load_evidence(conn, "rfp_fields", &id)?,
                })
            },
        )
        .collect()
}

fn load_evidence(
    conn: &Connection,
    target_table: &str,
    target_id: &str,
) -> AppResult<Vec<EvidenceDraft>> {
    let mut statement = conn.prepare(
        "SELECT document_block_id, quote, confidence
         FROM evidence_links
         WHERE target_table = ? AND target_id = ?
         ORDER BY confidence DESC",
    )?;
    let evidence = statement
        .query_map(params![target_table, target_id], |row| {
            Ok(EvidenceDraft {
                block_id: row.get(0)?,
                quote: Some(row.get::<_, String>(1)?),
                confidence: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(evidence)
}

fn load_candidate_bundles(
    conn: &Connection,
    rfp_project_id: &str,
) -> AppResult<BTreeMap<String, CandidateBundle>> {
    let mut statement = conn.prepare(
        "SELECT bundle_key, bundle_json
         FROM candidate_bundles
         WHERE rfp_project_id = ?
         ORDER BY bundle_key",
    )?;
    let rows = statement
        .query_map([rfp_project_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut bundles = BTreeMap::new();
    for (bundle_key, bundle_json) in rows {
        bundles.insert(bundle_key, serde_json::from_str(&bundle_json)?);
    }
    Ok(bundles)
}

fn best_snippets(bundle: Option<&CandidateBundle>, max: usize) -> Vec<&CandidateSnippet> {
    let mut snippets = bundle
        .into_iter()
        .flat_map(|bundle| bundle.snippets.iter())
        .filter(|snippet| useful_snippet(snippet))
        .collect::<Vec<_>>();
    snippets.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.page_number.cmp(&right.page_number))
            .then(left.block_index.cmp(&right.block_index))
            .then(left.document_block_id.cmp(&right.document_block_id))
    });

    let mut seen = BTreeSet::new();
    snippets
        .into_iter()
        .filter(|snippet| seen.insert(normalize_for_dedupe(&snippet.quote)))
        .take(max)
        .collect()
}

fn first_domain_snippet(bundles: &BTreeMap<String, CandidateBundle>) -> Option<&CandidateSnippet> {
    [
        CandidateBundleKey::RequirementCandidates,
        CandidateBundleKey::ProcurementCandidates,
        CandidateBundleKey::StaffingCandidates,
        CandidateBundleKey::DeliverableCandidates,
        CandidateBundleKey::AcceptanceCandidates,
        CandidateBundleKey::RiskCandidates,
    ]
    .iter()
    .find_map(|key| {
        best_snippets(bundles.get(key.as_str()), 1)
            .into_iter()
            .next()
    })
}

fn requirement_from_snippet(snippet: &CandidateSnippet, index: usize) -> RequirementDraft {
    let quote = clean_text(&snippet.quote);
    RequirementDraft {
        requirement_code: requirement_code(&quote, index),
        title: title_from_quote(&quote, "요구사항"),
        description: quote.clone(),
        category: requirement_category(&quote).to_string(),
        mandatory: true,
        confidence: confidence(snippet, 0.68),
        evidence: evidence(snippet),
    }
}

fn fallback_requirement_from_snippet(snippet: &CandidateSnippet) -> RequirementDraft {
    let quote = clean_text(&snippet.quote);
    RequirementDraft {
        requirement_code: DEFAULT_REQUIREMENT_CODE.to_string(),
        title: title_from_quote(&quote, "후보 기반 검토 항목"),
        description: quote.clone(),
        category: requirement_category(&quote).to_string(),
        mandatory: true,
        confidence: confidence(snippet, 0.6),
        evidence: evidence(snippet),
    }
}

fn procurement_from_snippet(
    snippet: &CandidateSnippet,
    requirement_code: &str,
) -> ProcurementItemDraft {
    let quote = clean_text(&snippet.quote);
    ProcurementItemDraft {
        requirement_code: requirement_code.to_string(),
        item_type: procurement_item_type(&quote).to_string(),
        name: title_from_quote(&quote, "구매 항목"),
        spec: quote.clone(),
        quantity_text: quantity_text(&quote),
        unit: unit_hint(&quote),
        required: true,
        confidence: confidence(snippet, 0.66),
        evidence: evidence(snippet),
    }
}

fn staffing_from_snippet(
    snippet: &CandidateSnippet,
    requirement_code: &str,
) -> StaffingRequirementDraft {
    let quote = clean_text(&snippet.quote);
    StaffingRequirementDraft {
        requirement_code: requirement_code.to_string(),
        role: staffing_role(&quote).to_string(),
        grade: grade_text(&quote),
        headcount_text: if contains_any(&quote, &["명", "인"]) {
            quote.clone()
        } else {
            String::new()
        },
        mm_text: if contains_any(&quote, &["MM", "M/M", "M·M", "맨먼스"]) {
            quote.clone()
        } else {
            String::new()
        },
        onsite_text: onsite_text(&quote),
        period_text: if contains_any(&quote, &["기간", "개월", "착수", "종료"]) {
            quote.clone()
        } else {
            String::new()
        },
        confidence: confidence(snippet, 0.64),
        evidence: evidence(snippet),
    }
}

fn deliverable_from_snippet(
    snippet: &CandidateSnippet,
    requirement_code: &str,
) -> DeliverableDraft {
    let quote = clean_text(&snippet.quote);
    DeliverableDraft {
        requirement_code: requirement_code.to_string(),
        name: title_from_quote(&quote, "산출물"),
        due_text: if contains_any(&quote, &["제출", "완료", "검수", "납품"]) {
            quote.clone()
        } else {
            String::new()
        },
        format_text: deliverable_format(&quote).to_string(),
        description: quote.clone(),
        confidence: confidence(snippet, 0.64),
        evidence: evidence(snippet),
    }
}

fn acceptance_from_snippet(
    snippet: &CandidateSnippet,
    requirement_code: &str,
) -> AcceptanceCriterionDraft {
    let quote = clean_text(&snippet.quote);
    AcceptanceCriterionDraft {
        requirement_code: requirement_code.to_string(),
        criterion_type: acceptance_type(&quote).to_string(),
        description: quote.clone(),
        threshold: if contains_any(&quote, &["이상", "이하", "%", "통과", "승인"]) {
            quote.clone()
        } else {
            String::new()
        },
        due_text: if contains_any(&quote, &["검수", "인수", "완료"]) {
            quote.clone()
        } else {
            String::new()
        },
        confidence: confidence(snippet, 0.64),
        evidence: evidence(snippet),
    }
}

fn risk_from_snippet(snippet: &CandidateSnippet, requirement_code: &str) -> RiskClauseDraft {
    let quote = clean_text(&snippet.quote);
    RiskClauseDraft {
        requirement_code: requirement_code.to_string(),
        risk_type: risk_type(&quote).to_string(),
        severity: risk_severity(&quote).to_string(),
        description: quote,
        recommended_action: "계약 전 범위, 비용, 책임 한계를 질의서로 명확히 합니다.".to_string(),
        confidence: confidence(snippet, 0.62),
        evidence: evidence(snippet),
    }
}

fn evidence(snippet: &CandidateSnippet) -> Vec<EvidenceDraft> {
    vec![EvidenceDraft {
        block_id: snippet.document_block_id.clone(),
        quote: Some(clean_text(&snippet.quote)),
        confidence: confidence(snippet, 0.7),
    }]
}

fn confidence(snippet: &CandidateSnippet, cap: f64) -> f64 {
    let value = snippet.score.min(cap).max(0.55);
    (value * 100.0).round() / 100.0
}

fn requirement_code(quote: &str, index: usize) -> String {
    let upper = quote.to_ascii_uppercase();
    for prefix in [
        "SFR-", "REQ-", "FUR-", "NFR-", "COR-", "SER-", "DAR-", "SIR-", "QUR-", "SWR-",
    ] {
        if let Some(start) = upper.find(prefix) {
            let code = upper[start..]
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
                .collect::<String>();
            if code.chars().count() > prefix.chars().count() {
                return code;
            }
        }
    }
    format!("REQ-{index:03}")
}

fn requirement_category(quote: &str) -> &'static str {
    if contains_any(quote, &["보안", "인증", "암호", "취약점"]) {
        "security"
    } else if contains_any(quote, &["데이터", "DB", "데이터베이스"]) {
        "data"
    } else if contains_any(quote, &["성능", "처리속도", "응답시간"]) {
        "performance"
    } else if contains_any(quote, &["품질", "검수", "시험", "테스트"]) {
        "quality"
    } else if contains_any(quote, &["인력", "PM", "PL", "MM", "M/M"]) {
        "staffing"
    } else if contains_any(quote, &["관리", "보고", "일정", "산출물"]) {
        "management"
    } else if contains_any(quote, &["기능", "연계", "시스템", "API", "화면", "요구"]) {
        "functional"
    } else {
        "other"
    }
}

fn procurement_item_type(quote: &str) -> &'static str {
    if contains_any(quote, &["클라우드", "cloud", "AWS", "Azure", "GCP"]) {
        "cloud"
    } else if contains_any(quote, &["서버", "스토리지", "장비", "PC"]) {
        "hardware"
    } else if contains_any(quote, &["DB", "데이터베이스"]) {
        "database"
    } else if contains_any(quote, &["네트워크", "스위치", "방화벽"]) {
        "network"
    } else if contains_any(quote, &["보안솔루션", "보안"]) {
        "security"
    } else if contains_any(quote, &["라이선스", "license"]) {
        "license"
    } else if contains_any(quote, &["소프트웨어", "SW", "software"]) {
        "software"
    } else if contains_any(quote, &["용역", "서비스"]) {
        "service"
    } else {
        "other"
    }
}

fn staffing_role(quote: &str) -> &'static str {
    if contains_any(quote, &["PM", "프로젝트 관리자"]) {
        "PM"
    } else if contains_any(quote, &["PL"]) {
        "PL"
    } else if contains_any(quote, &["개발자", "개발"]) {
        "개발자"
    } else if contains_any(quote, &["디자이너", "UX", "UI"]) {
        "디자이너"
    } else if contains_any(quote, &["보안"]) {
        "보안 담당"
    } else if contains_any(quote, &["품질", "QA"]) {
        "QA"
    } else {
        "투입인력"
    }
}

fn grade_text(quote: &str) -> String {
    ["특급", "고급", "중급", "초급"]
        .iter()
        .find(|grade| quote.contains(**grade))
        .map(|grade| (*grade).to_string())
        .unwrap_or_default()
}

fn onsite_text(quote: &str) -> String {
    if quote.contains("비상주") {
        "비상주".to_string()
    } else if quote.contains("상주") {
        "상주".to_string()
    } else {
        String::new()
    }
}

fn deliverable_format(quote: &str) -> &'static str {
    if contains_any(quote, &["PDF", "HWP", "엑셀", "Excel", "PPT"]) {
        "전자파일"
    } else if contains_any(quote, &["보고서", "설계서", "매뉴얼", "문서", "자료"]) {
        "문서"
    } else {
        ""
    }
}

fn acceptance_type(quote: &str) -> &'static str {
    if contains_any(quote, &["성능"]) {
        "performance"
    } else if contains_any(quote, &["보안"]) {
        "security"
    } else if contains_any(quote, &["SLA"]) {
        "sla"
    } else if contains_any(quote, &["하자"]) {
        "warranty"
    } else if contains_any(quote, &["시험", "테스트", "통과"]) {
        "test"
    } else if contains_any(quote, &["검수", "검사", "인수", "승인"]) {
        "inspection"
    } else {
        "other"
    }
}

fn risk_type(quote: &str) -> &'static str {
    if contains_any(quote, &["무상", "추가 요청", "추가 과업"]) {
        "free_work"
    } else if contains_any(quote, &["지체상금", "손해배상", "책임", "위약"]) {
        "liability"
    } else if contains_any(quote, &["협의", "필요 시", "별도 협의"]) {
        "ambiguous_spec"
    } else if contains_any(quote, &["비용 부담", "대금", "지급"]) {
        "payment"
    } else if contains_any(quote, &["짧은 기간", "단기간", "긴급"]) {
        "short_schedule"
    } else if contains_any(quote, &["특정 업체", "벤더", "종속"]) {
        "vendor_lock"
    } else if contains_any(quote, &["보안", "개인정보"]) {
        "security"
    } else if contains_any(quote, &["범위", "과업"]) {
        "scope_creep"
    } else {
        "other"
    }
}

fn risk_severity(quote: &str) -> &'static str {
    if contains_any(quote, &["손해배상", "위약", "지체상금"]) {
        "high"
    } else if contains_any(quote, &["무상", "책임", "비용 부담"]) {
        "medium"
    } else {
        "low"
    }
}

fn quantity_text(quote: &str) -> String {
    if quote.chars().any(|ch| ch.is_ascii_digit()) {
        quote.to_string()
    } else {
        String::new()
    }
}

fn unit_hint(quote: &str) -> String {
    [
        "식", "대", "명", "개", "건", "copy", "Copy", "개월", "MM", "M/M",
    ]
    .iter()
    .find(|unit| quote.contains(**unit))
    .map(|unit| (*unit).to_string())
    .unwrap_or_default()
}

fn useful_snippet(snippet: &CandidateSnippet) -> bool {
    let quote = clean_text(&snippet.quote);
    quote.chars().count() >= 8 && quote.matches('.').count() < 20
}

fn title_from_quote(quote: &str, fallback: &str) -> String {
    let line = quote
        .split(['\n', '\r'])
        .find(|line| !line.trim().is_empty())
        .unwrap_or(fallback);
    let trimmed = line
        .trim()
        .trim_start_matches(['-', '*', 'ㆍ', '·', '•', ' ', '\t'])
        .trim();
    let title = if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    };
    truncate_chars(title, 80)
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_for_dedupe(text: &str) -> String {
    clean_text(text).to_lowercase()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn contains_any(text: &str, terms: &[&str]) -> bool {
    let lower = text.to_lowercase();
    terms
        .iter()
        .any(|term| lower.contains(&term.to_lowercase()))
}

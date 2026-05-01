pub fn parse_number(text: &str) -> Option<f64> {
    let mut value = String::new();
    let mut started = false;
    let mut seen_digit = false;
    for ch in text.chars() {
        if (ch == '-' || ch == '+') && !started {
            value.push(ch);
            started = true;
        } else if ch.is_ascii_digit() {
            value.push(ch);
            started = true;
            seen_digit = true;
        } else if ch == '.' && started {
            value.push(ch);
        } else if ch == ',' && started {
            continue;
        } else if started && seen_digit {
            break;
        } else if started {
            value.clear();
            started = false;
        }
    }
    if !seen_digit {
        None
    } else {
        value.parse::<f64>().ok()
    }
}

pub fn parse_onsite(text: &str) -> Option<i64> {
    let normalized = text.trim();
    if normalized.contains("비상주") || normalized.contains("원격") {
        Some(0)
    } else if normalized.contains("상주") {
        Some(1)
    } else {
        None
    }
}

pub fn parse_unit(text: &str) -> Option<String> {
    let mut started = false;
    let mut seen_digit = false;
    let mut unit = String::new();
    for ch in text.chars() {
        if (ch == '-' || ch == '+') && !started {
            started = true;
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' || (ch == ',' && started) {
            started = true;
            if ch.is_ascii_digit() {
                seen_digit = true;
            }
            continue;
        }
        if started && seen_digit && !ch.is_whitespace() {
            unit.push(ch);
        } else if started && seen_digit && !unit.is_empty() {
            break;
        } else if started && !seen_digit {
            started = false;
        }
    }
    let trimmed = unit.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn parse_mm(text: &str) -> Option<f64> {
    let normalized = text.to_ascii_lowercase().replace(' ', "");
    if normalized.contains("mm") || normalized.contains("m/m") || text.contains("개월") {
        parse_number(text)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quantity_and_unit_from_korean_text() {
        assert_eq!(parse_number("3대"), Some(3.0));
        assert_eq!(parse_number("1.5식"), Some(1.5));
        assert_eq!(parse_number("총 12 M/M"), Some(12.0));
        assert_eq!(parse_number("1,200대"), Some(1200.0));
        assert_eq!(parse_number("-1대"), Some(-1.0));
    }

    #[test]
    fn parses_onsite_text() {
        assert_eq!(parse_onsite("상주"), Some(1));
        assert_eq!(parse_onsite("비상주"), Some(0));
        assert_eq!(parse_onsite("원격 수행"), Some(0));
        assert_eq!(parse_onsite("협의"), None);
    }
}

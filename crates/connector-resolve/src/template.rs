//! The template vocabulary: one brace grammar, and flux-lang's value semantics reproduced against
//! it.
//!
//! Every function here is moved verbatim from `connector-pack`'s `request.rs`, where it was written
//! against `flux_lang`'s `runtime.rs` — `interpolate_str`, `json_truthy` and `lit_text`. Moving them
//! is what lets the derivation leave the engine behind without changing a single rule: the document
//! is evaluated by the semantics the emitted Flux was evaluated by, which is the only form of
//! agreement a differential can prove.

use serde_json::Value;

/// **The one brace grammar**, shared by every template this crate reads.
///
/// `fill` is called with each placeholder name in order; `Some` replaces it, `None` leaves it
/// verbatim — which is flux-lang's own `interpolate_str` behaviour and the whole reason a guarded
/// filter means anything. A filled value is never rescanned, so nothing a tenant configures can
/// splice a caller's parameter in.
pub fn scan_template(template: &str, mut fill: impl FnMut(&str) -> Option<String>) -> String {
    if !template.contains('{') {
        return template.to_string();
    }
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let at_brace = &rest[open..];
        let (open_token, close_token) = if at_brace.starts_with("{{") {
            ("{{", "}}")
        } else {
            ("{", "}")
        };
        let inner = &at_brace[open_token.len()..];
        let Some(close) = inner.find(close_token) else {
            out.push_str(at_brace);
            return out;
        };
        match fill(inner[..close].trim()) {
            Some(value) => {
                out.push_str(&value);
                rest = &inner[close + close_token.len()..];
            }
            None => {
                out.push_str(open_token);
                rest = inner;
            }
        }
    }
    out.push_str(rest);
    out
}

/// flux-lang's `lit_text`: a string is itself, `null` is empty, anything else is compact JSON.
pub fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// flux-lang's `json_truthy`: null/false/0/empty are falsey, and so is the *text* `"false"`/`"0"`.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(bit) => *bit,
        Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("false") && trimmed != "0"
        }
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

/// The sentinel that stands in for a placeholder while a template is being read positionally.
///
/// `\u{1}` because a template is generator-authored text and carries no control characters. **A
/// configuration value never passes through a marked string** — markers are resolved against the
/// template alone, and values are substituted only into the result — so a value cannot forge one.
pub(crate) const MARK: char = '\u{1}';

/// Wrap `name` as a positional marker.
pub(crate) fn mark(name: &str) -> String {
    format!("{MARK}{name}{MARK}")
}

/// Each `\u{1}name\u{1}` in `marked`, as `(byte offset of the marker, name)`.
pub(crate) fn marked_placeholders(marked: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut rest = marked;
    let mut consumed = 0;
    while let Some(open) = rest.find(MARK) {
        let after = &rest[open + MARK.len_utf8()..];
        let Some(close) = after.find(MARK) else {
            break;
        };
        found.push((consumed + open, &after[..close]));
        consumed += open + close + 2 * MARK.len_utf8();
        rest = &after[close + MARK.len_utf8()..];
    }
    found
}

/// Replace each `\u{1}name\u{1}` in `marked` with `fill(name)`, leaving an unfilled marker's name
/// verbatim.
///
/// The counterpart of [`marked_placeholders`], and the only place a configuration value enters a
/// marked string — after every marker has been located, so no value can be read as one.
pub(crate) fn fill_marked(marked: &str, mut fill: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(marked.len());
    let mut rest = marked;
    while let Some(open) = rest.find(MARK) {
        out.push_str(&rest[..open]);
        let after = &rest[open + MARK.len_utf8()..];
        let Some(close) = after.find(MARK) else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        out.push_str(&fill(name).unwrap_or_else(|| name.to_owned()));
        rest = &after[close + MARK.len_utf8()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unfilled_placeholder_stays_verbatim() {
        assert_eq!(scan_template("a/{b}/c", |_| None), "a/{b}/c");
        assert_eq!(
            scan_template("a/{b}/c", |name| Some(name.to_uppercase())),
            "a/B/c"
        );
    }

    #[test]
    fn a_doubled_brace_is_an_escape_and_an_unterminated_one_is_text() {
        assert_eq!(scan_template("{{a}}", |_| None), "{{a}}");
        assert_eq!(scan_template("a/{b", |_| Some("x".into())), "a/{b");
    }

    #[test]
    fn value_to_text_is_flux_langs() {
        assert_eq!(text(&Value::String("a".into())), "a");
        assert_eq!(text(&Value::Null), "");
        assert_eq!(text(&serde_json::json!(1)), "1");
        assert_eq!(text(&serde_json::json!({"a": 1})), "{\"a\":1}");
    }

    #[test]
    fn truthiness_is_flux_langs() {
        assert!(!truthy(&Value::Null));
        assert!(!truthy(&serde_json::json!("false")));
        assert!(!truthy(&serde_json::json!("0")));
        assert!(!truthy(&serde_json::json!(0)));
        assert!(truthy(&serde_json::json!("a")));
        assert!(truthy(&serde_json::json!([1])));
    }

    #[test]
    fn markers_are_located_and_filled_by_offset() {
        let marked = format!("{}/{}", mark("a"), mark("b"));
        let found = marked_placeholders(&marked);
        assert_eq!(
            found.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            fill_marked(&marked, |name| Some(name.to_uppercase())),
            "A/B"
        );
        assert_eq!(fill_marked(&marked, |_| None), "a/b");
    }
}

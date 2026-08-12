//! **The request plan**: the unit a consumer wraps and dispatches, and the unit the differential
//! gate compares.
//!
//! One boundary this type exists to hold, named on the Exchange side and worth stating here too:
//! **projecting a plan into a Tool is not composing a request.** A consumer wraps and dispatches the
//! plan it was handed; a consumer that *edits* one has become the second request path this family
//! already rejected.

use crate::request::Request;

/// Text that may contain credential material and therefore never reveals itself through `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveText(String);

impl SensitiveText {
    /// Declare that `text` is secret-bearing.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Deliberately expose the value. Every call site is a place a reviewer should stop at.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SensitiveText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// **What one call would send, as data.**
///
/// The request itself, where the host's network policy should judge it, and every string the host's
/// redactor must be holding before any of it reaches a surface.
///
/// # The redaction set is a *requirement*, not a record
///
/// A plan is derived with the credential already placed, so [`redactions`](Self::redactions) names
/// the strings that are *on* the request. A consumer registers them **before** it does anything
/// with the plan — formats it, logs it, dispatches it. `connector-pack` keeps a stronger order than
/// that and registers each value as it is resolved, before a request exists at all, which is why
/// this field is a checkable restatement rather than the only defence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RequestPlan {
    /// The request as it goes out: method, URL (query included), headers, body.
    pub request: Request,
    /// **Where this call goes**, for the host's network policy to judge — the request URL *before*
    /// any credential was placed, so a query-placed secret never enters an approval prompt, a
    /// policy rule or an evidence record.
    pub permission_subjects: Vec<String>,
    /// Every credential-derived string that travels on this request, in the form it travels in.
    pub redactions: Vec<SensitiveText>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_text_never_prints() {
        let text = SensitiveText::new("SENTINEL");
        assert_eq!(format!("{text:?}"), "<redacted>");
        assert_eq!(text.expose_secret(), "SENTINEL");
    }

    #[test]
    fn a_plans_debug_carries_no_credential() {
        let plan = RequestPlan {
            request: Request {
                method: "GET".to_string(),
                url: "https://vendor.example/things?api_key=SENTINEL".to_string(),
                headers: std::collections::BTreeMap::from([(
                    "Authorization".to_string(),
                    "Bearer SENTINEL".to_string(),
                )]),
                body: None,
            },
            permission_subjects: vec!["https://vendor.example/things".to_string()],
            redactions: vec![SensitiveText::new("SENTINEL")],
        };
        let printed = format!("{plan:?}");
        assert!(!printed.contains("SENTINEL"), "{printed}");
    }
}

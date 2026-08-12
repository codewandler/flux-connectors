//! **Where a configuration variable lands on the request** — and therefore what its value may be.
//!
//! Moved from `connector-pack`'s `request.rs` unchanged (C-538). What changed is *where the answer
//! comes from*: the pack read a slot off the emitted Flux by scanning literals, and this crate reads
//! it off the canonical document's own `endpoint` map. The rules a value is then held to are the
//! same rules, which is why they moved rather than being rewritten.

use connector_address::HttpsOrigin;

use crate::template::{fill_marked, mark, marked_placeholders, scan_template, MARK};

/// **Where a configuration variable lands on the request.**
///
/// The three request positions need three answers, and "percent-encode it" is the wrong answer to
/// two of them. Encoding a *path segment* is meaningful; encoding a **host** is not, because a host
/// has different legal syntax and a different failure mode, and there is no encoding at all for an
/// HTTP field value — a bad one can only be refused.
///
/// | slot | answer | why |
/// |---|---|---|
/// | [`Origin`](Self::Origin) | refuse unless the value is a canonical HTTPS origin | the operator names the whole destination |
/// | [`Host`](Self::Host) | refuse unless the whole composed authority is a hostname | a value here moves the **origin** |
/// | [`Path`](Self::Path) | refuse | a `zone_id` with a `/` in it is an operator's mistake, and silently encoding it produces a 404 they cannot diagnose |
/// | [`Query`](Self::Query) | refuse structural characters, then pass the raw value to the query encoder | request assembly encodes every query key and value exactly once |
/// | [`Header`](Self::Header) | refuse | a CR or LF appends a header of the value's choosing, and no encoding exists to make it safe |
/// | [`Unplaced`](Self::Unplaced) | refuse unless **every** rule above accepts it, the host rule included | fail closed |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    /// A complete operator-approved HTTPS origin. The connector appends its reviewed API path.
    Origin,
    /// The authority of a templated base URL — the `{subdomain}` of
    /// `https://{subdomain}.zendesk.com`. The severe one.
    Host,
    /// A path segment: a `{var}` in the path half of a base URL, or a `path.` pin.
    Path,
    /// A query parameter value — a `query.` pin, such as vercel's `{teamId}`.
    Query,
    /// A request header value — a `header.` pin.
    Header,
    /// **A value this derivation could not place in exactly one position** — either because the
    /// document names no position for it, or because it genuinely lands in more than one (C-229).
    ///
    /// Its rule is the **intersection of every position's rule**, the host's included, and it does
    /// not encode: an encoding is only safe where the position is known. `algolia/app_id` is the
    /// shipped instance — one field binding both `endpoint.app_id` and
    /// `header.X-Algolia-Application-Id`, so one value composes the authority *and* travels as a
    /// header, and holding it to both predicates is the answer rather than a degradation.
    Unplaced,
}

impl Slot {
    /// The word a refusal calls this position by.
    pub fn word(self) -> &'static str {
        match self {
            Self::Origin => "HTTPS origin",
            Self::Host => "host",
            Self::Path => "path segment",
            Self::Query => "query parameter",
            Self::Header => "header",
            Self::Unplaced => "unplaced position",
        }
    }

    /// The slot the document spells `name`, or [`Unplaced`](Self::Unplaced) for anything else.
    ///
    /// A spelling this crate does not model is [`Unplaced`](Self::Unplaced) rather than an error,
    /// which is the fail-closed direction: an unrecognised position refuses the most rather than
    /// admitting a value nothing checked.
    pub fn from_document(name: &str) -> Slot {
        match name {
            "origin" => Slot::Origin,
            "host" => Slot::Host,
            "path" => Slot::Path,
            "query" => Slot::Query,
            "header" => Slot::Header,
            _ => Slot::Unplaced,
        }
    }

    /// **Whether `value` may be substituted here at all** — for the one caller that cannot fail.
    ///
    /// `Tool::permission_subjects` has nowhere to put a refusal, so it needs the predicate without
    /// the error. For [`Host`](Self::Host) it applies [`validate_authority`] to the value alone,
    /// which is a sufficient condition rather than the full check: a value made only of host
    /// characters cannot introduce a delimiter into a template that had none either.
    pub fn substitutable(self, value: &str) -> bool {
        match self {
            Self::Origin => HttpsOrigin::parse(value).is_ok(),
            Self::Host => validate_authority(value).is_ok(),
            other => other.validate(value).is_ok(),
        }
    }

    /// **Whether `value` can be substituted here without reshaping the request** — and the text to
    /// substitute if it can.
    ///
    /// Query values stay raw here because the query encoder is the single encoding boundary;
    /// pre-encoding a pin here would turn `%2F` into `%252F` when request assembly applies Flux's
    /// semantics.
    ///
    /// [`Host`](Self::Host) is not answered here. Its question is about the authority the
    /// *template* composes rather than about the value alone, so it is [`validate_authority`]'s.
    ///
    /// # Errors
    ///
    /// The reason, phrased for the operator who supplied the value.
    pub fn validate(self, value: &str) -> Result<String, String> {
        if value.trim().is_empty() {
            return Err("a configuration value must not be empty or all whitespace".to_owned());
        }
        // Refused in every position: substitution fills `{placeholder}`s, so a value spelling one
        // would either be filled in a second time or survive into the URL as text.
        if let Some(bad) = value.chars().find(|c| *c == '{' || *c == '}') {
            return Err(format!(
                "{value:?} contains {bad:?}, and a configuration value is substituted into a \
                 `{{placeholder}}`, so a value spelling one of its own would be filled in twice or \
                 reach the vendor verbatim"
            ));
        }
        match self {
            // **The substituted text is the normalized origin, not the supplied one** (C-523).
            Self::Origin => HttpsOrigin::parse(value)
                .map(HttpsOrigin::into_string)
                .map_err(|refusal| refusal.to_string()),
            Self::Host => Ok(value.to_owned()),
            Self::Path => {
                validate_path(value)?;
                Ok(value.to_owned())
            }
            Self::Query => {
                validate_query(value)?;
                Ok(value.to_owned())
            }
            Self::Header => {
                validate_header(value)?;
                Ok(value.to_owned())
            }
            // Fail closed: a value nothing placed is held to **every** rule at once, the host rule
            // included, and is not encoded. The host rule is load-bearing rather than decorative —
            // without it this arm accepts `acme.zendesk.com@evil.example`, because neither `@` nor
            // `:` appears in the path, query or header charsets.
            Self::Unplaced => {
                validate_path(value)?;
                validate_query(value)?;
                validate_header(value)?;
                validate_authority(value)?;
                Ok(value.to_owned())
            }
        }
    }
}

/// A value that stays inside one path segment.
pub fn validate_path(value: &str) -> Result<(), String> {
    if value == "." || value == ".." {
        return Err(format!(
            "{value:?} is a relative path segment, so the request would address the segment above \
             or beside the one it was configured for"
        ));
    }
    match value
        .chars()
        .find(|c| "/?#%\\".contains(*c) || c.is_whitespace() || c.is_control())
    {
        Some(bad) => Err(format!(
            "{value:?} contains {bad:?}, which does not stay inside one path segment — a `/` (or a \
             `%` that could encode one) reshapes the URL, and a `?` or `#` ends the path entirely"
        )),
        None => Ok(()),
    }
}

/// A value that adds no parameter of its own to the query string.
pub fn validate_query(value: &str) -> Result<(), String> {
    match value
        .chars()
        .find(|c| "&=?#+".contains(*c) || c.is_whitespace() || c.is_control())
    {
        Some(bad) => Err(format!(
            "{value:?} contains {bad:?}, which is query-string structure rather than a value — an \
             `&` or `=` would add a parameter of its own to every request this connector makes"
        )),
        None => Ok(()),
    }
}

/// A value that is an HTTP field value and nothing more (RFC 9110 §5.5).
pub fn validate_header(value: &str) -> Result<(), String> {
    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii() || c.is_ascii_control())
    {
        return Err(format!(
            "{value:?} contains {bad:?}, which is not an HTTP field value (RFC 9110 §5.5) — a \
             newline in particular would append a header of its own to every request"
        ));
    }
    if value.trim() != value {
        return Err(format!(
            "{value:?} starts or ends with whitespace, which an HTTP field value may not \
             (RFC 9110 §5.5)"
        ));
    }
    Ok(())
}

/// **Whether `authority` is still an authority once a configuration value has been substituted into
/// it.**
///
/// It must consist only of characters that **cannot delimit an authority**: ASCII alphanumerics,
/// `-`, `.` and `_`. Every dot-separated label must be non-empty. An allow-list rather than a
/// blocklist of `@`, `/` and `:`, because a blocklist stops the measured case
/// (`acme.zendesk.com@evil.example`) and not the ones nobody enumerated.
///
/// # Errors
///
/// The reason, phrased for the operator who supplied the value.
pub fn validate_authority(authority: &str) -> Result<(), String> {
    if authority.is_empty() {
        return Err("a host must not be empty".to_owned());
    }
    if let Some(bad) = authority
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == '_'))
    {
        return Err(format!(
            "the composed host {authority:?} contains {bad:?}, which is not a host character — a \
             configuration value may not introduce one, because `@`, `:`, `/` and `%` all move or \
             truncate the authority the connector declared"
        ));
    }
    if authority.split('.').any(str::is_empty) {
        return Err(format!(
            "the composed host {authority:?} has an empty label, so it is not a hostname"
        ));
    }
    Ok(())
}

/// Validate an authority whose **template**, rather than a configured value, may state a port.
///
/// Asterisk ARI is the first shipped case: `https://{host}:8089/ari`. Only one decimal port written
/// literally in the connector is admitted, while every substituted host value remains subject to
/// [`validate_authority`].
pub fn validate_templated_authority(template: &str, composed: &str) -> Result<(), String> {
    let Some((_template_host, template_port)) = template.rsplit_once(':') else {
        return validate_authority(composed);
    };
    if template_port.contains(MARK)
        || template_port.is_empty()
        || !template_port.chars().all(|c| c.is_ascii_digit())
    {
        return validate_authority(composed);
    }
    let port: u16 = template_port
        .parse()
        .map_err(|_| format!("the declared port {template_port:?} is not between 1 and 65535"))?;
    if port == 0 {
        return Err("the declared port 0 is not between 1 and 65535".to_owned());
    }
    let Some((composed_host, composed_port)) = composed.rsplit_once(':') else {
        return Err(format!(
            "the template declares port {template_port}, but the composed authority {composed:?} does not"
        ));
    };
    if composed_port != template_port {
        return Err(format!(
            "the template declares port {template_port}, but the composed authority uses {composed_port:?}"
        ));
    }
    validate_authority(composed_host)
}

/// **The host half**: the authority a templated URL composes must still be the authority the
/// template declared.
///
/// The check runs against the authority the *template* delimits — `literal` is marked first, so the
/// span ends at a generator-authored `/`, `?` or `#` and never at one a value carried in. The raw
/// values are then substituted into that span and the result must be a hostname.
///
/// Returns `Ok(None)` when the template's authority carries no configured variable at all, and
/// `Ok(Some(_))` — the variable a refusal should name — otherwise.
///
/// # Errors
///
/// `(variable, reason)`: the first configured variable in the authority, and why the composed
/// authority is not one.
pub(crate) fn check_authority(
    literal: &str,
    filled: &str,
    value_of: &dyn Fn(&str) -> Option<String>,
) -> Result<(), (String, String)> {
    let marked = scan_template(literal, |name| Some(mark(name)));
    let Some((_, after_scheme)) = marked.split_once("://") else {
        return Ok(());
    };
    let end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let template_authority = &after_scheme[..end];
    let hosted: Vec<&str> = marked_placeholders(template_authority)
        .into_iter()
        .map(|(_, name)| name)
        .filter(|name| value_of(name).is_some())
        .collect();
    let Some(&first) = hosted.first() else {
        return Ok(());
    };

    let composed = fill_marked(template_authority, value_of);
    let refuse = |reason: String| (first.to_owned(), reason);
    validate_templated_authority(template_authority, &composed).map_err(refuse)?;

    // A restatement of the property, executed rather than asserted in prose: no character the rule
    // above permits can delimit, so reading the finished URL the way a transport does must yield
    // exactly the string that was checked.
    let resolved = filled
        .split_once("://")
        .map(|(_, rest)| &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())]);
    if resolved != Some(composed.as_str()) {
        return Err(refuse(format!(
            "the composed host is {composed:?} but the URL resolves to \
             {resolved:?}; the two must agree"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_rule_refuses_what_moves_the_authority() {
        assert!(validate_authority("acme.zendesk.com").is_ok());
        for composed in [
            "acme.zendesk.com@evil.example.zendesk.com",
            "acme:8080.zendesk.com",
            "acme/x.zendesk.com",
            "acme%2e.zendesk.com",
            "acme .zendesk.com",
            "acme\n.zendesk.com",
            "acmé.zendesk.com",
            "..zendesk.com",
            "{subdomain}.zendesk.com",
        ] {
            assert!(
                validate_authority(composed).is_err(),
                "{composed:?} was accepted as a host"
            );
        }
    }

    #[test]
    fn an_unplaced_value_is_held_to_every_rule_including_the_hosts() {
        assert!(Slot::Unplaced
            .validate("acme.zendesk.com@evil.example")
            .is_err());
        assert!(Slot::Header
            .validate("acme.zendesk.com@evil.example")
            .is_ok());
        assert!(Slot::Unplaced.validate("acme").is_ok());
    }

    #[test]
    fn a_document_position_maps_onto_a_slot_and_anything_else_fails_closed() {
        assert_eq!(Slot::from_document("host"), Slot::Host);
        assert_eq!(Slot::from_document("origin"), Slot::Origin);
        assert_eq!(Slot::from_document("path"), Slot::Path);
        assert_eq!(Slot::from_document("query"), Slot::Query);
        assert_eq!(Slot::from_document("header"), Slot::Header);
        assert_eq!(Slot::from_document("cookie"), Slot::Unplaced);
    }

    /// The measured case, at the level of the composed template rather than the finished URL.
    #[test]
    fn a_value_that_moves_the_authority_is_refused_in_context() {
        let values = |name: &str| match name {
            "subdomain" => Some("acme.zendesk.com@evil.example".to_owned()),
            _ => None,
        };
        let literal = "https://{subdomain}.zendesk.com/api/v2";
        let filled = scan_template(literal, values);
        let refusal = check_authority(literal, &filled, &values).expect_err("it is refused");
        assert_eq!(refusal.0, "subdomain");
        assert!(refusal.1.contains('@'), "{}", refusal.1);
    }
}

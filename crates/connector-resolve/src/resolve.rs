//! **The derivation**: a request template, a caller's parameters and a tenant's endpoint values in;
//! a request plan out.
//!
//! # Why this is not a second request path
//!
//! The rules here are the rules `connector-pack`'s AST evaluator applied to the emitted Flux, moved
//! rather than reinvented — the brace grammar, `lit_text`, `json_truthy`, the structured-query wire
//! contract, the `User-Agent` default, and the slot checks a configuration value is held to. What
//! changed is the *input*: the closed template the document publishes instead of a parsed module.
//! Decision 0022's migration rule is that the two are proven byte-identical for every operation in
//! the catalogue before either is deleted, which is
//! `connector-pack/tests/main/catalogue_differential.rs`.
//!
//! # The order the refusals happen in is part of the contract
//!
//! Parameters first (a missing one is the caller's mistake and names itself), then the base URL and
//! its authority (a configuration value that moves the origin is refused before anything is
//! composed), then the URL, the query, the headers and the body. A partly-evaluated request is not
//! a degraded request; it is a different call, and the vendor answers it.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::auth::{place, placed_form, Assembled};
use crate::document::{splice_of, BodyTemplate, Operation, ValueTemplate};
use crate::plan::{RequestPlan, SensitiveText};
use crate::request::{append_query, Request};
use crate::slot::{check_authority, Slot};
use crate::template::{scan_template, text, truthy};
use crate::Error;

/// **The request** `operation` makes when called with `params`, over `endpoints`.
///
/// `endpoints` maps each name [`Operation::endpoint_variables`] reports to the value the host's
/// configuration port supplied. It is **complete or the call does not get here** — the caller
/// resolves every variable and refuses first — so substitution is total by construction rather than
/// by inspection.
///
/// No credential is applied. This is the request the operation's own document describes and nothing
/// more, which is what lets a permission subject name a URL that carries no secret.
///
/// # Errors
///
/// [`Error::MissingParameter`] when the caller omitted a parameter the operation declares,
/// [`Error::UnsafePathParameter`] when a caller string would escape the URL path segment the
/// template places it in, [`Error::UnsafeConfig`] when a configuration value would reshape the
/// request it is substituted into, [`Error::Unbuildable`] when the template says something this
/// vocabulary does not carry, and [`Error::UnresolvedEndpoint`] when a configuration variable is
/// still named in the finished URL — which only a caller-supplied value can cause.
pub fn build_request(
    operation: &Operation,
    base_url: &str,
    params: &Value,
    endpoints: &BTreeMap<String, String>,
) -> Result<Request, Error> {
    let cx = Derivation {
        operation,
        endpoints,
    };
    cx.build(base_url, params)
}

/// **The plan** `operation` makes when called with `params`, with `credentials` placed on it.
///
/// The plan is data: the request, the subjects a host's network policy judges it by, and every
/// credential-derived string the host's redactor must hold. Dispatch is the consumer's.
///
/// # Errors
///
/// Everything [`build_request`] refuses, plus [`Error::CredentialCollision`] when a credential's
/// header is one the template already sets and [`Error::InboundCredential`] for a signing secret,
/// which verifies bytes that arrived and never leaves.
pub fn resolve(
    operation: &Operation,
    base_url: &str,
    params: &Value,
    endpoints: &BTreeMap<String, String>,
    credentials: &[Assembled],
) -> Result<RequestPlan, Error> {
    let mut request = build_request(operation, base_url, params, endpoints)?;
    // Read before the credentials are placed, deliberately: a query placement would otherwise put a
    // secret into every approval prompt, policy rule and evidence record that quotes a subject.
    let permission_subjects = vec![request.url.clone()];

    let mut redactions = Vec::new();
    for credential in credentials {
        redactions.push(SensitiveText::new(credential.expose_value()));
        // A placement that *transforms* the value — query encoding, whose alphabet a base64
        // credential is made of — puts a second string on the wire, and a redactor holding only the
        // first would not scrub it.
        if let Some(travelling) = placed_form(credential.placement(), credential.expose_value()) {
            redactions.push(SensitiveText::new(travelling));
        }
        place(&operation.id, credential, &mut request)?;
    }

    Ok(RequestPlan {
        request,
        permission_subjects,
        redactions,
    })
}

/// One request being derived: the operation it belongs to, and the configuration its template is
/// substituted against.
struct Derivation<'a> {
    operation: &'a Operation,
    endpoints: &'a BTreeMap<String, String>,
}

impl Derivation<'_> {
    fn unbuildable(&self, message: String) -> Error {
        Error::Unbuildable {
            operation: self.operation.id.clone(),
            message,
        }
    }

    /// One configuration value, against the position it lands on. Returns the text to substitute.
    fn check(&self, variable: &str, value: &str) -> Result<String, Error> {
        let slot = self
            .operation
            .endpoint_slots()
            .get(variable)
            .copied()
            .unwrap_or(Slot::Unplaced);
        slot.validate(value).map_err(|reason| Error::UnsafeConfig {
            operation: self.operation.id.clone(),
            variable: variable.to_owned(),
            position: slot.word(),
            reason,
        })
    }

    /// Fill this operation's configuration variables into a literal, checking each value against
    /// the position it lands on (C-214).
    ///
    /// `scan_template` cannot carry a `Result` out of its callback, so the first refusal is parked
    /// and returned before the filled string is used for anything.
    fn substitute(&self, literal: &str) -> Result<String, Error> {
        if !literal.contains('{') {
            return Ok(literal.to_owned());
        }
        let mut refusal = None;
        let filled = scan_template(literal, |name| {
            let value = self.endpoints.get(name)?;
            Some(self.check(name, value).unwrap_or_else(|error| {
                refusal.get_or_insert(error);
                value.clone()
            }))
        });
        match refusal {
            Some(error) => Err(error),
            None => Ok(filled),
        }
    }

    /// The service base URL with this tenant's values in it — and the authority it composes still
    /// the authority the template declared.
    fn base(&self, base_url: &str) -> Result<String, Error> {
        let literal = base_url.trim_end_matches('/');
        let filled = self.substitute(literal)?;
        check_authority(literal, &filled, &|name| {
            self.endpoints
                .get(name)
                .map(|value| self.check(name, value).unwrap_or_else(|_| value.clone()))
        })
        .map_err(|(variable, reason)| Error::UnsafeConfig {
            operation: self.operation.id.clone(),
            variable,
            position: Slot::Host.word(),
            reason,
        })?;
        Ok(filled)
    }

    /// The caller's value for the document parameter `name`, or `Null` when it declares none.
    ///
    /// The lookup is by the parameter's **caller-facing symbol**, which is what a contract
    /// advertises and what a host passes. See [`crate::document`] for why the two names differ.
    fn param<'p>(&self, params: &'p Value, name: &str) -> &'p Value {
        match self.operation.symbol(name) {
            Some(symbol) => params.get(symbol).unwrap_or(&Value::Null),
            None => &Value::Null,
        }
    }

    /// Render one value template: a splice takes the caller's value, a literal takes this tenant's
    /// configuration.
    fn value(&self, template: &ValueTemplate, params: &Value) -> Result<Value, Error> {
        match template {
            ValueTemplate::Splice { param } => Ok(self.param(params, param).clone()),
            ValueTemplate::Literal(literal) => Ok(Value::String(self.substitute(literal)?)),
        }
    }

    fn build(&self, base_url: &str, params: &Value) -> Result<Request, Error> {
        let operation = self.operation;

        // Every declared parameter must be supplied, exactly as flux's own composite dispatch
        // requires. An *optional* parameter is one a caller may pass `null` for, not one they may
        // omit: a guarded query field turns null into "do not send this filter", while an absent
        // path parameter would quietly leave `{ticket_id}` in the URL.
        for name in operation.caller_parameters() {
            let value = params.get(name).ok_or_else(|| Error::MissingParameter {
                operation: operation.id.clone(),
                parameter: name.to_owned(),
            })?;
            if operation.caller_path_parameters().contains(name) {
                if let Value::String(text) = value {
                    Slot::Path
                        .validate(text)
                        .map_err(|reason| Error::UnsafePathParameter {
                            operation: operation.id.clone(),
                            parameter: name.to_owned(),
                            reason,
                        })?;
                }
            }
        }

        let base = self.base(base_url)?;

        // The URL: `{base}` first, then this tenant's configuration, then the caller's parameters.
        // A filled value is never rescanned, so nothing a tenant configures can splice a caller's
        // parameter in — and a name nothing binds stays verbatim, which is what the
        // `UnresolvedEndpoint` guard below then refuses rather than sends.
        let mut refusal = None;
        let mut url = scan_template(&operation.request.url, |name| {
            if name == "base" {
                return Some(base.clone());
            }
            if let Some(value) = self.endpoints.get(name) {
                return Some(self.check(name, value).unwrap_or_else(|error| {
                    refusal.get_or_insert(error);
                    value.clone()
                }));
            }
            match operation.symbol(name) {
                Some(symbol) => params.get(symbol).map(text),
                None => None,
            }
        });
        if let Some(error) = refusal {
            return Err(error);
        }

        // The structured query: null omitted, everything encoded exactly once (C-30). A list or a
        // record has no wire spelling and is refused rather than flattened.
        let mut pairs = Vec::new();
        for entry in &operation.request.query {
            let rendered = match self.value(&entry.value, params)? {
                Value::Null => continue,
                Value::String(value) => value,
                Value::Number(value) => value.to_string(),
                Value::Bool(value) => value.to_string(),
                Value::Array(_) => {
                    return Err(self.unbuildable(format!(
                        "its structured query field {:?} is an array",
                        entry.name
                    )))
                }
                Value::Object(_) => {
                    return Err(self.unbuildable(format!(
                        "its structured query field {:?} is an object",
                        entry.name
                    )))
                }
            };
            pairs.push((entry.name.clone(), rendered));
        }
        append_query(&mut url, &pairs)
            .map_err(|message| self.unbuildable(format!("its structured query {message}")))?;

        let mut headers = BTreeMap::new();
        for (name, template) in &operation.request.headers {
            headers.insert(name.clone(), text(&self.value(template, params)?));
        }

        let body = match &operation.request.body {
            None => None,
            Some(BodyTemplate::Json { template }) => {
                let mut value = self.instantiate(template, params)?;
                // A free-form body supplied as text travels through `parse(…, as: "json")`: it is
                // validated and canonicalized rather than sent verbatim. A string that is not JSON
                // is refused here rather than sent.
                if operation.has_free_form_body() {
                    if let Value::String(text) = &value {
                        value = serde_json::from_str(text).map_err(|source| {
                            self.unbuildable(format!(
                                "its body was supplied as text that is not JSON: {source}"
                            ))
                        })?;
                    }
                }
                Some(value.to_string())
            }
            Some(BodyTemplate::Form { fields }) => {
                let mut pairs = Vec::new();
                for field in fields {
                    let value = self.value(&field.value, params)?;
                    if field.required || truthy(&value) {
                        pairs.push(format!("{}={}", field.name, text(&value)));
                    }
                }
                Some(pairs.join("&"))
            }
        };

        let mut request = Request {
            method: operation.request.method.clone(),
            url,
            headers,
            body,
        };

        // **The second lock on "total or refused".** Every configuration variable had a value, so
        // no literal can have carried one through — but a caller may have passed a *parameter*
        // whose text spells one, and interpolating that leaves the brace in the URL.
        if let Some(variable) = self
            .endpoints
            .keys()
            .find(|variable| request.url.contains(&format!("{{{variable}}}")))
        {
            return Err(Error::UnresolvedEndpoint {
                operation: operation.id.clone(),
                variable: variable.clone(),
                url: request.url,
            });
        }

        request.identify();
        Ok(request)
    }

    /// Instantiate a JSON body template: objects and arrays recurse, a `$param` splices the
    /// caller's value verbatim, and every other literal is itself.
    fn instantiate(&self, template: &Value, params: &Value) -> Result<Value, Error> {
        if let Some(name) = splice_of(template) {
            return Ok(self.param(params, name).clone());
        }
        Ok(match template {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(name, child)| Ok((name.clone(), self.instantiate(child, params)?)))
                    .collect::<Result<_, Error>>()?,
            ),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|child| self.instantiate(child, params))
                    .collect::<Result<_, Error>>()?,
            ),
            literal => literal.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document;
    use serde_json::json;

    fn endpoints(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    fn zendesk(id: &str) -> (&'static document::Operation, &'static str) {
        let provider = document::provider("zendesk").expect("the pack carries zendesk");
        let operation = provider.operation(id).expect("the document carries it");
        let base = provider
            .base_url(&operation.service)
            .expect("the service has a base URL");
        (operation, base)
    }

    /// One operation, every field, against the document it was derived from.
    #[test]
    fn the_request_is_the_documents_request() {
        let (operation, base) = zendesk("zendesk-ticket-update");
        let request = build_request(
            operation,
            base,
            &json!({"ticket_id": 42, "ticket": {"status": "open"}}),
            &endpoints(&[("subdomain", "acme")]),
        )
        .expect("it builds");

        assert_eq!(request.method, "PUT");
        assert_eq!(request.url, "https://acme.zendesk.com/api/v2/tickets/42");
        assert_eq!(
            request.headers.get("content-type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            request.body.as_deref(),
            Some(r#"{"ticket":{"status":"open"}}"#)
        );
    }

    #[test]
    fn an_omitted_parameter_is_refused_and_names_itself() {
        let (operation, base) = zendesk("zendesk-ticket-update");
        assert!(matches!(
            build_request(operation, base, &json!({"ticket_id": 42}), &endpoints(&[("subdomain", "acme")])),
            Err(Error::MissingParameter { parameter, .. }) if parameter == "ticket"
        ));
    }

    #[test]
    fn a_configuration_value_that_moves_the_authority_is_refused() {
        let (operation, base) = zendesk("zendesk-ticket-show");
        let refusal = build_request(
            operation,
            base,
            &json!({"ticket_id": 1}),
            &endpoints(&[("subdomain", "acme.zendesk.com@evil.example")]),
        )
        .expect_err("it refuses");
        assert!(matches!(refusal, Error::UnsafeConfig { position, .. } if position == "host"));
    }

    #[test]
    fn a_caller_parameter_that_leaves_its_path_segment_is_refused() {
        let (operation, base) = zendesk("zendesk-ticket-show");
        let refusal = build_request(
            operation,
            base,
            &json!({"ticket_id": "1/../2"}),
            &endpoints(&[("subdomain", "acme")]),
        )
        .expect_err("it refuses");
        assert!(
            matches!(refusal, Error::UnsafePathParameter { .. }),
            "{refusal}"
        );
    }

    /// **A caller value spelling a configuration variable never reaches the wire**, and the two
    /// locks are in the order that gives the better diagnostic.
    ///
    /// The path guard refuses it first, naming the *parameter*, because a brace is not a legal path
    /// segment character. [`Error::UnresolvedEndpoint`] is the second lock, for the residual case
    /// where such a value survives into a finished URL some other way — it is asserted directly
    /// below rather than through a caller, because no shipped operation can reach it.
    #[test]
    fn a_caller_value_spelling_a_configuration_variable_does_not_reach_the_wire() {
        let (operation, base) = zendesk("zendesk-ticket-show");
        let refusal = build_request(
            operation,
            base,
            &json!({"ticket_id": "{subdomain}"}),
            &endpoints(&[("subdomain", "acme")]),
        )
        .expect_err("it refuses");
        assert!(
            matches!(refusal, Error::UnsafePathParameter { ref parameter, .. } if parameter == "ticket_id"),
            "{refusal}"
        );

        // The second lock, stated on its own terms: a URL still naming a configured variable is a
        // request to a host nobody resolved, and is refused rather than sent.
        let url = "https://acme.zendesk.com/api/v2/tickets/{subdomain}";
        assert!(endpoints(&[("subdomain", "acme")])
            .keys()
            .any(|variable| url.contains(&format!("{{{variable}}}"))));
    }

    #[test]
    fn a_null_query_field_is_omitted_rather_than_sent_empty() {
        let (operation, base) = zendesk("zendesk-ticket-search");
        let with = build_request(
            operation,
            base,
            &json!({"query": "printer"}),
            &endpoints(&[("subdomain", "acme")]),
        )
        .expect("it builds");
        assert!(with.url.ends_with("/search?query=printer"), "{}", with.url);

        let without = build_request(
            operation,
            base,
            &json!({ "query": Value::Null }),
            &endpoints(&[("subdomain", "acme")]),
        )
        .expect("it builds");
        assert!(without.url.ends_with("/search"), "{}", without.url);
    }

    #[test]
    fn the_plan_carries_the_placed_credential_and_the_set_to_redact() {
        use catalog::Placement;

        let (operation, base) = zendesk("zendesk-ticket-show");
        let plan = resolve(
            operation,
            base,
            &json!({"ticket_id": 1}),
            &endpoints(&[("subdomain", "acme")]),
            &[Assembled::new(
                "zendesk.api_token",
                "SENTINEL".to_string(),
                Placement::Header {
                    name: "Authorization",
                    prefix: "Basic ",
                },
            )],
        )
        .expect("it resolves");

        assert_eq!(
            plan.request
                .headers
                .get("Authorization")
                .map(String::as_str),
            Some("Basic SENTINEL")
        );
        assert_eq!(
            plan.permission_subjects,
            vec!["https://acme.zendesk.com/api/v2/tickets/1".to_string()]
        );
        assert_eq!(
            plan.redactions
                .iter()
                .map(SensitiveText::expose_secret)
                .collect::<Vec<_>>(),
            vec!["SENTINEL"]
        );
    }

    /// The default identity is applied here, once, so a rehearsal and the wire cannot disagree.
    #[test]
    fn every_request_carries_this_softwares_identity() {
        let (operation, base) = zendesk("zendesk-ticket-show");
        let request = build_request(
            operation,
            base,
            &json!({"ticket_id": 1}),
            &endpoints(&[("subdomain", "acme")]),
        )
        .expect("it builds");
        assert_eq!(
            request.headers.get("User-Agent").map(String::as_str),
            Some(crate::DEFAULT_USER_AGENT)
        );
    }
}

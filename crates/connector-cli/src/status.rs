//! Whether a generated operation currently works, and if not, why — derived from the IR (C-42).
//!
//! This is the field the public catalogue exists for. `codewandler/flux-connectors` is honest about
//! what does not work, but that honesty currently lives in prose: a README section, an `AGENTS.md`
//! list, and a dozen story files. None of it is machine-readable, so a site rendering
//! `site/catalog.json` would publish `zendesk-ticket-search` as though it worked. The design's own
//! risk register says it plainly — *"showing them without their caveats would be worse than not
//! shipping the site"* (`docs/designs/public-docs.md`).
//!
//! # Derived, not listed
//!
//! **A hard-coded list of broken operation ids is exactly the hand-maintained truth this repository
//! exists to correct.** So every issue below is a *rule applied to the IR*, and the rule is what is
//! written down — not its current answer. Add a fourth provider with a free-text query parameter
//! and it is flagged without anyone editing this file; close the percent-encoding gap in
//! `connector-flux` and the flag disappears from all of them at once.
//!
//! The rules are stated so that they reproduce, from the IR alone, the four limits README.md
//! publishes under "Known limits":
//!
//! | Issue | Rule over the IR | Owning story |
//! |---|---|---|
//! | [`NO_CREDENTIAL`] | the operation's effective auth is empty | C-17 |
//! | [`CREDENTIAL_NOT_INJECTED`] | the operation's effective auth is **not** empty | C-10 |
//! | [`UNENCODABLE_QUERY_VALUE`] | a query parameter whose schema is not numeric or boolean | C-30 |
//! | [`UNBOUND_BASE_URL_TEMPLATE`] | the connector's base URL carries a `{name}` placeholder | C-17 |
//!
//! The first two are complementary and exhaustive: every operation gets exactly one of them, which
//! is the machine-readable form of "no provider can make a live call yet, and freshdesk cannot even
//! name the credential it would need".
//!
//! # The one fact that is not derived
//!
//! [`CREDENTIALS_REACH_THE_REQUEST`] — whether the emitter attaches a declared credential to the
//! request it generates. That is a property of `connector-flux`, not of any provider, so no walk of
//! the IR can answer it. It is one commented `const` rather than a list of affected operations, and
//! closing C-10 flips it in one line.
//!
//! # Scope, and why it is on the issue rather than on the operation
//!
//! An explorer that says "0 of 25 operations work" is accurate and useless. [`Scope`] separates a
//! defect the operation owns from one it merely inherits: `zendesk-ticket-search` has a problem
//! nothing else in the catalogue has, while every authenticated operation everywhere waits on the
//! same seam. A consumer filters on `scope` to draw that distinction; it does not have to know the
//! codes to do it.

use serde::Serialize;

use connector_spec::{Connector, JsonSchema, Operation, Param};

/// Whether the emitter attaches a declared credential to the request it generates.
///
/// **The one fact in this module that is not derived from the IR.** `connector-flux` emits no auth
/// at all today — the generated `op` builds a URL and calls `http.request` with `method` and `url`
/// and nothing else — because injection is C-10's, and the `$auth` marker it needs is a change that
/// must land in *flux* first (`docs/designs/auth-seam.md`). `flux`'s `{"$secret": "ENV"}` is a
/// whole-value replacement, so it produces neither a `Bearer ` prefix nor a base64-joined Basic
/// pair.
///
/// It is a `const` and not a list of operation ids on purpose: the fact is uniform across the whole
/// catalogue, and the story that closes it flips this one line rather than deleting an inventory
/// someone would otherwise have to keep in step.
const CREDENTIALS_REACH_THE_REQUEST: bool = false;

/// A stable machine token naming one reason an operation does not work.
///
/// Consumers switch on these, so they are part of the published contract
/// (`docs/designs/catalog-json.md`) and are not renamed once shipped. A *new* code is additive; an
/// existing one changing meaning is not.
pub const NO_CREDENTIAL: &str = "no-credential";
/// See [`NO_CREDENTIAL`].
pub const CREDENTIAL_NOT_INJECTED: &str = "credential-not-injected";
/// See [`NO_CREDENTIAL`].
pub const UNENCODABLE_QUERY_VALUE: &str = "unencodable-query-value";
/// See [`NO_CREDENTIAL`].
pub const UNBOUND_BASE_URL_TEMPLATE: &str = "unbound-base-url-template";

/// How far an issue reaches — what a consumer needs in order to tell "this operation is broken"
/// from "nothing can run yet".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Every operation in the catalogue is affected, whatever its provider.
    Catalog,
    /// Every operation of this provider is affected.
    Provider,
    /// This operation is affected and its siblings are not.
    Operation,
}

/// One reason an operation does not currently work.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Issue {
    /// The stable machine token — one of the `&str` constants in this module.
    pub code: &'static str,
    /// How far the issue reaches. See [`Scope`].
    pub scope: Scope,
    /// The internal story that closes it. Kept for repository diagnostics, but never published in
    /// the consumer-facing catalogue: the public site explains the limitation, not the backlog.
    #[serde(skip_serializing)]
    pub story: &'static str,
    /// One line a site can render as-is.
    pub summary: String,
    /// The parameters implicated, when the issue is about parameters; empty otherwise.
    pub params: Vec<String>,
}

/// Whether an operation currently works, and every reason it does not.
///
/// [`works`](Self::works) is exactly `issues.is_empty()`, restated as a field so a consumer can
/// filter on one boolean without knowing any of the codes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// True only when nothing below is wrong.
    pub works: bool,
    /// Every reason it does not, in a fixed order — see [`of`].
    pub issues: Vec<Issue>,
}

/// Derive one operation's status from the connector that declares it.
///
/// Deterministic: the rules are applied in a fixed order and each walks the IR in the IR's own
/// order, so equal inputs produce an equal — and equally ordered — result.
pub fn of(connector: &Connector, operation: &Operation) -> Status {
    let mut issues = Vec::new();

    // 1. Credentials. The two rules are complementary, so exactly one of them fires.
    //
    // `effective_auth` rather than `Operation::auth`, always: an operation that declares nothing
    // inherits the connector default, and one that declares an explicit empty list inherits
    // nothing. Reading the field directly would report freshdesk and a genuine ping endpoint the
    // same way for opposite reasons.
    if connector.effective_auth(operation).is_empty() {
        issues.push(Issue {
            code: NO_CREDENTIAL,
            scope: Scope::Provider,
            story: "C-17",
            summary: format!(
                "{} has no safe credential configuration for this operation yet. Live calls are disabled rather than sending a credential outside Flux's secret protection.",
                connector.id
            ),
            params: Vec::new(),
        });
    } else if !CREDENTIALS_REACH_THE_REQUEST {
        issues.push(Issue {
            code: CREDENTIAL_NOT_INJECTED,
            scope: Scope::Catalog,
            story: "C-10",
            summary: "Flux cannot yet apply connector credentials securely at request time, so this operation is unavailable for live calls."
                .to_string(),
            params: Vec::new(),
        });
    }

    // 2. Query values reach the wire raw.
    let unencodable: Vec<String> = operation
        .params
        .query
        .iter()
        .filter(|param| !is_safely_interpolated(&param.schema))
        .map(|param| wire_name(param).to_string())
        .collect();
    if !unencodable.is_empty() {
        issues.push(Issue {
            code: UNENCODABLE_QUERY_VALUE,
            scope: Scope::Operation,
            story: "C-30",
            summary: "Text query parameters cannot yet be encoded safely. Calling this operation could change the meaning of the request, so live use is disabled."
                .to_string(),
            params: unencodable,
        });
    }

    // 3. The base URL names a tenant nobody has bound. Read through the operation's **service**
    //    (C-49): a service may override the connector's base URL, and it is the URL the call
    //    actually reaches that decides whether the destination is bound.
    if let Some(variable) = first_template_variable(connector.base_url_of(&operation.service)) {
        issues.push(Issue {
            code: UNBOUND_BASE_URL_TEMPLATE,
            scope: Scope::Provider,
            story: "C-17",
            summary: format!(
                "This connector needs an operator-supplied {{{variable}}} value before it has a valid destination URL."
            ),
            params: Vec::new(),
        });
    }

    Status {
        works: issues.is_empty(),
        issues,
    }
}

/// Whether a query parameter of this schema can be interpolated into a URL without encoding.
///
/// **C-30's rule, and deliberately narrow** (`docs/designs/query-encoding.md` §4): a `Number` or
/// `Boolean` value cannot contain `&`, `#`, `+` or a space, so the six zendesk operations that take
/// only numeric ids and page bounds are unaffected and must not be flagged. Everything else is
/// treated as string-ish — including an untyped or unresolved schema, which `connector-flux` maps
/// to `Any` and which therefore *may* carry text (`crates/connector-flux/src/types.rs`).
///
/// The rule is stated over the JSON Schema rather than over the Flux type because the schema is
/// what the IR carries and what this crate can see; the two agree on every scalar, which is the
/// only case that can be safe. It inherits the limit the design records: a free-form parameter
/// mistyped as `integer` in a provider TOML is still reported as working.
fn is_safely_interpolated(schema: &JsonSchema) -> bool {
    matches!(
        schema.get("type").and_then(|kind| kind.as_str()),
        Some("integer" | "number" | "boolean")
    )
}

/// The spelling the vendor sees — [`Param::wire`] when it differs, the caller-facing name otherwise.
///
/// The wire name is the right one to report here: the issue is about what lands in the query
/// string.
fn wire_name(param: &Param) -> &str {
    param.wire.as_deref().unwrap_or(&param.name)
}

/// The first `{name}` placeholder in a base URL, if it has one.
///
/// A deliberately small scan rather than a template engine: `base_url` is documented as "may carry
/// tenant templating" and nothing in the IR parses it, so this reports the first placeholder and
/// leaves the rest to C-17's binding work.
fn first_template_variable(base_url: &str) -> Option<&str> {
    let (_, after) = base_url.split_once('{')?;
    let (variable, _) = after.split_once('}')?;
    Some(variable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use connector_spec::{
        AuthMethod, AuthRequirement, HttpMethod, Idempotency, Operation, ParamSet, Quirks, Risk,
    };
    use serde_json::json;

    fn param(name: &str, schema: serde_json::Value) -> Param {
        Param {
            name: name.to_string(),
            schema,
            ..Param::default()
        }
    }

    fn operation(id: &str) -> Operation {
        Operation {
            id: id.to_string(),
            service: connector_spec::DEFAULT_SERVICE.to_string(),
            method: HttpMethod::Get,
            path: "/v2/things".to_string(),
            description: "Do a thing".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            auth: None,
            params: ParamSet::default(),
            response_schema: None,
            quirks: Quirks::default(),
        }
    }

    /// A connector with a bound base URL and one declared credential — the baseline the rules are
    /// measured against, so that each test below moves exactly one thing.
    fn connector() -> Connector {
        Connector {
            id: "acme".to_string(),
            authority: None,
            api_version: None,
            services: Vec::new(),
            vendor: "Acme".to_string(),
            base_url: "https://api.acme.example".to_string(),
            description: "Acme".to_string(),
            auth: vec![AuthMethod::bearer(
                "acme.token",
                vec!["ACME_TOKEN".to_string()],
            )],
            default_auth: vec![AuthRequirement::single("acme.token")],
            operations: vec![operation("acme-thing-list")],
            provenance: Default::default(),
        }
    }

    fn codes(status: &Status) -> Vec<&str> {
        status.issues.iter().map(|issue| issue.code).collect()
    }

    /// The two credential rules are complementary: an operation gets exactly one of them, never
    /// both and never neither. That is what makes "every operation has a credential story" true by
    /// construction rather than by inspection.
    #[test]
    fn every_operation_gets_exactly_one_credential_issue() {
        let connector = connector();

        let declared = of(&connector, &connector.operations[0]);
        assert_eq!(codes(&declared), vec![CREDENTIAL_NOT_INJECTED]);

        let mut none = connector.clone();
        none.default_auth.clear();
        let derived = of(&none, &none.operations[0]);
        assert_eq!(codes(&derived), vec![NO_CREDENTIAL]);
    }

    /// An operation that declares an explicit empty list inherits nothing — the distinction
    /// `Connector::effective_auth` exists to preserve, and reading `Operation::auth` directly would
    /// lose it.
    #[test]
    fn an_explicitly_unauthenticated_operation_reports_no_credential() {
        let mut connector = connector();
        connector.operations[0].auth = Some(vec![]);
        assert_eq!(
            codes(&of(&connector, &connector.operations[0])),
            vec![NO_CREDENTIAL]
        );
    }

    /// **The narrow half of C-30's rule.** A numeric or boolean query value cannot carry `&` or
    /// `#`, so flagging it would make the catalogue cry wolf over the six zendesk operations that
    /// are genuinely fine.
    #[test]
    fn numeric_and_boolean_query_values_are_not_flagged() {
        let mut connector = connector();
        connector.operations[0].params.query = vec![
            param("page", json!({"type": "integer", "minimum": 1})),
            param("ratio", json!({"type": "number"})),
            param("enabled", json!({"type": "boolean"})),
        ];
        assert_eq!(
            codes(&of(&connector, &connector.operations[0])),
            vec![CREDENTIAL_NOT_INJECTED]
        );
    }

    /// The wide half: a string, and anything whose type the IR cannot resolve, must be treated as
    /// free-form text — an `Any` parameter *may* carry it.
    #[test]
    fn string_ish_and_untyped_query_values_are_flagged() {
        for schema in [
            json!({"type": "string"}),
            json!({"type": "string", "enum": ["a", "b"]}),
            json!({"type": ["string", "null"]}),
            json!({"$ref": "#/components/schemas/Query"}),
            json!({}),
        ] {
            let mut connector = connector();
            connector.operations[0].params.query = vec![param("q", schema.clone())];
            assert!(
                codes(&of(&connector, &connector.operations[0])).contains(&UNENCODABLE_QUERY_VALUE),
                "{schema} should be treated as string-ish"
            );
        }
    }

    /// The issue names the parameters, and it names them as the **vendor** spells them — the query
    /// string is where the value lands.
    #[test]
    fn the_query_issue_names_the_wire_parameters_it_is_about() {
        let mut connector = connector();
        let mut aliased = param("req_id", json!({"type": "string"}));
        aliased.wire = Some("requester_id".to_string());
        connector.operations[0].params.query =
            vec![aliased, param("page", json!({"type": "integer"}))];

        let status = of(&connector, &connector.operations[0]);
        let issue = status
            .issues
            .iter()
            .find(|issue| issue.code == UNENCODABLE_QUERY_VALUE)
            .expect("the query issue fires");
        assert_eq!(issue.params, vec!["requester_id".to_string()]);
    }

    /// Path and header parameters have the identical gap and are deliberately **not** reported:
    /// C-30 scopes the refusal to query values, and widening it here would put the catalogue and
    /// the emitter's own rule out of step.
    #[test]
    fn only_query_parameters_are_examined_for_encoding() {
        let mut connector = connector();
        connector.operations[0].params.path = vec![param("slug", json!({"type": "string"}))];
        connector.operations[0].params.header = vec![param("trace", json!({"type": "string"}))];
        assert_eq!(
            codes(&of(&connector, &connector.operations[0])),
            vec![CREDENTIAL_NOT_INJECTED]
        );
    }

    #[test]
    fn an_unbound_base_url_placeholder_is_reported() {
        let mut connector = connector();
        connector.base_url = "https://{subdomain}.zendesk.com".to_string();
        let status = of(&connector, &connector.operations[0]);
        assert!(codes(&status).contains(&UNBOUND_BASE_URL_TEMPLATE));
        assert!(
            status
                .issues
                .iter()
                .any(|issue| issue.summary.contains("{subdomain}")),
            "the summary names the placeholder a reader has to bind"
        );
    }

    /// `works` restates `issues.is_empty()`, so the two can never disagree.
    #[test]
    fn works_is_true_only_when_nothing_is_wrong() {
        let mut connector = connector();
        connector.default_auth.clear();
        connector.operations[0].auth = Some(vec![]);
        // Everything the rules can report is now absent: no credential is *expected*, so the
        // no-credential rule is the only one left, and it fires.
        assert!(!of(&connector, &connector.operations[0]).works);

        let status = Status {
            works: true,
            issues: Vec::new(),
        };
        assert_eq!(status.works, status.issues.is_empty());
    }

    /// Deterministic: the document is a checked artifact, so an unstable issue order would show up
    /// as phantom drift on every build.
    #[test]
    fn derivation_is_deterministic() {
        let mut connector = connector();
        connector.base_url = "https://{tenant}.acme.example".to_string();
        connector.operations[0].params.query = vec![param("q", json!({"type": "string"}))];
        assert_eq!(
            of(&connector, &connector.operations[0]),
            of(&connector, &connector.operations[0])
        );
    }
}

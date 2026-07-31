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
//! | [`NO_CREDENTIAL`] | the operation's effective auth is empty, and it did not *declare* it so | C-17 |
//! | [`CREDENTIAL_NOT_INJECTED`] | the operation's effective auth is **not** empty | C-10 |
//! | [`UNENCODABLE_QUERY_VALUE`] | a query parameter whose schema is not numeric or boolean | C-30 |
//! | [`UNBOUND_BASE_URL_TEMPLATE`] | the connector's base URL carries a `{name}` placeholder | C-17 |
//!
//! The first two are complementary over every operation that has *not* declared itself public: one
//! of them fires, which is the machine-readable form of "no provider can make a live call yet, and
//! freshdesk cannot even name the credential it would need".
//!
//! # Withheld is not the same as unnecessary (C-206)
//!
//! An empty effective auth has **two opposite meanings**, and publishing one sentence for both is
//! what this module used to do:
//!
//! - **withheld** — a credential exists and this repository cannot hold it safely yet. Freshdesk is
//!   the case: its API key occupies the Basic *username* position, which the IR cannot mark secret,
//!   so `providers/freshdesk.toml` deliberately declares none and every request 401s. That is the
//!   fail-closed outcome working, and [`NO_CREDENTIAL`] is its name.
//! - **unnecessary** — the vendor requires nothing. Nothing is withheld, no credential exists to
//!   withhold, and the unauthenticated call is the correct working call. [`NO_CREDENTIAL_REQUIRED`]
//!   is its name, and it is a [`Note`] rather than an [`Issue`] because there is nothing wrong.
//!
//! **The difference is declared, never inferred.** [`Operation::auth`] already carries it and says
//! so: `None` inherits [`Connector::default_auth`], while `Some(vec![])` is OpenAPI's *explicitly
//! none* — "a health or ping endpoint", in the IR's own words. So an author writes `auth = []` on
//! the operation to say the vendor needs nothing, and writes nothing at all when a credential is
//! merely missing. Guessing "public" from the absence of a credential field would reproduce exactly
//! the conflation this rule exists to end.
//!
//! One consequence is deliberate and visible: a public operation with a bound base URL and no
//! free-text query parameter is the **first operation in this catalogue to report `works: true`**,
//! because nothing is in fact standing in its way — see `docs/designs/catalog-json.md`.
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

/// A stable machine token naming one condition of an operation — for the four below, one reason it
/// does not work.
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

/// The one token that names something **right** with an operation: its vendor requires no
/// credential, so nothing is missing and nothing is withheld (C-206).
///
/// It is a [`Note`] code rather than an [`Issue`] code, and that is the whole point. Spelling it as
/// a fifth issue would have kept `works: false` on an operation that works, which is the same lie
/// [`NO_CREDENTIAL`] was telling — one code further along.
///
/// [`NO_CREDENTIAL`] is untouched and keeps its exact shipped sense: a credential exists and this
/// repository cannot hold it safely yet. Every operation carrying it before this code existed
/// carries it still.
pub const NO_CREDENTIAL_REQUIRED: &str = "no-credential-required";

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

/// One thing worth publishing about an operation that is **not** a reason it fails.
///
/// The sibling of [`Issue`], and deliberately a separate type rather than a flag on that one. The
/// contract `works == issues.is_empty()` is what lets a consumer filter on one boolean without
/// knowing a single code, and folding a non-defect into `issues` would have broken it for every
/// consumer already reading the field — an operation would have carried a listed reason and claimed
/// to work at the same time.
///
/// It has no `story`, because a note is not something anyone is going to close, and no `params`,
/// because nothing here is about parameters. Both would be permanently empty fields inviting a
/// consumer to look for meaning in them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    /// The stable machine token — see [`NO_CREDENTIAL_REQUIRED`].
    pub code: &'static str,
    /// How far the fact reaches. See [`Scope`].
    pub scope: Scope,
    /// One line a site can render as-is.
    pub summary: String,
}

/// Whether an operation currently works, every reason it does not, and what else a consumer needs.
///
/// [`works`](Self::works) is exactly `issues.is_empty()`, restated as a field so a consumer can
/// filter on one boolean without knowing any of the codes. [`notes`](Self::notes) does not enter
/// into it: a note is never a reason to fail, which is what makes it a note.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// True only when nothing in [`issues`](Self::issues) is wrong.
    pub works: bool,
    /// Every reason it does not, in a fixed order — see [`of`].
    pub issues: Vec<Issue>,
    /// Facts about the operation that are not defects, in the same fixed order — see [`Note`].
    ///
    /// **Always encoded, `[]` when there are none.** No `skip_serializing_if`, deliberately: the
    /// published document promises that every key is present so a consumer types it once and never
    /// tests for existence (`crate::site`, `docs/designs/catalog-json.md`). Omitting this one to
    /// spare the committed whole-catalogue artifact a rewrite would have bought that saving with a
    /// permanent hole in the promise — and the artifact is regenerated at integration anyway, which
    /// is what `AGENTS.md` means by a story leaving whole-catalogue staleness checks red.
    pub notes: Vec<Note>,
}

/// Derive one operation's status from the connector that declares it.
///
/// Deterministic: the rules are applied in a fixed order and each walks the IR in the IR's own
/// order, so equal inputs produce an equal — and equally ordered — result.
pub fn of(connector: &Connector, operation: &Operation) -> Status {
    let mut issues = Vec::new();
    let mut notes = Vec::new();

    // 1. Credentials. Three states over one axis, so exactly one of the arms fires.
    //
    // Both halves of `Operation::auth` are read, and that is the correction C-206 made: the
    // *resolved* list says whether a credential applies, and the *declaration* says whether its
    // absence was chosen. `effective_auth` alone answers only the first, so freshdesk — a credential
    // withheld because the IR cannot hold it safely — and a genuine ping endpoint came out
    // identical, and the endpoint was published as disabled for the reader's protection.
    //
    // Declared, never inferred. Treating "no credential field anywhere" as evidence of a public
    // endpoint would put the guess back with more steps, and it would guess wrong on the one
    // connector already shipping in that shape.
    if declares_no_credential_is_needed(operation) {
        notes.push(Note {
            code: NO_CREDENTIAL_REQUIRED,
            scope: Scope::Operation,
            summary: format!(
                "{} requires no credential for this operation, and none is withheld: the unauthenticated call is the correct one.",
                connector.id
            ),
        });
    } else if connector.effective_auth(operation).is_empty() {
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
    //
    //    Every variable, not the first one. This used to call a `first_template_variable` whose name
    //    was an accurate description of its bug: `https://{region}.{tenant}.example` reported one and
    //    left the other invisible to every consumer of this status. The loader now refuses a variable
    //    no `[[config]]` field binds (C-86), so a provider reaching this point has declared what it
    //    needs — and the issue's job is to say the values are not *supplied* yet, naming all of them.
    let unbound =
        connector_spec::config::template_variables(connector.base_url_of(&operation.service));
    if !unbound.is_empty() {
        let named = unbound
            .iter()
            .map(|variable| format!("{{{variable}}}"))
            .collect::<Vec<_>>()
            .join(", ");
        issues.push(Issue {
            code: UNBOUND_BASE_URL_TEMPLATE,
            scope: Scope::Provider,
            story: "C-17",
            summary: format!(
                "This connector needs an operator-supplied {named} value before it has a valid destination URL."
            ),
            params: unbound.iter().map(|v| (*v).to_string()).collect(),
        });
    }

    Status {
        works: issues.is_empty(),
        issues,
        notes,
    }
}

/// Whether the operation **declares** that its vendor requires no credential.
///
/// The positive declaration C-206 needs, and it needed no new IR field: [`Operation::auth`] already
/// separates the two empties and documents them as OpenAPI does — `None` is *unset* and inherits
/// [`Connector::default_auth`], `Some(vec![])` is *explicitly none*, "a health or ping endpoint".
/// Only the second is a statement about the vendor; the first is a statement about nothing.
///
/// # The limit this leaves, and why it is left
///
/// It is per **operation**. A connector whose whole vendor is public writes `auth = []` on each of
/// its operations, because `Connector::default_auth` is a plain `Vec` — `default_auth = []` and an
/// omitted key deserialize to the same value, so a connector cannot make this declaration once.
/// Closing that means an `Option` on the connector field, which is `connector-spec`'s to change and
/// touches the loader, the lockfile encoding and every provider file. Repetition that is visible in
/// a diff is the better failure while it stands.
fn declares_no_credential_is_needed(operation: &Operation) -> bool {
    matches!(operation.auth.as_deref(), Some([]))
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
            idempotent_because: None,
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
            events: Vec::new(),
            channels: Vec::new(),
            config: Vec::new(),
            verify: None,
            graphs: Vec::new(),
            provenance: Default::default(),
        }
    }

    fn codes(status: &Status) -> Vec<&str> {
        status.issues.iter().map(|issue| issue.code).collect()
    }

    fn note_codes(status: &Status) -> Vec<&str> {
        status.notes.iter().map(|note| note.code).collect()
    }

    /// The two credential rules are complementary over every operation that has not declared itself
    /// public: it gets exactly one of them, never both and never neither. That is what makes "every
    /// operation has a credential story" true by construction rather than by inspection.
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
    /// `Connector::effective_auth` exists to preserve — **and it is a statement about the vendor**,
    /// so it is published as a note and not as a defect (C-206).
    ///
    /// Note what does *not* change: the connector still declares `acme.token` and still defaults to
    /// it. The declaration overrides that for this one operation, which is exactly what a ping
    /// endpoint on an otherwise authenticated API is.
    #[test]
    fn an_explicitly_unauthenticated_operation_is_a_note_and_not_a_defect() {
        let mut connector = connector();
        connector.operations[0].auth = Some(vec![]);
        let status = of(&connector, &connector.operations[0]);

        assert_eq!(codes(&status), Vec::<&str>::new());
        assert_eq!(note_codes(&status), vec![NO_CREDENTIAL_REQUIRED]);
        assert!(status.works);
        assert!(
            status.notes[0].summary.contains(&connector.id),
            "the note names the connector it is about, as the issue beside it does"
        );
    }

    /// **The declaration is read off the operation, never inferred from an absence.**
    ///
    /// The bug C-206 closed was a rule that could not see the difference; a rule that *guessed* it
    /// would be the same bug with more steps, and it would guess wrong on the one connector already
    /// shipping with no credential declared for a reason that is not publicness.
    #[test]
    fn a_missing_credential_is_never_read_as_a_public_endpoint() {
        let mut connector = connector();
        connector.auth.clear();
        connector.default_auth.clear();

        // Nothing at all is declared — no method, no default, no operation-level list.
        assert_eq!(connector.operations[0].auth, None);
        let status = of(&connector, &connector.operations[0]);
        assert_eq!(codes(&status), vec![NO_CREDENTIAL]);
        assert_eq!(note_codes(&status), Vec::<&str>::new());
        assert!(!status.works);
    }

    /// **C-206.** Two operations whose effective auth is empty for *opposite* reasons must not be
    /// published the same way.
    ///
    /// Freshdesk's emptiness is a credential deliberately **withheld**: a real API key exists, the
    /// IR cannot yet mark it secret in the Basic username position, and the resulting 401 is the
    /// correct fail-closed outcome. A ping endpoint's emptiness is a vendor that requires nothing at
    /// all. Reading only `effective_auth(op).is_empty()` reports both with freshdesk's wording,
    /// which tells a consumer that a working public endpoint is disabled for their protection.
    ///
    /// Asserted over the **published** encoding rather than over the Rust value: "published
    /// differently" is the property the story is about, and the catalogue is what a consumer reads.
    #[test]
    fn a_public_operation_is_not_published_as_a_withheld_credential() {
        // Freshdesk's shape: the connector declares no credential and no default, so the operation
        // inherits emptiness. Nothing here says a credential is unnecessary.
        let mut withheld = connector();
        withheld.auth.clear();
        withheld.default_auth.clear();
        let withheld_status = of(&withheld, &withheld.operations[0]);

        // A vendor that requires none, as a positive declaration on the operation itself.
        let mut public = withheld.clone();
        public.operations[0].auth = Some(vec![]);
        let public_status = of(&public, &public.operations[0]);

        let published =
            |status: &Status| serde_json::to_value(status).expect("a status serializes");
        assert_ne!(
            published(&withheld_status),
            published(&public_status),
            "a withheld credential and a vendor that needs none are published identically"
        );

        // The withheld half keeps freshdesk's code exactly — it is a published contract token.
        assert_eq!(codes(&withheld_status), vec![NO_CREDENTIAL]);
        assert!(!withheld_status.works);

        // The public half is told apart by a code of its own, and nothing stands in its way.
        assert!(!codes(&public_status).contains(&NO_CREDENTIAL));
        assert_eq!(
            published(&public_status)["notes"][0]["code"],
            json!("no-credential-required"),
            "the public declaration is published under a code of its own"
        );
        assert!(
            public_status.works,
            "a genuinely public operation has nothing left for anyone to close"
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

    /// `works` restates `issues.is_empty()`, so the two can never disagree — **and a note does not
    /// enter into it**, which is the property that let C-206 add a fifth code without breaking
    /// every consumer already filtering on the boolean.
    #[test]
    fn works_is_true_only_when_nothing_is_wrong_and_a_note_is_not_wrong() {
        let mut connector = connector();
        connector.default_auth.clear();
        // A credential is missing and nothing says it is unnecessary, so the withheld rule fires.
        assert!(!of(&connector, &connector.operations[0]).works);

        // Declaring that the vendor needs none removes the last thing in the way. This is the first
        // operation in the catalogue that can report `works: true`, and it carries a note while
        // doing so.
        connector.operations[0].auth = Some(vec![]);
        let public = of(&connector, &connector.operations[0]);
        assert!(public.works);
        assert!(public.issues.is_empty());
        assert!(!public.notes.is_empty());

        let status = Status {
            works: true,
            issues: Vec::new(),
            notes: Vec::new(),
        };
        assert_eq!(status.works, status.issues.is_empty());
    }

    /// A note is a fact about the operation, not a licence to stop reporting defects.
    ///
    /// A public endpoint with a free-text query parameter and an unbound host is still broken in
    /// both of those ways, and `works` says so.
    #[test]
    fn a_public_operation_still_reports_every_defect_it_has() {
        let mut connector = connector();
        connector.base_url = "https://{tenant}.acme.example".to_string();
        connector.operations[0].auth = Some(vec![]);
        connector.operations[0].params.query = vec![param("q", json!({"type": "string"}))];

        let status = of(&connector, &connector.operations[0]);
        assert_eq!(
            codes(&status),
            vec![UNENCODABLE_QUERY_VALUE, UNBOUND_BASE_URL_TEMPLATE]
        );
        assert_eq!(note_codes(&status), vec![NO_CREDENTIAL_REQUIRED]);
        assert!(!status.works);
    }

    /// **Every key is always present**, `notes` included — an operation with none encodes `[]` and
    /// never drops the key.
    ///
    /// The published document's oldest promise (`crate::site`, `docs/designs/catalog-json.md`): a
    /// consumer types it once and never tests for existence. An omitted-when-empty `notes` would
    /// have spared the committed catalogue a rewrite and made that promise false for all 242 shipped
    /// operations, every one of which has no note — so the exception would have been invisible in
    /// exactly the case a consumer meets first.
    #[test]
    fn a_status_with_no_note_still_carries_the_key() {
        let connector = connector();
        let encoded = serde_json::to_value(of(&connector, &connector.operations[0]))
            .expect("a status serializes");
        assert_eq!(encoded["notes"], json!([]));
        assert_eq!(
            encoded
                .as_object()
                .expect("an object")
                .keys()
                .collect::<Vec<_>>(),
            // Key-sorted, not declaration-ordered: `serde_json::Value` is `BTreeMap`-backed unless
            // `preserve_order` is on, which is the same property `crate::site` relies on to make
            // the whole document deterministic.
            vec!["issues", "notes", "works"],
            "the published status shape changed: {encoded}"
        );
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

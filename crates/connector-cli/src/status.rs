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
//! written down — not its current answer. C-30 is the example of the lifecycle working: structured
//! query encoding closed the gap in `connector-flux`, so this module no longer publishes the
//! operation-level issue it used to derive for text queries.
//!
//! The rules are stated so that they reproduce, from the IR alone, the remaining limits README.md
//! publishes under "Known limits":
//!
//! | Issue | Rule over the IR | Owning story |
//! |---|---|---|
//! | [`NO_CREDENTIAL`] | the operation's effective auth is empty, and it did not *declare* it so | C-17 |
//! | [`CREDENTIAL_NOT_INJECTED`] | the operation's effective auth is **not** empty | C-534 |
//! | [`UNBOUND_BASE_URL_TEMPLATE`] | the operation's base URL carries a `{name}` placeholder whose declared configuration field has no default | C-17 |
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
//! the program that replaces the emitted module with the catalog artifact (C-534) flips it in one
//! line.
//!
//! # Scope, and why it is on the issue rather than on the operation
//!
//! An explorer that says "0 of 25 operations work" is accurate and useless. [`Scope`] separates a
//! defect the operation owns from one it merely inherits: `zendesk-ticket-search` has a problem
//! nothing else in the catalogue has, while every authenticated operation everywhere waits on the
//! same seam. A consumer filters on `scope` to draw that distinction; it does not have to know the
//! codes to do it.

use serde::Serialize;

use connector_spec::{Binding, Connector, Operation};

/// Whether the emitter attaches a declared credential to the request it generates.
///
/// **The one fact in this module that is not derived from the IR.** `connector-flux` emits no auth
/// at all today — the generated `op` builds a URL and calls `http.request` with `method` and `url`
/// and nothing else.
///
/// Injection was C-10's, and C-535 closed C-10 as superseded-never-implemented: Decision 0022 rules
/// that flux never grows a connector module loader, so a module naming a credential for flux to
/// resolve has no consumer and the `$auth` marker is not coming
/// (`docs/designs/auth-seam.md` records the road not taken). Auth assembly landed in Rust instead —
/// the `Bearer ` prefix, the base64-joined Basic pair and query placement are `connector-pack`'s
/// (C-114/C-115/C-116) — which is what a *host* uses; the emitted module stays credential-free, and
/// the compiled form a resolver reads becomes the catalog artifact under C-534's program.
///
/// It is a `const` and not a list of operation ids on purpose: the fact is uniform across the whole
/// catalogue, and the program that closes it flips this one line rather than deleting an inventory
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
/// The retired C-30 token. Kept as a public spelling for consumers that still deserialize older
/// catalogue documents; current catalogues never produce it because scalar queries are structured
/// and unmodelled collections fail before an operation can publish.
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

/// **Which of three states an operation's credential requirement is in** — the classification both
/// catalogue backends read (C-235).
///
/// It exists because the answer was being derived twice from the same two facts and published in
/// two shapes: this module turned it into an [`Issue`] or a [`Note`] for `catalog.json`, and
/// [`crate::catalog`] flattened it into a mechanism list for the embedded table — where the two
/// empty cases became one empty slice and the distinction stopped at the crate boundary.
///
/// Deriving it once is what makes the two surfaces unable to disagree. [`code`](Self::code) is the
/// link to the published vocabulary: it returns the constants above rather than restating them, so
/// a fourth state cannot be invented here with a fifth spelling.
///
/// `catalog::CredentialRequirement` mirrors this enum for the embedded table, exactly as
/// `catalog::Risk` mirrors [`connector_spec::Risk`] — the `catalog` crate has no dependencies, and
/// the mapping in [`crate::catalog`] is exhaustive, so a state added here is a compile error there
/// rather than a silent omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialRequirement {
    /// The operation authenticates: its effective auth names at least one mechanism.
    Declared,
    /// The vendor requires none, **declared positively** by the operation (`auth = []`).
    NoneRequired,
    /// A credential is withheld: nothing is declared, and nothing declares that nothing is needed.
    Withheld,
}

impl CredentialRequirement {
    /// The published code this state carries, or `None` for the ordinary case.
    ///
    /// [`Declared`](Self::Declared) has none because there is nothing to report about it — the
    /// operation names its credentials, and that is the whole statement. What it does carry is
    /// [`CREDENTIAL_NOT_INJECTED`], which is about the *emitter* rather than about the operation's
    /// requirement, and which disappears for every operation at once when C-534's program lands.
    pub const fn code(self) -> Option<&'static str> {
        match self {
            CredentialRequirement::Declared => None,
            CredentialRequirement::NoneRequired => Some(NO_CREDENTIAL_REQUIRED),
            CredentialRequirement::Withheld => Some(NO_CREDENTIAL),
        }
    }
}

/// Classify one operation's credential requirement — **the one derivation** (C-235).
///
/// Both halves of [`Operation::auth`] are read, and that is C-206's correction: the *resolved* list
/// says whether a credential applies, and the *declaration* says whether its absence was chosen.
/// [`Connector::effective_auth`] alone answers only the first, so freshdesk — a credential withheld
/// because the IR cannot hold it safely — and a genuine ping endpoint came out identical.
///
/// **Declared, never inferred.** Treating "no credential field anywhere" as evidence of a public
/// endpoint would put the guess back with more steps, and it would guess wrong on the one connector
/// already shipping in that shape.
pub fn credential_requirement(
    connector: &Connector,
    operation: &Operation,
) -> CredentialRequirement {
    if declares_no_credential_is_needed(operation) {
        CredentialRequirement::NoneRequired
    } else if connector.effective_auth(operation).is_empty() {
        CredentialRequirement::Withheld
    } else {
        CredentialRequirement::Declared
    }
}

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

    // 1. Credentials. Three states over one axis, so exactly one of the arms fires — and the
    //    classification is [`credential_requirement`]'s rather than this function's, because the
    //    embedded catalogue derives the same three states and two derivations of one fact is how
    //    two surfaces come to disagree (C-235).
    match credential_requirement(connector, operation) {
        CredentialRequirement::NoneRequired => notes.push(Note {
            code: NO_CREDENTIAL_REQUIRED,
            scope: Scope::Operation,
            summary: format!(
                "{} requires no credential for this operation, and none is withheld: the unauthenticated call is the correct one.",
                connector.id
            ),
        }),
        CredentialRequirement::Withheld => issues.push(Issue {
            code: NO_CREDENTIAL,
            scope: Scope::Provider,
            story: "C-17",
            summary: format!(
                "{} has no safe credential configuration for this operation yet. Live calls are disabled rather than sending a credential outside Flux's secret protection.",
                connector.id
            ),
            params: Vec::new(),
        }),
        CredentialRequirement::Declared => {
            if !CREDENTIALS_REACH_THE_REQUEST {
                issues.push(Issue {
                    code: CREDENTIAL_NOT_INJECTED,
                    scope: Scope::Catalog,
                    // C-534's program, not C-10 (C-545). C-535 closed C-10 as superseded, so this
                    // field — structured data a consumer may branch on — was naming work that is
                    // never coming. It is the program rather than one of its children because no
                    // child *delivers* credentialed requests: that arrived by another route
                    // (`connector-pack`, C-114/C-115/C-116), and what closes this issue is the
                    // program replacing the credential-free emitted module as the compiled form.
                    story: "C-534",
                    summary: "Flux cannot yet apply connector credentials securely at request time, so this operation is unavailable for live calls."
                        .to_string(),
                    params: Vec::new(),
                });
            }
        }
    }

    // 2. The base URL names a tenant nobody has bound. Read through the operation's **service**
    //    (C-49): a service may override the connector's base URL, and it is the URL the call
    //    actually reaches that decides whether the destination is bound.
    //
    //    Every variable, not the first one. This used to call a `first_template_variable` whose name
    //    was an accurate description of its bug: `https://{region}.{tenant}.example` reported one and
    //    left the other invisible to every consumer of this status. The loader now refuses a variable
    //    no `[[config]]` field binds (C-86), so a provider reaching this point has declared what it
    //    needs — and the issue's job is to say the values are not *supplied* yet, naming all of them.
    let unbound =
        connector_spec::config::template_variables(connector.base_url_of(&operation.service))
            .into_iter()
            .filter(|variable| {
                !connector.config.iter().any(|field| {
                    field.service == operation.service
                        && field.default.is_some()
                        && matches!(
                            field.binding(),
                            Some(Binding::Endpoint { variable: bound }) if bound == *variable
                        )
                })
            })
            .collect::<Vec<_>>();
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

#[cfg(test)]
mod tests {
    use super::*;
    use connector_spec::{
        AuthMethod, AuthRequirement, HttpMethod, Idempotency, Operation, Param, ParamSet, Quirks,
        Risk,
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
            direction: connector_spec::OperationDirection::Read,
            path: "/v2/things".to_string(),
            description: "Do a thing".to_string(),
            risk: Risk::Low,
            idempotency: Idempotency::Idempotent,
            semantic_effects: Vec::new(),
            repeatable_because: None,
            expose: true,
            auth: None,
            params: ParamSet::default(),
            response_schema: None,
            credential_response: Vec::new(),
            produces_credential: None,
            quirks: Quirks::default(),
        }
    }

    /// A connector with a bound base URL and one declared credential — the baseline the rules are
    /// measured against, so that each test below moves exactly one thing.
    fn connector() -> Connector {
        Connector {
            id: "acme".to_string(),
            authority: None,
            runtime: connector_spec::Runtime::Http,
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

    fn stories(status: &Status) -> Vec<&str> {
        status.issues.iter().map(|issue| issue.story).collect()
    }

    /// **No issue points a consumer at a story that is never coming** (C-545).
    ///
    /// [`Issue::story`] is not published, but it is still structured data a repository consumer may
    /// branch on — which is why this is a pinned assertion rather than the prose correction C-542
    /// made to the emitted manifest header. C-535 closed C-10 as superseded-never-implemented:
    /// Decision 0022 states that flux never grows a connector module loader, so the `$auth` marker
    /// the old pointer promised has no consumer and no story is going to deliver it. What replaces
    /// it is C-534's program, exactly as the manifest header now says.
    ///
    /// The sweep is over every issue this module can derive, not only the credential one, so a
    /// future rule cannot reintroduce the pointer in a branch nobody thought to check.
    #[test]
    fn no_issue_names_a_superseded_story() {
        let declared = connector();
        assert_eq!(
            codes(&of(&declared, &declared.operations[0])),
            vec![CREDENTIAL_NOT_INJECTED],
            "the fixture must still be the shape that derives the credential-injection issue"
        );
        assert_eq!(
            stories(&of(&declared, &declared.operations[0])),
            vec!["C-534"],
            "the credential-injection issue must name the program that replaces the emitted \
             module, not C-10 — closed as superseded by C-535"
        );

        // Every other issue-producing shape: a withheld credential, an unbound base URL, and both
        // at once. None of them may name C-10 either.
        let mut withheld = connector();
        withheld.auth.clear();
        withheld.default_auth.clear();
        let mut unbound = connector();
        unbound.base_url = "https://{tenant}.acme.example".to_string();
        let mut both = withheld.clone();
        both.base_url = "https://{tenant}.acme.example".to_string();

        for connector in [&declared, &withheld, &unbound, &both] {
            let status = of(connector, &connector.operations[0]);
            assert!(
                !stories(&status).contains(&"C-10"),
                "a route still serves the superseded C-10: {:?}",
                status.issues
            );
        }
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

    /// C-30 moved scalar queries into `http.request(query: ...)`. Status must therefore stop
    /// repeating the old URL-interpolation refusal for every schema shape the emitter admits.
    #[test]
    fn scalar_query_values_are_not_flagged_after_c30() {
        let mut connector = connector();
        connector.operations[0].params.query = vec![
            param("query", json!({"type": "string"})),
            param("page", json!({"type": "integer", "minimum": 1})),
            param("ratio", json!({"type": "number"})),
            param("enabled", json!({"type": "boolean"})),
        ];
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
    /// A public endpoint with an unbound host is still broken, and `works` says so.
    #[test]
    fn a_public_operation_still_reports_every_defect_it_has() {
        let mut connector = connector();
        connector.base_url = "https://{tenant}.acme.example".to_string();
        connector.operations[0].auth = Some(vec![]);
        connector.operations[0].params.query = vec![param("q", json!({"type": "string"}))];

        let status = of(&connector, &connector.operations[0]);
        assert_eq!(codes(&status), vec![UNBOUND_BASE_URL_TEMPLATE]);
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

//! The Google Workspace connector, and the property that makes it different from every other
//! shipped provider: **it is the first one that declares services** (C-69, C-49).
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical, and `connector-cli`'s `service_units.rs` asserts that a multi-service provider
//! emits one module and one manifest per service. This file adds the claims that are specific to
//! *this* provider, because they are the reason C-69 exists:
//!
//! - **Three services, three API versions.** `gmail:v1`, `calendar:v3` and `drive:v3` version
//!   independently, which is the concrete case that forced `api_version` onto the service rather than
//!   the connector. A connector-level version would have to state one of the three and be wrong about
//!   the other two.
//! - **Every operation names its service explicitly.** A multi-service provider has no implicit
//!   `default` to fall into, so the loader refuses an operation that omits it; the assertion here is
//!   the positive half — every declared service actually owns operations, so none of the three is a
//!   declaration with nothing behind it.
//! - **A service's host is its own.** Gmail is documented on `gmail.googleapis.com` while Calendar and
//!   Drive are on `www.googleapis.com`, so the emitted `$base` of an operation has to be *its
//!   service's* base URL. Binding the provider's would send every Gmail call to the wrong host while
//!   the manifest claimed the right one — the two artifacts installing together would disagree.
//!
//! The two negative claims — no query parameter, no optional body field — deliberately duplicate what
//! `asana_connector.rs`, `github_connector.rs` and `slack_connector.rs` assert for their own
//! providers. Both are gaps in the emitter rather than in Google (C-30 and C-56), so each connector
//! has to hold the line for itself: a shared test would be a line four concurrent stories edit, and
//! the derived per-provider gates cannot express "and this provider chose its operations around that
//! gap".

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency, Risk};

/// The provider id, and therefore the file stem, the catalogue module and every op id's prefix.
const PROVIDER: &str = "google";

/// The connector-level base URL: the host Calendar and Drive are documented on, and the default a
/// service inherits when it declares no override of its own.
const BASE_URL: &str = "https://www.googleapis.com";

/// The credential the connector declares, and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the file happens to say.
const CREDENTIAL: &str = "google.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "GOOGLE_ACCESS_TOKEN";

/// The three services, each with the API version it is addressed at, the host its calls reach, and
/// the operations it owns — in the order `providers/google.toml` declares them.
///
/// One table rather than three constants, because the point of C-69 is that these three facts vary
/// *per service*: a reader comparing the rows sees the reason `api_version` and `base_url` belong to
/// the service rather than to the connector.
const SERVICES: &[(&str, &str, &str, &[&str])] = &[
    (
        "gmail",
        "v1",
        "https://gmail.googleapis.com",
        &[
            "google-gmail-message-get",
            "google-gmail-message-send",
            "google-gmail-labels-list",
        ],
    ),
    (
        "calendar",
        "v3",
        BASE_URL,
        &[
            "google-calendar-event-get",
            "google-calendar-event-insert",
            "google-calendar-calendar-get",
        ],
    ),
    (
        "drive",
        "v3",
        BASE_URL,
        &["google-drive-file-get", "google-drive-file-update"],
    ),
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-69 ships the Google Workspace connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and authenticates the way C-69 specifies: one bearer credential over
/// `GOOGLE_ACCESS_TOKEN`, named by `default_auth` and inherited by every operation.
#[test]
fn the_google_connector_loads_and_authenticates_with_a_bearer_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Google Workspace");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the connector-level host is the one Calendar and Drive are documented on"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "google authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("google declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "every Google API takes `Authorization: Bearer <token>`, whether the token came from a \
         service account or from a user's OAuth2 grant"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );
    assert!(
        !connector.default_auth.is_empty(),
        "`default_auth` must name the credential, or every operation would have to repeat it"
    );

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; google is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the access token",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; the Authorization header is injected \
             by the host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// **The headline claim: three services, each versioned on its own.**
///
/// Gmail is `v1` while Calendar and Drive are `v3`. That is not a detail — it is the case that
/// settles where `api_version` lives. A connector-level version could state only one of the three, so
/// two of the three services would publish an address naming a version they are not served at.
#[test]
fn each_google_service_declares_its_own_api_version() {
    let connector = load();

    let declared: Vec<&str> = connector.service_names();
    let expected: Vec<&str> = SERVICES.iter().map(|(name, ..)| *name).collect();
    assert_eq!(
        declared, expected,
        "the declared services are not the three C-69 ships, in order"
    );
    assert!(
        !connector.is_default_only(),
        "google is the first multi-service provider; a `default`-only google would emit one \
         `google.flux` for three unrelated APIs"
    );

    for (name, version, ..) in SERVICES {
        let service = connector
            .service(name)
            .unwrap_or_else(|| panic!("`{name}` is declared"));
        assert_eq!(
            service.api_version.as_deref(),
            Some(*version),
            "service `{name}` must state its own `api_version`; inheriting the connector's is what \
             C-49 rejects"
        );
        assert_eq!(connector.api_version_of(name), Some(*version));
        assert!(
            !service.description.is_empty(),
            "service `{name}` describes itself in one line; a manifest and the public catalogue both \
             publish it"
        );
    }

    // Two of the three resolve to `v3` and one to `v1`. Asserted as a set so the claim cannot be
    // satisfied by three services that happen to agree — which is the shape that would make a
    // connector-level version sufficient after all.
    let versions: BTreeSet<&str> = SERVICES.iter().map(|(_, version, ..)| *version).collect();
    assert!(
        versions.len() > 1,
        "the versions no longer differ, so this connector stopped being the case C-49 was designed \
         for: {versions:?}"
    );
}

/// Every service owns operations, and every operation belongs to the service that owns it.
///
/// The loader refuses the negative halves — an operation naming an undeclared service, and an
/// operation in a multi-service provider naming none — so what is left to assert is that the
/// partition is the one C-69 curated, and that no service is a declaration with nothing behind it.
#[test]
fn every_google_service_owns_the_operations_it_declares() {
    let connector = load();

    let mut expected_all: Vec<&str> = Vec::new();
    for (name, _, _, operations) in SERVICES {
        let owned: Vec<&str> = connector
            .operations_of(name)
            .map(|operation| operation.id.as_str())
            .collect();
        assert_eq!(
            owned, *operations,
            "service `{name}` does not own the operations C-69 curates for it"
        );
        assert!(
            !owned.is_empty(),
            "service `{name}` owns no operation, so it is a declaration with nothing behind it — and \
             it would still emit an empty module and manifest"
        );
        expected_all.extend_from_slice(operations);
    }

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, expected_all,
        "the operation set is not the union of the three services' — either an operation is missing \
         or one belongs to a service this test does not know about"
    );
}

/// **No query parameter at all** — the strong form, on the IR.
///
/// The story's requirement is that no operation declares a string-ish or `Any`-typed query parameter.
/// This asserts the stronger and simpler property that implies it: the query surface is empty.
/// Nothing in this pipeline percent-encodes a query value — the emitter interpolates it verbatim into
/// a `fmt` template and flux registers no encoding op (C-30) — and `zendesk-ticket-search` is the
/// standing demonstration that AGENTS.md lists under *Intentional gaps*.
///
/// "Empty" also survives editing in a way "no string-ish value" does not: a later author cannot
/// satisfy it by picking a narrower type for an injectable value. Google's excluded surface is every
/// list-with-filter endpoint (`q` on Gmail and Drive search, `pageToken` paging, Calendar's
/// `timeMin`/`timeMax`) plus the parameters that change a *response* rather than select it —
/// Gmail's `format`, Drive's `alt=media` and `fields=` — all named in the story's Notes.
#[test]
fn no_google_operation_declares_a_query_parameter() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            declared.is_empty(),
            "operation `{}` declares query parameters {declared:?}. Nothing percent-encodes a query \
             value (C-30), so a value carrying `&` or `#` corrupts the request or injects a \
             parameter — the `zendesk-ticket-search` failure AGENTS.md records. If C-30 has landed, \
             change this test deliberately",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads.
///
/// **Every `$url = ` line is checked, not just the first, and that is the substance of this test.**
/// The emitter binds `$url` once for the path and the required query parameters, then re-binds it
/// once more per *optional* query parameter inside a `when` guard, with the `?` on a separate `$sep`
/// binding — `connectors/zendesk.flux` shows the shape:
///
/// ```flux
/// $url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// $sep = "?"
/// when $page
///   $url = fmt("{url}{sep}page={page}")
/// ```
///
/// So inspecting only the first binding would pass while an operation quietly appended optional
/// filters, and a check for `?` alone would miss it too.
#[test]
fn no_google_module_assembles_a_query_string() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("$url = "))
            .collect();
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` binds $url {} times; the emitter does that once for the path and once per optional \
             query parameter, so anything but one binding means a query string:\n{emitted}",
            operation.id,
            url_lines.len()
        );
        assert!(
            !url_lines[0].contains('?'),
            "`{}` emits a query string: {}",
            operation.id,
            url_lines[0]
        );
        assert!(
            !emitted.contains("$sep"),
            "`{}` emits the `$sep` query separator, which exists only to join query parameters:\n\
             {emitted}",
            operation.id
        );
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// An omitted optional body field is not omitted: the emitter binds every declared field into the
/// payload record, so a caller who passes nothing sends an explicit `null`. Google's JSON APIs
/// type-check the members of a resource and reject `null` where they expect a string or an object —
/// and Drive's `files.update` is worse than a rejection, because a field that *is* nullable is
/// **cleared** by one. A connector that offered the field would therefore destroy data because the
/// caller left it alone, which is the worst available failure mode.
///
/// So the surface is required fields only, and what that costs is written down in the story's Notes
/// and in the header comment of `providers/google.toml`.
#[test]
fn no_google_body_field_is_optional() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`, so the resource shape becomes the \
             caller's problem and nothing checks it; every Google body here is a field list with \
             explicit `wire` paths",
            operation.id
        );
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}`: body field `{}` is optional. An omitted optional field travels as \
                 an explicit `null` (C-56), which Google rejects — or, on `files.update`, applies as \
                 a clear — so declare it required or leave it out",
                operation.id,
                param.name
            );
        }
    }
}

/// **Every request targets its own service's host, and none is wider than one name.**
///
/// This is the claim that only a multi-service provider can make. Gmail is documented on
/// `gmail.googleapis.com` and the other two on `www.googleapis.com`, so `$base` has to come from
/// [`Connector::base_url_of`] rather than from the connector: an operation that bound the provider's
/// base URL would send every Gmail call to a host that does not serve it, while
/// `connectors/google-gmail.connector.toml` — the manifest that installs alongside it, and the value
/// C-10's `http_hosts` derives from — named the right one. The two artifacts of one service would
/// then disagree about where the traffic goes.
#[test]
fn every_google_request_targets_its_own_services_host() {
    let connector = load();

    for (name, _, base_url, _) in SERVICES {
        assert_eq!(
            connector.base_url_of(name),
            *base_url,
            "service `{name}` does not resolve to the host the vendor documents"
        );
        assert!(
            !base_url.contains('{') && !base_url.contains('*'),
            "service `{name}` must name one bound host, never a template or a wildcard: {base_url}"
        );
    }

    for operation in &connector.operations {
        let expected = connector.base_url_of(&operation.service);
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!(r#"$base = "{expected}""#)),
            "`{}` belongs to service `{}`, whose host is {expected}, but its module binds a \
             different base URL:\n{emitted}",
            operation.id,
            operation.service
        );
        assert!(
            operation
                .path
                .starts_with(&format!("/{}/", operation.service)),
            "`{}` has path {:?}; every Google endpoint is served under its service's own versioned \
             prefix, e.g. `/drive/v3/…`",
            operation.id,
            operation.path
        );
        assert!(
            operation.path.contains(&format!(
                "/{}/",
                connector
                    .api_version_of(&operation.service)
                    .expect("every google service states its api_version")
            )),
            "`{}` has path {:?}, which does not name its service's API version — the path and the \
             declared version would then describe two different surfaces",
            operation.id,
            operation.path
        );
    }
}

/// No credential value, and nothing shaped like one, reaches a generated artifact.
///
/// AGENTS.md's hard invariant. The connector carries the environment variable *name* so a host can
/// resolve it; the emitted module carries neither that name nor a value, because auth injection is
/// the `$auth` seam and is deliberately absent rather than stubbed — which is also why this connector
/// cannot yet make a live call.
#[test]
fn no_google_module_carries_a_credential() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            !emitted.contains(TOKEN_ENV),
            "`{}` names {TOKEN_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        // Google's OAuth2 access tokens are `ya29.`-prefixed and its API keys `AIza`-prefixed. A
        // literal one in a generated artifact is the failure this invariant exists to prevent, so it
        // is checked for by shape as well as by name.
        for shape in ["ya29.", "AIza", "client_secret"] {
            assert!(
                !emitted.contains(shape),
                "`{}` embeds something shaped like a Google credential (`{shape}`):\n{emitted}",
                operation.id
            );
        }
    }
}

/// **Risk and idempotency describe what the call does, not which verb it uses.**
///
/// Sending a mail and inserting an event are externally visible and cannot be recalled, so neither is
/// `low` and neither may claim idempotency: a retry around one delivers a second mail or creates a
/// duplicate event. Google documents no idempotency key on any of the three writes here — no
/// `If-Match`, no client-supplied request id — so `idempotent` would be an assertion the vendor does
/// not make, which the story rules out explicitly.
#[test]
fn no_google_write_claims_idempotency_the_vendor_does_not_document() {
    let connector = load();

    let mut writes = 0;
    for operation in &connector.operations {
        let reads = matches!(operation.method, HttpMethod::Get | HttpMethod::Head);
        if reads {
            assert_eq!(
                operation.idempotency,
                Idempotency::Idempotent,
                "`{}` is a read; a read is idempotent by construction",
                operation.id
            );
            assert_eq!(
                operation.risk,
                Risk::Low,
                "`{}` is a read and changes nothing",
                operation.id
            );
            assert!(
                operation.params.body.is_empty() && operation.params.body_schema.is_none(),
                "`{}` is a read carrying a request body",
                operation.id
            );
            continue;
        }
        writes += 1;

        assert_ne!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{}` is a write marked idempotent. Google documents no idempotency key on it, so a \
             retry repeats the effect — a second mail, a duplicate event",
            operation.id
        );
        assert_ne!(
            operation.risk,
            Risk::Low,
            "`{}` writes; `low` is for reads and writes that cannot surprise anyone",
            operation.id
        );
    }

    assert_eq!(
        writes, 3,
        "C-69 ships three writes — a mail send, an event insert and a file-metadata update — so a \
         different count means the curated set moved and the claims above cover something else"
    );
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and reloads through flux-lang's module loader as exactly one
/// exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set; it is restated here so the Google
/// connector's own file fails on its own when a module stops being analyzable. A module that parsed
/// but did not load publishes no ops at all, so a consumer handing it to flux would get silence
/// rather than an error.
#[test]
fn every_google_operation_emits_an_analyzable_module() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "`{}` emits Flux that does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would rewrite `{}`",
            operation.id
        );

        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}

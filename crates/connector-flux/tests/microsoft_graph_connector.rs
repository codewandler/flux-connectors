//! The Microsoft Graph connector, and the question C-108 exists to answer: **is a service a
//! genuine addressing level, or was it Google's host problem wearing a hat?**
//!
//! `google_connector.rs` is the first multi-service provider, and its three services differ in
//! both host (`gmail.googleapis.com` vs `www.googleapis.com`) and `api_version` (`v1`, `v3`, `v3`).
//! Microsoft Graph is the second multi-service provider, and it is built to isolate the opposite
//! case: `mail`, `calendar` and `files` share **one host** (`graph.microsoft.com`) and **one
//! `api_version`** (`v1.0`). If the partition were only ever justified by a differing host or
//! version, this connector would have no reason to declare three services at all. This file's
//! headline test is the one that checks whether the partition still earns its place when neither
//! excuse is available — see `graph_services_share_a_host_and_version_but_still_partition_cleanly`.
//!
//! The body-surface claim and the closed query exception are provider-specific. C-471 adds integer
//! `$top`/`$skip` only to four exact spec-backed reads; the eight operations C-108 shipped remain
//! query-free, and no string-shaped OData expression is admitted.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider id, and therefore the file stem, the catalogue module and every op id's prefix.
const PROVIDER: &str = "microsoft_graph";

/// The one host every service resolves to — the point of this connector.
const BASE_URL: &str = "https://graph.microsoft.com";

/// The one `api_version` every service resolves to — the other half of the point.
const API_VERSION: &str = "v1.0";

/// The credential the connector declares, and the environment variable it resolves from.
const CREDENTIAL: &str = "microsoft_graph.access_token";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "MICROSOFT_GRAPH_ACCESS_TOKEN";

/// The three services and the operations curated for each, in the order
/// `providers/microsoft_graph.toml` declares them.
const SERVICES: &[(&str, &[&str])] = &[
    (
        "mail",
        &[
            "microsoft_graph-mail-message-get",
            "microsoft_graph-mail-message-reply",
            "microsoft_graph-mail-folder-list",
            "microsoft_graph-mail-message-list",
        ],
    ),
    (
        "calendar",
        &[
            "microsoft_graph-calendar-event-get",
            "microsoft_graph-calendar-event-create",
            "microsoft_graph-calendar-calendar-get",
            "microsoft_graph-calendar-category-list",
            "microsoft_graph-calendar-time-zone-list",
            "microsoft_graph-calendar-language-list",
        ],
    ),
    (
        "files",
        &[
            "microsoft_graph-files-item-get",
            "microsoft_graph-files-item-update",
        ],
    ),
];

/// The closed C-471 exception to the eight existing operations' query-free contract.
const INTEGER_PAGED_READS: [&str; 4] = [
    "microsoft_graph-mail-message-list",
    "microsoft_graph-calendar-category-list",
    "microsoft_graph-calendar-time-zone-list",
    "microsoft_graph-calendar-language-list",
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
            "cannot read {} ({error}) — C-108 ships the Microsoft Graph connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The connector exists, loads, and authenticates with one bearer credential inherited by every
/// operation — the same already-minted-token shape Google and Zoom use, and the shape this
/// connector takes because C-88 (proving `OAuth2Spec` on a real provider) is unshipped and earmarked
/// for a different provider. See `providers/microsoft_graph.toml`'s header comment.
#[test]
fn the_microsoft_graph_connector_loads_and_authenticates_with_a_bearer_token() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        1,
        "microsoft_graph authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("microsoft_graph declares `{CREDENTIAL}`"));
    assert_eq!(method.scheme, AuthScheme::Bearer);
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` is single-mechanism",
            operation.id
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

/// **The headline claim.** Every service resolves to the same host and the same `api_version` —
/// the opposite of Google's case — and the partition is asserted to hold anyway: each service owns
/// a disjoint, non-empty operation set and is a real installable unit, even though nothing about
/// its host or version distinguishes it from its siblings.
#[test]
fn graph_services_share_a_host_and_version_but_still_partition_cleanly() {
    let connector = load();

    let declared: Vec<&str> = connector.service_names();
    let expected: Vec<&str> = SERVICES.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        declared, expected,
        "the declared services are not the three C-108 ships, in order"
    );
    assert!(
        !connector.is_default_only(),
        "microsoft_graph is a multi-service provider; a `default`-only shape would emit one \
         `microsoft_graph.flux` for three curated surfaces"
    );

    let mut hosts: BTreeSet<&str> = BTreeSet::new();
    let mut versions: BTreeSet<&str> = BTreeSet::new();
    for (name, _) in SERVICES {
        hosts.insert(connector.base_url_of(name));
        versions.insert(
            connector
                .api_version_of(name)
                .unwrap_or_else(|| panic!("service `{name}` states its own `api_version`")),
        );
        let service = connector
            .service(name)
            .unwrap_or_else(|| panic!("`{name}` is declared"));
        assert!(
            !service.description.is_empty(),
            "service `{name}` describes itself in one line"
        );
    }
    assert_eq!(
        hosts,
        BTreeSet::from([BASE_URL]),
        "every service must resolve to the one Graph host — a divergence here would mean this \
         connector accidentally reproduced Google's shape rather than testing the opposite of it"
    );
    assert_eq!(
        versions,
        BTreeSet::from([API_VERSION]),
        "every service must resolve to the one Graph api_version, for the same reason"
    );
}

/// Every service owns operations, and every operation belongs to the service that owns it — the
/// same partition-completeness claim `google_connector.rs` makes, restated for this provider's own
/// curated set.
#[test]
fn every_microsoft_graph_service_owns_the_operations_it_declares() {
    let connector = load();

    let mut expected_all: BTreeSet<&str> = BTreeSet::new();
    for (name, operations) in SERVICES {
        let owned: Vec<&str> = connector
            .operations_of(name)
            .map(|operation| operation.id.as_str())
            .collect();
        assert_eq!(
            owned, *operations,
            "service `{name}` does not own the operations C-108 curates for it"
        );
        assert!(
            !owned.is_empty(),
            "service `{name}` owns no operation, so it is a declaration with nothing behind it"
        );
        expected_all.extend(operations.iter().copied());
    }

    let declared: BTreeSet<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, expected_all,
        "the operation set is not the union of the three services'"
    );
    assert_eq!(
        declared.len(),
        12,
        "C-108's eight plus C-471's four reads are four-six-two"
    );
}

/// Four exact reads keep integer `$top`/`$skip`; every other operation remains query-free.
///
/// `$filter`, `$search`, `$orderby`, `$select`, `$expand`, string `includeHiddenMessages` and
/// boolean `$count` remain omitted. A name-derived or suffix-derived exception would let the next
/// operation widen silently, so the four public ids are a literal closed set.
#[test]
fn only_four_named_reads_declare_integer_paging_queries() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        if INTEGER_PAGED_READS.contains(&operation.id.as_str()) {
            assert_eq!(
                declared,
                ["$top", "$skip"],
                "{} widened paging",
                operation.id
            );
            for param in &operation.params.query {
                assert_eq!(
                    param.schema.get("type").and_then(|value| value.as_str()),
                    Some("integer"),
                    "{} on {} is not integer-shaped",
                    param.name,
                    operation.id
                );
            }
        } else {
            assert!(
                declared.is_empty(),
                "the existing operation `{}` gained query parameters {declared:?}",
                operation.id
            );
        }
    }
}

/// The same closed exception over emitted Flux: four named modules assemble precisely two integer
/// options, while every previously shipped module remains byte-shaped as one fixed URL.
#[test]
fn only_four_named_modules_assemble_integer_paging() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        if INTEGER_PAGED_READS.contains(&operation.id.as_str()) {
            assert_eq!(
                url_lines.len(),
                3,
                "{} emitted an unexpected query shape",
                operation.id
            );
            assert!(emitted.contains(r#"url = fmt("{url}{sep}$top={_top}")"#));
            assert!(emitted.contains(r#"url = fmt("{url}{sep}$skip={_skip}")"#));
            assert_eq!(emitted.matches("sep = ").count(), 2);
        } else {
            assert_eq!(
                url_lines.len(),
                1,
                "`{}` binds url {} times, which means a query string:\n{emitted}",
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
                !emitted.contains("sep = "),
                "`{}` emits the `sep` query separator, which exists only to join query parameters",
                operation.id
            );
        }
    }
}

/// **No optional request-body field, until C-56 lands.** An omitted optional field would travel as
/// an explicit `null`, which Graph's JSON validator is not documented to accept in place of an
/// absent member. So every body field declared below is required, and what that costs — attendees
/// on the event create, a caller-chosen recipient override on the mail reply — is recorded in
/// `providers/microsoft_graph.toml`'s header comment.
#[test]
fn no_microsoft_graph_body_field_is_optional() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.params.body_schema.is_none(),
            "operation `{}` declares a free-form `body_schema`",
            operation.id
        );
        for param in &operation.params.body {
            assert!(
                param.required,
                "operation `{}`: body field `{}` is optional, which travels as an explicit `null` \
                 (C-56)",
                operation.id, param.name
            );
        }
    }
}

/// No array-of-objects requiring a `wire` path to decompose across nested segments (C-185). Every
/// body field below is either a body-root scalar or nests under a `wire` object path with no array
/// anywhere in the tree — checked directly on the declared schema rather than assumed from the
/// TOML.
#[test]
fn no_microsoft_graph_body_field_is_an_array() {
    let connector = load();

    for operation in &connector.operations {
        for param in &operation.params.body {
            assert_ne!(
                param.schema.get("type").and_then(|v| v.as_str()),
                Some("array"),
                "operation `{}`: body field `{}` is an array — C-185 is not exercised by this \
                 connector, and this test is the tripwire for a future edit that reaches for one",
                operation.id,
                param.name
            );
        }
    }
}

/// Every request targets the one Graph host, at the one declared version — checked on the emitted
/// text as well as on the IR, so a service that resolved correctly in the loader but emitted the
/// wrong `$base` would still be caught.
#[test]
fn every_microsoft_graph_request_targets_the_shared_host_and_version() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            emitted.contains(&format!(r#"base = "{BASE_URL}""#)),
            "`{}` does not bind the shared Graph host:\n{emitted}",
            operation.id
        );
        assert!(
            operation.path.starts_with("/v1.0/"),
            "`{}` has path {:?}, which does not name the one `v1.0` prefix every operation shares",
            operation.id,
            operation.path
        );
    }
}

/// No credential value, and nothing shaped like one, reaches a generated artifact.
#[test]
fn no_microsoft_graph_module_carries_a_credential() {
    let connector = load();

    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        assert!(
            !emitted.contains(TOKEN_ENV),
            "`{}` names {TOKEN_ENV} in generated Flux:\n{emitted}",
            operation.id
        );
        for shape in ["Bearer ey", "client_secret"] {
            assert!(
                !emitted.contains(shape),
                "`{}` embeds something shaped like a credential (`{shape}`):\n{emitted}",
                operation.id
            );
        }
    }
}

/// **Risk and idempotency describe what the call does, not which verb it uses**, and every write
/// here is `POST` or `PATCH` — the two methods `check_write_metadata` (C-186) refuses to let claim
/// `idempotent` by method, regardless of vendor semantics. That refusal happens to agree with what
/// Graph itself documents (no idempotency key on any of the three writes below), so this connector
/// does not hit the C-186 gap the way `cloudflare-cache-purge` does — it is recorded anyway because
/// a future edit that adds a genuinely idempotent Graph write (there are some, e.g. a `PUT` upload)
/// would need to know the distinction.
#[test]
fn no_microsoft_graph_write_claims_idempotency_the_vendor_does_not_document() {
    let connector = load();

    let mut writes = 0;
    for operation in &connector.operations {
        let reads = matches!(operation.method, HttpMethod::Get | HttpMethod::Head);
        if reads {
            assert_eq!(
                operation.idempotency,
                Idempotency::Idempotent,
                "`{}` is a read",
                operation.id
            );
            assert_eq!(
                operation.risk,
                Risk::Low,
                "`{}` is a read and changes nothing",
                operation.id
            );
            continue;
        }
        writes += 1;
        assert!(
            matches!(operation.method, HttpMethod::Post | HttpMethod::Patch),
            "`{}` uses a method this test does not expect: {:?}",
            operation.id,
            operation.method
        );
        assert_ne!(
            operation.idempotency,
            Idempotency::Idempotent,
            "`{}` is a write marked idempotent, which the emitter refuses on POST/PATCH by method \
             (C-186) and which Graph does not document a key for either",
            operation.id
        );
        assert_ne!(
            operation.risk,
            Risk::Low,
            "`{}` writes; `low` is for reads",
            operation.id
        );
    }

    assert_eq!(
        writes, 3,
        "C-108 ships three writes — a mail reply, an event create and a file rename"
    );
}

/// The `verify` operation is a zero-argument, low-risk read, so a settings page can press it
/// unattended — the configuration contract's own rule (`AGENTS.md`).
#[test]
fn verify_is_a_bounded_zero_argument_read() {
    let connector = load();

    let verify_id = connector
        .verify
        .as_deref()
        .expect("microsoft_graph declares a `verify` operation");
    let operation = connector
        .operation(verify_id)
        .unwrap_or_else(|| panic!("`verify` names an operation that does not exist"));

    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert!(
        operation.params.path.is_empty()
            && operation.params.query.is_empty()
            && operation.params.body.is_empty(),
        "`{}` is `verify` but takes an argument; a settings page cannot supply one",
        verify_id
    );
}

/// **The C-11 gate, per operation.** Each one emits, parses with no diagnostics, is already a fixed
/// point of flux's own formatter, and loads through flux-lang's module loader as exactly one exposed
/// composite op.
#[test]
fn every_microsoft_graph_operation_emits_an_analyzable_module() {
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
            "`{}` must be exposed as a tool",
            operation.id
        );
    }
}

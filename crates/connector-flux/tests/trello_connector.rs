//! Trello (C-165) is the epic's probe for a **query-placed credential** — `?key=…&token=…`, the
//! one placement the three-axis auth model has always modelled
//! (`AuthScheme::Query`, `crates/connector-spec/src/auth.rs:119-123`;
//! `catalog::Placement::Query`, `crates/catalog/src/lib.rs:219-223`) and that no shipped connector
//! had ever declared. [C-159](../../../docs/stories/C-159-request-debug-and-query-encoding.md) §2
//! measured the committed catalogue as **18 `Placement::Header`, 2 `Placement::Inbound`, zero
//! `Placement::Query`**, and called the divergence it found there "unreachable today" for exactly
//! that reason. This connector is what makes it reachable, so this file measures the reach rather
//! than asserting that a TOML file parses.
//!
//! Four findings, each pinned below:
//!
//! 1. **Two credentials, both query-placed, both required on every request.** `key` and `token`
//!    travel together as one AND-mechanism; neither authenticates anything alone.
//! 2. **Trello is the only query placement in the shipped catalogue**, so the *whole* of C-159 §2's
//!    newly-reachable surface is this one connector — which is the fact that makes the hazard
//!    reviewable at all.
//! 3. **This connector puts nothing else in a query string.** Not one operation declares a query
//!    parameter, and no emitted `op` writes a `?` into its URL. That is deliberate curation, not an
//!    accident of the endpoints chosen: query values are interpolated verbatim by the emitter
//!    (`crates/connector-flux/src/op.rs:138-143`, `AGENTS.md`'s `zendesk-ticket-search` gap), so the
//!    free text this connector's writes carry — a card's name and description — travels in a JSON
//!    body instead, which Trello's own reference documents as an equal alternative to its query
//!    parameters. The consequence is the property this file exists to pin: **the two credentials the
//!    host appends are the only thing that is ever in a Trello request's query string.**
//! 4. **The query placement forces `secret = true` on the key, and Trello's documentation calls the
//!    key public.** The over-classification is accepted deliberately and is recorded in the provider
//!    file's header; the loader refuses the alternative, and this file pins that refusal so the
//!    decision cannot be quietly reversed by editing one word.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk};

/// The provider under test.
const PROVIDER: &str = "trello";

const KEY: &str = "trello.key";
const KEY_PARAM: &str = "key";
/// A variable *name*; no credential value appears in this repository.
const KEY_ENV: &str = "TRELLO_API_KEY";

const TOKEN: &str = "trello.token";
const TOKEN_PARAM: &str = "token";
/// A variable *name*; no credential value appears in this repository.
const TOKEN_ENV: &str = "TRELLO_API_TOKEN";

const BASE_URL: &str = "https://api.trello.com/1";

/// The verification read — argument-free, so a settings page can run it unattended.
const VERIFY: &str = "trello-board-list";

/// The six curated operations, in the order `providers/trello.toml` declares them.
const OPERATIONS: &[&str] = &[
    "trello-board-list",
    "trello-board-get",
    "trello-board-lists",
    "trello-list-cards",
    "trello-card-create",
    "trello-card-archive",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    load_provider(PROVIDER)
}

fn load_provider(id: &str) -> Connector {
    let path = providers_dir().join(format!("{id}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-165 ships the Trello connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{id}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{id}.toml does not load: {error}"))
        .connector
}

/// **Finding 1: two credentials, both placed in the query string, and always together.**
///
/// Trello has one authentication mechanism and it is a pair: `?key=<key>&token=<token>`. Neither
/// half authenticates a call on its own — a token is minted against a specific key — so this is an
/// AND-set (one `AuthRequirement` naming both), the shape `providers/datadog.toml` shipped first,
/// met here for the first time at a *query* placement rather than at two headers.
#[test]
fn both_credentials_are_placed_in_the_query_string_and_travel_together() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Trello");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.authority.as_deref(), Some("com.trello.api"));

    assert_eq!(
        connector.auth.len(),
        2,
        "Trello authenticates with a key and a token, never one alone"
    );

    let key = connector
        .auth_method(KEY)
        .unwrap_or_else(|| panic!("trello declares `{KEY}`"));
    assert_eq!(
        key.scheme,
        AuthScheme::Query {
            name: KEY_PARAM.to_string()
        },
        "the API key is a query parameter — the placement no shipped connector had declared"
    );
    assert_eq!(key.env, [KEY_ENV]);

    let token = connector
        .auth_method(TOKEN)
        .unwrap_or_else(|| panic!("trello declares `{TOKEN}`"));
    assert_eq!(
        token.scheme,
        AuthScheme::Query {
            name: TOKEN_PARAM.to_string()
        },
        "the token is the second query parameter, on the same request as the key"
    );
    assert_eq!(token.env, [TOKEN_ENV]);
    assert_ne!(
        key.env, token.env,
        "the two credentials must never resolve from the same variable"
    );

    assert_eq!(
        connector.default_auth.len(),
        1,
        "Trello offers no alternative mechanism, so there is one alternative, not two"
    );
    let mechanism: Vec<&str> = connector.default_auth[0]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(
        mechanism,
        [KEY, TOKEN],
        "one mechanism naming both credentials — an AND-set. Two single-credential alternatives \
         would tell a host that either half authenticates a request, and neither does"
    );

    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("trello declares `{id}`"));
        let effective: Vec<Vec<&str>> = connector
            .effective_auth(operation)
            .iter()
            .map(|requirement| requirement.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            effective,
            vec![vec![KEY, TOKEN]],
            "every operation carries both credentials; none overrides the default"
        );
    }
}

/// **Finding 2: Trello is what made C-159 §2's hazard reachable** — and the catalogue-wide half of
/// that finding lives in `crates/connector-flux/tests/query_placed_credentials.rs`, not here.
///
/// C-159 §2 recorded a real divergence in `connector-pack` — a query-placed credential is
/// percent-encoded on its way onto the URL (`crates/connector-pack/src/auth.rs:157-164`, `:204-215`)
/// while the *unencoded* value is what was registered with flux's redactor — and closed the finding
/// as unreachable, because the committed catalogue declared no query placement at all. This
/// connector is what made it reachable, which is the Trello-scoped fact and is asserted here.
///
/// **What this test used to be, and why it is not that any more (C-230).** It walked every
/// `providers/*.toml` and asserted the query-placed set equalled `[trello:key, trello:token]`. That
/// was green only because no provider since Trello had placed a credential in the query string, and
/// the next one that did would have turned *Trello's* test red — from a worktree holding a different
/// connector, for a reason having nothing to do with Trello, discovered at integration and blamed on
/// whichever merge happened to be second. `AGENTS.md`'s parallel-provider guarantee is that two
/// implementors' write sets are disjoint; a catalogue-walking assertion breaks it without touching a
/// shared file.
///
/// The measurement was not deleted. It became a **property** — every connector that places a
/// credential in the query string puts nothing else there — which is the question the hazard
/// actually poses, and which a fifty-fourth connector cannot falsify merely by existing.
#[test]
fn trello_made_the_query_placement_hazard_reachable() {
    let connector = load();

    let query_placed: Vec<String> = connector
        .auth
        .iter()
        .filter(|method| matches!(method.scheme, AuthScheme::Query { .. }))
        .map(|method| format!("{PROVIDER}:{}", method.name))
        .collect();

    assert_eq!(
        query_placed,
        [format!("{PROVIDER}:{KEY}"), format!("{PROVIDER}:{TOKEN}")],
        "both of Trello's credentials are query-placed, and they are the whole of its exposure to \
         C-159 §2. This is a closed claim about one connector: the catalogue-wide half — that no \
         connector combines a query-placed credential with caller text in the same query string — is \
         `crates/connector-flux/tests/query_placed_credentials.rs`"
    );
}

/// **Finding 3, and the heart of the probe: nothing but the credentials ever reaches the query
/// string.**
///
/// The emitter interpolates a query value verbatim — it percent-encodes nothing, deliberately, and
/// says so at `crates/connector-flux/src/op.rs:138-143` — which is why `zendesk-ticket-search` is a
/// recorded intentional gap rather than a working operation. A connector whose credential lives in
/// the query string is the worst possible place to also put unencoded caller text: a value carrying
/// `&` would not merely corrupt a filter, it would land *before* the credential the host appends and
/// could inject a parameter of its own.
///
/// So this connector declares no query parameter anywhere, and its two writes carry their free text
/// in a JSON body instead — an equal alternative in Trello's own reference, which states that its
/// query parameters "may also be replaced with a JSON request body instead". Both halves are
/// asserted: the declaration (no `params.query`) and the emitted Flux (no `?` in any URL, and none
/// of the `sep` machinery `crates/connector-flux/src/op.rs` emits for an optional filter).
#[test]
fn no_operation_puts_anything_in_the_query_string() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "the curated set, in declaration order"
    );

    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "{} declares a query parameter. Every query value this emitter writes is interpolated \
             verbatim, and this connector's query string is where its credential travels",
            operation.id
        );

        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        assert!(
            !flux.contains('?'),
            "{} emits a `?` into its URL:\n{flux}",
            operation.id
        );
        assert!(
            !flux.contains("sep = "),
            "{} emits the optional-query-parameter machinery:\n{flux}",
            operation.id
        );
    }

    // The positive half: the free text a caller supplies really is declared, and really is in a
    // body. Without this, "no query parameters" would be satisfiable by a connector that asks for
    // nothing at all.
    let create = connector
        .operation("trello-card-create")
        .expect("the curated set includes the card write");
    let body: Vec<(&str, &str)> = create
        .params
        .body
        .iter()
        .map(|param| {
            (
                param.name.as_str(),
                param.wire.as_deref().unwrap_or(param.name.as_str()),
            )
        })
        .collect();
    assert_eq!(
        body,
        [
            ("list_id", "idList"),
            ("name", "name"),
            ("description", "desc"),
        ],
        "the card's name and description are body fields, not query parameters — caller-facing \
         name on the left, the spelling Trello sees on the right"
    );
    let flux = emit_operation(&connector, create).expect("the card write emits");
    assert!(
        flux.contains("content_type = \"application/json\""),
        "the card write sends a JSON body:\n{flux}"
    );
}

/// **Finding 4: reaching a query parameter forces a `secret = true` claim that Trello's own
/// documentation contradicts for the key — and this connector accepts it rather than lying the
/// other way.**
///
/// The only route from a `[[config]]` field to a request query parameter is an `[[auth]]`-declared
/// credential, and `Binding::Credential::is_secret()` is unconditionally `true`
/// (`crates/connector-spec/src/config.rs:223-231`), enforced at the loader
/// (`crates/connector-spec/src/provider.rs`, the `secret`/`binds` agreement). Trello's authorization
/// guide says "It is ok for your API key to be publicly available, but a token should never be
/// publicly available" — so the key is declared here as *more* protected than the vendor requires.
///
/// That direction is the safe one, and it is the only one available: the alternative is a field
/// declaring `secret = false` while binding a credential, which is the contradicting second source
/// of truth the configuration contract exists to refuse. This test pins both — the shipped file
/// classifies both halves as secret at connection level, and the loader refuses the alternative —
/// so overturning the decision means overturning a test, not editing a word. The Algolia probe
/// (C-164) met the same wall from the other side, where over-classification was *not* acceptable
/// because the value also had to be readable back into a hostname.
#[test]
fn the_query_placement_forces_the_key_to_be_declared_secret() {
    let connector = load();

    for (field_name, credential) in [("api_key", KEY), ("api_token", TOKEN)] {
        let field = connector
            .config
            .iter()
            .find(|field| field.name == field_name)
            .unwrap_or_else(|| panic!("trello declares the `{field_name}` config field"));
        assert_eq!(
            field.binding(),
            Some(Binding::Credential { name: credential })
        );
        assert!(
            field.secret,
            "`{field_name}` binds a credential, so it is a secret field — the agreement is a loader \
             rule, not a convention"
        );
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "`{field_name}` is renderable"
        );
        assert!(
            field.example.is_none(),
            "`{field_name}` is a secret and carries no example — a placeholder shaped like a real \
             token has tripped push protection before"
        );
    }

    // The refusal, measured rather than described: declaring the credentials non-secret — which is
    // what Trello's own documentation would justify for the key — does not load.
    let source = std::fs::read_to_string(providers_dir().join(format!("{PROVIDER}.toml")))
        .expect("the shipped file is readable");
    let honest_about_the_vendor = source.replace("secret = true", "secret = false");
    assert_ne!(
        honest_about_the_vendor, source,
        "the substitution must actually apply, or the refusal below proves nothing"
    );
    let error = provider::load(
        &format!("providers/{PROVIDER}.toml"),
        &honest_about_the_vendor,
    )
    .expect_err("a config field binding a credential while declaring `secret = false` is refused");
    let message = error.to_string();
    assert!(
        message.contains("api_key") && message.contains("secret"),
        "expected the secret/binds agreement refusal naming the field, got: {message}"
    );
}

/// **`verify` is a read that runs unattended.**
///
/// A "Test connection" button is pressed whenever someone opens a settings page, so it must be a
/// read (the loader checks the declared risk) *and* it must need no argument, which the loader does
/// not check and a connector can still get wrong. `GET /1/members/me/boards` is the request Trello's
/// own API introduction uses as its worked example, and it takes nothing but the credential pair.
#[test]
fn verify_is_an_argument_free_read() {
    let connector = load();

    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    let operation = connector
        .operation(VERIFY)
        .expect("verify names an operation");

    assert_eq!(operation.method, HttpMethod::Get);
    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert!(
        operation.params.path.is_empty()
            && operation.params.query.is_empty()
            && operation.params.body.is_empty()
            && operation.params.header.is_empty()
            && operation.params.body_schema.is_none(),
        "a connection test that needs an argument cannot run unattended"
    );
}

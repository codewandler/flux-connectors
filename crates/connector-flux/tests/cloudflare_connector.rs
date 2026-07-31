//! `providers/cloudflare.toml` exists, emits analyzable Flux, and answers C-169's actual question:
//! **is a Cloudflare zone id configuration or a per-call argument?**
//!
//! Nearly every Cloudflare endpoint is scoped `/zones/{zone_id}/…`. `[[config]]` could in principle
//! pin one zone per installed connector, the way `providers/zendesk.toml` pins `{subdomain}` into its
//! host — but `ConfigField::binds` (`crates/connector-spec/src/config.rs`) only ever reaches a
//! `{placeholder}` in `Connector::base_url`, through `endpoint.<variable>`, and Cloudflare's
//! `base_url` is one host shared by every zone with no such placeholder to bind. So this connector
//! declares `zone_id` as a required `params.path` argument on every zone-scoped operation, and the
//! only one that plausibly omits it is `cloudflare-zone-list` — the call that exists to *discover*
//! zone ids, so there is nothing yet to scope. `the_zone_id_is_a_per_call_argument_everywhere_but_zone_list`
//! is the test for that claim, and `no_config_field_binds_a_zone` is what stops a later edit from
//! quietly reintroducing a config-level zone as an unbound `endpoint.zone_id` the loader would refuse
//! outright, or a *bound* one this file's own header comment says was deliberately not chosen.
//!
//! The other half of the story is getting `risk`/`idempotency` right on the two operations where they
//! are least alike: a DNS record delete (destructive, and the vendor documents no repeat guarantee)
//! and a cache purge (genuinely idempotent by vendor behaviour, but `POST`, and
//! `crates/connector-flux/src/op.rs`'s `check_write_metadata` refuses `idempotency = "idempotent"` on
//! any `POST` outright — so it is declared `non_idempotent` not because that is true but because the
//! compiler will not accept the true answer, the same trade `providers/notion.toml` records for its
//! own `POST` reads). `the_dns_record_delete_is_destructive_and_not_claimed_idempotent` and
//! `the_cache_purge_is_high_risk_and_forced_non_idempotent_by_the_post_rule` are the tests for that.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector, HttpMethod, Idempotency, Risk};

/// `<repo root>/providers/cloudflare.toml`, derived from this crate's manifest directory so the test
/// is independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("cloudflare.toml")
}

fn cloudflare() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-169 ships the Cloudflare connector",
            path.display()
        )
    });
    provider::load("providers/cloudflare.toml", &source)
        .expect("providers/cloudflare.toml does not load")
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a connector_spec::Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?} in the curated set"))
}

/// The connector exists, loads through the real loader, and is the one C-169 describes.
#[test]
fn the_cloudflare_connector_loads() {
    let connector = cloudflare();

    assert_eq!(connector.id, "cloudflare");
    assert_eq!(connector.vendor, "Cloudflare");
    assert_eq!(
        connector.base_url, "https://api.cloudflare.com/client/v4",
        "one host shared by every zone and every account — the fact that forces zone_id to be an \
         argument rather than `[[config]]`"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// The curated set C-169 names, exactly: list zones, list/create/delete DNS records, purge cache.
#[test]
fn the_curated_operation_set_is_the_one_the_story_selected() {
    let connector = cloudflare();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "cloudflare-cache-purge",
            "cloudflare-dns-record-create",
            "cloudflare-dns-record-delete",
            "cloudflare-dns-record-list",
            "cloudflare-zone-list",
        ],
        "the curated set changed from the one C-169 names"
    );
}

/// **The acceptance assertion: `zone_id` is a per-call argument, not configuration.**
///
/// Every operation that reaches into a zone declares `zone_id` as a required path parameter;
/// `cloudflare-zone-list` is the sole exception, because it is the call that discovers zone ids in
/// the first place and has nothing yet to scope.
#[test]
fn the_zone_id_is_a_per_call_argument_everywhere_but_zone_list() {
    let connector = cloudflare();

    for operation in &connector.operations {
        let zone_param = operation
            .params
            .path
            .iter()
            .find(|param| param.name == "zone_id");

        if operation.id == "cloudflare-zone-list" {
            assert!(
                zone_param.is_none(),
                "`cloudflare-zone-list` declares a `zone_id` path parameter, but it is the operation \
                 that discovers zone ids — it has nothing to scope yet"
            );
            continue;
        }

        let zone_param = zone_param.unwrap_or_else(|| {
            panic!(
                "`{}` is scoped under /zones/{{zone_id}}/… but declares no `zone_id` path \
                 parameter — every zone-scoped operation must take it as a caller argument, because \
                 `[[config]]` has no binding that reaches an operation path (only \
                 `endpoint.<var>` in `base_url`)",
                operation.id
            )
        });
        assert!(
            zone_param.required,
            "`{}`'s `zone_id` must be required — an optional tenant id is an unaddressed request",
            operation.id
        );
        assert!(
            operation.path.contains("{zone_id}"),
            "`{}` declares a `zone_id` parameter but its path {:?} has no `{{zone_id}}` placeholder",
            operation.id,
            operation.path
        );
    }
}

/// The design decision's other half: no `[[config]]` field binds a zone at all. A config-level zone
/// would have to be `endpoint.<var>`, and `base_url` carries no zone placeholder for one to bind —
/// this pins the decision against a later edit re-adding it as a dead or refused binding.
#[test]
fn no_config_field_binds_a_zone() {
    let connector = cloudflare();
    for field in &connector.config {
        assert!(
            !field.name.to_ascii_lowercase().contains("zone") && !field.binds.contains("zone"),
            "config field `{}` (binds = {:?}) names a zone. `[[config]]` cannot pin a Cloudflare \
             zone: `endpoint.<var>` only reaches a `{{placeholder}}` in `base_url`, and this \
             connector's `base_url` is one host shared by every zone with no such placeholder",
            field.name,
            field.binds
        );
    }
}

/// The DNS record delete: destructive, and not claimed idempotent on a guess.
#[test]
fn the_dns_record_delete_is_destructive_and_not_claimed_idempotent() {
    let connector = cloudflare();
    let delete = op(&connector, "cloudflare-dns-record-delete");

    assert_eq!(delete.method, HttpMethod::Delete);
    assert_eq!(
        delete.risk,
        Risk::Destructive,
        "a DNS record delete has no undo route in the API — the only tier that stops flux's \
         approval gate for it"
    );
    assert_eq!(
        delete.idempotency,
        Idempotency::NonIdempotent,
        "Cloudflare answers a repeat delete with 404, not a repeat of the first 200, and documents \
         no idempotency guarantee — `idempotent` would be a claim this file cannot back"
    );
}

/// The cache purge: high risk (instantaneous, global, externally visible), and — the forced half of
/// the story — declared `non_idempotent` even though it is genuinely idempotent by vendor behaviour,
/// because `check_write_metadata` refuses `idempotency = "idempotent"` on any `POST`.
#[test]
fn the_cache_purge_is_high_risk_and_forced_non_idempotent_by_the_post_rule() {
    let connector = cloudflare();
    let purge = op(&connector, "cloudflare-cache-purge");

    assert_eq!(purge.method, HttpMethod::Post);
    assert_eq!(
        purge.risk,
        Risk::High,
        "a whole-zone purge is instantaneous, global and can spike origin load — 'a write a \
         reviewer would want to see first', not a limited-blast-radius medium write and not a \
         destructive one (the cache repopulates from origin)"
    );
    assert_eq!(
        purge.idempotency,
        Idempotency::NonIdempotent,
        "genuinely idempotent by Cloudflare's own behaviour, but declared non_idempotent because \
         `check_write_metadata` refuses `idempotency = idempotent` on a POST outright regardless of \
         vendor truth — see the provider file's header comment for the trade"
    );

    // The emitter's own refusal is the mechanism backing the claim above: declaring this operation
    // idempotent would fail to emit at all, which is exactly why it is not declared that way.
    let mut idempotent_purge = connector.clone();
    let purge_index = idempotent_purge
        .operations
        .iter()
        .position(|operation| operation.id == "cloudflare-cache-purge")
        .expect("the purge operation exists");
    idempotent_purge.operations[purge_index].idempotency = Idempotency::Idempotent;
    let attempt = emit_operation(&idempotent_purge, &idempotent_purge.operations[purge_index]);
    assert!(
        attempt.is_err(),
        "declaring the purge idempotent should be refused by check_write_metadata — if this now \
         emits, the compiler rule changed and the provider file's comment should be revisited rather \
         than this test silently passing"
    );
}

/// The purge body is the constant `purge_everything = true`, never a caller-supplied argument: there
/// is no selective purge in this connector, so nothing about the body varies per call.
#[test]
fn the_purge_body_is_a_constant_not_a_caller_supplied_argument() {
    let connector = cloudflare();
    let purge = op(&connector, "cloudflare-cache-purge");

    let field = purge
        .params
        .body
        .iter()
        .find(|param| param.name == "purge_everything")
        .expect("the purge always sends purge_everything");
    assert_eq!(
        field.schema.get("const"),
        Some(&serde_json::Value::Bool(true)),
        "purge_everything must be pinned with a JSON Schema `const`, the same mechanism \
         `providers/zendesk.toml` uses for `ticket.safe_update` — otherwise it becomes a required \
         argument a model has to guess"
    );

    let emitted = emit_operation(&connector, purge)
        .unwrap_or_else(|error| panic!("cloudflare-cache-purge is not emittable: {error}"));
    let signature = emitted.lines().next().expect("a declaration line");
    assert!(
        !signature.to_lowercase().contains("purge_everything"),
        "purge_everything must not appear in the emitted signature — it is a constant, not an \
         input:\n{signature}"
    );
}

/// Auth is one bearer API token, and `verify` is a genuine, parameter-free read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = cloudflare();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["cloudflare.api_token"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("cloudflare-zone-list"),
        "the `verify` operation is the Test-connection button and must be a read"
    );
    let verify = op(&connector, "cloudflare-zone-list");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.path.is_empty() && verify.params.query.is_empty(),
        "`verify` runs unattended whenever a settings page opens and must not require an argument \
         a human has not supplied yet"
    );
}

/// The connection-level configuration surface: the token, and **no realistic-looking example on
/// it** — a Cloudflare API Token is a 40-character shape secret scanners match on.
#[test]
fn the_token_is_configurable_and_carries_no_example_value() {
    let connector = cloudflare();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "api_token")
        .expect("the API token is the one thing a human must supply");

    assert!(field.secret, "an API token is a secret");
    assert_eq!(field.binds, "credential.cloudflare.api_token");
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "a field must be renderable: `label` and `help` are what a settings page shows"
    );
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning"
    );
}

/// Every Cloudflare operation emits Flux that parses, is canonical under flux's own formatter, and
/// loads as exactly one composite op — the C-11 gate, held against cloudflare specifically.
#[test]
fn every_cloudflare_operation_emits_an_analyzable_module() {
    let connector = cloudflare();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

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
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{}` loaded {}",
            operation.id,
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, operation.id);
    }
}

/// No cloudflare operation declares a query parameter — a deliberate curation choice recorded in the
/// provider file's header comment, not a workaround for the query-encoding gap (an integer page
/// number needs no percent-encoding). This is what keeps this file from asserting a `per_page` cap it
/// is not confident of.
#[test]
fn no_cloudflare_operation_declares_a_query_parameter() {
    let connector = cloudflare();
    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "operation `{}` declares query parameters, which the provider file's header comment \
             says this connector deliberately does not — pagination bounds this file is not \
             confident of should not be asserted",
            operation.id
        );
    }
}

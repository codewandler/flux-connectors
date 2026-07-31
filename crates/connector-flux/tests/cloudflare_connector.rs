//! `providers/cloudflare.toml` exists, emits analyzable Flux, and answers C-169's actual question —
//! **is a Cloudflare zone id configuration or a per-call argument?** — which C-187 re-opened by
//! making the first of the two expressible at all.
//!
//! Nearly every Cloudflare endpoint is scoped `/zones/{zone_id}/…`. C-169 asked whether that id is
//! configuration or a per-call argument and got the argument, because `ConfigField::binds` reached a
//! `{placeholder}` in `Connector::base_url` and nothing else — and Cloudflare's `base_url` is one
//! host shared by every zone, with no such placeholder to bind. C-169 recorded that if `[[config]]`
//! ever reached an operation path, this connector's operations "would drop the parameter in favour
//! of it".
//!
//! **C-187 reached it, and they did.** `zone_id` is now pinned by a `[[config]]` field
//! (`binds = "path.zone_id"`), is a parameter of no operation, and reaches the emitted module as a
//! substitutable placeholder — so one installed connection addresses one zone, and a model holding
//! it cannot address another. `cloudflare-zone-list` stays unscoped, because it is the call that
//! discovers the value an operator pins.
//! `the_zone_id_is_pinned_by_configuration_and_is_a_parameter_of_no_operation` is the test for that
//! claim, and `the_pinned_zone_reaches_the_module_as_a_substitutable_placeholder` is what stops a
//! later edit from turning the pin back into an argument, or into a literal committed here.
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
use connector_spec::{
    provider, Binding, Connector, HttpMethod, Idempotency, Level, Position, Risk,
};

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
        "one host shared by every zone and every account — the fact that puts the zone scope in the \
         operation path rather than in the address, and therefore out of reach of \
         `endpoint.<variable>`"
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

/// **The acceptance assertion: `zone_id` is an operator's pin, not a caller's argument** (C-187).
///
/// Every operation scoped into a zone carries `{zone_id}` in its path and declares **no** parameter
/// for it: the value is supplied once, when the connection is made. `cloudflare-zone-list` carries
/// no zone scope at all, because it is the call that discovers zone ids and therefore the one an
/// operator uses to *find* the value this connector then asks them to pin.
///
/// The half that makes this a pin rather than a default is the second loop: nothing declares
/// `zone_id` as a parameter anywhere, so a model holding this connector cannot address a zone the
/// operator did not choose — whatever the underlying token would permit.
#[test]
fn the_zone_id_is_pinned_by_configuration_and_is_a_parameter_of_no_operation() {
    let connector = cloudflare();

    let zone = connector
        .config_field("zone_id")
        .expect("`[[config]]` must ask for the zone this connection manages");
    assert_eq!(
        zone.binding(),
        Some(Binding::Request {
            position: Position::Path,
            name: "zone_id"
        })
    );
    assert_eq!(
        zone.level(),
        Some(Level::Connection),
        "a zone is one per tenant, not one per vendor — the level is derived from `binds`, never \
         authored"
    );
    assert!(
        !zone.secret,
        "a zone id is a public identifier an operator reads back off a settings page; marking it \
         secret would claim gating this repository does not provide"
    );
    assert!(
        zone.required,
        "a host refuses the whole request when a pinned value is missing, so an optional pin is a \
         connector that composes no URL"
    );

    for operation in &connector.operations {
        assert!(
            operation.params.path.iter().all(|p| p.name != "zone_id"),
            "`{}` declares `zone_id` as a path parameter. A value an operator pins at install time \
             and a caller may also pass is not pinned — the caller's wins, and the operator's \
             choice of zone becomes a suggestion",
            operation.id
        );
        if operation.id == "cloudflare-zone-list" {
            assert!(
                !operation.path.contains("{zone_id}"),
                "`cloudflare-zone-list` is the operation that discovers zone ids — it has nothing \
                 to scope yet"
            );
        } else {
            assert!(
                operation.path.contains("{zone_id}"),
                "`{}` is not scoped to the pinned zone; its path is {:?}",
                operation.id,
                operation.path
            );
        }
    }
}

/// **The pin reaches the emitted module as a placeholder, not as a value** — which is what keeps it
/// operator data rather than repository data.
///
/// The zone id is bound to a *string literal spelling its own placeholder*, exactly as a templated
/// `base_url` is (`base = "https://{subdomain}.zendesk.com"`). That is the mechanism and not a
/// resemblance: `connector-pack` reads a connector's configuration variables off the braces
/// surviving in its emitted literals, and substitutes a tenant's value into literals only. Nothing
/// in this repository holds a zone id, which is C-187's own constraint — a tenant id committed here
/// is the same category error as a credential value.
#[test]
fn the_pinned_zone_reaches_the_module_as_a_substitutable_placeholder() {
    let connector = cloudflare();
    let flux = emit_operation(&connector, op(&connector, "cloudflare-dns-record-list"))
        .expect("the operation must emit");

    assert!(
        flux.contains("zone_id = \"{zone_id}\""),
        "the pin must be bound as a literal carrying its placeholder:\n{flux}"
    );
    assert!(
        flux.contains("url = fmt(\"{base}/zones/{zone_id}/dns_records\")"),
        "the URL must interpolate the pinned symbol:\n{flux}"
    );
    assert!(
        flux.starts_with("op cloudflare-dns-record-list -> Any"),
        "the pinned value must not reach the declared parameter list:\n{flux}"
    );
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

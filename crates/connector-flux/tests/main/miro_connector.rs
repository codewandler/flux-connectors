//! `providers/miro.toml` exists, emits analyzable Flux, and answers the question C-183 was chosen
//! for: **is a shallow, non-recursive discriminated union expressible in this pipeline?**
//!
//! Miro's board items (`sticky_note`, `shape`, `text`, `frame`) are a discriminated union by `type`,
//! the same shape problem `providers/notion.toml`'s header comment records refusing for blocks — but
//! unlike a block, an item's children are never other items embedded in its own body: a frame *claims*
//! items through their own `parent.id`, it does not contain them recursively. Notion's refusal has two
//! independent parts and this connector answers each separately:
//!
//! 1. **"`JsonSchema` here has no `$ref` and no recursion, so that union is not expressible."** This
//!    reason does not apply to Miro at all — nothing here is recursive, so no `$ref` is ever needed.
//! 2. **"`body_schema` could take it as a free-form object, but that ships an untyped blob."** This
//!    reason would apply to a *write* that had to express the union in a request body — but Miro's own
//!    API never asks a caller to: each item type is created and updated through its own
//!    type-specific path (`/sticky_notes`, not a generic `/items` with a `type` field in the body), so
//!    the discriminator is resolved by the URL, not by this pipeline's schema. `the_write_side_never_
//!    declares_a_type_discriminator_body_field` is the test for that: no operation below ever declares
//!    a body field literally named or wired to `type`.
//!
//! The union survives only on the **read** side (`miro-board-item-list`, `miro-board-item-get`), where
//! Miro answers with a heterogeneous array/object it tags by `type`. And there it *is* expressible,
//! because `response_schema` (`connector_spec::ir::Operation::response_schema`) is a raw
//! `serde_json::Value` with no `BodyNode`/flat-parameter-list constraint — unlike `params.body`, which
//! is what `providers/notion.toml`'s comment is actually about. `oneOf` is even explicitly recognised
//! as informative by `crates/connector-spec/tests/response_schema_coverage.rs`'s own
//! `is_permissive`. `the_read_side_union_is_expressed_as_a_oneof_over_the_four_item_types` is the test
//! for that half.
//!
//! So the net finding, which this file exists to prove rather than assert in prose: **a shallow,
//! non-recursive union is expressible here — on the read side directly, as `oneOf`, and on the write
//! side vacuously, because the vendor's own routing dissolves it before this pipeline ever sees a
//! union to express.** That narrows Notion's recorded gap: recursion, not "union-ness" alone, is what
//! `JsonSchema` here cannot do.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{Connector, HttpMethod, Idempotency, Risk};

use crate::shipped_provider;

/// `<repo root>/providers/miro.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("miro.toml")
}

fn miro() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-183 ships the Miro connector",
            path.display()
        )
    });
    shipped_provider::load_definition("miro", &source)
        .expect("providers/miro.toml does not load")
        .connector
}

fn op<'a>(connector: &'a Connector, id: &str) -> &'a connector_spec::Operation {
    connector
        .operations
        .iter()
        .find(|operation| operation.id == id)
        .unwrap_or_else(|| panic!("no operation named {id:?} in the curated set"))
}

/// The four board-item variants this connector's union is over.
const ITEM_TYPES: [&str; 4] = ["sticky_note", "shape", "text", "frame"];

/// The connector exists, loads through the real loader, and is the one C-183 describes.
#[test]
fn the_miro_connector_loads() {
    let connector = miro();

    assert_eq!(connector.id, "miro");
    assert_eq!(connector.vendor, "Miro");
    assert_eq!(
        connector.base_url, "https://api.miro.com/v2",
        "one tenant-independent host; board_id is a per-call argument, not a config binding"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// The curated set: board discovery (also `verify`), the two generic item reads that carry the
/// union, and sticky-note create/update/delete as the one type-specific write surface exercised.
#[test]
fn the_curated_operation_set_is_the_one_this_story_selected() {
    let connector = miro();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "miro-board-item-get",
            "miro-board-item-list",
            "miro-board-list",
            "miro-sticky-note-create",
            "miro-sticky-note-delete",
            "miro-sticky-note-update",
        ],
        "the curated set changed from the one C-183 names"
    );
}

/// **The archetype's read-side half: the union is expressed as `oneOf` over the four item types**,
/// because `response_schema` is raw JSON with no flat-parameter-list constraint.
#[test]
fn the_read_side_union_is_expressed_as_a_oneof_over_the_four_item_types() {
    let connector = miro();

    for id in ["miro-board-item-get", "miro-board-item-list"] {
        let operation = op(&connector, id);
        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("`{id}` declares no response_schema at all"));

        // `miro-board-item-list` wraps its union in a `data` array; `miro-board-item-get` returns one
        // item directly. Either way, a `oneOf` naming all four item types must be reachable.
        let text = schema.to_string();
        assert!(
            text.contains("oneOf"),
            "`{id}`'s response_schema declares no `oneOf` — the discriminated union over \
             sticky_note/shape/text/frame is not expressed:\n{text}"
        );
        for item_type in ITEM_TYPES {
            assert!(
                text.contains(item_type),
                "`{id}`'s response_schema does not mention item type {item_type:?} — the union is \
                 not over all four variants:\n{text}"
            );
        }
    }
}

/// **The archetype's write-side half: no operation ever declares a body field discriminating on
/// `type`.** Miro resolves the variant through the URL (`/sticky_notes`, not `/items` + a body
/// `type`), so the union this connector's writes could have needed to express never reaches this
/// pipeline's schema at all — the write side of Notion's second refusal reason simply does not arise.
#[test]
fn the_write_side_never_declares_a_type_discriminator_body_field() {
    let connector = miro();
    for operation in &connector.operations {
        for field in &operation.params.body {
            assert_ne!(
                field.name, "type",
                "`{}` declares a body field named `type` — that would be re-inventing the \
                 discriminator this connector's write side never needs, because Miro routes each \
                 item type to its own path",
                operation.id
            );
            assert_ne!(
                field.wire.as_deref(),
                Some("type"),
                "`{}` wires a body field to `type` — same problem, via `wire` instead of `name`",
                operation.id
            );
        }
    }
}

/// Sticky-note create/update reach Miro's type-specific path, never the generic `/items` collection —
/// this is the mechanism that dissolves the union on the write side.
#[test]
fn sticky_note_writes_use_the_type_specific_path_not_the_generic_items_path() {
    let connector = miro();
    for id in [
        "miro-sticky-note-create",
        "miro-sticky-note-update",
        "miro-sticky-note-delete",
    ] {
        let operation = op(&connector, id);
        assert!(
            operation.path.contains("/sticky_notes"),
            "`{id}` must reach Miro's type-specific sticky_notes path, not a generic /items \
             collection: {:?}",
            operation.path
        );
    }
}

/// No request body anywhere declares an array — this connector never needed C-168/C-185's gap
/// (`BodyNode` builds nested objects via `wire` and never arrays). A single sticky-note create/update
/// is a plain nested object, so this connector does not need the fix C-185 tracks.
#[test]
fn no_body_field_declares_an_array_this_connector_does_not_need_c_185() {
    let connector = miro();
    for operation in &connector.operations {
        for field in &operation.params.body {
            assert_ne!(
                field.schema.get("type").and_then(|value| value.as_str()),
                Some("array"),
                "`{}`'s body field `{}` declares an array — this connector was designed around \
                 avoiding exactly that (see C-185); if a real Miro operation needs one, exclude it \
                 and cite C-185 rather than working around the gap",
                operation.id,
                field.name
            );
        }
    }
}

/// The sticky-note update is a `PATCH` that is genuinely safe to repeat by vendor behaviour — it
/// sets absolute values, never a delta — and since C-186 it says so.
///
/// It used to declare `non_idempotent` with a comment instructing whoever landed C-186 to come back
/// and fix it. This is that fix, and the value it lands on is `conditional`, not `idempotent`:
/// `flux_spec::coherence` reserves `idempotent` for something stronger — it licenses flux's op cache
/// to serve a stored result *instead of executing* — while naming `conditional` as the escape hatch
/// for exactly this. Both refusals are asserted below rather than assumed.
#[test]
fn the_sticky_note_update_is_conditional_and_states_its_condition() {
    let connector = miro();
    let update = op(&connector, "miro-sticky-note-update");

    assert_eq!(update.method, HttpMethod::Patch);
    assert_eq!(
        update.idempotency,
        Idempotency::Conditional,
        "the request carries the note's whole content as an absolute value, so re-sending it lands \
         in one state — `non_idempotent` was the compiler's answer, never Miro's (C-186)"
    );
    let condition = update
        .repeatability_condition()
        .expect("the update states the condition under which repeating it is safe");
    assert!(
        condition.contains("absolute value") && condition.contains("delta"),
        "the condition must name the vendor behaviour it rests on — an absolute value rather than \
         a delta — since that is the whole difference between this PATCH and one that increments a \
         counter: {condition:?}"
    );

    // The description must no longer explain this repository's compiler to a model. It carried a
    // sentence about `check_write_metadata` — a fact about the build, in the one string that
    // reaches a model as its tool contract (`AGENTS.md`: "`description` is not UI copy") — and once
    // the rule changed, that sentence was false in a shipped artifact.
    assert!(
        !update.description.contains("check_write_metadata")
            && !update.description.contains("non_idempotent"),
        "the model-facing description must describe Miro, not the emitter: {:?}",
        update.description
    );

    let index = connector
        .operations
        .iter()
        .position(|operation| operation.id == "miro-sticky-note-update")
        .expect("the update operation exists");

    // `idempotent` on a PATCH stays refused outright — C-186 did not buy that.
    let mut over_claiming = connector.clone();
    over_claiming.operations[index].idempotency = Idempotency::Idempotent;
    // Cleared so that only `WriteDeclaredIdempotent` can refuse this — see the note in
    // `cloudflare_connector.rs` for the false-green this avoids.
    over_claiming.operations[index].repeatable_because = None;
    assert!(
        emit_operation(&over_claiming, &over_claiming.operations[index]).is_err(),
        "`idempotent` on this PATCH must still be refused; the escape C-186 added is `conditional`"
    );

    // And `conditional` without its condition is refused too, which is the rule C-186 added.
    let mut unstated = connector.clone();
    unstated.operations[index].repeatable_because = None;
    assert!(
        emit_operation(&unstated, &unstated.operations[index]).is_err(),
        "a `conditional` PATCH that states no condition must be refused — otherwise the claim is \
         back to meaning nothing, which is where six shipped operations sat before C-186"
    );
}

/// The sticky-note delete: destructive, and not claimed idempotent on a guess (Miro documents no
/// repeat-delete guarantee, the same gap `providers/cloudflare.toml` records for its own DNS delete).
#[test]
fn the_sticky_note_delete_is_destructive_and_not_claimed_idempotent() {
    let connector = miro();
    let delete = op(&connector, "miro-sticky-note-delete");

    assert_eq!(delete.method, HttpMethod::Delete);
    assert_eq!(delete.risk, Risk::Destructive);
    assert_eq!(delete.idempotency, Idempotency::NonIdempotent);
}

/// Auth is one bearer access token, and `verify` is a genuine, parameter-free read: listing the
/// boards the token can see needs no board id, the same role `cloudflare-zone-list` plays for zones.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_token() {
    let connector = miro();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(credentials, ["miro.access_token"]);

    assert_eq!(connector.verify.as_deref(), Some("miro-board-list"));
    let verify = op(&connector, "miro-board-list");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.path.is_empty() && verify.params.query.is_empty(),
        "`verify` runs unattended whenever a settings page opens and must not require an argument a \
         human has not supplied yet — this is why board discovery, not an item read, is `verify`"
    );
}

/// Every board-scoped operation (all but `miro-board-list`) declares `board_id` as a required path
/// parameter — the same schema-driven reasoning `providers/cloudflare.toml` gives `zone_id`: this
/// connector's `base_url` is one host with no board placeholder to bind, so `[[config]]` has no
/// vocabulary to pin one board at install time.
#[test]
fn board_id_is_a_per_call_argument_everywhere_but_board_list() {
    let connector = miro();
    for operation in &connector.operations {
        let board_param = operation
            .params
            .path
            .iter()
            .find(|param| param.name == "board_id");
        if operation.id == "miro-board-list" {
            assert!(
                board_param.is_none(),
                "`miro-board-list` is the discovery call and has nothing yet to scope"
            );
            continue;
        }
        let board_param = board_param
            .unwrap_or_else(|| panic!("`{}` declares no `board_id` path parameter", operation.id));
        assert!(
            board_param.required,
            "`{}`'s board_id must be required",
            operation.id
        );
    }
    for field in &connector.config {
        assert!(
            !field.binds.contains("board"),
            "config field `{}` binds a board id; `base_url` carries no board placeholder for \
             `endpoint.<var>` to bind",
            field.name
        );
    }
}

/// The connection-level configuration surface: the access token, secret, and carrying no
/// realistic-looking example.
#[test]
fn the_access_token_is_configurable_and_carries_no_example_value() {
    let connector = miro();
    let field = connector
        .config
        .iter()
        .find(|field| field.name == "access_token")
        .expect("the access token is the one thing a human must supply");

    assert!(field.secret);
    assert_eq!(field.binds, "credential.miro.access_token");
    assert!(!field.label.is_empty() && !field.help.is_empty());
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning"
    );
}

/// Every Miro operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op.
#[test]
fn every_miro_operation_emits_an_analyzable_module() {
    let connector = miro();
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
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
    }
}

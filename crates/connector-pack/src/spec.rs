//! A catalogue operation, projected onto a flux [`ToolSpec`].
//!
//! # Where each field comes from, and why not from somewhere else
//!
//! The catalogue entry carries the operation's Flux **verbatim** — `Operation::flux` is byte for
//! byte the `op` declaration `connectors/<provider>.flux` ships, and
//! `connector-flux`'s metadata block is documented as *"the `ToolSpec` surface flux exposes to a
//! model, and the fields its approval gate reads"*. So the declaration already **is** a `ToolSpec`
//! in Flux's own spelling, and this module reads it back rather than re-deriving one from the
//! catalogue's flattened columns.
//!
//! That is the anti-drift choice, and it is the risk [the design](../../../docs/designs/connector-tool-pack.md)
//! names first: two surfaces generated from one IR can drift into disagreeing about the same
//! operation. Reading the shipped artifact makes the pack's answer *the module's answer* by
//! construction, so there is one fewer pair of things that must be kept in step. C-117 adds the
//! differential test for the half this does not cover — the request.
//!
//! | `ToolSpec` field | source |
//! |---|---|
//! | `name` | [`crate::dotted_name`] over `Operation::id` — the one thing this repository derives |
//! | `description` | the declaration's `description` |
//! | `input_schema` | the declaration's parameter list, through flux's own `OpSpec::input_schema` |
//! | `output_schema` | the declaration's return type; `Any` is `None` |
//! | `effects` | the declaration's `effects` |
//! | `risk` | the declaration's `risk` |
//! | `idempotency` | the declaration's `idempotency` |
//! | `access` | the host capability flux **requires** for each declared effect — see below |
//! | `group` | **`None`** — nothing in the IR names a tool group |
//!
//! ## `description` is the declaration's, not the catalogue column's
//!
//! They are not the same string, deliberately. `Operation::description` is the raw one-line
//! summary; the emitter extends it with the vendor's error envelope when the connector declares
//! one, because `http.request` returns one flat string and the model has to be told where the
//! error message is. The extended text is what a model gets from the module, so it is what a model
//! must get from the pack — otherwise one operation tells the model two different things depending
//! on which surface a host installed it through. The prefix relationship between the two is
//! asserted over the whole catalogue by `the_description_extends_the_catalogue_summary_and_nothing_else`
//! below, so the extension can only ever be the emitter's and never this crate's.
//!
//! ## `input_schema` is the declaration's, and the catalogue composes its own
//!
//! There is a second answer to "what does this operation receive":
//! `connector_spec::Operation::input_schema` composes one from the IR's declared parameters, and it
//! is what `web/public/catalog.json` publishes (C-125). This one stays the declaration's, and the
//! two are held together by a test rather than merged, because neither can be the other:
//!
//! - **This crate cannot key by the IR's names.** A composite op declares *symbols*, so babelforce's
//!   `time.start` is `time_start` here — and the name→symbol mapping lives in `connector-flux`,
//!   which this crate deliberately does not depend on (its input is the catalogue, not the loader).
//!   Keying a `ToolSpec` by the IR's spelling would hand a model an argument name
//!   [`crate::request`] then refuses.
//! - **This crate cannot key by the vendor's `required`.** Flux has no optional composite-op
//!   parameter and [`crate::request::build`] refuses a call that omits one, so *every* declared
//!   parameter is required here. The composed schema states what the **vendor** requires. Both are
//!   true of the same operation; they answer different questions.
//!
//! `crates/connector-flux/tests/input_schema_agreement.rs` asserts over every shipped operation that
//! the two describe the same parameter set modulo that symbol mapping, and that the composed
//! `required` is a subset of it. That is the anti-drift device here, in the same spirit as reading
//! the declaration in the first place.
//!
//! ## `access` is derived from `effects`, because flux refuses the spec otherwise
//!
//! A `CompositeOpMeta` has no access field, so there is nothing in the declaration to copy — and
//! the first instinct, leaving `access` empty to match, does not survive contact with flux:
//!
//! ```text
//! invalid authority contract for `zendesk.test` from `connector-pack:zendesk`:
//! tool `zendesk.test` declares a network effect without network, process, browser, or provider access
//! ```
//!
//! `try_register_from` calls `authority_requirements` on every tool it accepts, and flux's default
//! adapter **refuses** an effect with no host capability that could carry it. So `access` is not a
//! free field to leave blank; it is the other half of `effects`, and [`access_for`] derives it from
//! the effects the declaration states — nothing more. That is a mapping owned by flux's checker,
//! reproduced against it, not a policy decision made here.
//!
//! Where flux accepts several carriers for one effect, the **direct** one is chosen: a connector
//! reaches the vendor over HTTP itself, so a network effect is [`AccessKind::Network`], never the
//! `Process` carrier that exists for tools which shell out to a program that reaches the network on
//! their behalf.
//!
//! ## The gate is not in the spec, and that is worth saying here
//!
//! `access` and `effects` are what flux's *authority* checker reads. They are not the network gate:
//! `Tool::permission_subjects` and `Tool::intents` are, and both live on [`crate::Operation`]
//! because both depend on the **call's params**, which a spec never sees. Since C-115 `execute`
//! delegates to `http.request` by calling its `execute` directly, which bypasses
//! `Executor::dispatch` and so consults neither of `http.request`'s own — the projected Tool
//! declares them itself, and `tests/network_gate.rs` holds it to that over every shipped operation.

use flux_lang::ast::TypeRef;
use flux_lang::opspec::{OpSpec, Param};
use flux_lang::program::CompositeOpDecl;
use flux_spec::{AccessKind, Effect, ToolSpec};

use crate::{dotted_name, Error};

/// Project one catalogue operation onto the spec a host registers it by.
///
/// # Errors
///
/// Returns [`Error`] when the id has no dotted tool name, when the entry's embedded Flux is not the
/// single `op` declaration a catalogue rendering is, or when the resulting spec is one flux's own
/// authority checker will not register. The last two cannot happen for a catalogue this repository
/// generated — `crates/catalog/tests/embedded_operations.rs` is the gate for the parse, and every
/// generated op declares `["network"]` alone — so they are reported as the corrupt-input cases they
/// would be rather than unwrapped.
pub fn project(operation: &catalog::Operation) -> Result<ToolSpec, Error> {
    let declaration = declaration_of(operation.id, operation.flux)?;
    project_declaration(operation.id, &declaration)
}

/// **Whether this catalogue operation reaches a model as an LLM tool** (C-413).
///
/// The catalogue states this **positively**, on every operation, in the `expose` line of the embedded
/// Flux — flux's formatter writes `expose true` or `expose false` and never elides either
/// (`flux_lang::format`). So a consumer asking this question gets an answer rather than an inference
/// from an absence, which is the distinction
/// [C-235](../../../docs/stories/C-235-the-catalogue-cannot-say-an-operation-is-public.md) records as
/// missing for credentials and the reason this is a function here rather than a `bool` a caller
/// derives for itself.
///
/// **Unexposed is not uncatalogued.** An operation this returns `false` for is still in the
/// manifest's `operations` list, still in `catalog.json`, still in the embedded catalogue, and
/// [`crate::Operation::project`] still builds it into something [`crate::Operation::build_request`]
/// and [`Rehearsal`](crate::Rehearsal) can drive. The single consequence is that [`crate::pack`]
/// does not register it, so no model is handed it as a tool.
///
/// # Errors
///
/// [`Error::Unparsable`], [`Error::NotOneOperation`] or [`Error::Mismatched`] for an entry whose
/// embedded Flux is not the single `op` declaration a catalogue rendering is — the same corrupt-input
/// cases [`project`] reports, and unreachable for a catalogue this repository generated.
pub fn is_exposed(operation: &catalog::Operation) -> Result<bool, Error> {
    declares_exposure(operation.id, operation.flux)
}

/// [`is_exposed`], over the id and Flux rather than the entry.
///
/// The split is the one [`declaration_of`] already documents and takes for the same reason:
/// `catalog::Operation` is `#[non_exhaustive]`, so no synthetic entry can be built outside the
/// `catalog` crate, and nothing shipped is unexposed yet. **This is the exact predicate
/// [`crate::pack`] branches on**, so testing it here is testing that branch — which is otherwise
/// unreachable until a provider declares `expose = false`.
pub(crate) fn declares_exposure(id: &str, flux: &str) -> Result<bool, Error> {
    Ok(declaration_of(id, flux)?.meta.expose)
}

/// [`project`], over a declaration already parsed.
///
/// The split exists because [`crate::Operation`] needs the declaration itself — C-115 builds the
/// request by evaluating it — and parsing the same Flux twice to get two views of it is one parse
/// that could disagree with the other.
pub(crate) fn project_declaration(
    id: &str,
    declaration: &CompositeOpDecl,
) -> Result<ToolSpec, Error> {
    let name = dotted_name(id).map_err(|source| Error::Name {
        operation: id.to_owned(),
        source,
    })?;

    // Schema projection is flux's, not ours: `OpSpec::lower` is the function flux itself uses to
    // put a typed op contract in front of a model, including `type_ref_to_schema` and the
    // `Any` → `None` output rule. Re-deriving either here would be a second implementation of a
    // mapping that has to agree with flux's exactly.
    let mut spec = OpSpec {
        name,
        description: declaration.meta.description.clone(),
        inputs: declaration
            .params
            .iter()
            .map(|param| Param {
                name: param.name.to_string(),
                ty: param.ty.clone(),
                // A composite op declaration has no optional-parameter concept, so every declared
                // parameter is required. Marking any of them optional would be this crate deciding
                // something the declaration does not say.
                optional: false,
            })
            .collect(),
        output: declaration.returns.clone().unwrap_or(TypeRef::Any),
        // `OpSpec::effects` is the *semantic* tier (`money`, `delete`, `send_external`), which a
        // composite declaration does not carry — `CompositeOpMeta::effects` is already the
        // host-resource `flux_spec::Effect` a `ToolSpec` wants. Lowering an empty semantic list and
        // then copying the declared host effects across is what keeps this a copy rather than a
        // guess at which semantic effect a `network` host effect came from.
        effects: Vec::new(),
        risk: declaration.meta.risk,
        idempotency: declaration.meta.idempotency,
    }
    .lower();

    spec.effects.clone_from(&declaration.meta.effects);
    spec.access = access_for(&spec.effects);

    // The same check `try_register_from` will apply, applied here so the diagnostic names the
    // **catalogue operation** rather than only the tool name a host sees at startup. Unreachable
    // for a declaration this repository emits — every generated op declares `["network"]` alone —
    // and it stays that way by refusing rather than by inventing an access kind that would satisfy
    // flux while claiming a capability the connector does not have.
    flux_runtime::authority_requirements_from_declaration(&spec, &[], &[]).map_err(|error| {
        Error::Unregistrable {
            operation: id.to_owned(),
            message: error.to_string(),
        }
    })?;

    Ok(spec)
}

/// The host capabilities flux requires in order to accept a spec declaring `effects`.
///
/// Deliberately an exhaustive match: `flux_spec::Effect` is a closed enum, so a variant added
/// upstream becomes a compile error here rather than a registration that fails at a host's startup
/// with a message about an "invalid authority contract".
///
/// The pairs are read off `flux_runtime::authority_requirements_from_declaration`, which is the
/// function that will reject the result. `Read` and `Write` need no carrier of their own — `Write`
/// is separately gated as `operation.mutate` once any carrier access is present.
fn access_for(effects: &[Effect]) -> Vec<AccessKind> {
    let mut access = Vec::new();
    for effect in effects {
        let required = match effect {
            Effect::Read | Effect::Write => None,
            Effect::Network => Some(AccessKind::Network),
            Effect::Process => Some(AccessKind::Process),
            Effect::Browser => Some(AccessKind::Browser),
            Effect::Filesystem => Some(AccessKind::Filesystem),
            Effect::LocalSystem => Some(AccessKind::LocalSystem),
        };
        if let Some(kind) = required {
            if !access.contains(&kind) {
                access.push(kind);
            }
        }
    }
    access
}

/// The single composite op declaration a catalogue rendering embeds.
///
/// Takes the id and the Flux separately rather than the entry, so the corrupt-input paths can be
/// tested without fabricating a `catalog::Operation` — the type is `#[non_exhaustive]`, and rightly
/// so.
pub(crate) fn declaration_of(id: &str, flux: &str) -> Result<CompositeOpDecl, Error> {
    let module =
        flux_lang::program::Module::parse_str(flux).map_err(|error| Error::Unparsable {
            operation: id.to_owned(),
            message: error.to_string(),
        })?;

    let ops = module
        .program()
        .map(|program| program.ops.as_slice())
        .unwrap_or_default();

    let [declaration] = ops else {
        return Err(Error::NotOneOperation {
            operation: id.to_owned(),
            found: ops.len(),
        });
    };

    if declaration.name != id {
        return Err(Error::Mismatched {
            operation: id.to_owned(),
            declared: declaration.name.clone(),
        });
    }

    Ok(declaration.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalog::OperationKey;

    fn operation(id: &str) -> &'static catalog::Operation {
        catalog::operation(OperationKey::id(id))
            .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"))
    }

    /// One operation, every field, against the declaration it was projected from. This is the unit
    /// the integration test then applies to all 97.
    #[test]
    fn every_field_comes_from_the_declaration() {
        let entry = operation("zendesk-ticket-comment-add");
        let declaration =
            declaration_of(entry.id, entry.flux).expect("the entry embeds one declaration");
        let spec = project(entry).expect("the entry projects");

        assert_eq!(spec.name, "zendesk.ticket.comment.add");
        assert_eq!(spec.description, declaration.meta.description);
        assert_eq!(spec.risk, declaration.meta.risk);
        assert_eq!(spec.idempotency, declaration.meta.idempotency);
        assert_eq!(spec.effects, declaration.meta.effects);
        assert_eq!(spec.effects, vec![flux_spec::Effect::Network]);
    }

    /// The declared parameters become a real JSON Schema, with their declared types. An empty
    /// `{"type": "object"}` would satisfy "the spec has an input schema" and tell a model nothing.
    #[test]
    fn the_declared_parameters_become_the_input_schema() {
        let spec = project(operation("zendesk-ticket-comment-add")).expect("projects");

        let properties = spec.input_schema["properties"]
            .as_object()
            .expect("an object schema declares properties");
        assert_eq!(
            properties["ticket_id"],
            serde_json::json!({"type": "number"})
        );
        assert_eq!(
            properties["updated_stamp"],
            serde_json::json!({"type": "string"})
        );
        assert_eq!(properties["body"], serde_json::json!({"type": "string"}));
        assert_eq!(properties["public"], serde_json::json!({"type": "boolean"}));

        let required = spec.input_schema["required"]
            .as_array()
            .expect("an object schema declares its required properties");
        assert_eq!(required.len(), properties.len());
    }

    /// A field the entry has no value for is absent, never invented. Every generated op returns
    /// `Any` and none names a tool group, so both are `None`.
    #[test]
    fn a_field_the_declaration_lacks_is_absent_rather_than_invented() {
        let spec = project(operation("zendesk-ticket-show")).expect("projects");

        assert_eq!(spec.output_schema, None);
        assert_eq!(spec.group, None);
    }

    /// `access` follows `effects`, and only `effects`. An operation declaring a network effect gets
    /// network access because flux refuses the spec otherwise — and nothing else, because nothing
    /// else was declared.
    #[test]
    fn access_carries_the_declared_effects_and_no_more() {
        let spec = project(operation("zendesk-ticket-show")).expect("projects");

        assert_eq!(spec.effects, vec![Effect::Network]);
        assert_eq!(spec.access, vec![AccessKind::Network]);
    }

    /// Every shipped operation's spec passes the checker `try_register_from` will apply. This is
    /// the assertion that matters: it is driven by the real declarations rather than by effect sets
    /// imagined here, and it is what caught `access: Vec::new()` being wrong in the first place.
    #[test]
    fn every_shipped_operation_satisfies_fluxs_authority_checker() {
        for entry in catalog::operations() {
            let spec = project(entry).unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
            flux_runtime::authority_requirements_from_declaration(&spec, &[], &[])
                .unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
        }
    }

    /// Each effect that flux gates on a *carrier capability* derives that capability, and only it.
    ///
    /// `Read` and `Write` are absent on purpose: flux gates `Write` on a typed write **resource**
    /// (filesystem or local-system), not on a carrier, so there is no access kind this mapping
    /// could add for a bare `Write` that would not be a claim to a capability the declaration never
    /// made. Such a declaration is refused by [`Error::Unregistrable`](crate::Error::Unregistrable)
    /// instead.
    #[test]
    fn each_carrier_effect_derives_the_capability_flux_names_for_it() {
        for (effect, kind) in [
            (Effect::Network, AccessKind::Network),
            (Effect::Process, AccessKind::Process),
            (Effect::Browser, AccessKind::Browser),
            (Effect::Filesystem, AccessKind::Filesystem),
            (Effect::LocalSystem, AccessKind::LocalSystem),
        ] {
            assert_eq!(access_for(&[effect]), vec![kind], "for {effect:?}");
        }

        assert!(access_for(&[Effect::Read]).is_empty());
        assert!(access_for(&[Effect::Write]).is_empty());
    }

    /// A repeated effect must not repeat its access kind.
    #[test]
    fn a_repeated_effect_does_not_repeat_its_access() {
        assert_eq!(
            access_for(&[Effect::Network, Effect::Network]),
            vec![AccessKind::Network]
        );
    }

    /// The projected description is the catalogue's summary, extended exactly as the module
    /// extends it — so the two surfaces cannot describe one operation differently, and the
    /// extension cannot be something this crate made up.
    #[test]
    fn the_description_extends_the_catalogue_summary_and_nothing_else() {
        for entry in catalog::operations() {
            let spec = project(entry).unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));
            assert!(
                spec.description.starts_with(entry.description),
                "`{}` describes itself as `{}`, which does not begin with the catalogue's `{}`",
                entry.id,
                spec.description,
                entry.description
            );
        }
    }

    /// The catalogue's own typed columns and the Flux it embeds must agree, or a caller deciding
    /// whether to run an operation could get two answers. `crates/catalog` pins this from its side;
    /// pinning it here too is what lets the projection read the declaration without the columns
    /// silently becoming a second, drifting source.
    #[test]
    fn the_projected_metadata_agrees_with_the_catalogue_columns() {
        for entry in catalog::operations() {
            let spec = project(entry).unwrap_or_else(|error| panic!("`{}`: {error}", entry.id));

            // Compared through serde because that is the spelling both sides agree on: flux's
            // enums rename to snake_case on the wire, and `catalog`'s mirrors return exactly the
            // string the embedded Flux declares.
            assert_eq!(
                serde_json::to_value(spec.risk).expect("risk serializes"),
                serde_json::json!(entry.risk.as_str()),
                "`{}` declares one risk and tabulates another",
                entry.id
            );
            assert_eq!(
                serde_json::to_value(spec.idempotency).expect("idempotency serializes"),
                serde_json::json!(entry.idempotency.as_str()),
                "`{}` declares one idempotency and tabulates another",
                entry.id
            );
        }
    }

    /// A rendering that is not exactly one `op` is reported, not unwrapped. This is the
    /// corrupt-catalogue path, and it must name the entry rather than panic inside a host's
    /// registration call.
    #[test]
    fn a_rendering_that_is_not_one_operation_is_reported() {
        assert!(matches!(
            declaration_of("zendesk-ticket-show", ""),
            Err(Error::NotOneOperation { found: 0, .. })
        ));
    }

    /// A rendering declaring a different operation than the entry claims is a corrupt catalogue,
    /// and registering it would put a tool under a name derived from one id and a contract taken
    /// from another.
    #[test]
    fn a_rendering_of_a_different_operation_is_reported() {
        let entry = operation("zendesk-ticket-show");
        assert!(matches!(
            declaration_of("zendesk-ticket-hide", entry.flux),
            Err(Error::Mismatched { .. })
        ));
    }

    /// **The predicate [`crate::pack`] branches on, in both directions** (C-413).
    ///
    /// Nothing shipped declares `expose = false`, so the `false` arm of that branch is unreachable
    /// from the real catalogue and would otherwise be untested until a provider first used the
    /// feature — a filter nobody had ever seen remove anything. The entry's own emitted Flux is
    /// edited here rather than a fixture being invented, so what is tested is a real rendering with
    /// one token changed.
    #[test]
    fn declares_exposure_reads_both_states_from_the_rendering() {
        let entry = operation("zendesk-ticket-show");
        assert!(
            entry.flux.contains("expose true"),
            "the shipped rendering must state its exposure for this test to change it"
        );

        assert!(declares_exposure(entry.id, entry.flux).expect("it reads"));

        let unexposed = entry.flux.replace("expose true", "expose false");
        assert!(!declares_exposure(entry.id, &unexposed).expect("it reads"));
    }

    /// A rendering that states no `expose` at all reads as **exposed**, because flux's
    /// `CompositeOpMeta::default()` is `true`.
    ///
    /// This is the failure direction that matters: the one way this can go wrong by accident is a
    /// rendering losing its `expose` line, and the consequence of that is a tool staying visible
    /// rather than silently vanishing from every host that installs it.
    #[test]
    fn a_rendering_stating_no_exposure_fails_open() {
        let entry = operation("zendesk-ticket-show");
        let silent: String = entry
            .flux
            .lines()
            .filter(|line| !line.trim_start().starts_with("expose "))
            .map(|line| format!("{line}\n"))
            .collect();

        assert!(!silent.contains("expose"), "the fixture must drop the line");
        assert!(declares_exposure(entry.id, &silent).expect("it reads"));
    }
}

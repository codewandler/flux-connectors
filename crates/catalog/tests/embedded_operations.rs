//! **Every embedded operation is valid Flux on its own** — the C-11 gate applied per operation
//! rather than per provider module.
//!
//! `connector-flux`'s `shipped_modules.rs` makes the same three assertions, and they are not the
//! same claim. That file tests what the *emitter produces right now*; this one tests the text that
//! is actually **compiled into this crate**, which is what a consumer receives. Between the two
//! sits a generated, committed artifact that can be stale, hand-edited or truncated, and every one
//! of those failures is invisible to a test that re-emits.
//!
//! Per operation rather than per module also matters on its own terms. The catalog's unit is one
//! `op`, so a consumer takes one declaration and hands it to flux; a declaration that only parses
//! in the company of its siblings would be one nobody could use that way. Each rendering is
//! therefore parsed, formatter-checked and *loaded* by itself.
//!
//! The staleness half of the story lives next door in
//! `crates/connector-cli/tests/catalog_artifacts.rs`, which recomputes these bytes from
//! `providers/*.toml`; this file needs no filesystem access at all except for the two tests that
//! check the embedded tables against the repository — the provider index in `src/generated.rs`, and
//! the sizes of the catalog, which are derived from `providers/` and `ops/` rather than written down
//! (C-54).

use catalog::{Operation, OperationKey, ProviderKey};

/// Every operation in the catalog, as a flat list.
fn all() -> Vec<&'static Operation> {
    catalog::providers()
        .iter()
        .flat_map(|provider| provider.operations.iter())
        .collect()
}

/// An empty catalog would satisfy every `for` loop below without saying a word.
///
/// Both sizes are **derived from the repository** rather than written down (C-54). A total is only a
/// sum: `6` and `38` were two more copies of the shipped inventory, and a copy that has to be edited
/// by hand is one that gets edited on some branches and not others — three of the C-51/52/53 merge
/// conflicts were exactly these two numbers, computed against different baselines and unresolvable by
/// taking either side. A per-provider *curated* count is a different thing and stays explicit; it
/// lives in `connector-spec`'s `operation_selection_stays_curated`.
///
/// What the derived form still catches is everything the constants caught except the size itself: the
/// catalog is non-empty, it holds one provider per `providers/*.toml`, and it embeds one operation per
/// committed rendering under `ops/`. A provider or an operation dropped from the embedded tables fails
/// here; the *set* is pinned name for name by
/// [`the_provider_list_matches_the_repository`] below.
#[test]
fn the_catalog_is_not_empty() {
    let providers = shipped_providers();
    let renderings = committed_renderings();

    assert!(
        !providers.is_empty() && !renderings.is_empty(),
        "the repository ships {} providers and {} renderings; a check against an empty tree proves \
         nothing",
        providers.len(),
        renderings.len()
    );

    assert_eq!(
        catalog::providers().len(),
        providers.len(),
        "the catalog carries {} providers, but `providers/` holds {} ({providers:?}) — run \
         `cargo run -p connector-cli -- build`",
        catalog::providers().len(),
        providers.len()
    );
    assert_eq!(
        all().len(),
        renderings.len(),
        "the catalog embeds {} operations, but `crates/catalog/ops/` holds {} committed renderings \
         — a rendering that no generated table includes is one no consumer can reach",
        all().len(),
        renderings.len()
    );
}

/// Every tag crossing the dependency-free catalogue seam is still one Flux itself recognizes.
#[test]
fn every_semantic_effect_is_a_flux_flow_effect() {
    for operation in all() {
        for effect in operation.semantic_effects {
            assert!(
                flux_lang::ast::FlowEffect::from_tag(effect).is_some(),
                "`{}` carries unknown semantic effect `{effect}`",
                operation.id
            );
        }
    }
}

/// Semantic effects travel beside the emitted host effect; they never replace or broaden it.
#[test]
fn every_shipped_operation_keeps_its_network_host_effect() {
    for operation in all() {
        assert!(
            operation.flux.contains("  effects [\"network\"]\n"),
            "`{}` no longer declares exactly the network host effect:\n{}",
            operation.id,
            operation.flux
        );
    }
}

/// The known money-moving Stripe writes are held at the shipped-catalogue boundary, not merely in
/// provider TOML that a stale generated table could fail to carry.
#[test]
fn every_known_money_moving_write_declares_it() {
    for id in [
        "stripe-payment-intent-capture",
        "stripe-charge-refund-create",
    ] {
        let operation = catalog::operation(OperationKey::id(id))
            .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
        assert!(
            operation.semantic_effects.contains(&"money"),
            "`{id}` moves money but carries {:?}",
            operation.semantic_effects
        );
    }
}

/// Channel routing metadata must be queryable without reparsing the declaration JSON.
///
/// Slack's Socket Mode binding is the shipped positive case: `event_id` identifies one delivery
/// across retries, so dropping it from the embedded projection would make a generic host unable to
/// apply the declaration's dedupe contract.
#[test]
fn channel_delivery_id_is_part_of_the_embedded_projection() {
    let slack = catalog::provider(ProviderKey::id("slack")).expect("Slack is shipped");
    let socket = slack.channel("socket").expect("Socket Mode is shipped");

    assert_eq!(
        socket.delivery_id,
        Some(catalog::Selector {
            source: "body",
            name: "event_id",
        })
    );
}

/// The provider ids the repository ships, from `providers/*.toml`.
///
/// The one filesystem read this crate's tests perform, shared by the two tests that check the
/// embedded tables against the repository. `catalog` itself never touches the filesystem — that is
/// its contract in AGENTS.md — and neither does anything here outside these helpers.
fn shipped_providers() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the repository's providers/ directory is readable")
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// Every committed per-operation rendering, as `<provider>/<operation>.flux`.
///
/// `ops/` also carries a `README.md`, and nothing but a `.flux` file under a provider directory is a
/// rendering, so both are filtered rather than assumed away.
fn committed_renderings() -> Vec<String> {
    let ops = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ops");
    let mut found: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&ops).expect("crates/catalog/ops is readable") {
        let provider = entry.expect("readable directory entry").path();
        if !provider.is_dir() {
            continue;
        }
        let id = provider
            .file_name()
            .expect("a named directory")
            .to_string_lossy()
            .into_owned();

        for entry in std::fs::read_dir(&provider)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", provider.display()))
        {
            let path = entry.expect("readable directory entry").path();
            if path.extension().is_some_and(|ext| ext == "flux") {
                found.push(format!(
                    "{id}/{}",
                    path.file_name().expect("a named file").to_string_lossy()
                ));
            }
        }
    }

    found.sort();
    found
}

/// **The gate.** Each embedded rendering parses on its own, with no diagnostics.
#[test]
fn every_embedded_operation_parses() {
    for operation in all() {
        let parsed = flux_lang::parser::parse_cst(operation.flux);
        assert!(
            parsed.errors.is_empty(),
            "`{}` embeds Flux that does not parse: {:?}\n{}",
            operation.id,
            parsed.errors,
            operation.flux
        );
    }
}

/// And each is already canonical: flux's own formatter would not rewrite a byte of it.
///
/// This is the half that keeps the artifact reviewable. A generated file that reformats the first
/// time a human opens it in an editor produces a diff nobody can read.
#[test]
fn every_embedded_operation_is_a_fixed_point_of_the_flux_formatter() {
    for operation in all() {
        let parsed = flux_lang::parser::parse_cst(operation.flux);
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(operation.flux),
            "the flux formatter would rewrite `{}`",
            operation.id
        );
    }
}

/// **The analyze half**: each rendering loads through flux-lang's module loader as exactly one
/// composite op, carrying the name and the exposure flux's approval gate and tool registry read.
///
/// Parsing is not enough — a module that parsed but did not load would publish no ops at all, and
/// a consumer handing it to flux would get silence rather than an error.
///
/// **Exposure is asserted as readable, not as universally true** (C-413). This test used to require
/// `meta.expose` of every embedded operation, which was accurate while the emitter hard-coded it and
/// became a rule *forbidding the feature* the moment `expose` became a declaration: the first
/// provider to ship `expose = false` would have failed here, in a crate its story never touches, for
/// doing exactly what the field is for. What is worth pinning is that the flag survives the
/// round-trip into the artifact a host links against — so the assertion is that the loaded value
/// **agrees with the emitted text**, which is a claim about this crate's renderings rather than about
/// anybody's curation choices.
#[test]
fn every_embedded_operation_loads_as_a_composite_op() {
    for operation in all() {
        let module = flux_lang::program::Module::parse_str(operation.flux)
            .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));

        assert_eq!(
            program.ops.len(),
            1,
            "one rendering is one operation; `{}` loaded {}",
            operation.id,
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, operation.id);

        // flux's formatter writes `expose true` or `expose false` and elides neither, so the text
        // and the loaded flag are two statements of one fact and must not disagree — that agreement
        // is what lets `connector-pack` decide registration by reading the embedded Flux.
        let declared = if program.ops[0].meta.expose {
            "expose true"
        } else {
            "expose false"
        };
        assert!(
            operation.flux.contains(declared),
            "`{}` loads as `{declared}`, which its own embedded Flux does not state — the artifact \
             and the value a host reads from it disagree:\n{}",
            operation.id,
            operation.flux
        );
    }
}

/// **The metadata describes the Flux it is attached to.** `risk`, `idempotency` and the host are
/// carried twice — once as a queryable field and once inside the declaration flux reads — and a
/// caller deciding whether to run an operation must not be able to get a different answer from the
/// two.
///
/// This is also what pins the catalog's mirrored [`catalog::Risk`] and [`catalog::Idempotency`]
/// vocabularies against flux's own spelling, without this crate depending on either.
#[test]
fn metadata_agrees_with_the_embedded_flux() {
    for operation in all() {
        assert!(
            operation
                .flux
                .contains(&format!("risk \"{}\"", operation.risk.as_str())),
            "`{}` is catalogued as {:?} but does not declare it:\n{}",
            operation.id,
            operation.risk,
            operation.flux
        );
        assert!(
            operation.flux.contains(&format!(
                "idempotency \"{}\"",
                operation.idempotency.as_str()
            )),
            "`{}` is catalogued as {:?} but does not declare it:\n{}",
            operation.id,
            operation.idempotency,
            operation.flux
        );
        for host in operation.hosts {
            assert!(
                operation.flux.contains(host),
                "`{}` is catalogued as reaching `{host}`, which its request never mentions:\n{}",
                operation.id,
                operation.flux
            );
        }
        assert!(
            operation.flux.contains(&format!("op {} ", operation.id))
                || operation.flux.contains(&format!("op {}(", operation.id)),
            "`{}` embeds a declaration of something else:\n{}",
            operation.id,
            operation.flux
        );
    }
}

/// **No credential ever enters a generated artifact** (AGENTS.md). The catalog carries credential
/// *references* so a caller can see what an operation needs; the Flux carries not even that yet
/// (auth injection is C-10). Neither carries an environment variable's value, and neither may.
#[test]
fn no_operation_names_a_credential_it_does_not_reference() {
    for operation in all() {
        for mechanism in operation.credentials {
            assert!(
                !mechanism.is_empty(),
                "`{}` declares an empty mechanism — \"no auth\" is an empty alternatives list, not \
                 a list holding an empty one",
                operation.id
            );
            for credential in *mechanism {
                assert!(
                    !credential.is_empty(),
                    "`{}` names an empty credential",
                    operation.id
                );
            }
        }
    }
}

/// **Every operation says *why* it names the credentials it does** — C-235.
///
/// The invariant [`catalog::Operation::credential_requirement`]'s doc states, held over whatever
/// ships: the requirement is [`Declared`](catalog::CredentialRequirement::Declared) exactly when
/// the mechanism list is non-empty. The two empty states are what the field exists for, and they
/// are the ones a consumer could not previously tell apart.
///
/// A **property**, not a census (`AGENTS.md`, "A per-provider test asserts about its provider,
/// never about the catalogue"): it names no connector and counts nothing, so a fifty-fourth
/// provider satisfies it or is exactly the arrival it should fail on. What it cannot assert is that
/// any particular state is *reached* — nothing declares `auth = []` yet, and requiring one here
/// would be a rule forbidding the catalogue from being what it currently is.
#[test]
fn the_declared_requirement_agrees_with_the_mechanism_list() {
    use catalog::CredentialRequirement;

    for operation in all() {
        let declared = operation.credential_requirement == CredentialRequirement::Declared;
        assert_eq!(
            declared,
            !operation.credentials.is_empty(),
            "`{}` is catalogued as {:?} with {} mechanism(s) — a requirement of `Declared` and a \
             mechanism list are the same claim, and they disagree",
            operation.id,
            operation.credential_requirement,
            operation.credentials.len()
        );
    }
}

/// **The tokens are C-206's published ones**, character for character.
///
/// `no-credential-required` and `no-credential` are the two codes `web/public/catalog.json` already
/// publishes for these states (`docs/designs/catalog-json.md`), and `connectors-api`'s `Wiring`
/// serializes them onwards to an operator page. They are published contract tokens: extended, never
/// renamed. This crate depends on nothing, so the agreement is asserted against the literals rather
/// than against `connector_cli::status`'s constants — which is also the direction that matters,
/// since a consumer switching on the string is who both surfaces are for.
#[test]
fn the_requirement_tokens_are_the_published_ones() {
    use catalog::CredentialRequirement;

    assert_eq!(CredentialRequirement::Declared.as_str(), "declared");
    assert_eq!(
        CredentialRequirement::NoneRequired.as_str(),
        "no-credential-required"
    );
    assert_eq!(CredentialRequirement::Withheld.as_str(), "no-credential");
}

/// The two directions of the middle level: every operation belongs to a provider that lists it, and
/// every provider's listing holds only its own.
#[test]
fn listing_by_provider_round_trips() {
    for provider in catalog::providers() {
        let listed = catalog::operations_of(ProviderKey::id(provider.id));
        assert!(!listed.is_empty(), "`{}` publishes nothing", provider.id);
        for operation in listed {
            assert_eq!(operation.provider, provider.id);
            assert_eq!(
                catalog::operation(OperationKey::id(operation.id)),
                Some(operation),
                "`{}` is listed under `{}` but not findable by key",
                operation.id,
                provider.id
            );
        }
    }
}

/// **The compiled-in provider index matches what ships.**
///
/// `src/generated.rs` is generated by a full build since C-104, which changes what this test is for
/// but not whether it is needed. It used to catch a *forgotten line*: the index was hand-written, so
/// a provider added to `providers/` without its two lines was compiled, written into `ops/` and
/// `src/generated/`, and then silently left out of every query. That failure is now unrepresentable.
///
/// What remains is **staleness**, and it is the same class of failure the whole catalog crate has:
/// the index is a *committed* artifact compiled into this crate, so a tree where someone added a
/// provider and did not run a full build carries an index that still compiles, still answers every
/// query, and does not mention them. Nothing in `cargo build` would say a word. That is exactly why
/// this comparison reads `providers/` from disk rather than any list: it is the one check in this
/// crate that can see a provider the compiled-in tables cannot.
///
/// The non-emptiness guard is load-bearing rather than decorative — against an empty `providers/`
/// the equality would hold between two empty vectors and prove nothing.
#[test]
fn the_provider_list_matches_the_repository() {
    let shipped = shipped_providers();
    assert!(
        !shipped.is_empty(),
        "`providers/` holds no definitions, so comparing it against the index proves nothing"
    );

    let catalogued: Vec<String> = catalog::providers()
        .iter()
        .map(|provider| provider.id.to_string())
        .collect();

    assert_eq!(
        catalogued, shipped,
        "`crates/catalog/src/generated.rs` and `providers/` disagree — the committed index is \
         stale. Run `cargo run -p connector-cli -- build` (a *full* build: the index is a \
         whole-catalogue artifact and a `--provider` run deliberately leaves it alone)"
    );
}

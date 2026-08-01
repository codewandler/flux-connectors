//! `providers/openrouter.toml` exists, emits analyzable Flux, and holds the four properties C-76
//! ships it for. OpenRouter is the cheapest connector in the fleet because it speaks the OpenAI
//! request shape, so this file is deliberately the OpenAI test's shape too — and the places where it
//! differs are the places where the two vendors differ.
//!
//! **1. No query parameter, of any type.** Nothing in this pipeline percent-encodes a query value:
//! the emitter interpolates it verbatim into a `fmt` template (`crates/connector-flux/src/op.rs`) and
//! flux registers no URL-encoding op, so C-30 is unimplemented and `zendesk-ticket-search` is the
//! standing demonstration AGENTS.md records under *Intentional gaps*. This is the constraint that
//! reshaped C-76's selection rather than merely trimming it: **`GET /api/v1/generation` takes its
//! generation id as a required query parameter** (`?id=…`), so the third operation the story named
//! cannot be emitted honestly at all and is excluded. The absence is asserted in the strong form —
//! *zero* query parameters, not merely zero string-ish ones — over the IR *and* over the emitted
//! text, because the two can disagree.
//!
//! **2. No optional request-body field.** `body_tree` inserts every declared body field
//! unconditionally (`op.rs`), so an optional field the caller omits travels as an explicit
//! `{"field": null}` — C-56. OpenRouter fans a request out to an upstream provider whose own schema
//! it does not control, so a `{"temperature": null}` it forwards is a null this repository cannot
//! reason about. Every declared field is therefore required, which is why every OpenAI-compatible
//! inference knob is absent.
//!
//! **3. The token budget is required, though OpenRouter documents it as optional.** The same
//! deliberate narrowing C-51 applied to OpenAI's `max_completion_tokens`, and for the same two
//! reasons: an operation an LLM can invoke that spends money must state its cost bound, and *required*
//! means always sent, which sidesteps the null-body gap above by construction rather than by trusting
//! a vendor to tolerate a null. C-76's Acceptance names the field `max_tokens`; the vendor's own
//! document calls that one deprecated, so the connector declares `max_completion_tokens` instead and
//! [`the_chat_completion_requires_a_non_deprecated_token_budget`] is where that is pinned.
//!
//! **4. No caller-supplied header, and `HTTP-Referer`/`X-Title` in particular are not smuggled in as
//! ones.** OpenRouter's two attribution headers are genuinely optional, so the connector is
//! well-formed without them. Declaring them as `params.header` entries with a `const` would not pin
//! them: `op.rs` filters `constant(...)` out of the declared parameter list for `body` params only,
//! so a `const`-pinned header emits as a required argument every caller must pass and any caller may
//! set to anything, with the constraint dropped. That is C-52's finding and C-55's story; until it
//! lands, nothing is declared and [`no_openrouter_operation_declares_a_header_parameter`] stops the
//! disguise from landing in the meantime.
//!
//! The structural claims deliberately restate what `shipped_modules.rs` asserts across every
//! provider, so C-76's gate fails on its own file rather than only inside a shared loop.

use std::path::{Path, PathBuf};

use connector_spec::{AuthScheme, Connector, HttpMethod, Idempotency, Risk};

use connector_flux::emit_operation;

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The provider under test. Named once so the file reads as being about OpenRouter rather than about
/// a string.
const PROVIDER: &str = "openrouter";

/// The credential the connector declares and the environment variable it resolves from. Both are
/// public contract — an operator sets the variable, a manifest names the credential — so they are
/// pinned here rather than left to whatever the provider file happens to say.
const CREDENTIAL: &str = "openrouter.api_key";
/// See [`CREDENTIAL`]. A variable *name*; no credential value appears in this repository.
const SECRET_ENV: &str = "OPENROUTER_API_KEY";

/// One tenant-independent host, so unlike zendesk's `{subdomain}` there is no template variable to
/// bind and the connector's whole egress surface is this single literal name.
const BASE_URL: &str = "https://openrouter.ai";

/// Every selected endpoint lives under this prefix. OpenRouter's OpenAI-compatible surface is
/// `/api/v1/…`, not OpenAI's bare `/v1/…`, which is the one difference that touches every path.
const PATH_PREFIX: &str = "/api/v1/";

/// The four curated operations, in the order `providers/openrouter.toml` declares them.
///
/// This names one provider's operations rather than the shipped-provider set, so it is the curated
/// inventory claim C-54 deliberately kept, not the duplicated provider list it removed.
const OPERATIONS: &[&str] = &[
    "openrouter-models-list",
    "openrouter-model-endpoints-list",
    "openrouter-chat-completion",
    "openrouter-credits-get",
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
            "cannot read {} ({error}) — C-76 ships the OpenRouter connector",
            path.display()
        )
    });
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// Every operation's emitted module, paired with its id.
fn emitted() -> Vec<(String, String)> {
    let connector = load();
    connector
        .operations
        .iter()
        .map(|operation| {
            let flux = emit_operation(&connector, operation)
                .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
            (operation.id.clone(), flux)
        })
        .collect()
}

/// **The connector exists and is the one C-76 specifies**: one bearer key over `openrouter.ai`, with
/// the curated operation set. Everything below reads the loaded IR, so a failure here would otherwise
/// surface four times over as something less specific.
#[test]
fn the_openrouter_connector_loads_and_authenticates_with_one_bearer_key() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "OpenRouter");
    assert_eq!(
        connector.base_url, BASE_URL,
        "the host is `openrouter.ai` and carries no tenant template, so `http_hosts` derives from \
         this one value"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    let [method] = connector.auth.as_slice() else {
        panic!(
            "OpenRouter authenticates with exactly one credential; found {}",
            connector.auth.len()
        );
    };
    assert_eq!(method.name, CREDENTIAL);
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "OpenRouter takes its key as `Authorization: Bearer <key>`"
    );
    assert_eq!(method.env, [SECRET_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bearer has no user half"
    );
    assert!(
        method.oauth2.is_none(),
        "the key is minted in OpenRouter's dashboard; flux runs no grant for it"
    );

    // `effective_auth`, not `Operation::auth`: an operation that declares nothing inherits
    // `default_auth`, and one declaring an explicit `[]` inherits nothing. Reading the field
    // directly would let an accidentally-unauthenticated operation pass.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; openrouter is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the bearer key",
            operation.id
        );
        assert!(
            operation.id.starts_with("openrouter-"),
            "`{}` does not name its provider; the op id is the public name a model calls",
            operation.id
        );
        assert!(
            operation.path.starts_with(PATH_PREFIX),
            "`{}` has path {:?}; every selected OpenRouter endpoint is under `{PATH_PREFIX}`",
            operation.id,
            operation.path
        );
    }
}

/// **The headline invariant, on the IR.** See the module documentation: C-30 is not implemented, so a
/// query value reaches the wire unencoded, and the whole point of this selection is that it has none.
/// Stated over every operation in the strong form, including the ones a later author adds.
#[test]
fn no_openrouter_operation_declares_a_query_parameter() {
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
             value (C-30 is unimplemented), so a value carrying `&`, `#` or `+` corrupts the request \
             or injects a parameter — the `zendesk-ticket-search` failure AGENTS.md records. This is \
             why `GET /api/v1/generation?id=` is excluded rather than shipped. C-76 ships the \
             path-and-body surface only; if C-30 has landed, change this test deliberately",
            operation.id
        );
    }
}

/// The same claim over the **emitted text**, which is what flux actually loads — so an emitter that
/// synthesised a query parameter from somewhere other than `params.query` could not slip past.
///
/// **Every `url = ` line is checked, not just the first, and that is the substance of this test.**
/// The emitter binds `$url` once for the path and the required query parameters, then re-binds it once
/// more per *optional* query parameter inside a `when` guard (`op.rs`, the `optional` loop), and the
/// `?` lives on a separate `sep` binding rather than on the `$url` line. `connectors/zendesk.flux`
/// shows the shape:
///
/// ```flux
/// url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// sep = "?"
/// when $page
///   url = fmt("{url}{sep}page={page}")
/// ```
///
/// So inspecting only the first binding, or only looking for a literal `?`, would pass while an
/// operation quietly appended optional filters. All three are checked: one `$url` binding, no `?`
/// anywhere, and no `sep` at all.
#[test]
fn no_openrouter_module_assembles_a_query_string() {
    for (id, flux) in emitted() {
        let url_lines: Vec<&str> = flux
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert!(!url_lines.is_empty(), "`{id}` binds no $url:\n{flux}");
        assert_eq!(
            url_lines.len(),
            1,
            "`{id}` re-binds $url {} times; the emitter does that once per optional query parameter, \
             so this operation is appending a query string:\n{flux}",
            url_lines.len()
        );
        for line in &url_lines {
            assert!(!line.contains('?'), "`{id}` emits a query string: {line}");
        }
        assert!(
            !flux.contains('?'),
            "`{id}` emits a `?`, so a value is reaching the query string unencoded:\n{flux}"
        );
        // `sep` exists only to carry the `?`/`&` between query parameters, so an operation that
        // binds it is building a query string even if no single line spells the `?`.
        assert!(
            !flux
                .lines()
                .any(|line| line.trim_start().starts_with("sep = ")),
            "`{id}` binds $sep, which the emitter emits only to separate query parameters:\n{flux}"
        );
    }
}

/// **No optional request-body field, until C-56 lands.**
///
/// `body_tree` inserts every declared body field unconditionally, so an optional field the caller
/// leaves out is sent as an explicit `null`. OpenRouter forwards the body to an upstream provider
/// whose schema it does not own, so what a `null` inference knob does there is not a property this
/// repository can check. Every declared field is required, and what was dropped for this reason is
/// named in the provider file rather than shipped as a latent null.
#[test]
fn no_openrouter_operation_declares_an_optional_body_field() {
    let connector = load();

    for operation in &connector.operations {
        let optional: Vec<&str> = operation
            .params
            .body
            .iter()
            .filter(|param| !param.required)
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            optional.is_empty(),
            "operation `{}` declares optional body fields {optional:?}. An unsupplied optional body \
             field is emitted as an explicit `null` rather than omitted (C-56, \
             `crates/connector-flux/src/op.rs` `body_tree`), and OpenRouter forwards that null to an \
             upstream provider this repository cannot reason about. Declare it required or leave it \
             out until C-56 lands",
            operation.id
        );
    }
}

/// **The cost bound is a parameter the caller must state, and it is not the deprecated spelling.**
///
/// Two claims, and the second is why this test names a field at all.
///
/// *Required* is the narrowing C-76's Acceptance asks for and C-51 established: OpenRouter's
/// `ChatRequest.required` is `["messages"]` and nothing else, an unbounded completion has an unbounded
/// bill, and required also means always sent, which sidesteps C-56's null entirely.
///
/// *`max_completion_tokens`* is a deliberate departure from the Acceptance text, which names
/// `max_tokens`. OpenRouter's own OpenAPI document describes that field as *"Maximum tokens
/// (deprecated, use max_completion_tokens)"*, so the story's premise — that OpenRouter's
/// OpenAI-compatible surface wants the legacy name — does not hold, and the name transfers from
/// `providers/openai.toml` unchanged after all. The deprecated spelling is asserted **absent**, not
/// merely unused: an author reading only the story would add it back, and this is the test that says
/// why not.
#[test]
fn the_chat_completion_requires_a_non_deprecated_token_budget() {
    let connector = load();

    let chat = connector
        .operation("openrouter-chat-completion")
        .expect("the connector declares `openrouter-chat-completion`");

    let budget = chat
        .params
        .body
        .iter()
        .find(|param| param.name == "max_completion_tokens")
        .expect(
            "`openrouter-chat-completion` declares `max_completion_tokens` — C-76's Acceptance says \
             `max_tokens`, but OpenRouter's own document marks that one deprecated in favour of this \
             one, so the OpenAI spelling transfers unchanged",
        );
    assert!(
        budget.required,
        "the token budget is optional at OpenRouter and required here on purpose (C-51's precedent): \
         an operation an LLM can call that spends money must not be unbounded, and required is also \
         what keeps it out of C-56's null-body gap"
    );

    assert!(
        !chat
            .params
            .body
            .iter()
            .any(|param| param.name == "max_tokens"),
        "`max_tokens` is declared. OpenRouter's document calls it deprecated (\"Maximum tokens \
         (deprecated, use max_completion_tokens)\"), exactly as OpenAI does, so a connector \
         authored against it would be born on its way out"
    );

    // The budget has to reach the emitted signature, since the IR is what an author reads and the
    // emitted op is what a model calls.
    let flux = emit_operation(&connector, chat).expect("`openrouter-chat-completion` emits");
    assert!(
        flux.contains("max_completion_tokens"),
        "the emitted op does not take a token budget:\n{flux}"
    );
}

/// **No caller-supplied header — and the two attribution headers in particular are not smuggled in as
/// ones.**
///
/// `HTTP-Referer` and `X-Title` (spelled `X-OpenRouter-Title` in OpenRouter's current documentation)
/// are optional app-attribution headers, so the connector is well-formed without them. What is not
/// acceptable is declaring them as `params.header` entries with a `const`: `op.rs` filters
/// `constant(...)` out of the declared parameter list for `body` params only, so a `const`-pinned
/// header emits as a required argument every caller must pass and any caller may set to anything,
/// with the constraint dropped. That is C-52's finding; pinning a constant header is C-55's story.
///
/// The Authorization header is the host's business either way — it is injected at the `$auth` seam and
/// must never travel through the parameter surface.
#[test]
fn no_openrouter_operation_declares_a_header_parameter() {
    let connector = load();

    for operation in &connector.operations {
        let declared: Vec<&str> = operation
            .params
            .header
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        assert!(
            declared.is_empty(),
            "operation `{}` declares header parameters {declared:?}. A `const`-pinned header is \
             still emitted as a caller-overridable argument (C-52), so declaring `HTTP-Referer` or \
             `X-Title` this way would be a disguise rather than a constant — that waits for C-55. \
             The Authorization header is injected by the host and never a parameter",
            operation.id
        );
    }

    // The emitted text, since a header could in principle reach the request from somewhere other
    // than `params.header`. `content-type` is the one constant the emitter hard-codes for a JSON
    // body, so it is the only header a shipped OpenRouter module may carry.
    for (id, flux) in emitted() {
        let lowered = flux.to_lowercase();
        for header in ["http-referer", "http_referer", "x-title", "x_title"] {
            assert!(
                !lowered.contains(header),
                "`{id}` emits `{header}`; an attribution header cannot be pinned until C-55, and a \
                 caller-supplied one is not a pin:\n{flux}"
            );
        }
    }
}

/// **An operation an LLM can call that spends money is not `low` risk**, and no write claims an
/// idempotency OpenRouter does not document.
///
/// `connector-flux`'s `check_write_metadata` already refuses both for any state-changing method, so
/// this is not the emitter's gate restated: it is the *reason* stated where a reader looking at
/// OpenRouter will find it. Inference is billed per token and `risk` is what flux's approval gate
/// reads before letting a model run the call unattended.
///
/// `medium` rather than `high`, following C-51: nothing here mutates state the account's own users
/// can see — no object is created, updated or deleted at the vendor — and the cost of a single
/// bounded call is bounded. `high` is reserved for a write a reviewer would want to see first.
#[test]
fn the_cost_bearing_operation_declares_what_it_costs() {
    let connector = load();

    for operation in &connector.operations {
        let mutates = matches!(
            operation.method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
        );
        if !mutates {
            continue;
        }
        assert_ne!(
            operation.risk,
            Risk::Low,
            "`{}` bills the account per call; `low` is what flux's approval gate waves through",
            operation.id
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::NonIdempotent,
            "`{}` is a POST that spends money: declaring it idempotent makes a `retry` around it \
             turn one charge into three",
            operation.id
        );
    }

    let chat = connector
        .operation("openrouter-chat-completion")
        .expect("the connector declares `openrouter-chat-completion`");
    assert_eq!(
        chat.risk,
        Risk::Medium,
        "inference is billed per token, so the chat completion is not `low`; it creates nothing the \
         account's users can see, so it is not `high` either (C-51's reasoning)"
    );
}

/// **No credential reaches a generated module** — not its value, and today not even its name — and the
/// one host a module names is the one `base_url` derives.
///
/// Auth injection is C-10 and the `$auth` seam it needs must land in flux first, so the emitted `op`
/// builds a URL and calls `http.request` with `method` and `url` and nothing else. That is a recorded
/// gap rather than a bug, and this pins the direction of it: a future edit that starts splicing the
/// key into the module fails here. The `http_hosts` half sits alongside it because it is the same
/// claim about the same text — one absolute URL, never a wildcard.
#[test]
fn no_credential_and_no_widened_host_reaches_a_generated_module() {
    let connector = load();
    assert!(
        !connector.base_url.contains('{'),
        "`base_url` must be a bound literal, not a template: {:?}",
        connector.base_url
    );

    for (id, flux) in emitted() {
        assert!(
            !flux.contains(SECRET_ENV),
            "`{id}` names {SECRET_ENV} in generated Flux; a generated module carries no credential \
             at all until C-10:\n{flux}"
        );
        // OpenRouter's keys are `sk-or-`-prefixed. A literal one in a generated artifact is the
        // failure this invariant exists to prevent, so it is checked for by shape and not only by
        // name.
        assert!(
            !flux.contains("sk-or-"),
            "`{id}` embeds something shaped like an OpenRouter key:\n{flux}"
        );
        assert_eq!(
            flux.matches("https://").count(),
            1,
            "`{id}` names more than one absolute URL, so `http_hosts` would have to be widened \
             beyond what the base URL derives:\n{flux}"
        );
        assert!(
            flux.contains(&format!("base = \"{BASE_URL}\"")),
            "`{id}` does not bind the OpenRouter base URL:\n{flux}"
        );
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical
/// under flux's own formatter, and **loads** as exactly one exposed composite op.
///
/// `shipped_modules.rs` makes this claim for the whole shipped set. It is restated here so C-76's own
/// test file fails on its own when the module stops being analyzable — a provider whose emitted Flux
/// does not load publishes no ops at all, and a consumer handing it to flux would get silence rather
/// than an error.
#[test]
fn every_openrouter_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
    for (id, flux) in emitted() {
        let parsed = flux_lang::parser::parse_cst(&flux);
        assert!(
            parsed.errors.is_empty(),
            "`{id}` emits Flux that does not parse: {:?}\n{flux}",
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(flux.as_str()),
            "the flux formatter would rewrite `{id}`"
        );

        let module = flux_lang::program::Module::parse_str(&flux)
            .unwrap_or_else(|error| panic!("`{id}` does not load: {error}"));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{id}` is not a program"));
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{id}` loaded {}",
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, id);
        assert!(
            program.ops[0].meta.expose,
            "`{id}` must be exposed to the model as a tool"
        );
    }
}

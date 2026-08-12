//! The Supabase connector, and the credential decision it exists to force — C-221.
//!
//! `shipped_modules.rs` next door asserts that every provider's operations emit, parse, analyze and
//! are canonical. This file adds the claims specific to Supabase, because they are the reasons the
//! connector looks the way it does and the reasons a later reader must not "complete" it.
//!
//! # The probe: two keys, and only one of them is declared
//!
//! Supabase issues every project two keys, and both are "the API key":
//!
//! - **`anon`** (its successor is spelled `sb_publishable_…`) — public, and constrained by Postgres
//!   row-level security through the `anon` and `authenticated` roles.
//! - **`service_role`** (successor `sb_secret_…`) — which the vendor's own documentation says
//!   "provide[s] *full access* to your project's data, bypassing Row Level Security", by way of the
//!   Postgres `BYPASSRLS` attribute. It is database-owner authority in a string.
//!
//! A connector that declared one credential called `supabase.api_key` and let an operator paste
//! either would have made that choice on their behalf silently, and every `risk` in the catalogue
//! would then be describing the *operation* while saying nothing about the *authority the credential
//! carries*. So this connector does two things, and both are pinned below:
//!
//! 1. **It names the key it wants.** The credential is `supabase.anon_key`, never `api_key` —
//!    [`the_only_credential_is_the_anon_key_and_its_name_says_so`].
//! 2. **It declares `service_role` nowhere, and explains it anyway.** Every shipped operation is a
//!    read that the `anon` key satisfies, so the narrower connector is the correct one and the
//!    second key has no slot to be pasted into. The file still tells the operator what that key
//!    would do, in plain words, in the `help` a person reads —
//!    [`service_role_is_explained_in_prose_and_declared_nowhere`].
//!
//! [`every_shipped_operation_is_a_read_which_is_why_the_anon_key_is_enough`] is what keeps claim 2
//! honest over time: it fails the moment somebody adds a write, which is the moment the "do we need
//! `service_role`?" question has to be reopened rather than answered by a quiet second credential.
//!
//! # The limit of all this, stated rather than papered over
//!
//! Nothing here can *refuse* the wrong key. [`Format`](connector_spec::Format) is a closed enum with
//! no `pattern` (deliberately — `crates/connector-spec/src/config.rs:31`), and the only evidence
//! distinguishing the two keys is a `sb_secret_` prefix or a `"role":"service_role"` claim inside a
//! legacy JWT. An operator who pastes the secret key into this connector's one credential field gets
//! a connector that works and silently bypasses every row-level-security policy their project has.
//! The connector's answer is to say so where they are looking — which is why the `help` text is
//! asserted here as a contract and not left as prose nobody checks.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{AuthScheme, Connector, Format, HttpMethod, Idempotency, Risk};

use crate::shipped_provider;

/// The provider under test.
const PROVIDER: &str = "supabase";

/// The one credential this connector declares. The name is the assertion: it says *which* of
/// Supabase's two keys it is, not merely that it is a key.
const CREDENTIAL: &str = "supabase.anon_key";
/// Its environment variable. A *name*; no credential value appears in this repository.
const KEY_ENV: &str = "SUPABASE_ANON_KEY";

/// The header the key travels in. Supabase's gateway requires it on every request and the vendor
/// documents it as "mandatory and not configurable".
const AUTH_HEADER: &str = "apikey";

/// The key that is deliberately **not** declared.
const FORBIDDEN_KEY: &str = "service_role";

/// The curated operations, in the order `providers/supabase.toml` declares them.
const OPERATIONS: &[&str] = &[
    "supabase-schema-describe",
    "supabase-rows-list",
    "supabase-auth-settings",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn provider_path() -> PathBuf {
    providers_dir().join(format!("{PROVIDER}.toml"))
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// The provider TOML as written, for the assertions that are about the authored file rather than
/// about the IR it loads into.
fn source() -> String {
    let path = provider_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// **One credential, and its name says which of Supabase's two keys it is.**
///
/// This is C-221's load-bearing assertion. `supabase.api_key` would be a true statement and a
/// useless one: both keys are "the API key", and the generic name is exactly what lets an operator
/// paste the one that bypasses row-level security without ever being told there was a choice.
///
/// The placement is asserted too. Supabase requires the key in the `apikey` header — its security
/// guide calls that header "mandatory and not configurable" — and this connector places it there and
/// nowhere else. The vendor's `curl` examples also repeat the same value in
/// `Authorization: Bearer …`, which this connector deliberately does not do: an
/// [`AuthMethod`](connector_spec::AuthMethod) is one credential in one placement, so the alternative
/// would be declaring the same secret twice under two names — two provisioning slots for one value,
/// which is the pre-composed-credential mistake `docs/designs/auth-seam.md` §7.5 rejects. The
/// redundant half carries no additional authority: for the new key format the vendor documents that
/// `Authorization` may hold the API key only when it *equals* the `apikey` header.
#[test]
fn the_only_credential_is_the_anon_key_and_its_name_says_so() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Supabase");
    assert_eq!(
        connector.base_url, "https://{project_ref}.supabase.co",
        "every Supabase project is its own host; the ref is a bound endpoint variable"
    );

    assert_eq!(
        connector.auth.len(),
        1,
        "Supabase ships exactly one credential. The second key exists and is deliberately not \
         declared — see this file's module docs"
    );

    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("supabase declares `{CREDENTIAL}`"));

    // The name is the point. A generic name is what this test exists to refuse.
    assert!(
        method.name.contains("anon"),
        "the credential is named `{}`; it must name *which* Supabase key it is. Both of Supabase's \
         keys are \"the API key\", and a slot called `api_key` invites the one that bypasses \
         row-level security",
        method.name
    );
    for generic in ["supabase.api_key", "supabase.key", "supabase.token"] {
        assert_ne!(
            method.name, generic,
            "`{generic}` does not say which key it is — that ambiguity is the whole finding"
        );
    }

    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string(),
            prefix: String::new(),
        },
        "Supabase's gateway takes the key in a bare `apikey` header, with no scheme word in front \
         of it"
    );
    assert_eq!(method.env, [KEY_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "a header placement has no Basic username half"
    );
    assert!(
        method.oauth2.is_none(),
        "the anon key is a static project key; nothing here runs a grant"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(declared, OPERATIONS);

    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` offers {} auth alternatives. Supabase's two keys must never be \
             alternatives of one mechanism — that is precisely the choice this connector refuses to \
             make silently",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains(CREDENTIAL) && effective[0].len() == 1,
            "operation `{}` names a credential other than the anon key",
            operation.id
        );
    }

    assert_eq!(
        connector.verify.as_deref(),
        Some("supabase-schema-describe"),
        "verify must be a read that runs unattended and takes no required argument"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("verify names a declared operation");
    assert_eq!(verify.method, HttpMethod::Get);
    assert_eq!(verify.risk, Risk::Low);
    assert!(
        verify.params.iter().all(|param| !param.required),
        "verify runs unattended whenever a settings page opens; it can be given no argument"
    );
}

/// **`service_role` is named in the words an operator reads, and declared nowhere in the IR.**
///
/// Both halves matter, and each fails a different way without the other:
///
/// - Declaring it (as a second credential, an env var, or a config field) would put the
///   row-level-security bypass one paste away from an operator who was never told what it does, for
///   operations that do not need it.
/// - *Not mentioning* it would be the quieter failure. An operator who has both keys in front of
///   them and no guidance picks whichever they find first, and the more capable one works. So the
///   connector spends words on the key it does not use, in the `help` a person sees next to an empty
///   input box — not in `description`, which is the text a *model* receives.
///
/// The `help` assertions are written against stable words (`service_role`, and "row-level
/// security"/"row level security") rather than exact prose, so the text can be improved without
/// breaking the contract that the warning is *there*.
#[test]
fn service_role_is_explained_in_prose_and_declared_nowhere() {
    let connector = load();
    let source = source();

    // Declared nowhere: not as a credential, not as an environment variable, not as configuration.
    for method in &connector.auth {
        assert!(
            !method.name.contains(FORBIDDEN_KEY),
            "credential `{}` declares the `{FORBIDDEN_KEY}` key. Every shipped operation is a read \
             the anon key satisfies, so this connector has no use for a credential that bypasses \
             row-level security — and a declared slot is a slot somebody fills",
            method.name
        );
        for env in &method.env {
            assert!(
                !env.to_ascii_lowercase().contains(FORBIDDEN_KEY),
                "credential `{}` resolves from `{env}`; that is the bypass key by another spelling",
                method.name
            );
        }
    }
    for field in &connector.config {
        assert!(
            !field.name.contains(FORBIDDEN_KEY) && !field.binds.contains(FORBIDDEN_KEY),
            "config field `{}` collects the `{FORBIDDEN_KEY}` key",
            field.name
        );
    }

    // Explained anyway. The file must say the word, and the credential's own `help` must say what
    // that key does — in the plain words the vendor uses, not a euphemism.
    assert!(
        source.contains(FORBIDDEN_KEY),
        "providers/{PROVIDER}.toml never mentions `{FORBIDDEN_KEY}`. An operator holding both keys \
         and told about neither pastes whichever they found first, and the wrong one works"
    );

    let key_field = connector
        .config
        .iter()
        .find(|field| field.binds == format!("credential.{CREDENTIAL}"))
        .expect("a config field collects the anon key");
    let help = key_field.help.to_ascii_lowercase();
    assert!(
        help.contains(FORBIDDEN_KEY),
        "the anon key's `help` does not name `{FORBIDDEN_KEY}`. This is the one place a person is \
         looking at the two keys and choosing between them:\n{}",
        key_field.help
    );
    assert!(
        help.contains("row-level security") || help.contains("row level security"),
        "the anon key's `help` does not say what `{FORBIDDEN_KEY}` bypasses. Naming the key without \
         naming the consequence tells an operator there is a choice and not which way to make \
         it:\n{}",
        key_field.help
    );
    assert!(
        help.contains("bypass"),
        "the anon key's `help` must say `service_role` *bypasses* row-level security, in that \
         word — it is the difference between a stronger key and no policy enforcement at all:\n{}",
        key_field.help
    );
}

/// **Every shipped operation is a read, which is the entire justification for shipping one key.**
///
/// The narrow connector is only defensible while it stays narrow. `anon` is sufficient here because
/// nothing this connector does needs to escape a row-level-security policy: three `GET`s, no bodies,
/// nothing destructive. The moment a write lands, "is the anon key enough?" becomes a live question
/// again — an insert or update against a table an `anon` role has no policy for fails, and the
/// tempting fix is a quietly added `service_role` credential.
///
/// This test is what makes that impossible to do quietly. It fails on the write, before the
/// credential question is reached.
#[test]
fn every_shipped_operation_is_a_read_which_is_why_the_anon_key_is_enough() {
    let connector = load();

    for operation in &connector.operations {
        assert_eq!(
            operation.method,
            HttpMethod::Get,
            "operation `{}` is not a GET. Shipping only the anon key is justified by every \
             operation being a read; a write reopens that decision and must not slip past it",
            operation.id
        );
        assert_eq!(
            operation.risk,
            Risk::Low,
            "operation `{}` is not `low`. A read bounded by the caller's own row-level-security \
             policies is the lowest-authority thing this vendor offers",
            operation.id
        );
        assert_eq!(
            operation.idempotency,
            Idempotency::Idempotent,
            "operation `{}` is a GET that is not idempotent",
            operation.id
        );
        assert!(
            operation.params.body.is_empty(),
            "operation `{}` declares a request body; none of these operations sends one",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares a caller-supplied header. The `apikey` header is the host's to \
             place, and nothing else here needs one",
            operation.id
        );
    }
}

/// **No caller-supplied value reaches this connector's request as free text.**
///
/// PostgREST's whole expressive power is in its query string — `select`, `order`, and a filter per
/// column — and this emitter interpolates a query value verbatim and percent-encodes nothing
/// (`crates/connector-flux/src/op.rs`; `AGENTS.md`, Intentional gaps). So `providers/supabase.toml`
/// ships the *reading* half of PostgREST and not the *filtering* half, and its header comment names
/// every parameter left out and why. That is a curation claim, and this test is what keeps it from
/// decaying into a comment describing a file that has since grown a `select`.
///
/// Both positions a caller can steer are pinned, because both are substituted without a predicate
/// at request time (C-214):
///
/// - **query** — the one parameter declared is a bare `integer`. A `string` here would be free text
///   next to nothing that encodes it.
/// - **path** — `{table}` is a Postgres relation name, and its schema carries a `pattern` that
///   admits an identifier and refuses `/`, `?` and `.`. Without it a caller-supplied "table" could
///   steer the path the operation builds.
#[test]
fn no_caller_supplied_value_reaches_the_request_as_free_text() {
    let connector = load();

    for operation in &connector.operations {
        for param in &operation.params.query {
            assert_eq!(
                param.schema.get("type").and_then(|ty| ty.as_str()),
                Some("integer"),
                "`{}` declares query parameter `{}` as {}. Every query value this emitter writes is \
                 interpolated verbatim with no percent-encoding, so a free-text query parameter \
                 here is the `zendesk-ticket-search` defect reproduced knowingly — `select`, \
                 `order` and PostgREST's column filters are excluded for exactly this reason and \
                 the provider header says so",
                operation.id,
                param.name,
                param.schema.get("type").unwrap_or(&serde_json::Value::Null)
            );
        }

        for param in &operation.params.path {
            let pattern = param
                .schema
                .get("pattern")
                .and_then(|pattern| pattern.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "`{}`'s path parameter `{}` states no `pattern`. A path value is \
                         substituted with no predicate at request time (C-214), so the declaration \
                         is the only place the shape can be constrained",
                        operation.id, param.name
                    )
                });
            for steering in ['/', '?', '#', '.', ':', '%'] {
                assert!(
                    !regex_admits(pattern, steering),
                    "`{}`'s path parameter `{}` has pattern `{pattern}`, which admits `{steering}` \
                     — a caller could steer the path this operation builds",
                    operation.id,
                    param.name
                );
            }
        }
    }
}

/// Whether a character appears inside the character-class body of an anchored identifier pattern.
///
/// Deliberately not a regex engine: `connector-spec` carries no regex dependency and this crate is
/// not the place to introduce one. The patterns this file pins are anchored character classes, so
/// "does the pattern mention this character at all" is a sound over-approximation — it can only
/// fail a pattern that a real engine would also have to be checked against by hand.
fn regex_admits(pattern: &str, candidate: char) -> bool {
    pattern.contains(candidate)
}

/// **The project ref is a bound endpoint variable, and its `help` warns about the host position.**
///
/// `https://{project_ref}.supabase.co` is the same shape as `zendesk`'s `{subdomain}`, and it
/// inherits the same open defect:
/// [C-214](../../../docs/stories/C-214-a-pinned-value-reaches-the-wire-unvalidated.md) measured that
/// a host-position configuration value is substituted at
/// `crates/connector-pack/src/request.rs:484` with no predicate at all, so a value containing `@`
/// makes everything before it userinfo and moves the authority to a host the operator never named.
///
/// This connector cannot fix that — the fix is in `connector-pack` and belongs to C-214 — so it does
/// the one thing an author *can* do: tell the operator that only the bare ref belongs in the box.
/// The declaration half (`format = "subdomain"`, which the loader validates the `example` against)
/// is a statement of the same rule that a renderer can enforce before the value ever travels.
#[test]
fn the_project_ref_is_a_bound_endpoint_variable_and_the_help_says_only_the_ref_belongs_in_it() {
    let connector = load();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "project_ref")
        .expect("supabase declares a `project_ref` config field");

    assert_eq!(
        field.binds, "endpoint.project_ref",
        "the ref must bind the base URL's own variable; the loader refuses a template variable \
         nothing binds, and a field binding nothing would be collected and dropped"
    );
    assert!(
        !field.secret,
        "the project ref is in the URL of every request and every client bundle; classifying it \
         secret would contradict `binds`, which the loader refuses"
    );
    assert_eq!(
        field.format,
        Format::Subdomain,
        "the ref is one DNS label. `subdomain` is the format that says so, and the loader checks \
         the `example` against it — which is what stops a full URL being offered as a placeholder"
    );

    let help = field.help.to_ascii_lowercase();
    assert!(
        help.contains("https://") || help.contains("url"),
        "the ref's `help` must tell the operator not to paste the whole URL — that is the mistake \
         it is trying to prevent:\n{}",
        field.help
    );
    assert!(
        help.contains('@'),
        "the ref's `help` must name the `@` case. A host-position value is not re-checked where it \
         is substituted (C-214), so a pasted value containing `@` sends the request, carrying this \
         project's key, to a host the operator did not name:\n{}",
        field.help
    );
}

/// Every config field is renderable, and no secret field carries an `example`.
///
/// A placeholder shaped like a real key is worse than none — a user copies it, and one has already
/// tripped GitHub's push protection in this repository's history.
#[test]
fn the_config_surface_is_renderable_and_no_secret_field_is_exemplified() {
    let connector = load();

    assert_eq!(
        connector.config.len(),
        2,
        "two fields: the project ref and the anon key. That is the whole `Connect Supabase` form"
    );

    for field in &connector.config {
        assert!(
            !field.label.is_empty() && !field.help.is_empty(),
            "config field `{}` must be renderable: `label` and `help` are mandatory",
            field.name
        );
        if field.secret {
            assert!(
                field.example.is_none(),
                "secret field `{}` carries an `example`; a token-shaped placeholder is worse than \
                 no placeholder",
                field.name
            );
        }
    }

    let key_field = connector
        .config
        .iter()
        .find(|field| field.name == "anon_key")
        .expect("supabase declares an `anon_key` config field");
    assert_eq!(key_field.binds, format!("credential.{CREDENTIAL}"));
    assert!(
        key_field.secret,
        "the anon key is gated and redacted like any other credential. The vendor calls it safe to \
         expose; classifying it as a secret is the safe direction, and `Binding::Credential` \
         enforces it anyway"
    );
}

/// **No credential reaches a generated module** — not a value, not a variable name, not the header
/// it is placed in. The placement lives in the manifest's auth declaration and has no business in an
/// emitted `op`.
#[test]
fn no_supabase_module_carries_a_credential_or_its_variable_name() {
    let connector = load();

    for operation in &connector.operations {
        let text = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
        for forbidden in [KEY_ENV, CREDENTIAL, "$secret", AUTH_HEADER, FORBIDDEN_KEY] {
            assert!(
                !text.contains(forbidden),
                "`{}` names `{forbidden}` in generated Flux; a generated module carries no \
                 credential and no credential reference (C-10, AGENTS.md):\n{text}",
                operation.id
            );
        }
    }
}

/// The C-11 gate for this provider: every operation emits Flux that parses, is already canonical,
/// and **loads** as exactly one exposed composite op.
#[test]
fn every_supabase_operation_emits_a_module_that_parses_analyzes_and_is_canonical() {
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

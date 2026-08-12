//! The provider definitions this repository actually ships, loaded through the real loader.
//!
//! The fixtures in `provider_toml.rs` prove the *loader* works on hand-written excerpts. This file
//! proves the opposite direction: that `providers/*.toml` as committed is something the loader
//! accepts. Those are different claims, and only this one fails when someone edits a shipped
//! provider file into a shape the schema rejects — which is the failure a reviewer would otherwise
//! not see until a build.
//!
//! It reads from the repository root rather than an inline constant on purpose: a copy of the file
//! embedded here would be the thing under test drifting away from the thing that ships.

use std::path::{Path, PathBuf};

use connector_spec::{provider, AuthScheme};

use crate::shipped_provider;

/// `<repo root>/providers`, derived from this crate's manifest directory so the test is independent
/// of the working directory a runner happens to use.
fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// Every provider this repository ships, **read from `providers/` rather than listed here** (C-54).
///
/// A constant naming the six current ids would be a second source of truth, and the copies drift in
/// exactly one direction: a provider lands in `providers/` and not in the list, so the gates below
/// silently stop covering it. That is not hypothetical — C-53 shipped `slack` while one of five such
/// constants still named only five providers, and two build gates never ran for it.
///
/// Sorted, so a failure names providers in a stable order. Empty is a failure rather than a vacuous
/// pass: every gate here is a `for` loop, and an unreadable or empty `providers/` would satisfy all
/// of them without checking anything.
fn shipped() -> Vec<String> {
    let dir = providers_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();
    names.sort();

    assert!(
        !names.is_empty(),
        "{} holds no provider definitions, so every gate in this file would pass vacuously",
        dir.display()
    );
    names
}

fn load(name: &str) -> provider::LoadedProvider {
    let path = providers_dir().join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    shipped_provider::load_definition(name, &source)
        .unwrap_or_else(|error| panic!("providers/{name}.toml does not load: {error}"))
}

/// The story's load gate: every shipped definition parses and validates.
#[test]
fn every_shipped_provider_loads() {
    for name in shipped() {
        let name = name.as_str();
        let loaded = load(name);
        assert_eq!(
            loaded.connector.id, name,
            "providers/{name}.toml declares id `{}`, but the file name is what names the generated \
             `<id>.flux`",
            loaded.connector.id
        );
        assert!(
            !loaded.connector.operations.is_empty(),
            "providers/{name}.toml declares no operations, so it compiles to an empty module"
        );
    }
}

/// Selection is curated, not exhaustive — **for every provider but one**, and the exception is what
/// the exposure tier bought.
///
/// The rule this list was written under was that generated ops *are* LLM tools, so a curated count
/// was the only thing standing between a vendor's several-hundred-operation document and a model's
/// context. C-413 split those two claims and C-417 spent the split: babelforce now emits 388
/// operations and exposes **nine**, so its number here is a coverage claim rather than a curation
/// one. Every other entry still means what it always meant.
///
/// **This list stays explicit, and is not the duplication C-54 removed.** A count per provider is an
/// inventory claim: someone read the vendor's surface, chose these operations, and wrote the number
/// down. Deriving it from the file it describes would assert nothing at all. What C-54 derived is the
/// provider *set* — see [`shipped`] — so a provider added without a curated count is covered by every
/// other gate here and simply makes no claim about its own selection until someone reviews it.
#[test]
fn operation_selection_stays_curated() {
    let expected = [
        // C-461/C-462 publish 13 Support operations, C-466 adds 8 Support foundation reads, C-463
        // adds 7 Help Center operations, and C-464 adds 9 Messaging operations. Two message
        // operations use bounded inline schemas because their pinned OpenAPI responses are
        // recursive; the other 28 are exact selections or the seven original Support operations.
        ("zendesk", 35),
        ("freshdesk", 9),
        // **Not a curated count — a coverage one.** C-417 widened babelforce to the whole surface
        // `manager-sdk` covers: the five vendored documents declare 398 operations, the canonical
        // scope is 397 (less `POST /api/v1/webhook/zendesk`, a receiver), and 388 of those are
        // emitted. The other nine are `388 + 5 + 4 = 397`, in three categories: five
        // `multipart/form-data` uploads ingest cannot express — C-426 established the blocker is
        // **flux**, whose `http.request` takes a string body, so this is not closable here — and
        // four withheld for the credentials they carry: the three `/oauth/*` endpoints, because an
        // authentication endpoint is never an operation, and `GET /api/v2/user/account`, whose 200
        // body delivers two tokens (`AGENTS.md` § Authentication contract). Nine of the 388 are
        // exposed. The
        // accounting lives in `babelforce_coverage.rs`, which refuses a gap with no reason string;
        // this line is the number that makes a silent collapse of the selectors visible from here.
        ("babelforce", 388),
        // C-111 curates the generally available Machines lifecycle surface: regions, list/get/event
        // history and create/start/stop/restart/delete. Optional filters, force deletion and signal
        // bodies wait for C-30/C-56, and the event history is not misrepresented as a channel.
        ("fly", 9),
        // C-52's established 5, C-469's 4 exact first-party OpenAPI selections (repository issues,
        // pull-request files, workflow runs and commits) and C-527's 4 discovery reads (the
        // authenticated user, their organisations, an organisation's repositories and their own).
        // The C-469 four still retain integer page/per_page only — that is now a *compatibility*
        // bound on published request bytes rather than the safety one it began as, since C-30 landed
        // structured query encoding. The C-527 four therefore carry the vendor's scalar filters.
        // See the header comment in `providers/github.toml`.
        ("github", 13),
        // C-51's established 4 now sit beside C-472's 4 exact first-party OpenAPI selections:
        // stored response get/input items, files and batches. Integer limits survive; string
        // cursors and expansion filters remain excluded pending C-30. See `providers/openai.toml`.
        ("openai", 8),
        // C-76 curates 4 of the 68 paths in `https://openrouter.ai/openapi.json`: the models list,
        // one model's upstream endpoints, chat completion and the credit balance. The story asked for
        // roughly three including `GET /api/v1/generation`, which is excluded because its generation
        // id is a **required** query parameter — the query-encoding gap again, and here it removed an
        // operation outright rather than trimming one. See the header comment in
        // `providers/openrouter.toml`.
        ("openrouter", 4),
        ("slack", 4),
        // C-69 curates 8 across **three services** — gmail (message get, message send, labels list),
        // calendar (event get, event insert, calendar get) and drive (file get, file metadata
        // update). The cut is the query-encoding gap once more, and it bites hardest here: Gmail's
        // and Drive's `q` search syntaxes, every `pageToken` and every `fields=` projection are
        // excluded, which is also why the one listing that survives — `labels.list` — is the one
        // Google gives no parameters at all. C-56 is why every declared body field is required. See
        // the header comment in `providers/google.toml`.
        ("google", 8),
        // C-71 curates 5 of some 300 operations in `Asana/openapi`: task get, create, update, a
        // comment, and project get. The cut is the same query-encoding gap — `opt_fields`,
        // `limit`/`offset` paging and task search are all query values. See `providers/asana.toml`.
        ("asana", 5),
        // C-72 curates 5 over `/crm/v3/objects`: contact get, create and update, company get and
        // deal get. The cut is the query-encoding gap again — HubSpot's search endpoints and every
        // `properties=` projection are excluded. See `providers/hubspot.toml`.
        ("hubspot", 5),
        // C-70 curates 6 of the Jira Cloud platform API's several hundred, all on the v2 issue
        // resource tree. Two cuts shaped it: JQL search is excluded pending C-30 — a `jql` value is
        // the most injectable query string in this fleet — and issue update is excluded pending
        // C-56, because an update whose untouched fields travel as explicit nulls *clears* them on
        // Jira. See the header comment in `providers/jira.toml`.
        ("jira", 6),
        // C-73 curates 5: contact get and create, conversation get and reply, contact note. The cut
        // is the same query-encoding gap — every listing endpoint pages with an opaque
        // `starting_after` cursor — plus C-56, which is why every declared body field is required.
        // See the header comment in `providers/intercom.toml`.
        ("intercom", 5),
        // C-74 curates 5 of the Admin REST API, all addressed by an integer id in the path. The cut
        // is the query-encoding gap again — every Shopify collection endpoint filters on merchant
        // text or pages with an opaque `page_info` cursor — plus C-56 for the body of the one write.
        // See `providers/shopify.toml`.
        ("shopify", 5),
        // C-75 curates 4, all on the single-record surface at `/v0/{baseId}/{tableIdOrName}`: record
        // get, create, update and delete. Two cuts shaped it. `listRecords` and every other
        // collection read are excluded pending C-30 — `filterByFormula` is not merely a query string
        // but a formula language whose ordinary syntax is made of the characters that corrupt an
        // unencoded one — and `PUT` record replace is excluded because under C-56 every column the
        // caller did not restate would travel as an explicit `null` and be cleared, which is why the
        // shipped update is the sparse `PATCH`. See the header comment in `providers/airtable.toml`.
        ("airtable", 4),
        // C-77 curates 4 over `/api/0`: issue get, issue update, project get and the issue's latest
        // event. The cut is the query-encoding gap once more — Sentry's issues *list* takes a `query`
        // in its own search syntax, the most injectable value the API exposes — plus C-56, which is
        // why the one write sets `status` and nothing else. See `providers/sentry.toml`.
        ("sentry", 4),
        // C-78 curated 4 of the several hundred operations in Zoom's Meeting API and **C-430 left
        // 2**: meeting delete and user get. Three cuts shaped it — meeting get and meeting create
        // are *withheld by rule*, because both return `start_url`, a URL embedding the host's ZAK
        // token; the meetings *list* is excluded pending C-30; and every meeting option but
        // `settings.waiting_room` was excluded pending C-56. See the header comment in
        // `providers/zoom.toml`.
        ("zoom", 2),
    ];
    for (name, count) in expected {
        let loaded = load(name);
        assert_eq!(
            loaded.connector.operations.len(),
            count,
            "providers/{name}.toml selects {} operations; the inventory selects {count}",
            loaded.connector.operations.len()
        );
    }
}

/// Every operation id has to be spellable as a Flux declaration name. flux-lang admits ASCII
/// alphanumerics, `_` and `-` only, so a dotted id such as `zendesk.ticket.show` is undeclarable
/// (C-8, `crates/connector-flux/src/op.rs:108`) and would emit text that does not parse.
#[test]
fn operation_ids_are_declarable_in_flux() {
    for name in shipped() {
        let loaded = load(&name);
        for operation in &loaded.connector.operations {
            assert!(
                !operation.id.is_empty()
                    && operation
                        .id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "operation `{}` in providers/{name}.toml is not a spellable Flux declaration name",
                operation.id
            );
        }
    }
}

/// Babelforce ships SSO-issued Bearer. The legacy `X-Auth-Access-Id` / `X-Auth-Access-Token` pair is
/// deprecated and is being removed from the API (`provider-operation-inventory.md` §5.1.3); it must
/// not be modelled or emitted, and its absence is the point rather than an oversight.
///
/// The check is structural rather than a text scan of the file: the file *does* name those headers,
/// in the comment that tells a future reader not to add them back, and a grep could not tell that
/// warning apart from a declaration. What matters is that nothing reaches the IR — no second
/// credential, no non-bearer scheme, no caller-supplied auth header smuggled in as a parameter.
#[test]
fn babelforce_is_bearer_only_and_never_the_deprecated_header_pair() {
    let loaded = load("babelforce");
    let connector = &loaded.connector;

    assert_eq!(
        connector.auth.len(),
        1,
        "babelforce declares {} credentials; the excluded header pair is the only reason it would \
         ever be more than one",
        connector.auth.len()
    );
    let method = &connector.auth[0];
    assert_eq!(method.name, "babelforce.access_token");
    assert_eq!(
        method.scheme,
        AuthScheme::Bearer,
        "babelforce's credential is not a bearer; the deprecated pair is `header`-schemed"
    );
    assert_eq!(method.env, ["BABELFORCE_ACCESS_TOKEN"]);

    // Every operation resolves to the one bearer, whether it declares auth or inherits the default.
    for operation in &connector.operations {
        let effective = connector.effective_auth(operation);
        assert_eq!(
            effective.len(),
            1,
            "operation `{}` has {} auth alternatives; babelforce is single-mechanism",
            operation.id,
            effective.len()
        );
        assert!(
            effective[0].contains("babelforce.access_token") && effective[0].len() == 1,
            "operation `{}` names a credential other than the bearer",
            operation.id
        );
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; auth headers are injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// Zendesk's Basic user half is `<email>/token` — an env value **plus a literal suffix**. The
/// suffix has to be declared, not baked into `ZENDESK_USER`: a pre-composed credential stores a
/// value that is not the thing it is named after and that nothing can validate
/// (`docs/designs/auth-seam.md` §7.5).
#[test]
fn zendesk_declares_the_token_suffix_rather_than_pre_composing_it() {
    let loaded = load("zendesk");
    let method = loaded
        .connector
        .auth_method("zendesk.api_token")
        .expect("zendesk declares `zendesk.api_token`");

    assert_eq!(method.scheme, AuthScheme::Basic);
    assert_eq!(method.env, ["ZENDESK_API_TOKEN"]);
    assert_eq!(method.user_env, ["ZENDESK_USER"]);
    assert_eq!(method.user_suffix.as_deref(), Some("/token"));
}

/// No credential value may appear in a provider file — environment variable *names* only
/// (AGENTS.md). A cheap structural check: nothing that looks like an assignment of a secret.
#[test]
fn no_provider_file_carries_a_credential_value() {
    for name in shipped() {
        let loaded = load(&name);
        for method in &loaded.connector.auth {
            for key in method.env.iter().chain(&method.user_env) {
                assert!(
                    key.chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                    "providers/{name}.toml: `{key}` is not an environment variable name"
                );
            }
        }
    }
}

/// **One schema per operation, covering everything it receives.**
///
/// The measurement this is written against: of the shipped operations, all but six declare
/// parameters, yet nothing said what an operation *receives* as a single schema — so every consumer
/// re-derived it and disagreed at the corners (C-125, `docs/designs/member-io-schemas.md` §1). This
/// asserts the composition over every shipped operation rather than a fixture, because the corners
/// are in the real data: babelforce's dotted `time.start`, zendesk's `const`-pinned `safe_update`,
/// babelforce's two free-form `body_schema` bodies, and six operations that take nothing at all.
///
/// Four claims, and each one is a corner a re-deriving consumer got wrong:
///
/// 1. it is always an `object` schema carrying both `properties` and `required`, present even when
///    empty — "takes nothing" is a real answer;
/// 2. every declared parameter is a property, keyed by its **caller-facing** name and carrying its
///    declared schema **verbatim** — composition, not enrichment;
/// 3. a free-form `body_schema` is the one property [`connector_spec::FREE_FORM_BODY`] names, which
///    is the same name the emitted `op` declares it under;
/// 4. `required` is *exactly* the required parameters — not everything, not nothing.
#[test]
fn every_operation_composes_an_input_schema_covering_its_parameters() {
    for name in shipped() {
        let connector = load(&name).connector;
        for operation in &connector.operations {
            let id = &operation.id;
            let schema = operation.input_schema();

            assert_eq!(
                schema["type"], "object",
                "providers/{name}.toml: `{id}` composes {schema}, which is not an object schema"
            );
            let properties = schema["properties"].as_object().unwrap_or_else(|| {
                panic!("providers/{name}.toml: `{id}` composes no `properties` object")
            });
            let required: Vec<&str> = schema["required"]
                .as_array()
                .unwrap_or_else(|| {
                    panic!("providers/{name}.toml: `{id}` composes no `required` array")
                })
                .iter()
                .map(|value| {
                    value.as_str().unwrap_or_else(|| {
                        panic!("providers/{name}.toml: `{id}` names a non-string in `required`")
                    })
                })
                .collect();

            for param in operation.params.iter() {
                let property = properties.get(&param.name).unwrap_or_else(|| {
                    panic!(
                        "providers/{name}.toml: `{id}` declares parameter `{}`, which is not in its \
                         composed input schema",
                        param.name
                    )
                });
                assert_eq!(
                    property, &param.schema,
                    "providers/{name}.toml: `{id}` composes a different schema for `{}` than the \
                     one it declares",
                    param.name
                );
                assert_eq!(
                    required.contains(&param.name.as_str()),
                    param.required,
                    "providers/{name}.toml: `{id}` declares `{}` as required={} and composes the \
                     opposite",
                    param.name,
                    param.required
                );
            }

            if let Some(body) = &operation.params.body_schema {
                let property = properties
                    .get(connector_spec::FREE_FORM_BODY)
                    .unwrap_or_else(|| {
                        panic!(
                            "providers/{name}.toml: `{id}` declares a free-form body, which is not \
                             in its composed input schema"
                        )
                    });
                assert_eq!(property, body);
                assert!(
                    required.contains(&connector_spec::FREE_FORM_BODY),
                    "providers/{name}.toml: `{id}` composes an optional free-form body, but the \
                     body is the request"
                );
            }

            // Nothing invented: the properties are the declared parameters and the free-form body,
            // and no more. A constant header is deliberately not among them — it is not a
            // parameter, and nothing about it is caller-supplied.
            let expected = operation.params.iter().count()
                + usize::from(operation.params.body_schema.is_some());
            assert_eq!(
                properties.len(),
                expected,
                "providers/{name}.toml: `{id}` composes {} properties for {expected} declarations",
                properties.len()
            );
            let expected_required = operation
                .params
                .iter()
                .filter(|param| param.required)
                .count()
                + usize::from(operation.params.body_schema.is_some());
            assert_eq!(
                required.len(),
                expected_required,
                "providers/{name}.toml: `{id}` requires {:?}, which is not exactly its required \
                 parameters",
                required
            );
        }
    }
}

/// An operation that takes nothing composes an **empty object schema, not absence.**
///
/// The asymmetry with the output side is the point (`docs/designs/member-io-schemas.md`): "this
/// operation takes nothing" is a real, derivable answer, while "we do not know what it returns" is
/// not — so a response schema publishes absence and an input schema never does. Six shipped
/// operations are in this state; `openai-models-list` is one.
#[test]
fn an_operation_that_takes_nothing_composes_an_empty_object_schema() {
    let connector = load("openai").connector;
    let operation = connector
        .operations
        .iter()
        .find(|operation| operation.id == "openai-models-list")
        .expect("providers/openai.toml declares `openai-models-list`");

    assert!(operation.params.is_empty());
    assert_eq!(
        operation.input_schema(),
        serde_json::json!({"type": "object", "properties": {}, "required": []})
    );
}

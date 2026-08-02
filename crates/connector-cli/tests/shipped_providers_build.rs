//! **Every shipped provider compiles**, through the real pipeline, against the real repository.
//!
//! `connector-spec`'s `shipped_providers.rs` proves the committed definitions *load*. That is
//! a strictly weaker claim than this one: a definition can load and still be something the emitter
//! refuses, or — worse — something it compiles into a request the vendor accepts and ignores. Both
//! happened, and both were invisible to every other test in the tree:
//!
//! - `babelforce-agent-status-update` declared a body field named `presence.name`, which the emitter
//!   refused. One refusal aborted the whole run, so **no `.flux` file was generated at all**.
//! - `providers/zendesk.toml` carried each body field's wire path in its `description` prose, so the
//!   three `PUT /api/v2/tickets/{ticket_id}.json` operations emitted a **flat** body. Zendesk ignores
//!   a flat body and answers 200. Nothing in the IR distinguished that from a correct result, and
//!   nothing here would have caught it either — which is why the nested-body assertion below is
//!   spelled out field by field rather than left to "it compiles".
//!
//! The test reads `providers/` from the repository root rather than a fixture, because the thing
//! under test is what ships. It goes through [`pipeline::plan`], which performs no writes, so it can
//! run before `build` has ever been executed and cannot disturb the committed artifacts.

use std::path::{Path, PathBuf};

use connector_cli::pipeline::{self, PlannedArtifact};
use connector_cli::workspace::Workspace;

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn workspace() -> Workspace {
    Workspace::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository root exists"),
    )
}

/// Every provider this repository ships, **read from `providers/` rather than listed here** (C-54).
///
/// This file is where the drift cost: `every_shipped_provider_compiles` and
/// `every_shipped_operation_reaches_its_module` are the two gates a new connector most needs, and a
/// constant that someone forgot to widen is a constant that skips them without failing. Reading the
/// directory means the gate covers whatever ships. Empty is a failure rather than a vacuous pass.
fn shipped() -> Vec<String> {
    let dir = workspace().providers_dir();
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

/// The artifacts a build would write for `provider`, or a panic naming why it could not.
fn plan_for(provider: &str) -> Vec<PlannedArtifact> {
    pipeline::plan(&workspace(), Some(provider))
        .unwrap_or_else(|error| panic!("providers/{provider}.toml does not compile: {error:#}"))
        .artifacts
}

/// The shipped provider definition, loaded.
fn load(provider: &str) -> connector_spec::Connector {
    let source =
        std::fs::read_to_string(workspace().providers_dir().join(format!("{provider}.toml")))
            .expect("the shipped provider file is readable");
    shipped_provider::load_definition(provider, &source)
        .expect("the shipped provider file loads")
        .connector
}

/// Whether the plan writes `path`, spelled relative to the repository root.
fn plans(artifacts: &[PlannedArtifact], path: &str) -> bool {
    let wanted = PathBuf::from(path);
    artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with(&wanted))
}

/// The file name of the module one service installs — `<provider>-<service>.flux`, or
/// `<provider>.flux` for the reserved `default` service (C-49).
///
/// Derived from the workspace's own path helper rather than restating the elision rule here: a second
/// spelling of it is a second thing to keep in step, and this file would then disagree with the
/// build about which file it is asserting on.
fn module_file(provider: &str, service: &str) -> String {
    file_name(workspace().service_module_path(provider, service))
}

/// The file name of the manifest that installs alongside [`module_file`].
fn manifest_file(provider: &str, service: &str) -> String {
    file_name(workspace().service_manifest_path(provider, service))
}

/// The babelforce service the nine curated operations live in — C-417.
///
/// They used to live in `connectors/babelforce.flux`, because a connector with no `[[services]]`
/// entry publishes one unnamed unit. Widening to the whole manager-sdk surface made babelforce a
/// five-document, five-service connector, so the installable unit that carries agents, calls and
/// sessions is `connectors/babelforce-manager.flux`. Named once here rather than spelled in each
/// test, because it is the same fact three times.
const MANAGER: &str = "manager";

fn file_name(path: PathBuf) -> String {
    path.file_name()
        .expect("an artifact path names a file")
        .to_string_lossy()
        .into_owned()
}

/// The planned contents of one artifact path.
fn planned(provider: &str, file: &str) -> String {
    let artifacts = plan_for(provider);
    let wanted = PathBuf::from(file);
    artifacts
        .into_iter()
        .find(|artifact| artifact.path.ends_with(&wanted))
        .unwrap_or_else(|| panic!("a build of {provider} must plan {file}"))
        .contents
}

/// **The gate this story exists to open.** Every shipped provider compiles to both its artifacts.
///
/// One provider the emitter refuses aborts the whole run — `plan` compiles everything before
/// anything is written — so a single unemittable operation is the difference between three
/// generated modules and none.
#[test]
fn every_shipped_provider_compiles() {
    for provider in shipped() {
        let provider = provider.as_str();
        let artifacts = plan_for(provider);
        let connector = load(provider);
        let operations = connector.operations.len();
        let services = connector.service_names();

        // The pair that ships, **once per service** (C-49): a connector is a module *plus* a
        // manifest, never one of them, and for a multi-service provider that pair is per surface
        // rather than per vendor.
        for service in &services {
            for expected in [
                module_file(provider, service),
                manifest_file(provider, service),
            ] {
                assert!(
                    plans(&artifacts, &format!("connectors/{expected}")),
                    "a build of {provider} must plan connectors/{expected} for service `{service}`"
                );
            }
        }

        // The catalog's half (C-38) is provider-unit rather than per service: the generated table,
        // plus one rendering per operation.
        assert!(
            plans(
                &artifacts,
                &format!("crates/catalog/src/generated/{provider}.rs")
            ),
            "a build of {provider} must plan its catalog table"
        );
        assert_eq!(
            artifacts.len(),
            2 * services.len() + 1 + operations,
            "{provider} publishes {operations} operations across {} service(s), so a build plans a \
             module and a manifest per service, one catalog table, and one rendering per operation; \
             it planned {} artifacts",
            services.len(),
            artifacts.len()
        );
    }
}

/// Every generated module declares every operation its provider does — an emitter that dropped one
/// would still produce a module that parses.
///
/// **In its own service's module** (C-49). The services partition the operation set, so each module
/// carries exactly its own service's operations; asserting only that the operation appears *somewhere*
/// under the provider would still pass if it were emitted into a sibling service's installable unit,
/// which is a wrong `http_hosts` and a wrong address away from correct.
#[test]
fn every_shipped_operation_reaches_its_module() {
    for provider in shipped() {
        let provider = provider.as_str();
        let connector = load(provider);

        for service in connector.service_names() {
            let module = planned(provider, &module_file(provider, service));

            for operation in connector.operations_of(service) {
                assert!(
                    module.contains(&format!("op {}(", operation.id))
                        || module.contains(&format!("op {} ", operation.id)),
                    "connectors/{} does not declare `{}`:\n{module}",
                    module_file(provider, service),
                    operation.id
                );
            }
            for other in &connector.operations {
                if other.service == service {
                    continue;
                }
                // **The same two delimiters the positive check uses, and for the same reason.**
                // A bare `op {id}` prefix-matches every longer id that starts with this one, so
                // babelforce's `op babelforce-authorize-integration(` — a manager operation — read
                // as the `auth` service's `babelforce-authorize` leaking into the manager module.
                // That was a false accusation of the one defect this assertion exists to catch,
                // and it only became reachable once a provider shipped enough operations for two
                // of its ids to be prefixes of each other.
                assert!(
                    !module.contains(&format!("op {}(", other.id))
                        && !module.contains(&format!("op {} ", other.id)),
                    "connectors/{} declares `{}`, which belongs to service `{}`",
                    module_file(provider, service),
                    other.id,
                    other.service
                );
            }
        }
    }
}

/// Zendesk's spec-selected update contract keeps the vendor's `ticket` request wrapper. A caller
/// supplies that object whole; the generated Flux must neither flatten it nor reconstruct a second,
/// hand-maintained ticket schema.
#[test]
fn zendesk_writes_a_nested_body() {
    let module = planned("zendesk", "zendesk.flux");

    assert!(
        module.contains("op zendesk-ticket-update(ticket_id: Number, ticket: Any) -> Any")
            && module.contains("payload = { ticket }"),
        "`zendesk-ticket-update` must preserve the vendor-declared ticket wrapper:\n{module}"
    );
    assert!(
        !module.contains("op zendesk-ticket-comment-add(")
            && !module.contains("op zendesk-ticket-tag-add("),
        "the two hand-authored UpdateTicket aliases must not survive beside the vendor operation:\n{module}"
    );
}

/// babelforce's agent-status update writes `presence.name`, and the label is **nested** — it never
/// reaches the root of the body. This is the refusal quoted in the story: it is the operation that
/// stopped the whole build, and it is loud rather than silent only because the dotted spelling
/// happened to be visible in the field's `name`.
///
/// **Asserted as the body's root key set rather than as one payload line** (C-421). The two
/// front-ends spell the same wire body differently and both are correct: hand-authored, the label is
/// a scalar `presence_name` carrying `wire = "presence.name"`, and the emitter builds the nesting
/// (`payload = { enabled, presence: { name: presence_name } }`); through the spec route, ingest
/// expands only the top level of a request body, so `presence` arrives as one object-typed parameter
/// and the caller supplies the nesting (`payload = { enabled, presence }`). Either way the vendor
/// receives `{"enabled": …, "presence": {"name": …}}`, and either way a *flat* `name` at the root —
/// the mistake this test exists to catch, and the one the vendor would accept and ignore — fails it.
#[test]
fn babelforce_nests_the_presence_label() {
    let module = planned("babelforce", &module_file("babelforce", MANAGER));
    let payload = payload_of(&module, "babelforce-agent-status-update");

    assert_eq!(
        root_keys(&payload),
        vec!["enabled", "presence"],
        "`babelforce-agent-status-update` must send the presence label under `presence`, never at \
         the root of the body — a root `name` is a request babelforce answers without applying:\n\
         {payload}"
    );
}

/// A free-form object body reaches the request whole. Before `ParamSet::body_schema` existed
/// `babelforce-session-update` declared no body parameter at all and emitted a `PUT` with no body,
/// which is indistinguishable from a legitimately bodiless write.
///
/// **Scoped to `babelforce-session-update`, which is the operation that is free-form in both
/// front-ends** (C-421). It used to count two such bodies, the second being
/// `babelforce-call-session-set` — and that operation's shape is a *vendor* question this repository
/// cannot settle offline, not a property of the emitter. The hand-authored file declared its body as
/// a bare free-form map (`{"app.priority": "high"}`) from the 0.7.0 document, in which
/// `SetCallSessionVariablesRequest` carried no `properties`; the 2026-07-10 document declares that
/// same schema as a wrapper with one `variables` property, so the body is
/// `{"variables": {"app.priority": "high"}}`. At most one of those is the request babelforce
/// applies, and only a live call can say which — see C-416's Progress §(a). Counting them together
/// made this test's verdict depend on that open question; it does not, and now it does not say so.
#[test]
fn babelforce_sends_its_free_form_session_bodies() {
    let module = planned("babelforce", &module_file("babelforce", MANAGER));
    let payload = payload_of(&module, "babelforce-session-update");

    assert_eq!(
        payload, "parse(body, as: \"json\")",
        "`babelforce-session-update` must send the caller's body whole rather than re-describing \
         it:\n{module}"
    );
}

/// The `payload = …` expression one operation's emitted `op` declaration binds.
///
/// Read out of the module by walking forward from the declaration, so a second operation's payload
/// cannot answer for the one being asked about — which a `module.contains(…)` cannot promise and
/// which is the whole difference between the two assertions above and the literal matches they
/// replaced.
fn payload_of(module: &str, operation: &str) -> String {
    let start = module
        .find(&format!("op {operation}("))
        .or_else(|| module.find(&format!("op {operation} ")))
        .unwrap_or_else(|| panic!("the module declares `{operation}`:\n{module}"));
    let rest = &module[start..];
    let end = rest[1..]
        .find("\nop ")
        .map_or(rest.len(), |offset| offset + 1);

    rest[..end]
        .lines()
        .find_map(|line| line.trim().strip_prefix("payload = "))
        .unwrap_or_else(|| panic!("`{operation}` binds no payload:\n{}", &rest[..end]))
        .trim()
        .to_owned()
}

/// The keys a record-literal payload writes at its **root**, in the order it writes them.
///
/// `{ enabled, presence: { name: presence_name } }` and `{ enabled, presence }` both answer
/// `["enabled", "presence"]`: the point is what the vendor sees at the top of the body, not how the
/// value under each key was assembled.
fn root_keys(payload: &str) -> Vec<&str> {
    let inner = payload
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap_or_else(|| panic!("not a record literal: {payload}"));

    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut field_start = 0usize;
    for (index, character) in inner.char_indices() {
        match character {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                keys.push(&inner[field_start..index]);
                field_start = index + 1;
            }
            _ => {}
        }
    }
    keys.push(&inner[field_start..]);

    keys.into_iter()
        // `key: value` is a binding under `key`; a bare `key` is shorthand for `key: key`.
        .map(|field| field.split(':').next().unwrap_or(field).trim())
        .filter(|key| !key.is_empty())
        .collect()
}

/// **Slack's arguments travel in the body, and nothing reaches the URL.** The mirror image of the
/// freshdesk assertion below: where freshdesk pins that a query value is spelled the vendor's way,
/// this pins that Slack has no query value to spell at all.
///
/// Asserted through the real pipeline rather than only over the IR, because the IR and the emitted
/// request can disagree — that is the whole premise of this file. Slack's body is flat, so the
/// payload is a single record with no nesting, and the URL is the method name and nothing else. A
/// read converted to a GET would break both halves at once.
#[test]
fn slack_sends_its_arguments_in_the_body_and_nothing_in_the_url() {
    let module = planned("slack", "slack.flux");

    assert!(
        module.contains("payload = { channel: $channel, text, thread_ts }"),
        "`slack-chat-post-message` must send its arguments as a flat JSON body:\n{module}"
    );
    assert!(
        module.contains(r#"url = fmt("{base}/api/conversations.history")"#),
        "`slack-conversations-history` must address the bare method path, with no query string \
         — an opaque channel id in a query value cannot be percent-encoded (C-30):\n{module}"
    );
    // The emitter binds `sep` only to carry a `?`/`&` between query parameters, so its absence is
    // a structural proof that no value was spliced into any of the four URLs.
    //
    // Match the binding, not the bare name: flux-lang 0.39 dropped the `$` sigil from local
    // bindings, so the old `contains("$sep")` spelling became vacuously true and would have passed
    // no matter what this emitter did.
    assert!(
        !module.contains("sep = ") && !module.contains('?'),
        "no Slack operation may assemble a query string:\n{module}"
    );
}

/// **Intercom's egress host is one of three enumerated regions, and its credential never leaves the
/// manifest as anything but a name** (C-73, C-225).
///
/// Two claims that the IR-level test in `crates/connector-flux/tests/intercom_connector.rs` cannot
/// make, because both are properties of what the *pipeline* derives rather than of what the provider
/// file declares:
///
/// - `http_hosts` is derived from `base_url` by `catalog::host_of` and is published to consumers in
///   `web/public/catalog.json`. A widened entry — an unlisted host, or a `*` — would enlarge the
///   egress allow-list of every operation at once, and nothing else in the tree would notice. Until
///   C-225 the base URL was the literal `api.intercom.io` and this test pinned that string;
///   Intercom's other regions (`api.eu.intercom.io`, `api.au.intercom.io`) were the tempting second
///   entry and the file recorded why they were a separate connector instead. They are now the
///   declared **closed set** of a `{host}` field, which is narrower than a widened list in the way
///   that matters: the set of reachable hosts is still enumerable from the artifacts, and
///   `uploads.intercom.io` and the app hosts are in neither the set nor the manifest.
/// - **The generated module names no credential at all.** Not the value, which does not exist in this
///   repository, and not even the environment variable: the bearer is applied by the host at the
///   `$auth` seam (`docs/designs/auth-seam.md`), so `INTERCOM_ACCESS_TOKEN` belongs in the manifest's
///   credential *reference* and must never appear in Flux a model can read. Asserting the name is
///   present in the manifest is what keeps the absence check from passing vacuously.
#[test]
fn intercom_publishes_a_closed_set_of_hosts_and_no_credential_in_its_module() {
    const TOKEN_ENV: &str = "INTERCOM_ACCESS_TOKEN";
    const REGIONS: [&str; 3] = [
        "api.intercom.io",
        "api.eu.intercom.io",
        "api.au.intercom.io",
    ];

    let connector = load("intercom");
    let module = planned("intercom", "intercom.flux");
    let manifest = planned("intercom", "intercom.connector.toml");

    assert_eq!(
        connector.base_url, "https://{host}",
        "the base URL is what the host is derived from, so widening it widens the allow-list"
    );
    assert!(
        module.contains(r#"base = "https://{host}""#),
        "every Intercom request must address the region the operator bound:\n{module}"
    );
    // The set is published, not merely declared: the manifest is where a host reads which three
    // values that `{host}` may take, and a set that reached no artifact would leave a consumer
    // rendering a text box and accepting anything.
    for region in REGIONS {
        assert!(
            manifest.contains(region),
            "the manifest must publish `{region}` as a permitted host:\n{manifest}"
        );
    }
    assert!(
        !manifest.contains("uploads.intercom.io"),
        "the upload host is not reachable from any operation and must not be offered:\n{manifest}"
    );
    assert!(
        !module.contains('*') && !manifest.contains('*'),
        "no Intercom artifact may carry a wildcard host:\n{module}\n{manifest}"
    );

    assert!(
        !module.contains(TOKEN_ENV) && !module.contains("access_token"),
        "connectors/intercom.flux names a credential; the bearer is applied by the host at the \
         `$auth` seam and generated Flux must name nothing:\n{module}"
    );
    assert!(
        connector
            .auth_method("intercom.access_token")
            .is_some_and(|method| method.env == [TOKEN_ENV]),
        "the connector must reference `{TOKEN_ENV}` by name, or the absence check above passes \
         vacuously"
    );
}

/// **Google's per-service egress is narrow and no credential value reaches any artifact** (C-69).
///
/// Two claims the IR-level test in `crates/connector-flux/tests/google_connector.rs` cannot make,
/// because both are properties of what the *pipeline* derives rather than of what the provider file
/// declares:
///
/// - **Each service's manifest names its own host and nothing wider.** `http_hosts` is C-10's and does
///   not exist yet; the manifest's `base_url` is the value it will derive from, so a widened one — a
///   `*.googleapis.com`, or the union of the provider's hosts — would enlarge one service's egress
///   allow-list to cover APIs it never calls. Google is the first shipped provider where "the union"
///   is even a different string, so it is the first that can regress this way.
/// - **No credential value reaches a module, a manifest, or the published catalogue.** Not the value,
///   which does not exist in this repository, and — in generated Flux — not even the environment
///   variable: the bearer is applied by the host at the `$auth` seam
///   (`docs/designs/auth-seam.md`). `GOOGLE_ACCESS_TOKEN` therefore belongs in the credential
///   *reference* the catalogue publishes and must never appear in Flux a model can read. Asserting the
///   name is present there is what keeps the absence check from passing vacuously.
#[test]
fn google_publishes_one_host_per_service_and_no_credential_value() {
    const TOKEN_ENV: &str = "GOOGLE_ACCESS_TOKEN";
    /// The prefixes a real Google credential carries: an OAuth2 access token, an API key, and the
    /// client-secret key name that a leaked OAuth client would travel under.
    const VALUE_SHAPES: [&str; 3] = ["ya29.", "AIza", "client_secret"];
    /// The subset of [`VALUE_SHAPES`] that is a **value**, and therefore worth scanning artifacts
    /// no Google story owns.
    ///
    /// `ya29.` and `AIza` are prefixes of secrets: a string carrying either is a leaked credential
    /// whichever provider emitted it, so the catalogue-wide scan below keeps them. `client_secret`
    /// is a **key name**, and a key name is only evidence about the file it appears in. C-417 is
    /// where that distinction started mattering: babelforce's `auth` document declares `/oauth/token`
    /// and `/oauth/revoke`, whose form bodies take a parameter the vendor calls `client_secret`, and
    /// those operations publish `payload = fmt("{payload}&client_secret={client_secret}")` — a
    /// parameter binding with no value in it. Scanning the whole catalogue for the key name accused
    /// babelforce of leaking a *Google* credential, which is a false positive that would recur for
    /// every provider whose vendor documents an OAuth exchange. The per-artifact loop above still
    /// holds Google's own module and manifest to all three shapes, which is where the claim belongs.
    const LEAKED_VALUE_SHAPES: [&str; 2] = ["ya29.", "AIza"];

    let connector = load("google");
    assert!(
        !connector.is_default_only(),
        "this test is about a multi-service provider's per-service egress; google declaring one \
         surface would make every claim below vacuous"
    );

    for service in connector.service_names() {
        let expected = connector.base_url_of(service);
        let manifest = planned("google", &manifest_file("google", service));
        assert!(
            manifest.contains(&format!("base_url = \"{expected}\"")),
            "the `{service}` manifest must name its own host, never the provider's union:\n{manifest}"
        );
        assert!(
            !manifest.contains('*'),
            "no Google manifest may carry a wildcard host:\n{manifest}"
        );

        let module = planned("google", &module_file("google", service));
        assert!(
            module.contains(&format!("base = \"{expected}\"")),
            "every request in the `{service}` module must address {expected} — the host its manifest \
             declares:\n{module}"
        );
        assert!(
            !module.contains(TOKEN_ENV) && !module.contains("access_token"),
            "the `{service}` module names a credential; the bearer is applied by the host at the \
             `$auth` seam and generated Flux must name nothing:\n{module}"
        );

        for text in [&manifest, &module] {
            for shape in VALUE_SHAPES {
                assert!(
                    !text.contains(shape),
                    "a `{service}` artifact carries something shaped like a Google credential \
                     (`{shape}`):\n{text}"
                );
            }
        }
    }

    let catalogue = std::fs::read_to_string(workspace().root().join("web/public/catalog.json"))
        .expect("the committed public catalogue is readable");
    for shape in LEAKED_VALUE_SHAPES {
        assert!(
            !catalogue.contains(shape),
            "the published catalogue carries something shaped like a Google credential (`{shape}`)"
        );
    }
    assert!(
        catalogue.contains(TOKEN_ENV),
        "the catalogue must reference `{TOKEN_ENV}` by name, or the value checks above pass vacuously"
    );
    assert!(
        connector
            .auth_method("google.access_token")
            .is_some_and(|method| method.env == [TOKEN_ENV]),
        "the connector must reference `{TOKEN_ENV}` by name, so an operator knows what to set"
    );
}

/// **No test hand-maintains the shipped-provider set** (C-54). The set lives in `providers/`, and
/// every per-provider gate derives it from that directory; a constant repeating it is a second
/// source of truth that nothing keeps in step.
///
/// This is not a style preference. Five constants held the same six ids, and the copies drifted:
/// C-53 reached review with `slack` present in four of them and absent from the fifth, so
/// `every_shipped_provider_compiles` and `every_shipped_operation_reaches_its_module` never ran for
/// the connector under review. A list that must be edited in five places to stay correct will be
/// edited in four.
///
/// **What the scan reaches:** a `const` or `static` item beginning a line in any `*.rs` under
/// `crates/*/tests`, under any visibility (`pub`, `pub(crate)`, `pub(super)`, `pub(in …)`, or none),
/// whose text up to the `;` that ends the item quotes two or more ids from `providers/`.
///
/// **What it does not reach, by construction** — a list, so that nobody mistakes a green run for a
/// proof of absence:
///
/// - a `let` binding or a `Vec` built inside a function body;
/// - ids assembled rather than written — `include_str!`, `concat!`, a formatted string;
/// - two constants that each name a single provider, since the threshold is two ids in one item;
/// - `#[cfg(test)]` modules under `crates/*/src`, which this scan does not walk.
///
/// Those are accepted gaps, not oversights. This guard is aimed at the exact shape that regressed —
/// one constant per test file, holding the whole inventory — and the coverage it protects comes from
/// `shipped()` in each of those files, not from here. A determined author can still write a second
/// source of truth; what they cannot do is reintroduce this one by accident.
///
/// One gap is deliberate rather than merely tolerated: a per-provider claim inside a test body — the
/// curated operation counts in `connector-spec`'s `operation_selection_stays_curated`, for instance —
/// is an assertion about each provider rather than a copy of the provider set, and stays.
///
/// It lives in `connector-cli` because this is the crate whose tests already read the repository
/// tree (see [`workspace`] above); no other crate should grow that reach for one check.
#[test]
fn no_test_hand_maintains_a_shipped_provider_list() {
    let root = workspace().root().to_path_buf();

    // The ids the repository actually ships — the set a constant would be duplicating.
    let ids: Vec<String> = std::fs::read_dir(root.join("providers"))
        .expect("the repository's providers/ directory is readable")
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();
    assert!(!ids.is_empty(), "providers/ holds no provider definitions");

    let mut offenders: Vec<String> = Vec::new();
    for source in test_sources(&root) {
        let text = std::fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));

        for item in const_items(&text) {
            let named = ids
                .iter()
                .filter(|id| item.contains(&format!("\"{id}\"")))
                .count();
            if named >= 2 {
                let head = item.lines().next().unwrap_or(item).trim();
                offenders.push(format!(
                    "{} — `{head}` names {named} shipped providers",
                    source
                        .strip_prefix(&root)
                        .unwrap_or(&source)
                        .display()
                        .to_string()
                        .replace('\\', "/")
                ));
            }
        }
    }
    offenders.sort();

    assert!(
        offenders.is_empty(),
        "a test constant hand-lists the shipped providers; derive the set from `providers/` \
         instead, so adding a provider costs one file:\n  {}",
        offenders.join("\n  ")
    );
}

/// Every `*.rs` under `crates/*/tests`, recursively — the test tree this check governs.
fn test_sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let crates = std::fs::read_dir(root.join("crates")).expect("crates/ is readable");

    for entry in crates {
        let tests = entry
            .expect("readable directory entry")
            .path()
            .join("tests");
        if !tests.is_dir() {
            continue;
        }

        let mut pending = vec![tests];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
            {
                let path = entry.expect("readable directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    found.push(path);
                }
            }
        }
    }

    found
}

/// The text of every `const`/`static` item in `source`, from the keyword to the `;` that ends it.
///
/// A textual scan rather than a parse, on purpose: this check has to run over sibling crates' test
/// sources, and pulling a Rust parser into `connector-cli`'s dev-dependencies to read six lines
/// would cost more than the duplication it guards against.
///
/// A declaration counts when the keyword opens a line or follows nothing but a visibility qualifier.
/// The qualifier is the whole reason this is a function rather than one `find`: requiring an empty
/// prefix skipped `pub const`, so a single word evaded the check — and evading it silently, which is
/// worse than not having it.
fn const_items(source: &str) -> Vec<&str> {
    let mut items = Vec::new();

    for keyword in ["const ", "static "] {
        let mut cursor = 0;
        while let Some(offset) = source[cursor..].find(keyword) {
            let start = cursor + offset;
            cursor = start + keyword.len();

            // Nothing but optional visibility before the keyword. This rejects `as const`, a doc
            // comment mentioning the word, and a `const` inside a signature or a `let`, none of
            // which is an inventory; it accepts `pub const` and `pub(crate) static`, which are.
            let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
            if !opens_an_item(source[line_start..start].trim()) {
                continue;
            }

            // From the line start, not the keyword, so a failure quotes `pub const …` as written.
            items.push(source[line_start..item_end(source, start)].trim_start());
        }
    }

    items
}

/// The offset of the `;` that ends the item beginning at `start`, tracking bracket depth.
///
/// Not simply the next `;`: an array type spells its length after one, so `const SHIPPED: [&str; 6]`
/// would otherwise be cut off at `[&str` — before a single id — and pass. Same evasion as `pub`,
/// different spelling, so it is closed the same way rather than left to the next reviewer.
fn item_end(source: &str, start: usize) -> usize {
    let mut depth = 0usize;

    for (offset, character) in source[start..].char_indices() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => return start + offset,
            _ => {}
        }
    }

    source.len()
}

/// Whether `prefix` — everything on the line before `const`/`static` — leaves it a declaration.
///
/// Empty, or a visibility qualifier: `pub`, `pub(crate)`, `pub(super)`, `pub(in some::path)`.
fn opens_an_item(prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }

    match prefix.strip_prefix("pub") {
        Some(rest) => {
            let rest = rest.trim();
            rest.is_empty() || (rest.starts_with('(') && rest.ends_with(')'))
        }
        None => false,
    }
}

/// A parameter whose wire name differs from its caller-facing one travels under the **vendor's**
/// spelling, while the op declares the name a caller reads (inventory §4.2 op 2).
#[test]
fn freshdesk_query_aliases_travel_under_their_wire_name() {
    let module = planned("freshdesk", "freshdesk.flux");
    assert!(
        module.contains("fmt(\"{url}{sep}requester_id={req_id}\")"),
        "`req_id` must reach Freshdesk as `requester_id`:\n{module}"
    );
    assert!(
        module.contains("req_id: String"),
        "the op must declare the caller-facing name:\n{module}"
    );
}

/// **Airtable's egress host is exactly `api.airtable.com`, and its credential never leaves the
/// manifest as anything but a name** (C-75).
///
/// The same pair of claims `intercom_publishes_one_host_and_no_credential_in_its_module` makes, for
/// the same reason it lives here rather than in `crates/connector-flux/tests/airtable_connector.rs`:
/// both are properties of what the *pipeline* derives, not of what the provider file declares.
/// `http_hosts` comes out of `catalog::host_of(base_url)`, so a widened entry — a second host, or a
/// `*` — would enlarge the egress allow-list of all four operations at once and nothing else in the
/// tree would notice. Airtable's attachment CDN is the tempting second entry; `providers/airtable.toml`
/// records why it is not one.
///
/// The credential half is AGENTS.md's hard invariant. Not the value, which does not exist in this
/// repository, and not even the variable's name: the bearer is applied by the host at the `$auth`
/// seam (`docs/designs/auth-seam.md`), so `AIRTABLE_ACCESS_TOKEN` belongs in the manifest's credential
/// *reference* and must never appear in Flux a model can read. Asserting the name is present on the
/// connector is what keeps the absence check from passing vacuously.
#[test]
fn airtable_publishes_one_host_and_no_credential_in_its_module() {
    const TOKEN_ENV: &str = "AIRTABLE_ACCESS_TOKEN";

    let connector = load("airtable");
    let module = planned("airtable", "airtable.flux");
    let manifest = planned("airtable", "airtable.connector.toml");

    assert_eq!(
        connector.base_url, "https://api.airtable.com",
        "the base URL is what the host is derived from, so widening it widens the allow-list"
    );
    assert!(
        module.contains(r#"base = "https://api.airtable.com""#),
        "every Airtable request must address `api.airtable.com`:\n{module}"
    );
    assert!(
        !module.contains('*') && !manifest.contains('*'),
        "no Airtable artifact may carry a wildcard host:\n{module}\n{manifest}"
    );

    assert!(
        !module.contains(TOKEN_ENV) && !module.contains("access_token"),
        "connectors/airtable.flux names a credential; the bearer is applied by the host at the \
         `$auth` seam and generated Flux must name nothing:\n{module}"
    );
    assert!(
        connector
            .auth_method("airtable.access_token")
            .is_some_and(|method| method.env == [TOKEN_ENV]),
        "the connector must reference `{TOKEN_ENV}` by name, or the absence check above passes \
         vacuously"
    );
}
/// **OpenRouter's egress host is exactly `openrouter.ai`, and its credential never leaves the manifest
/// as anything but a name** (C-76).
///
/// The same pair of claims `intercom_publishes_one_host_and_no_credential_in_its_module` makes, and
/// made here for the same reason: both are properties of what the *pipeline* derives rather than of
/// what the provider file declares, so the IR-level test in
/// `crates/connector-flux/tests/openrouter_connector.rs` cannot reach either of them.
///
/// This one carries the check one artifact further than intercom's, to the **public catalogue**.
/// `web/public/catalog.json` is the one generated document that leaves the repository, and it is a
/// function of a *full* run rather than a provider-scoped one (`pipeline::plan`'s note), so it is
/// planned that way here. A credential resolved while assembling it would be published to the open
/// web, which is the worst destination available for the failure this invariant exists to prevent —
/// and `openrouter.ai` is a plausible place for someone to widen a host, since OpenRouter deliberately
/// fans requests out to a few hundred upstream vendors and none of those hosts belongs in this
/// connector's allow-list.
#[test]
fn openrouter_publishes_one_host_and_no_credential_anywhere() {
    const SECRET_ENV: &str = "OPENROUTER_API_KEY";
    const BASE_URL: &str = "https://openrouter.ai";

    let connector = load("openrouter");
    let module = planned("openrouter", "openrouter.flux");
    let manifest = planned("openrouter", "openrouter.connector.toml");

    assert_eq!(
        connector.base_url, BASE_URL,
        "the base URL is what the host is derived from, so widening it widens the allow-list"
    );
    assert!(
        module.contains(&format!(r#"base = "{BASE_URL}""#)),
        "every OpenRouter request must address `openrouter.ai`:\n{module}"
    );
    assert!(
        !module.contains('*') && !manifest.contains('*'),
        "no OpenRouter artifact may carry a wildcard host:\n{module}\n{manifest}"
    );
    assert_eq!(
        module.matches("https://").count(),
        connector.operations.len(),
        "each operation binds the one base URL and no other absolute URL:\n{module}"
    );

    // Not the value, which does not exist in this repository, and not even the variable's name: the
    // bearer is applied by the host at the `$auth` seam (`docs/designs/auth-seam.md`), so
    // `OPENROUTER_API_KEY` belongs in a credential *reference* and must never appear in Flux a model
    // can read.
    assert!(
        !module.contains(SECRET_ENV) && !module.contains("api_key"),
        "connectors/openrouter.flux names a credential; generated Flux must name nothing:\n{module}"
    );
    // OpenRouter's keys are `sk-or-`-prefixed, so the shape is checked as well as the name.
    assert!(
        !module.contains(SECRET_ENV_VALUE_SHAPE) && !manifest.contains(SECRET_ENV_VALUE_SHAPE),
        "an OpenRouter artifact embeds something shaped like a key:\n{module}\n{manifest}"
    );
    assert!(
        connector
            .auth_method("openrouter.api_key")
            .is_some_and(|method| method.env == [SECRET_ENV]),
        "the connector must reference `{SECRET_ENV}` by name, or the absence checks above pass \
         vacuously"
    );

    // The public catalogue, which only a full run assembles — and the artifact that leaves the
    // repository, so its entry is read structurally rather than scanned. A text scan for `*` would
    // fail on another provider's prose: Jira's comment description quotes wiki markup (`*bold*`).
    let full = pipeline::plan(&workspace(), None).expect("the whole repository compiles");
    let catalogue = full
        .artifacts
        .iter()
        .find(|artifact| artifact.path.ends_with(PathBuf::from("catalog.json")))
        .expect("a full build plans the public catalogue");
    let document: serde_json::Value =
        serde_json::from_str(&catalogue.contents).expect("the public catalogue is JSON");
    let entry = document["providers"]
        .as_array()
        .expect("the catalogue lists providers")
        .iter()
        .find(|provider| provider["id"] == "openrouter")
        .expect("the public catalogue describes openrouter, or the checks below pass vacuously");

    let expected_hosts = serde_json::json!(["openrouter.ai"]);
    assert_eq!(
        entry["hosts"], expected_hosts,
        "the published host list is not exactly what `base_url` derives"
    );
    for operation in entry["operations"]
        .as_array()
        .expect("the entry lists operations")
    {
        assert_eq!(
            operation["hosts"], expected_hosts,
            "operation `{}` publishes a wider egress surface than the connector's",
            operation["id"]
        );
    }
    assert!(
        !catalogue.contents.contains(SECRET_ENV_VALUE_SHAPE),
        "the public catalogue carries something shaped like an OpenRouter key, and this document is \
         the one generated artifact that leaves the repository"
    );
}

/// The prefix every OpenRouter API key carries (`sk-or-v1-…`). Checked for by shape as well as by
/// name, because a leaked key would not be spelled `OPENROUTER_API_KEY`.
const SECRET_ENV_VALUE_SHAPE: &str = "sk-or-";

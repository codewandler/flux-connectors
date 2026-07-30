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
    connector_spec::provider::load(&format!("providers/{provider}.toml"), &source)
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
        let operations = load(provider).operations.len();

        // The two that ship. A connector is a module *plus* a manifest, never one of them.
        assert!(
            plans(&artifacts, &format!("connectors/{provider}.flux")),
            "a build of {provider} must plan connectors/{provider}.flux"
        );
        assert!(
            plans(&artifacts, &format!("connectors/{provider}.connector.toml")),
            "a build of {provider} must plan connectors/{provider}.connector.toml"
        );

        // The catalog's half (C-38): the generated table, plus one rendering per operation.
        assert!(
            plans(
                &artifacts,
                &format!("crates/catalog/src/generated/{provider}.rs")
            ),
            "a build of {provider} must plan its catalog table"
        );
        assert_eq!(
            artifacts.len(),
            3 + operations,
            "{provider} publishes {operations} operations, so a build plans its module, its \
             manifest, its catalog table and one rendering each; it planned {} artifacts",
            artifacts.len()
        );
    }
}

/// Every generated module declares every operation its provider does — an emitter that dropped one
/// would still produce a module that parses.
#[test]
fn every_shipped_operation_reaches_its_module() {
    for provider in shipped() {
        let provider = provider.as_str();
        let module = planned(provider, &format!("{provider}.flux"));
        let source =
            std::fs::read_to_string(workspace().providers_dir().join(format!("{provider}.toml")))
                .expect("the shipped provider file is readable");
        let loaded = connector_spec::provider::load(&format!("providers/{provider}.toml"), &source)
            .expect("the shipped provider file loads");

        for operation in &loaded.connector.operations {
            assert!(
                module.contains(&format!("op {}(", operation.id))
                    || module.contains(&format!("op {} ", operation.id)),
                "connectors/{provider}.flux does not declare `{}`:\n{module}",
                operation.id
            );
        }
    }
}

/// **The silent failure, pinned.** Zendesk's wire body is
/// `{"ticket": {"updated_stamp": …, "safe_update": true, "comment": {"body": …, "public": …}}}`
/// (`docs/designs/provider-operation-inventory.md` §3.3.1). A flat body is not an error Zendesk
/// reports — it answers 200 and applies nothing — so the shape is asserted here in full rather than
/// approximated by "the payload mentions `body`".
#[test]
fn zendesk_writes_a_nested_body() {
    let module = planned("zendesk", "zendesk.flux");

    assert!(
        module.contains(
            "$payload = { ticket: { comment: { body: $body, public: $public }, \
             safe_update: $safe_update, updated_stamp: $updated_stamp } }"
        ),
        "`zendesk-ticket-comment-add` must nest its comment under `ticket.comment`:\n{module}"
    );
    assert!(
        module.contains("$payload = { ticket: { additional_tags: $tags, safe_update: $safe_update, updated_stamp: $updated_stamp } }"),
        "`zendesk-ticket-tag-add` must write `ticket.additional_tags` — sending `tags` *replaces* \
         the ticket's tags (inventory §3.3.3):\n{module}"
    );
    assert!(
        !module.contains("$payload = { body:")
            && !module.contains(", updated_stamp: $updated_stamp }\n"),
        "no Zendesk payload may put a ticket field at the root of the body — Zendesk ignores it \
         and answers 200:\n{module}"
    );
}

/// babelforce's agent-status update writes `presence.name`. This is the refusal quoted in the story:
/// it is the operation that stopped the whole build, and it is loud rather than silent only because
/// the dotted spelling happened to be visible in the field's `name`.
#[test]
fn babelforce_nests_the_presence_label() {
    let module = planned("babelforce", "babelforce.flux");
    assert!(
        module.contains("$payload = { enabled: $enabled, presence: { name: $presence_name } }"),
        "`babelforce-agent-status-update` must nest `presence.name`:\n{module}"
    );
}

/// A free-form object body — babelforce's two session-variable writes — reaches the request. Before
/// `ParamSet::body_schema` existed these declared no body parameter at all and emitted a `PUT` with
/// no body, which is indistinguishable from a legitimately bodiless write.
#[test]
fn babelforce_sends_its_free_form_session_bodies() {
    let module = planned("babelforce", "babelforce.flux");
    assert_eq!(
        module
            .matches("$payload = parse($body, as: \"json\")")
            .count(),
        2,
        "both session-variable operations must send the caller's body:\n{module}"
    );
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
        module.contains("$payload = { channel: $channel, text: $text, thread_ts: $thread_ts }"),
        "`slack-chat-post-message` must send its arguments as a flat JSON body:\n{module}"
    );
    assert!(
        module.contains(r#"$url = fmt("{base}/api/conversations.history")"#),
        "`slack-conversations-history` must address the bare method path, with no query string \
         — an opaque channel id in a query value cannot be percent-encoded (C-30):\n{module}"
    );
    // The emitter binds `$sep` only to carry a `?`/`&` between query parameters, so its absence is
    // a structural proof that no value was spliced into any of the four URLs.
    assert!(
        !module.contains("$sep") && !module.contains('?'),
        "no Slack operation may assemble a query string:\n{module}"
    );
}

/// **Intercom's egress host is exactly `api.intercom.io`, and its credential never leaves the
/// manifest as anything but a name** (C-73).
///
/// Two claims that the IR-level test in `crates/connector-flux/tests/intercom_connector.rs` cannot
/// make, because both are properties of what the *pipeline* derives rather than of what the provider
/// file declares:
///
/// - `http_hosts` is derived from `base_url` by `catalog::host_of` and is published to consumers in
///   `web/public/catalog.json`. A widened entry — a second host, or a `*` — would enlarge the egress
///   allow-list of every operation at once, and nothing else in the tree would notice. Intercom's
///   regional hosts (`api.eu.intercom.io`, `api.au.intercom.io`) are exactly the tempting second
///   entry, and `providers/intercom.toml` records why they are a separate connector instead.
/// - **The generated module names no credential at all.** Not the value, which does not exist in this
///   repository, and not even the environment variable: the bearer is applied by the host at the
///   `$auth` seam (`docs/designs/auth-seam.md`), so `INTERCOM_ACCESS_TOKEN` belongs in the manifest's
///   credential *reference* and must never appear in Flux a model can read. Asserting the name is
///   present in the manifest is what keeps the absence check from passing vacuously.
#[test]
fn intercom_publishes_one_host_and_no_credential_in_its_module() {
    const TOKEN_ENV: &str = "INTERCOM_ACCESS_TOKEN";

    let connector = load("intercom");
    let module = planned("intercom", "intercom.flux");
    let manifest = planned("intercom", "intercom.connector.toml");

    assert_eq!(
        connector.base_url, "https://api.intercom.io",
        "the base URL is what the host is derived from, so widening it widens the allow-list"
    );
    assert!(
        module.contains(r#"$base = "https://api.intercom.io""#),
        "every Intercom request must address `api.intercom.io`:\n{module}"
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
        module.contains(&format!(r#"$base = "{BASE_URL}""#)),
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

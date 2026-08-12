//! Contract for babelforce, and the fence C-130 left behind instead of an `ivr` service.
//!
//! C-130 set out to publish babelforce's IVR-v2 atomics — `audioplayer`, `read`, `switchnode`,
//! `dial`, `recording`, `acd` — as operations. The inventory it wrote first
//! (`docs/designs/babelforce-ivr-atomics.md` §The inventory) found that it cannot be done, and the
//! short version is that **the atomics have no wire identity**: babelforce's own
//! `adapters/backend/settingsapi/parse_settings.go` maps *call-module* names (`promptPlayer`,
//! `simpleMenu`, `transfer`, …) onto the atomics, so the call modules are the only `module` values
//! the vendor's API accepts, and the atomics are an internal handler seam. There is also no endpoint
//! per module — there is one `Application` CRUD resource, which
//! `docs/designs/provider-operation-inventory.md` §5.3 already excluded as provisioning, and which
//! the IVR service does not even mount.
//!
//! One conclusion of that epic survives intact and is worth fencing rather than deferring: **no call
//! module becomes an operation.** They are compositions, and publishing one freezes a combination
//! while hiding its parts. `no_babelforce_operation_is_named_after_an_ivr_call_module` is that fence.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::Connector;

use crate::shipped_provider;

const PROVIDER: &str = "babelforce";
const BASE_URL: &str = "https://services.babelforce.com";
const CREDENTIAL: &str = "babelforce.access_token";
const TOKEN_ENV: &str = "BABELFORCE_ACCESS_TOKEN";

/// The nine operations curated in `docs/designs/provider-operation-inventory.md` §5.2, in file order.
const OPERATIONS: &[&str] = &[
    "babelforce-agent-list",
    "babelforce-agent-get",
    "babelforce-agent-status-update",
    "babelforce-call-list",
    "babelforce-call-get",
    "babelforce-call-hangup",
    "babelforce-call-session-set",
    "babelforce-session-get",
    "babelforce-session-update",
];

/// IVR call modules, in the kebab spelling an operation id would have to use.
///
/// Every entry is a *composition* of one or more atomics, and each is deliberately multi-word: these
/// names cannot collide with a legitimate future operation. The single-word module names babelforce
/// also accepts — `transfer`, `recording`, `flow`, `acd`, `agentic`, `realtime` — are **not** fenced,
/// because each is an ordinary English word that could honestly name something else (a call
/// recording read from the manager API is a plausible operation, and it is not a call module).
const CALL_MODULES: &[&str] = &[
    "simple-menu",
    "prompt-player",
    "audio-player",
    "input-reader",
    "speech-to-text",
    "text-to-speech",
    "switch-node",
    "consumer-queue",
    "agent-queue",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {} ({error})", path.display()));
    shipped_provider::load_definition(PROVIDER, &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// `haystack`'s `-`-separated segments contain `needle`'s as a contiguous run.
///
/// Segment-wise rather than substring, so `read` cannot match `thread` and `acd` cannot match a
/// hypothetical `acd-something` prefix of an unrelated word.
fn segments_contain(haystack: &str, needle: &str) -> bool {
    let id: Vec<&str> = haystack.split('-').collect();
    let module: Vec<&str> = needle.split('-').collect();
    id.windows(module.len())
        .any(|window| window == module.as_slice())
}

/// The five services babelforce publishes, one per vendored document — C-410, C-417.
///
/// The split was **not** speculative and it is not C-130's `ivr`. It arrived because the connector
/// compiles from five documents, and a document joins a service: the partition is what lets
/// `getUser` mean two different requests without either being compiled by accident. The address
/// cost `AGENTS.md` warns about was paid for a surface that exists, which is the distinction the
/// old comment here drew.
const SERVICES: &[&str] = &["manager", "user", "task-automation", "task-schedule"];

#[test]
fn babelforce_publishes_the_nine_curated_operations_across_four_named_services() {
    let connector = load();
    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Babelforce");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.verify.as_deref(), Some("babelforce-agent-list"));

    // **The nine lead, and they are the nine that reach a model.** C-417 widened this connector to
    // the whole manager-sdk surface, so the published set is no longer nine — but the nine are a
    // public contract, they still publish first and in this order because a `[[patch.operations]]`
    // block outranks a selector, and they are the only exposed operations in a set of 391.
    assert_eq!(
        connector
            .operations
            .iter()
            .take(OPERATIONS.len())
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        OPERATIONS
    );
    assert_eq!(
        connector
            .operations
            .iter()
            .filter(|operation| operation.expose)
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        OPERATIONS,
        "the curated nine are the exposed set; the rest are callable without being tools"
    );

    // Four named services, and **nothing in the reserved `default`**: every operation arrived
    // through a `[[spec]]` entry that names the service its document joins, so an operation left in
    // `default` would be one that reached the connector by some other route.
    assert_eq!(connector.service_names(), SERVICES);
    for operation in &connector.operations {
        assert!(
            SERVICES.contains(&operation.service.as_str()),
            "`{}` lands in service `{}`, which no `[[services]]` entry declares",
            operation.id,
            operation.service
        );
    }
    for id in OPERATIONS {
        let operation = connector
            .operations
            .iter()
            .find(|operation| operation.id == *id)
            .expect("a curated operation");
        assert_eq!(
            operation.service, "manager",
            "the curated nine all come from the manager document"
        );
    }
}

#[test]
fn no_babelforce_operation_is_named_after_an_ivr_call_module() {
    let connector = load();
    for operation in &connector.operations {
        for module in CALL_MODULES {
            assert!(
                !segments_contain(&operation.id, module),
                "operation `{}` is named after the IVR call module `{module}`, which is a \
                 composition of atomics and is excluded on purpose — see \
                 docs/designs/babelforce-ivr-atomics.md",
                operation.id
            );
        }
    }
}

#[test]
fn the_fence_would_catch_the_operation_a_contributor_is_tempted_to_add() {
    // A fence nobody has seen fail is a fence nobody can trust. These are the ids the epic's own
    // prose invites, and each must be rejected.
    for tempting in [
        "babelforce-ivr-simple-menu",
        "babelforce-ivr-prompt-player-create",
        "babelforce-audio-player",
        "babelforce-ivr-speech-to-text",
        "babelforce-agent-queue-enter",
    ] {
        assert!(
            CALL_MODULES
                .iter()
                .any(|module| segments_contain(tempting, module)),
            "the fence does not catch `{tempting}`"
        );
    }

    // And it must not fire on what babelforce already ships, nor on plausible neighbours.
    for allowed in [
        "babelforce-agent-list",
        "babelforce-agent-status-update",
        "babelforce-call-session-set",
        "babelforce-call-recording-get",
    ] {
        assert!(
            !CALL_MODULES
                .iter()
                .any(|module| segments_contain(allowed, module)),
            "the fence falsely rejects `{allowed}`"
        );
    }
}

#[test]
fn every_babelforce_operation_emits_an_analyzable_module_without_secret_material() {
    let connector = load();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{} does not emit: {error}", operation.id));
        assert!(emitted.contains(BASE_URL));
        assert!(!emitted.contains(TOKEN_ENV));
        assert!(!emitted.contains(CREDENTIAL));

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "{} does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str())
        );
        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("{} does not load: {error}", operation.id));
        let program = module.program().expect("program");
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, operation.id);
    }
}

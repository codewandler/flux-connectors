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
use connector_spec::{provider, Connector};

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
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
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

#[test]
fn babelforce_publishes_the_nine_curated_operations_under_one_default_service() {
    let connector = load();
    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Babelforce");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.verify.as_deref(), Some("babelforce-agent-list"));
    assert_eq!(
        connector
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        OPERATIONS
    );

    // No `[[services]]` entry, so all nine sit in the implicit `default` service. C-130 would have
    // added `ivr` and split these into named surfaces; it is blocked, and the split is deliberately
    // not done speculatively — naming a service renames the emitted artifacts and mints a new
    // address, and `AGENTS.md`'s service contract is that an address, once published, is never
    // reused. Paying that once is right; paying it for a service that never arrives is not.
    assert!(connector.services.is_empty());
    for operation in &connector.operations {
        assert_eq!(operation.service, "default", "{}", operation.id);
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

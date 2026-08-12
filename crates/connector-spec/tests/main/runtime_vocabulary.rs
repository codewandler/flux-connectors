//! **One closed runtime vocabulary, stated once** — C-405.
//!
//! The set lives in flux's `docs/designs/ecosystem.md`, which this repository mirrors: `http`,
//! `socket`, `process`, `container`, `plugin`, `remote`. A mirrored closed set that nothing verifies
//! stops being closed at the seam, so every restatement of it inside this repository is derived from
//! [`Runtime::ALL`] and checked here rather than hand-typed and hoped over.
//!
//! # What cannot be checked from here, stated rather than implied
//!
//! The flux side is a **design document in another repository**, and the pinned flux crates publish
//! no runtime type — `codewandler-flux-spec`, `-runtime`, `-core` and `-system` at 0.41 declare no
//! such enum, so there is nothing to link and nothing to compare against. Adding a git or path
//! dependency on `../flux` to reach the prose is forbidden (`AGENTS.md`, "Relationship to flux") and
//! would not be a check anyway. So the seam that is verifiable is verified — the loader, the
//! published JSON schema and the artifacts all read one constant — and the seam that is not is
//! named: **if flux grows a runtime type, this file is where the comparison belongs.**

use std::collections::BTreeSet;

use connector_spec::Runtime;
use serde_json::Value;

/// The schema the crate publishes, parsed.
fn schema() -> Value {
    serde_json::from_str(connector_spec::PROVIDER_TOML_JSON_SCHEMA)
        .expect("the published schema is valid JSON")
}

/// A minimal provider file, optionally declaring a runtime.
fn provider(runtime: Option<&str>) -> String {
    let declared = runtime
        .map(|runtime| format!("runtime = {runtime:?}\n"))
        .unwrap_or_default();
    format!(
        r#"id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
{declared}
[[operations]]
id = "acme-thing-get"
method = "GET"
direction = "read"
path = "/v1/things"
description = "List the things."
risk = "low"
idempotency = "idempotent"
"#
    )
}

/// **The word a runtime serializes as is the word `word()` returns.** Two spellings of one token is
/// how a manifest comes to say `Process` while a provider file has to say `process`; every artifact
/// in this repository renders through `word`, and every provider file is parsed through `serde`.
#[test]
fn each_runtimes_word_is_the_token_it_serializes_as() {
    for runtime in Runtime::ALL {
        let encoded = serde_json::to_value(runtime).expect("a runtime serializes");
        assert_eq!(
            encoded,
            Value::String(runtime.word().to_owned()),
            "`Runtime::word` and the `serde` encoding disagree for {runtime:?}"
        );
    }
}

/// **Every word the vocabulary names is a word the loader accepts**, so the set is honest in the
/// admitting direction and not only in the refusing one.
#[test]
fn the_loader_accepts_every_declared_runtime() {
    for runtime in Runtime::ALL {
        let loaded =
            connector_spec::provider::load("providers/acme.toml", &provider(Some(runtime.word())))
                .unwrap_or_else(|error| {
                    panic!(
                        "`{}` is in `Runtime::ALL` but the loader refuses it: {error}",
                        runtime.word()
                    )
                });
        assert_eq!(loaded.connector.runtime, runtime);
    }
}

/// **And a connector that declares nothing is `http`** — the property that keeps all 53 shipped
/// provider definitions unchanged.
#[test]
fn an_undeclared_runtime_is_http() {
    let loaded = connector_spec::provider::load("providers/acme.toml", &provider(None))
        .expect("a provider declaring no runtime loads");

    assert_eq!(
        loaded.connector.runtime,
        Runtime::Http,
        "the default must be `http`, or landing this field would have changed every provider"
    );
    assert_eq!(Runtime::default(), Runtime::Http);
}

/// **The published schema names exactly the set the loader accepts.** The schema is what an author's
/// editor validates against and the loader is what the build enforces; a hand-typed list in the JSON
/// is a second statement of one fact, and the two would drift the first time the axis grew a
/// runtime — an editor accepting a word the build refuses, or the reverse, with nothing to say which
/// was right. So this reads `Runtime::ALL` rather than a second copy of the words.
#[test]
fn the_schema_publishes_the_loaders_own_runtime_vocabulary() {
    let schema = schema();
    let published: Vec<String> = schema["$defs"]["provider"]["properties"]["runtime"]["enum"]
        .as_array()
        .expect("`runtime` publishes an `enum`")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("every runtime in the schema is a string")
                .to_owned()
        })
        .collect();
    let accepted: Vec<String> = Runtime::ALL
        .iter()
        .map(|runtime| runtime.word().to_owned())
        .collect();

    assert_eq!(
        published, accepted,
        "`schema/provider-toml.schema.json` names a different runtime set than the loader accepts, \
         so an author's editor and the build disagree about what a connector may declare"
    );
    assert_eq!(
        schema["$defs"]["provider"]["properties"]["runtime"]["default"],
        Value::String(Runtime::default().word().to_owned()),
        "the schema must publish the loader's own default, or an author's editor will fill in a \
         runtime the build does not"
    );
}

/// **The set has no duplicates and no gaps.** `ALL` is hand-written — it has to be, because it is
/// what a refusal prints — so the one thing a reader cannot verify by looking is that it names each
/// variant once.
#[test]
fn the_declared_set_names_each_runtime_exactly_once() {
    let words: BTreeSet<&str> = Runtime::ALL.iter().map(|runtime| runtime.word()).collect();
    assert_eq!(
        words.len(),
        Runtime::ALL.len(),
        "`Runtime::ALL` repeats a variant, so a refusal lists one runtime twice"
    );
}

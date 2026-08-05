//! The published JSON Schema for `providers/<name>.toml`, kept in sync with the loader by test.
//!
//! `schema/provider-toml.schema.json` is hand-written — generating it would mean a `schemars`
//! dependency this crate does not take — and a hand-written schema rots the moment someone adds a
//! field and forgets it. So the test does not read the schema and nod: it asks **serde** which keys
//! each type actually accepts, by handing the type a key it cannot know and reading the field list
//! out of `deny_unknown_fields`' own error, then compares that against the schema's `properties`.
//!
//! The answer therefore comes from the same `Deserialize` impl that will parse real provider files.
//! Adding a field to any IR type fails this test until the schema documents it, and documenting a
//! property the loader does not accept fails it too.

use std::collections::BTreeSet;

use connector_spec::PROVIDER_TOML_JSON_SCHEMA;
use serde_json::Value;

fn schema() -> Value {
    serde_json::from_str(PROVIDER_TOML_JSON_SCHEMA)
        .expect("the published schema must itself be valid JSON")
}

/// The `$defs` entries that describe an object with named properties. The rest — the enums, the
/// opaque `jsonSchema` — have nothing to keep in sync.
fn object_defs(schema: &Value) -> BTreeSet<String> {
    schema["$defs"]
        .as_object()
        .expect("$defs is an object")
        .iter()
        .filter(|(_, definition)| definition.get("properties").is_some())
        .map(|(name, _)| name.clone())
        .collect()
}

/// Every object the schema documents is one the loader accepts, and vice versa.
#[test]
fn the_schema_documents_exactly_the_objects_the_loader_accepts() {
    let schema = schema();
    let documented = object_defs(&schema);
    let probed: BTreeSet<String> = connector_spec::provider::accepted_keys()
        .into_iter()
        .map(|(name, _)| name.to_owned())
        .collect();

    let undocumented: Vec<&String> = probed.difference(&documented).collect();
    let unreachable: Vec<&String> = documented.difference(&probed).collect();

    assert!(
        undocumented.is_empty(),
        "these objects are accepted by the loader but absent from \
         `schema/provider-toml.schema.json`: {undocumented:?}"
    );
    assert!(
        unreachable.is_empty(),
        "these objects are documented in `schema/provider-toml.schema.json` but no probe covers \
         them, so nothing keeps them honest — add them to `provider::accepted_keys`: \
         {unreachable:?}"
    );
}

/// Every object's property set matches, key for key. This is the assertion that actually catches a
/// forgotten field.
#[test]
fn every_documented_object_lists_exactly_the_keys_the_loader_accepts() {
    let schema = schema();
    let mut mismatches = Vec::new();

    for (object, accepted) in connector_spec::provider::accepted_keys() {
        let Some(definition) = schema["$defs"].get(object) else {
            continue; // Reported by the test above; no need to fail twice for one cause.
        };

        let documented: BTreeSet<&str> = definition["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("`$defs.{object}` must have `properties`"))
            .keys()
            .map(String::as_str)
            .collect();
        let accepted: BTreeSet<&str> = accepted.iter().map(String::as_str).collect();

        if documented != accepted {
            let missing: Vec<&&str> = accepted.difference(&documented).collect();
            let extra: Vec<&&str> = documented.difference(&accepted).collect();
            mismatches.push(format!(
                "  {object}: undocumented keys {missing:?}, documented-but-unaccepted keys {extra:?}"
            ));
        }
    }

    assert!(
        mismatches.is_empty(),
        "`schema/provider-toml.schema.json` has drifted from the types it documents:\n{}",
        mismatches.join("\n")
    );
}

/// Every object is closed in the schema too. A schema that documented the right keys but permitted
/// extras would describe a laxer format than the loader implements — and the schema is what an
/// author's editor validates against, so it is the first thing that tells them a typo is fine.
#[test]
fn every_documented_object_forbids_additional_properties() {
    let schema = schema();
    let open: Vec<&String> = schema["$defs"]
        .as_object()
        .expect("$defs is an object")
        .iter()
        .filter(|(_, definition)| definition.get("properties").is_some())
        .filter(|(_, definition)| definition["additionalProperties"] != Value::Bool(false))
        .map(|(name, _)| name)
        .collect();

    assert!(
        open.is_empty(),
        "these schema objects accept unknown keys, but the loader does not: {open:?}"
    );
}

/// The properties the loader *requires* are the ones the schema marks required. Getting this wrong
/// is how `scheme` would quietly become optional in the published contract while the loader kept
/// rejecting files without it.
#[test]
fn the_schema_marks_the_mandatory_keys_required() {
    let schema = schema();

    let required = |object: &str| -> BTreeSet<String> {
        schema["$defs"][object]["required"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .expect("a required entry is a string")
                            .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let cases: [(&str, &[&str]); 8] = [
        ("provider", &["id", "base_url"]),
        // No default for `scheme`: how a secret reaches the wire is not decided by silence.
        ("authMethod", &["name", "scheme"]),
        // No default for direction or the request/policy fields, for the same reason.
        (
            "operation",
            &["id", "method", "direction", "path", "risk", "idempotency"],
        ),
        ("param", &["name", "schema"]),
        ("authRequirement", &["credentials"]),
        ("spec", &["path"]),
        ("operationPatch", &["select"]),
        (
            "operationSpecSource",
            &["operation_id", "upstream_version", "sha256"],
        ),
    ];

    for (object, expected) in cases {
        let expected: BTreeSet<String> = expected.iter().map(|key| (*key).to_owned()).collect();
        assert_eq!(
            required(object),
            expected,
            "`$defs.{object}.required` does not match what the loader insists on"
        );
    }
}

/// A mechanism must name at least one credential, in the schema as well as in the loader — the
/// "no second spelling of no-auth" rule has to be visible to an author's editor, not only to a
/// build that has already failed.
#[test]
fn the_schema_forbids_an_empty_auth_mechanism() {
    let schema = schema();
    assert_eq!(
        schema["$defs"]["authRequirement"]["properties"]["credentials"]["minItems"],
        serde_json::json!(1),
        "an empty `credentials` list is a second spelling of \"no auth\" and must be rejected by \
         the schema as well as by the loader"
    );
}

/// The schema is shipped as a crate constant so a consumer — an editor integration, a docs build —
/// can reach it without knowing where this crate keeps its files.
#[test]
fn the_schema_is_published_and_self_describing() {
    let schema = schema();
    assert_eq!(
        schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
        "the schema must declare its own dialect"
    );
    assert_eq!(schema["$ref"], "#/$defs/provider");
    assert!(
        schema["description"]
            .as_str()
            .is_some_and(|text| text.len() > 200),
        "the schema is the documentation of the file format; a bare title is not enough"
    );
}

/// **The schema's `minLength` for `repeatable_because` is the loader's constant, not a copy of it.**
///
/// The published schema is what an author's editor validates against, and the loader is what the
/// build enforces. A hand-typed `24` in the JSON is a second statement of one fact — the exact shape
/// C-186 was filed for, re-enacted inside C-186's own change — and the two would drift the first
/// time anyone tuned the floor: an editor would accept a condition the build then refuses, or the
/// reverse, with nothing to say which was right.
///
/// So the number is read from `connector_spec::MIN_REPEATABILITY_CONDITION` here rather than
/// written twice. Changing the constant fails this test until the schema follows.
#[test]
fn the_schema_publishes_the_loaders_own_repeatability_floor() {
    let schema = schema();
    let published = schema["$defs"]["operation"]["properties"]["repeatable_because"]["minLength"]
        .as_u64()
        .expect("`repeatable_because` publishes a `minLength`");

    assert_eq!(
        published as usize,
        connector_spec::MIN_REPEATABILITY_CONDITION,
        "`schema/provider-toml.schema.json` states a different minimum for `repeatable_because` \
         than the loader enforces, so an author's editor and the build disagree about what counts \
         as a stated condition"
    );
}

/// **The schema's documented default for `expose` is the loader's actual default.**
///
/// The key-set tests above prove the schema *mentions* `expose`; they say nothing about which way it
/// defaults, and that is the half worth guarding. A schema publishing `default: false` beside a
/// loader defaulting to `true` would tell an author their silent operations are hidden while the
/// build exposes every one of them — a disagreement about a safety-shaped value, reached by nobody
/// deciding anything.
///
/// The direction is checked by **loading a provider file that says nothing** rather than by reading a
/// constant, so this fails if the serde default is ever flipped without the schema following, and
/// fails the other way too.
#[test]
fn the_schema_publishes_the_exposure_default_the_loader_applies() {
    let schema = schema();

    for object in ["operation", "graph"] {
        let published = schema["$defs"][object]["properties"]["expose"]["default"]
            .as_bool()
            .unwrap_or_else(|| panic!("`{object}.expose` must publish a boolean `default`"));

        assert!(
            published,
            "`schema/provider-toml.schema.json` documents `{object}.expose` as defaulting to \
             `false`, but silence means exposed — an author's editor and the build disagree about \
             what a member that says nothing does"
        );
    }

    let connector = connector_spec::provider::load(
        "acme",
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"

[[operations]]
id = "acme-thing-list"
method = "GET"
direction = "read"
path = "/v1/things"
risk = "low"
idempotency = "idempotent"

[[graphs]]
name = "acme-thing-flow"

[[graphs.nodes]]
id = "list"
kind = { operation = { operation = "acme-thing-list" } }
"#,
    )
    .expect("the fixture loads")
    .connector;

    assert!(
        connector.operations[0].expose,
        "an operation saying nothing about `expose` must load as exposed, or landing this field \
         hid every operation in the repository"
    );
    assert!(
        connector.graphs[0].expose,
        "a flow saying nothing about `expose` must load as exposed"
    );
}

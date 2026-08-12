//! Flow graphs: connector members composed into one Flux `op`.
//!
//! The tests are arranged around the two things that could go wrong, and they are different in kind.
//!
//! **Structural** — Flux has no `goto`, so a cyclic graph and a graph whose control regions overlap
//! have no lowering at all. A compiler accepting them would have to guess, and a guess produces
//! plausible-but-wrong Flux, which this pipeline refuses everywhere else.
//!
//! **Categorical** — a node must never carry a formula. That is the line the vision's principle 2
//! actually draws (every rejection in this repository's history was an *expression* language; every
//! declarative structure was accepted), and `no_node_kind_carries_a_formula` is the test that keeps it
//! true as node kinds are added.

use connector_spec::graph::TextRole;
use connector_spec::{provider, Backoff, Compare, Connector, NodeKind};

/// A connector with two operations, an event and a binding — enough for a graph to reference real
/// members, since a dangling reference is refused.
fn fixture(graphs: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]

[[operations]]
id = "acme-thing-show"
method = "GET"
direction = "read"
path = "/things/{{id}}"
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "id"
required = true
schema = {{ type = "string" }}

[[operations]]
id = "acme-notify"
method = "POST"
direction = "write"
path = "/notify"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "text"
required = true
schema = {{ type = "string" }}

[[events]]
name = "thing.created"

[[channels]]
name = "hook"
transport = "socket"
events = ["thing.created"]

{graphs}
"#
    )
}

/// The worked example from the design: an event wakes the flow, an operation reads, a gate guards, an
/// operation writes. Every reference resolves to a member declared above.
const WORKED: &str = r#"
[[graphs]]
name = "on-thing-created"
description = "Read the thing an event names, and notify when it is urgent"

[[graphs.nodes]]
id = "wake"
kind = { trigger = { event = "thing.created" } }
outputs = [{ name = "event" }]

[[graphs.nodes]]
id = "thing_id"
kind = { select = { path = "thing.id" } }
inputs = [{ name = "in" }]
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "show"
kind = { operation = { operation = "acme-thing-show" } }
inputs = [{ name = "id" }]
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "urgent"
kind = { gate = { condition = { left = { node = "show", port = "out" }, op = "eq", right = "urgent" } } }

[[graphs.nodes]]
id = "message"
region = "urgent"
kind = { template = { format = "thing {id} is urgent" } }
inputs = [{ name = "id" }]
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "notify"
region = "urgent"
kind = { operation = { operation = "acme-notify" } }
inputs = [{ name = "text" }]

[[graphs.edges]]
from = { node = "wake", port = "event" }
to = { node = "thing_id", port = "in" }

[[graphs.edges]]
from = { node = "thing_id", port = "out" }
to = { node = "show", port = "id" }

[[graphs.edges]]
from = { node = "thing_id", port = "out" }
to = { node = "message", port = "id" }

[[graphs.edges]]
from = { node = "message", port = "out" }
to = { node = "notify", port = "text" }
"#;

fn load(source: &str) -> Connector {
    provider::load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load:\n{error}"))
        .connector
}

fn refuse(source: &str) -> String {
    let error = provider::load("providers/fixture.toml", source)
        .err()
        .unwrap_or_else(|| panic!("this definition must not load"));
    format!("{error}")
}

#[test]
fn the_worked_example_loads_and_composes_declared_members() {
    let connector = load(&fixture(WORKED));
    let graph = connector
        .graph("on-thing-created")
        .expect("the graph loads");

    assert_eq!(graph.nodes.len(), 6);
    assert_eq!(graph.edges.len(), 4);

    // Every reference is to a member this connector already declares — that is the whole thesis.
    assert!(connector.event("thing.created").is_some());
    assert!(connector.operation("acme-thing-show").is_some());
    assert!(connector.operation("acme-notify").is_some());

    // A value flows *into* a region freely: `thing_id` is at the top level, `message` is inside the
    // gate. Flux allows an inner statement to read an outer symbol.
    assert_eq!(
        graph.node("message").expect("declared").region.as_deref(),
        Some("urgent")
    );
    assert_eq!(graph.node("thing_id").expect("declared").region, None);

    let order = graph.topological_order().expect("acyclic");
    assert_eq!(order.first(), Some(&"wake"));
}

// ---------------------------------------------------------------------------------------------
// Structural — a graph Flux cannot express is refused, not guessed at
// ---------------------------------------------------------------------------------------------

#[test]
fn a_cycle_is_refused_because_flux_has_no_goto() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "loop"

[[graphs.nodes]]
id = "a"
kind = { select = { path = "x" } }
inputs = [{ name = "in" }]
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "b"
kind = { select = { path = "y" } }
inputs = [{ name = "in" }]
outputs = [{ name = "out" }]

[[graphs.edges]]
from = { node = "a", port = "out" }
to = { node = "b", port = "in" }

[[graphs.edges]]
from = { node = "b", port = "out" }
to = { node = "a", port = "in" }
"#,
    ));
    assert!(
        error.contains("has no `goto`"),
        "the refusal must say why a cycle has no lowering:\n{error}"
    );
    assert!(
        error.contains("bounded loop node"),
        "and must point at the alternative:\n{error}"
    );
}

/// The rule with teeth. Flux's `when` has no else here, so a symbol bound inside a gate is *unbound*
/// on the false path — reading it afterwards fails at runtime, long after the build passed.
#[test]
fn a_gate_cannot_export_a_value() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "leaky"

[[graphs.nodes]]
id = "check"
kind = { gate = { condition = { left = { node = "check", port = "x" }, op = "exists" } } }
outputs = [{ name = "escaped" }]
"#,
    ));
    assert!(
        error.contains("unbound"),
        "the refusal must name the runtime failure it prevents:\n{error}"
    );
    assert!(
        error.contains("branch with a default"),
        "and must point at what would work:\n{error}"
    );
}

/// A retry always runs its body or fails, so a value *may* escape it — the contrast that shows the
/// gate rule is about Flux's semantics rather than a blanket ban on region outputs.
#[test]
fn a_retry_may_export_a_value() {
    let connector = load(&fixture(
        r#"
[[graphs]]
name = "resilient"

[[graphs.nodes]]
id = "attempt"
kind = { retry = { max = 3, backoff = "exponential", delay_ms = 500 } }
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "show"
region = "attempt"
kind = { operation = { operation = "acme-thing-show" } }
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "after"
kind = { select = { path = "id" } }
inputs = [{ name = "in" }]
outputs = [{ name = "out" }]

[[graphs.edges]]
from = { node = "show", port = "out" }
to = { node = "after", port = "in" }
"#,
    ));
    let graph = connector.graph("resilient").expect("loads");
    assert!(matches!(
        graph.node("attempt").expect("declared").kind,
        NodeKind::Retry {
            max: 3,
            backoff: Backoff::Exponential,
            ..
        }
    ));
}

#[test]
fn a_value_may_not_leave_a_region_through_a_port_the_region_does_not_declare() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "escaping"

[[graphs.nodes]]
id = "attempt"
kind = { retry = { max = 2 } }

[[graphs.nodes]]
id = "inner"
region = "attempt"
kind = { select = { path = "a" } }
outputs = [{ name = "out" }]

[[graphs.nodes]]
id = "outer"
kind = { select = { path = "b" } }
inputs = [{ name = "in" }]

[[graphs.edges]]
from = { node = "inner", port = "out" }
to = { node = "outer", port = "in" }
"#,
    ));
    assert!(
        error.contains("declares no output port"),
        "a value leaves a region only through a declared port:\n{error}"
    );
}

#[test]
fn a_region_naming_a_node_that_contains_nothing_is_refused() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "confused"

[[graphs.nodes]]
id = "leaf"
kind = { literal = { value = 1 } }

[[graphs.nodes]]
id = "inner"
region = "leaf"
kind = { literal = { value = 2 } }
"#,
    ));
    assert!(
        error.contains("contains nothing"),
        "only region kinds contain nodes:\n{error}"
    );
}

#[test]
fn a_node_contained_in_itself_is_refused() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "ouroboros"

[[graphs.nodes]]
id = "a"
region = "b"
kind = { retry = { max = 1 } }

[[graphs.nodes]]
id = "b"
region = "a"
kind = { retry = { max = 1 } }
"#,
    ));
    assert!(
        error.contains("contained in itself"),
        "a containment cycle must be refused:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// References resolve, in the graph's own service
// ---------------------------------------------------------------------------------------------

#[test]
fn a_node_naming_an_operation_nobody_declares_is_refused() {
    let error = refuse(&fixture(&WORKED.replace("acme-notify", "acme-absent")));
    assert!(
        error.contains("which this connector does not declare"),
        "a dangling reference must be named:\n{error}"
    );
}

#[test]
fn a_trigger_naming_an_undeclared_event_is_refused() {
    let error = refuse(&fixture(
        &WORKED.replace(r#"event = "thing.created""#, r#"event = "thing.imagined""#),
    ));
    assert!(error.contains("does not declare"), "{error}");
}

#[test]
fn a_boundary_node_may_not_take_inputs_or_sit_in_a_region() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "confused-boundary"

[[graphs.nodes]]
id = "wake"
kind = { trigger = { event = "thing.created" } }
inputs = [{ name = "somehow" }]
outputs = [{ name = "event" }]
"#,
    ));
    assert!(
        error.contains("nothing inside the flow can feed it"),
        "a boundary declares what wakes the flow:\n{error}"
    );
}

#[test]
fn an_edge_naming_a_port_that_does_not_exist_is_refused() {
    let error = refuse(&fixture(&WORKED.replace(
        r#"to = { node = "show", port = "id" }"#,
        r#"to = { node = "show", port = "nonesuch" }"#,
    )));
    assert!(error.contains("has no input port"), "{error}");
}

#[test]
fn a_zero_bound_retry_is_refused() {
    let error = refuse(&fixture(
        r#"
[[graphs]]
name = "pointless"

[[graphs.nodes]]
id = "attempt"
kind = { retry = { max = 0 } }
"#,
    ));
    assert!(
        error.contains("not a loop at all"),
        "flux rejects unbounded loops; a zero bound is the other degenerate case:\n{error}"
    );
}

#[test]
fn graph_names_join_the_shared_member_namespace() {
    let error = refuse(&fixture(
        &WORKED.replace(r#"name = "on-thing-created""#, r#"name = "acme-notify""#),
    ));
    assert!(
        error.contains("names both an operation and a graph"),
        "every member kind shares one namespace:\n{error}"
    );
}

#[test]
fn a_graph_name_that_could_not_be_a_flux_declaration_is_refused() {
    let error = refuse(&fixture(
        &WORKED.replace(r#"name = "on-thing-created""#, r#"name = "On/Thing""#),
    ));
    assert!(error.contains("invalid `name`"), "{error}");
}

// ---------------------------------------------------------------------------------------------
// The categorical rule: no node carries a formula
// ---------------------------------------------------------------------------------------------

/// **The test that keeps this on the right side of principle 2.**
///
/// Every rejection in this repository's history was an *expression* language — a template DSL,
/// JSONPath, a vendor's remote expression evaluator, `retry: ':params.disable_retry ? -1 : 2'`. Every
/// acceptance was declarative structure. This asserts that every piece of free text in every node kind
/// is a name we resolve, a path we validate, or text nobody evaluates.
///
/// `NodeKind::free_text` destructures exhaustively, so a field added later fails to compile until
/// somebody classifies it — and there is deliberately no `Formula` role to classify it as.
#[test]
fn no_node_kind_carries_a_formula() {
    let every_kind = vec![
        NodeKind::Operation {
            operation: "acme-notify".into(),
        },
        NodeKind::Select { path: "a.b".into() },
        NodeKind::Template {
            format: "hello {name}".into(),
        },
        NodeKind::Object {
            fields: Default::default(),
        },
        NodeKind::Literal { value: 1.into() },
        NodeKind::Gate {
            condition: connector_spec::Condition {
                left: connector_spec::PortRef {
                    node: "a".into(),
                    port: "out".into(),
                },
                op: Compare::Eq,
                right: Some("x".into()),
            },
        },
        NodeKind::Approval {
            message: "Proceed?".into(),
            risk: connector_spec::Risk::High,
        },
        NodeKind::Retry {
            max: 3,
            backoff: Backoff::None,
            delay_ms: None,
        },
        NodeKind::Throttle {
            max: 5,
            window_ms: 60_000,
        },
        NodeKind::Trigger {
            event: "thing.created".into(),
        },
        NodeKind::Schedule {
            cron: "0 9 * * *".into(),
        },
        NodeKind::Endpoint {
            binding: "hook".into(),
        },
    ];

    for kind in &every_kind {
        for (field, role) in kind.free_text() {
            assert!(
                matches!(
                    role,
                    TextRole::Reference
                        | TextRole::Path
                        | TextRole::Template
                        | TextRole::Prose
                        | TextRole::Data
                        | TextRole::Schedule
                ),
                "`{}`.{field} is classified {role:?}, which is not one of the roles this repository \
                 admits. If a node needs an evaluated expression, that is the signal to stop and \
                 re-read the north star — not to add a role",
                kind.word()
            );
        }
    }

    // The condition — the one place an expression would naturally creep in — carries no text at all.
    let gate = &every_kind[5];
    assert!(
        gate.free_text().is_empty(),
        "a gate's condition is a structure: a port reference, a closed operator and a literal. The \
         Flux expression is generated from it, so nothing an author types is evaluated"
    );
}

/// The condition's operator set is closed and small. `Exists` is the one with no right-hand side.
#[test]
fn the_comparison_vocabulary_is_closed() {
    for op in [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Lte,
        Compare::Gt,
        Compare::Gte,
    ] {
        assert!(op.operator().is_some(), "{op:?} generates a Flux operator");
    }
    assert!(Compare::Exists.operator().is_none());
}

/// A graph is compiled meaning — it becomes an emitted `op`, so a changed edge must move the hash.
#[test]
fn a_graph_is_in_the_hash_domain() {
    let with = load(&fixture(WORKED)).ir_sha256().expect("hashes");
    let without = load(&fixture("")).ir_sha256().expect("hashes");
    assert_ne!(
        with, without,
        "a connector that declares a flow must not hash the same as one that does not"
    );

    let moved = load(&fixture(&WORKED.replace(
        r#"to = { node = "show", port = "id" }"#,
        r#"to = { node = "message", port = "id" }"#,
    )));
    assert_ne!(
        with,
        moved.ir_sha256().expect("hashes"),
        "moving an edge changes what would be emitted, so it must change the hash"
    );
}

/// A connector that declares no graph must encode exactly as it did before graphs existed, or landing
/// this moves every `ir_sha256` in the repository and churns the lockfile for a provider nobody
/// edited — the phantom drift the lockfile exists to rule out.
#[test]
fn a_connector_without_graphs_encodes_as_it_did_before() {
    let json = load(&fixture("")).canonical_json().expect("the IR encodes");
    assert!(
        !json.contains("graphs"),
        "an absent member kind must add nothing to the encoding: {json}"
    );
}

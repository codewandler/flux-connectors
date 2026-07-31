//! The graph-lowering contract: one IR [`Graph`] becomes one formatted Flux composite `op`.
//!
//! `op_emitter.rs` pins the same properties for a single operation. Four are restated here because a
//! graph is where they get hard, and one is new:
//!
//! 1. **Golden files.** `tests/golden/graph-*.flux` pin the exact generated text — the worked example
//!    and one graph carrying every region kind. Re-record with
//!    `UPDATE_GOLDEN=1 cargo test -p connector-flux`, then *read the diff*.
//! 2. **Canonical formatting.** The emitted module parses, is a fixed point of flux-lang's own
//!    formatter, and reloads as exactly one exposed composite op. The lowering asserts this on its own
//!    output, so a shape that stops round-tripping is a refusal rather than a committed artifact.
//! 3. **Symbols are the compiler's.** One `$symbol` per edge, generated from the edge's source port,
//!    stable across rebuilds, and never a Flux reserved word — an author names none of them.
//! 4. **Regions nest.** A region's nodes lower into its block, and a `retry`'s declared output port
//!    becomes the block's `-> $bind`.
//! 5. **The blocker, asserted.** A `Select` wired to an `Operation` output is *refused*:
//!    `http.request` returns `HTTP {status}\n{headers}\n{body}` as one flat string, so a path applied
//!    to it resolves to `null` on every response. Emitting the selector anyway is exactly the
//!    plausible-but-wrong output `AGENTS.md` forbids.

use std::collections::BTreeMap;

use connector_flux::emit_graph;
use connector_spec::{
    Compare, Condition, Connector, Edge, EventDecl, Graph, GraphNode, HttpMethod, Idempotency,
    NodeKind, Operation, Param, ParamSet, Port, PortRef, Provenance, Risk, DEFAULT_SERVICE,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn param(name: &str, schema: serde_json::Value) -> Param {
    Param {
        name: name.to_string(),
        wire: None,
        description: String::new(),
        required: true,
        schema,
    }
}

fn operation(
    id: &str,
    method: HttpMethod,
    risk: Risk,
    idempotency: Idempotency,
    params: ParamSet,
) -> Operation {
    Operation {
        id: id.to_string(),
        service: DEFAULT_SERVICE.to_string(),
        method,
        path: "/v1/things".to_string(),
        description: format!("The {id} operation."),
        risk,
        idempotency,
        repeatable_because: None,
        auth: None,
        params,
        response_schema: None,
        quirks: Default::default(),
    }
}

/// A connector with the members every graph below composes. The operations are synthetic — no
/// launch provider ships a graph yet — and each one exists to pin one property of the lowering.
fn vendor() -> Connector {
    Connector {
        id: "vendor".to_string(),
        authority: None,
        api_version: None,
        services: Vec::new(),
        vendor: String::new(),
        base_url: "https://api.example.com".to_string(),
        description: String::new(),
        auth: Vec::new(),
        default_auth: Vec::new(),
        operations: vec![
            operation(
                "vendor-thing-get",
                HttpMethod::Get,
                Risk::Low,
                Idempotency::Idempotent,
                ParamSet {
                    path: vec![param("id", json!({"type": "string"}))],
                    ..ParamSet::default()
                },
            ),
            operation(
                "vendor-thing-search",
                HttpMethod::Get,
                Risk::Low,
                Idempotency::Idempotent,
                ParamSet {
                    query: vec![param("q", json!({"type": "string"}))],
                    ..ParamSet::default()
                },
            ),
            operation(
                "vendor-thing-note",
                HttpMethod::Post,
                Risk::Medium,
                Idempotency::NonIdempotent,
                ParamSet {
                    body: vec![param("body", json!({"type": "string"}))],
                    ..ParamSet::default()
                },
            ),
            operation(
                "vendor-thing-delete",
                HttpMethod::Delete,
                Risk::Destructive,
                Idempotency::Idempotent,
                ParamSet {
                    path: vec![param("id", json!({"type": "string"}))],
                    ..ParamSet::default()
                },
            ),
            operation(
                "vendor-message-post",
                HttpMethod::Post,
                Risk::Medium,
                Idempotency::NonIdempotent,
                ParamSet {
                    body: vec![
                        param("channel", json!({"type": "string"})),
                        param("text", json!({"type": "string"})),
                    ],
                    ..ParamSet::default()
                },
            ),
        ],
        events: vec![EventDecl {
            name: "app_mention".to_string(),
            service: DEFAULT_SERVICE.to_string(),
            description: "Somebody mentioned the app.".to_string(),
            default: true,
            group: String::new(),
            when: BTreeMap::new(),
            schema: None,
        }],
        channels: Vec::new(),
        config: Vec::new(),
        verify: None,
        graphs: Vec::new(),
        provenance: Provenance::default(),
    }
}

fn port(name: &str) -> Port {
    Port {
        name: name.to_string(),
        schema: None,
        required: true,
    }
}

fn node(id: &str, kind: NodeKind) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        kind,
        region: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
    }
}

fn wired(mut node: GraphNode, inputs: &[&str], outputs: &[&str]) -> GraphNode {
    node.inputs = inputs.iter().map(|name| port(name)).collect();
    node.outputs = outputs.iter().map(|name| port(name)).collect();
    node
}

fn inside(mut node: GraphNode, region: &str) -> GraphNode {
    node.region = Some(region.to_string());
    node
}

fn edge(from: (&str, &str), to: (&str, &str)) -> Edge {
    Edge {
        from: PortRef {
            node: from.0.to_string(),
            port: from.1.to_string(),
        },
        to: PortRef {
            node: to.0.to_string(),
            port: to.1.to_string(),
        },
    }
}

fn graph(name: &str, nodes: Vec<GraphNode>, edges: Vec<Edge>) -> Graph {
    Graph {
        name: name.to_string(),
        service: DEFAULT_SERVICE.to_string(),
        description: format!("The {name} flow."),
        inputs: Vec::new(),
        output: None,
        nodes,
        edges,
    }
}

fn select(path: &str) -> NodeKind {
    NodeKind::Select {
        path: path.to_string(),
    }
}

fn template(format: &str) -> NodeKind {
    NodeKind::Template {
        format: format.to_string(),
    }
}

fn call(id: &str) -> NodeKind {
    NodeKind::Operation {
        operation: id.to_string(),
    }
}

/// **The worked example.** A vendor event wakes the flow, two selections pull the thread and the
/// channel out of its payload, a template composes the reply, and a gate keeps the reply inside a
/// thread. The reply declares no output port, so it lowers to `do` — a gate exports nothing, so a
/// value bound inside it could not be read afterwards anyway.
fn autoreply() -> Graph {
    let mut graph = graph(
        "message-autoreply",
        vec![
            wired(
                node(
                    "wake",
                    NodeKind::Trigger {
                        event: "app_mention".to_string(),
                    },
                ),
                &[],
                &["event"],
            ),
            wired(node("thread", select("event.thread_ts")), &["of"], &["out"]),
            wired(node("channel", select("event.channel")), &["of"], &["out"]),
            wired(
                node("greeting", template("Thanks for the ping in {thread}")),
                &["thread"],
                &["out"],
            ),
            wired(
                node(
                    "guard",
                    NodeKind::Gate {
                        condition: Condition {
                            left: PortRef {
                                node: "thread".to_string(),
                                port: "out".to_string(),
                            },
                            op: Compare::Exists,
                            right: None,
                        },
                    },
                ),
                &[],
                &[],
            ),
            inside(
                wired(
                    node("reply", call("vendor-message-post")),
                    &["channel", "text"],
                    &[],
                ),
                "guard",
            ),
        ],
        vec![
            edge(("wake", "event"), ("thread", "of")),
            edge(("wake", "event"), ("channel", "of")),
            edge(("thread", "out"), ("greeting", "thread")),
            edge(("channel", "out"), ("reply", "channel")),
            edge(("greeting", "out"), ("reply", "text")),
        ],
    );
    graph.description = "Reply in-thread when the app is mentioned.".to_string();
    graph
}

/// **One region per spellable kind.** A schedule wakes the flow; a `retry` bounds the read, an
/// `approval` fences the delete, and a `gate` guards the audit note. Both exporting kinds hand a
/// value out through a port they declare, and the gate exports nothing.
///
/// **`throttle` is deliberately absent, and so is the retry's delay** — flux-lang 0.39 cannot spell
/// either consistently (see [`a_duration_flux_cannot_spell_consistently_is_refused`]), so a fixture
/// carrying one would only ever assert the refusal. [`throttled_sweep`] is that fixture.
fn nightly_sweep() -> Graph {
    let mut graph = graph(
        "nightly-sweep",
        vec![
            wired(
                node(
                    "tick",
                    NodeKind::Schedule {
                        cron: "0 3 * * *".to_string(),
                    },
                ),
                &[],
                &["at"],
            ),
            wired(
                node("window", template("updated_after:{at}")),
                &["at"],
                &["query"],
            ),
            wired(
                node(
                    "read",
                    NodeKind::Retry {
                        max: 3,
                        backoff: connector_spec::Backoff::Exponential,
                        delay_ms: None,
                    },
                ),
                &[],
                &["result"],
            ),
            inside(
                wired(
                    node("fetch", call("vendor-thing-search")),
                    &["q"],
                    &["result"],
                ),
                "read",
            ),
            wired(
                node("note", call("vendor-thing-note")),
                &["body"],
                &["noted"],
            ),
            wired(
                node(
                    "ask",
                    NodeKind::Approval {
                        message: "Delete the swept thing?".to_string(),
                        risk: Risk::Destructive,
                    },
                ),
                &[],
                &["gone"],
            ),
            inside(
                wired(
                    node("wipe", call("vendor-thing-delete")),
                    &["id"],
                    &["gone"],
                ),
                "ask",
            ),
            wired(
                node(
                    "guard",
                    NodeKind::Gate {
                        condition: Condition {
                            left: PortRef {
                                node: "read".to_string(),
                                port: "result".to_string(),
                            },
                            op: Compare::Exists,
                            right: None,
                        },
                    },
                ),
                &[],
                &[],
            ),
            inside(
                wired(node("audit", call("vendor-thing-note")), &["body"], &[]),
                "guard",
            ),
        ],
        vec![
            edge(("tick", "at"), ("window", "at")),
            edge(("window", "query"), ("fetch", "q")),
            edge(("read", "result"), ("note", "body")),
            edge(("read", "result"), ("wipe", "id")),
            edge(("note", "noted"), ("audit", "body")),
        ],
    );
    graph.description =
        "Sweep yesterday's things, note them, and delete them under approval.".to_string();
    graph.output = Some(PortRef {
        node: "ask".to_string(),
        port: "gone".to_string(),
    });
    graph
}

/// [`nightly_sweep`] with its note wrapped in a `throttle` — the shape flux-lang 0.39 cannot spell.
fn throttled_sweep() -> Graph {
    let mut graph = nightly_sweep();
    graph.nodes.push(wired(
        node(
            "paced",
            NodeKind::Throttle {
                max: 5,
                window_ms: 60000,
            },
        ),
        &[],
        &["noted"],
    ));
    for node in &mut graph.nodes {
        if node.id == "note" {
            node.region = Some("paced".to_string());
        }
    }
    for edge in &mut graph.edges {
        if edge.to.node == "audit" {
            edge.from.node = "paced".to_string();
        }
    }
    graph
}

fn emit(graph: &Graph) -> String {
    emit_graph(&vendor(), graph)
        .unwrap_or_else(|error| panic!("graph `{}` must lower: {error}", graph.name))
}

// ---------------------------------------------------------------------------
// Reading the emitted module back
//
// The properties below are about *structure*, so they are asserted against the reparsed AST rather
// than against the emitted text — `op_emitter.rs` takes the same line for the body-tree test, and
// for the same reason: a formatting change in flux-lang must not be able to make one pass or fail.
// The two golden files are where exact text is pinned, and re-recording them is one command.
// ---------------------------------------------------------------------------

fn program_of(emitted: &str) -> flux_lang::program::Program {
    let module = flux_lang::program::Module::parse_str(emitted)
        .unwrap_or_else(|error| panic!("an emitted graph must load: {error}\n{emitted}"));
    module
        .program()
        .expect("an `op` declaration is a program")
        .clone()
}

fn op_of(emitted: &str) -> flux_lang::program::CompositeOpDecl {
    let program = program_of(emitted);
    assert_eq!(program.ops.len(), 1, "a graph emits exactly one op");
    program.ops[0].clone()
}

fn body_of(emitted: &str) -> Vec<flux_lang::ast::Node> {
    op_of(emitted).body.body
}

/// Walk every node of a body, including the ones nested inside region blocks.
fn for_each_node(body: &[flux_lang::ast::Node], f: &mut impl FnMut(&flux_lang::ast::Node)) {
    flux_lang::analyze::for_each_node(body, f);
}

/// The symbol a node *declares*, when it declares one. Every binder form this lowering emits.
fn declared_name(node: &flux_lang::ast::Node) -> Option<String> {
    use flux_lang::ast::Node;
    match node {
        Node::Bind { name, .. } | Node::Memo { name, .. } => Some(name.0.clone()),
        Node::Retry { bind, .. } => bind.as_ref().map(|s| s.0.clone()),
        _ => None,
    }
}

/// The first node of `body` matching `pick`, searched depth-first through region blocks.
fn find<T>(
    body: &[flux_lang::ast::Node],
    pick: impl Fn(&flux_lang::ast::Node) -> Option<T>,
) -> Option<T> {
    let mut found = None;
    for_each_node(body, &mut |node| {
        if found.is_none() {
            found = pick(node);
        }
    });
    found
}

// ---------------------------------------------------------------------------
// Golden files
// ---------------------------------------------------------------------------

fn assert_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden dir")).expect("create golden dir");
        std::fs::write(&path, actual).expect("write golden");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read golden {}: {e}\nre-record with UPDATE_GOLDEN=1 cargo test -p connector-flux",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "generated Flux drifted from {}\n--- generated ---\n{actual}\n--- end ---\n\
         re-record with UPDATE_GOLDEN=1 cargo test -p connector-flux, then read the diff",
        path.display()
    );
}

#[test]
fn golden_message_autoreply() {
    assert_golden("graph-message-autoreply.flux", &emit(&autoreply()));
}

#[test]
fn golden_nightly_sweep() {
    assert_golden("graph-nightly-sweep.flux", &emit(&nightly_sweep()));
}

// ---------------------------------------------------------------------------
// The load-bearing properties
// ---------------------------------------------------------------------------

/// The C-11 gate, held against a graph. The emitted module parses, flux's own formatter leaves it
/// unchanged, and it loads back as exactly one exposed composite op — the three things that decide
/// whether flux publishes the op at all.
#[test]
fn an_emitted_graph_parses_is_canonical_and_reloads_as_one_op() {
    for graph in [autoreply(), nightly_sweep()] {
        let emitted = emit(&graph);

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "graph `{}` emits Flux that does not parse: {:?}\n{emitted}",
            graph.name,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would rewrite graph `{}`",
            graph.name
        );

        let module =
            flux_lang::program::Module::parse_str(&emitted).expect("an emitted graph must load");
        let program = module.program().expect("an `op` declaration is a program");
        assert_eq!(program.ops.len(), 1);
        assert_eq!(program.ops[0].name, graph.name);
        assert!(program.ops[0].meta.expose);
    }
}

/// Two builds of one graph agree byte for byte. Symbol generation is the lowering's own, so it must
/// not depend on hashing, iteration order, or anything else that moves between runs — a regenerated
/// module that churns is one nobody can review a diff of.
#[test]
fn symbol_generation_is_stable_across_rebuilds() {
    for graph in [autoreply(), nightly_sweep()] {
        assert_eq!(emit(&graph), emit(&graph));
    }
}

/// **Edges are symbols the compiler owns.** Every symbol the body binds is generated from an edge's
/// source port, so it is spellable as a Flux identifier and is never one of flux's own reserved
/// words — which is what makes action-proxy's silent `$emit` shadowing unrepresentable here.
///
/// Asserted over the **reparsed AST** rather than over emitted text: a symbol's spelling is
/// flux-lang's business and moves between versions, while the property — every declared name is an
/// identifier and none is a keyword — does not.
#[test]
fn generated_symbols_are_identifiers_and_never_flux_keywords() {
    // Node ids an author is free to choose: one collides with a flux statement keyword, one carries
    // a dot, which is never valid in a declared Flux name.
    let sweep = renamed(
        nightly_sweep(),
        &[("window", "retry"), ("read", "throttle.read")],
    );

    let emitted = emit(&sweep);
    let mut declared = Vec::new();
    for_each_node(&body_of(&emitted), &mut |node| {
        if let Some(name) = declared_name(node) {
            declared.push(name);
        }
    });

    assert!(
        !declared.is_empty(),
        "the flow must bind something:\n{emitted}"
    );
    for name in declared {
        assert!(
            flux_lang::ast::SymbolName(name.clone()).is_identifier(),
            "`{name}` is not a spellable Flux symbol:\n{emitted}"
        );
        assert!(
            !flux_lang::ast::is_reserved_word(&name),
            "`{name}` is one of flux's own reserved words:\n{emitted}"
        );
        // Under 0.39 a local binding drops the `$` sigil unless it collides with a keyword, so a
        // generated name has to be spellable bare or the emitted text changes shape.
        assert!(
            flux_lang::ast::is_bare_symbol_name(&name),
            "`{name}` cannot be spelled without the sigil:\n{emitted}"
        );
    }
}

/// Rewrite node ids throughout a graph — every place an id appears is a place a rename has to reach,
/// which is exactly what makes an author-stable id worth having.
fn renamed(mut graph: Graph, renames: &[(&str, &str)]) -> Graph {
    let rename = |id: &mut String| {
        if let Some((_, to)) = renames.iter().find(|(from, _)| from == id) {
            *id = (*to).to_string();
        }
    };
    for node in &mut graph.nodes {
        rename(&mut node.id);
        if let Some(region) = &mut node.region {
            rename(region);
        }
        if let NodeKind::Gate { condition } = &mut node.kind {
            rename(&mut condition.left.node);
        }
    }
    for edge in &mut graph.edges {
        rename(&mut edge.from.node);
        rename(&mut edge.to.node);
    }
    if let Some(output) = &mut graph.output {
        rename(&mut output.node);
    }
    graph
}
/// **Regions nest, and a `retry`'s declared output port is its `-> $bind`.** Flux has no `goto`, so
/// a region's nodes must lower *into* its block; the declared output port is the phi node Flux does
/// not have, made explicit.
#[test]
fn a_region_lowers_into_its_block_and_binds_its_declared_output() {
    use flux_lang::ast::Node;
    let body = body_of(&emit(&nightly_sweep()));

    let retry = find(&body, |node| match node {
        Node::Retry {
            max,
            backoff,
            delay_ms,
            body,
            bind,
        } => Some((*max, backoff.clone(), *delay_ms, body.clone(), bind.clone())),
        _ => None,
    })
    .expect("the sweep declares a retry region");
    assert_eq!(retry.0, 3);
    assert_eq!(retry.1.as_deref(), Some("exponential"));
    assert_eq!(
        retry.2, None,
        "a delay is refused under flux-lang 0.39 — see a_duration_flux_cannot_spell_consistently_is_refused"
    );
    assert!(
        retry.4.is_some(),
        "a retry's declared output port must become the block's bind"
    );
    // The region's own node lowered *into* its block, not beside it.
    assert!(
        retry.3.iter().any(|node| matches!(
            node,
            Node::Bind { value, .. } if matches!(value.as_ref(), Node::Call { op, .. } if op == "vendor-thing-search")
        )),
        "the region's node must lower into its block: {:?}",
        retry.3
    );

    let confirm = find(&body, |node| match node {
        Node::Confirm {
            message,
            risk,
            body,
        } => Some((message.clone(), risk.clone(), body.len())),
        _ => None,
    })
    .expect("the sweep declares an approval region");
    assert_eq!(confirm.0, "Delete the swept thing?");
    assert_eq!(
        confirm.1.as_deref(),
        Some("destructive"),
        "an approval carries the risk it declared"
    );
    assert_eq!(confirm.2, 1);
}

/// An `approval` always runs its body or fails, so a value escapes it through the symbol the body
/// already bound — flux's `confirm` carries no `-> $bind`, and none is needed. A `throttle` exports
/// by the identical rule; flux-lang 0.39 cannot spell one at all, so only `confirm` is exercised.
#[test]
fn an_approval_exports_the_symbol_its_body_bound() {
    use flux_lang::ast::Node;
    let body = body_of(&emit(&nightly_sweep()));

    // The graph's output resolves through the approval's declared port to the symbol `wipe` bound.
    let confirm_bound = find(&body, |node| match node {
        Node::Confirm { body, .. } => body.iter().find_map(declared_name),
        _ => None,
    })
    .expect("the approval's body binds its exported value");
    let returned = body
        .last()
        .and_then(|node| match node {
            Node::Return { value } => match value.as_ref() {
                Node::Var { name } => Some(name.0.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("the graph declares an output, so the op ends in a return");
    assert_eq!(
        returned, confirm_bound,
        "the graph's output must resolve through the approval's declared port"
    );
}

/// **The blocker, stated loudly.** `http.request` returns one flat string —
/// `HTTP {status}\n{headers}\n{body}` — so a path applied to an operation's response resolves to
/// `null` on every response, success or failure. A selector that always yields null is precisely the
/// plausible-but-wrong output AGENTS.md forbids, so the lowering refuses it until `http.request`
/// returns a record.
#[test]
fn a_select_wired_to_an_operation_output_is_refused() {
    let mut sweep = nightly_sweep();
    sweep
        .nodes
        .push(wired(node("dig", select("body.id")), &["of"], &["out"]));
    sweep.edges.push(edge(("read", "result"), ("dig", "of")));

    let error = emit_graph(&vendor(), &sweep)
        .expect_err("a path applied to `http.request`'s flat string always resolves to null");
    let rendered = error.to_string();
    assert!(
        rendered.contains("http.request"),
        "the refusal must name what returns the flat string, got: {rendered}"
    );
    assert!(
        rendered.contains("null"),
        "the refusal must say what the selector would resolve to, got: {rendered}"
    );
    assert!(
        rendered.contains("dig") && rendered.contains("fetch"),
        "the refusal must name both ends of the wire, got: {rendered}"
    );
}

/// A selection off a *boundary* payload is a different case and still lowers: a trigger's event is a
/// record the host hands in, not `http.request`'s flat string.
#[test]
fn a_select_off_an_event_payload_still_lowers() {
    use flux_lang::ast::Node;
    let body = body_of(&emit(&autoreply()));

    let selection = find(&body, |node| match node {
        Node::Bind { value, .. } => match value.as_ref() {
            Node::Jq {
                path,
                input,
                optional,
            } if path == ".event.thread_ts" => Some((input.clone(), *optional)),
            _ => None,
        },
        _ => None,
    })
    .expect("the selection must lower to flux's own path node");
    assert!(
        matches!(selection.0.as_ref(), Node::Var { .. }),
        "a selection reads a bound symbol, never an inline value"
    );
    assert!(
        !selection.1,
        "a strict traversal is what makes a typo'd field name fail loudly rather than read empty"
    );
}

/// **The upstream blocker.** flux-lang 0.39's two formatters disagree about how to spell a
/// duration: `flux_lang::format` writes `per 1m` / `delay 250ms`, and
/// `flux_lang::format_cst::format_module` — the formatter a human editing the generated file runs —
/// accepts only bare milliseconds and returns `None` rather than re-printing the suffixed form.
///
/// Both spellings parse to the same AST, so this is an upstream defect and not an ambiguity this
/// repository has to resolve. It is refused rather than emitted, because the alternative is shipping
/// a generated module flux's own formatter cannot format — and rewriting the token afterwards would
/// be the string surgery on generated Flux that AGENTS.md exists to prevent.
///
/// It bites **every** `throttle` (no window value avoids the suffix) and **every** `retry` carrying
/// a delay. A `retry` without one is unaffected, which is what [`nightly_sweep`] still exercises.
#[test]
fn a_duration_flux_cannot_spell_consistently_is_refused() {
    // Every throttle, whatever its window.
    for window_ms in [1000u64, 1500, 60000, 90000] {
        let mut sweep = throttled_sweep();
        for node in &mut sweep.nodes {
            if let NodeKind::Throttle { window_ms: w, .. } = &mut node.kind {
                *w = window_ms;
            }
        }
        let error = emit_graph(&vendor(), &sweep)
            .expect_err("a throttle must be refused whatever its window")
            .to_string();
        assert!(
            error.contains("per") && error.contains("formatter"),
            "the refusal must name the clause and the property it protects, got: {error}"
        );
    }

    // And every retry that declares a delay.
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if let NodeKind::Retry { delay_ms, .. } = &mut node.kind {
            *delay_ms = Some(250);
        }
    }
    let error = emit_graph(&vendor(), &sweep).expect_err("a retry delay must be refused");
    assert!(
        error.to_string().contains("delay") && error.to_string().contains("250"),
        "the refusal must name the clause and the value, got: {error}"
    );

    // The same graph without the delay lowers, so the refusal is as narrow as it claims.
    emit_graph(&vendor(), &nightly_sweep()).expect("a retry without a delay is unaffected");
}

/// Flux has no `goto`, so a cyclic graph has no lowering at all — an iteration is a bounded loop
/// node, never an edge pointing backwards.
#[test]
fn a_cycle_has_no_lowering() {
    let mut sweep = nightly_sweep();
    sweep.edges.push(edge(("paced", "noted"), ("window", "at")));

    let error = emit_graph(&vendor(), &sweep).expect_err("a cyclic graph has no lowering");
    assert!(
        error.to_string().contains("cycle"),
        "the refusal must name the problem, got: {error}"
    );
}

/// A value leaves a region only through a port the region declares. Otherwise the symbol it lowers
/// to may not be bound when the block closes, and the failure lands at runtime rather than at build
/// time.
#[test]
fn an_edge_leaving_a_region_without_a_declared_port_is_refused() {
    let mut sweep = nightly_sweep();
    // A second node inside the approval, exporting on a port name the approval does not declare.
    sweep.nodes.push(inside(
        wired(node("leak", template("{of}")), &["of"], &["escapee"]),
        "ask",
    ));
    sweep.edges.push(edge(("wipe", "gone"), ("leak", "of")));
    for edge in &mut sweep.edges {
        if edge.to.node == "audit" {
            edge.from = PortRef {
                node: "leak".to_string(),
                port: "escapee".to_string(),
            };
        }
    }

    let error = emit_graph(&vendor(), &sweep).expect_err("a value may not leak out of a region");
    let rendered = error.to_string();
    assert!(
        rendered.contains("ask") && rendered.contains("escapee"),
        "the refusal must name the region and the port, got: {rendered}"
    );
}

/// A region's declared output port is a promise that the body binds it. Nothing inside binding it
/// means the symbol the port lowers to would be unbound wherever it is read.
#[test]
fn a_region_output_no_node_inside_binds_is_refused() {
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "fetch" {
            node.outputs = vec![port("something-else")];
        }
    }
    sweep.edges.retain(|e| e.from.node != "fetch");

    let error = emit_graph(&vendor(), &sweep).expect_err("an unbound region output has no symbol");
    let rendered = error.to_string();
    assert!(
        rendered.contains("read") && rendered.contains("result"),
        "the refusal must name the region and the port, got: {rendered}"
    );
}

/// **A gate exports nothing.** It lowers to `when`, which has no else here, so a symbol bound inside
/// it is *unbound* on the false path and reading it afterwards fails at runtime — long after the
/// build passed.
#[test]
fn a_gate_may_not_export_a_value() {
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "guard" {
            node.outputs = vec![port("noted")];
        }
    }

    let error = emit_graph(&vendor(), &sweep).expect_err("a gate exports nothing");
    assert!(
        error.to_string().contains("when"),
        "the refusal must name the Flux node it lowers to, got: {error}"
    );
}

/// The graph's name becomes the emitted `op`'s declaration name, and a dotted name is not one —
/// `flux_lang`'s `decl_name` grammar admits only ASCII alphanumerics, `_` and `-`. The same refusal
/// `op_emitter.rs` pins for an operation id, at the other front door.
#[test]
fn a_dotted_graph_name_cannot_be_declared() {
    let mut sweep = nightly_sweep();
    sweep.name = "vendor.nightly.sweep".to_string();

    let error = emit_graph(&vendor(), &sweep)
        .expect_err("a dotted declaration name does not parse in flux");
    assert!(
        error.to_string().contains("C-23"),
        "the refusal must point at the story that owns naming, got: {error}"
    );
}

/// A `gate` comparing against a literal generates the Flux expression from the closed comparison —
/// the author writes no formula, and the literal is bound to a symbol rather than spliced into the
/// expression text.
#[test]
fn a_comparison_generates_its_flux_expression_from_bound_values() {
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "guard" {
            node.kind = NodeKind::Gate {
                condition: Condition {
                    left: PortRef {
                        node: "read".to_string(),
                        port: "result".to_string(),
                    },
                    op: Compare::Eq,
                    right: Some(json!("ok")),
                },
            };
        }
    }

    use flux_lang::ast::Node;
    let body = body_of(&emit(&sweep));

    let condition = find(&body, |node| match node {
        Node::When { cond, .. } => Some(cond.clone()),
        _ => None,
    })
    .expect("the gate lowers to `when`");
    let Node::Expr { formula, vars } = condition.as_ref() else {
        panic!("a comparison generates a Flux expression, got {condition:?}");
    };
    assert_eq!(
        vars.len(),
        2,
        "both sides of the comparison are bound symbols, not spliced text: {formula}"
    );
    for (name, value) in vars {
        assert!(
            matches!(value.as_ref(), Node::Var { name: var } if &var.0 == name),
            "every variable must map to the symbol of the same name, or flux's formatter cannot \
             spell the expression natively: {formula}"
        );
        assert!(
            formula.contains(name.as_str()),
            "the formula must name its variables: {formula}"
        );
    }
    assert!(
        formula.contains("=="),
        "the closed comparison must generate its operator: {formula}"
    );
    assert!(
        !formula.contains("ok"),
        "the literal must be bound to a symbol, never spliced into the formula: {formula}"
    );

    // …and the literal itself is bound as a statement before the gate.
    let bound_literal = find(&body, |node| match node {
        Node::Bind { name, value, .. } => match value.as_ref() {
            Node::Lit { value } if value == &json!("ok") => Some(name.0.clone()),
            _ => None,
        },
        _ => None,
    });
    assert!(
        bound_literal.is_some_and(|name| formula.contains(&name)),
        "the expected value must be bound to the symbol the comparison reads: {formula}"
    );
}

/// A composite literal has no native Flux spelling that survives flux's own formatter — an object or
/// array `lit` re-prints with different spacing, so the module stops round-tripping. Refused rather
/// than emitted, for exactly the reason the fixed-point property exists.
#[test]
fn a_composite_literal_is_refused() {
    let mut sweep = nightly_sweep();
    sweep.nodes.push(wired(
        node(
            "blob",
            NodeKind::Literal {
                value: json!({"a": 1}),
            },
        ),
        &[],
        &["out"],
    ));
    sweep.edges.push(edge(("blob", "out"), ("audit", "body")));

    let error = emit_graph(&vendor(), &sweep).expect_err("a composite literal cannot round-trip");
    assert!(
        error.to_string().contains("formatter"),
        "the refusal must name the property it protects, got: {error}"
    );
}

/// The op's declared metadata is derived from the operations the graph actually calls: flux's
/// approval gate reads `risk` and `idempotency`, so a flow that deletes must not inherit the `low`
/// of the reads it also makes.
#[test]
fn the_metadata_is_derived_from_the_operations_the_graph_calls() {
    let emitted = emit(&nightly_sweep());
    let module = flux_lang::program::Module::parse_str(&emitted).expect("an emitted graph loads");
    let op = &module.program().expect("a program").ops[0];

    assert_eq!(
        serde_json::to_value(op.meta.risk).unwrap(),
        json!("destructive"),
        "the riskiest call the flow makes is the flow's risk"
    );
    assert_eq!(
        serde_json::to_value(op.meta.idempotency).unwrap(),
        json!("non_idempotent"),
        "one non-idempotent call makes the whole flow non-idempotent"
    );
    assert_eq!(
        serde_json::to_value(&op.meta.effects).unwrap(),
        json!(["network"])
    );
    assert_eq!(
        op.meta.description,
        "Sweep yesterday's things, note them, and delete them under approval."
    );
}

/// A boundary node declares what wakes the flow and is emitted nowhere: it becomes a parameter, and
/// no statement in the body corresponds to it.
#[test]
fn a_boundary_node_becomes_a_parameter_and_is_emitted_nowhere() {
    let emitted = emit(&autoreply());
    let op = op_of(&emitted);

    assert_eq!(
        op.params.len(),
        1,
        "the trigger's one payload port is the op's one parameter: {:?}",
        op.params
    );
    let payload = op.params[0].name.0.clone();
    assert!(
        emitted.contains(&payload),
        "the parameter must be the symbol the body reads:\n{emitted}"
    );
    assert!(
        !emitted.contains("app_mention"),
        "an event declaration reaches no emitted Flux:\n{emitted}"
    );
    assert!(
        program_of(&emitted).triggers.is_empty()
            && program_of(&emitted).channels.is_empty()
            && program_of(&emitted).journeys.is_empty(),
        "flux lifts `op` declarations only; the operator writes the program"
    );
}

/// The one shape a call site cannot be guessed at: an input port that names no parameter of the
/// operation it feeds would emit an argument flux rejects at analysis.
#[test]
fn an_input_port_naming_no_parameter_of_its_operation_is_refused() {
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "fetch" {
            node.inputs = vec![port("nope")];
        }
    }
    for edge in &mut sweep.edges {
        if edge.to.node == "fetch" {
            edge.to.port = "nope".to_string();
        }
    }

    let error = emit_graph(&vendor(), &sweep).expect_err("an undeclared argument is not emittable");
    let rendered = error.to_string();
    assert!(
        rendered.contains("nope") && rendered.contains("vendor-thing-search"),
        "the refusal must name the port and the operation, got: {rendered}"
    );
}

/// A template's `{port}` placeholders name the node's own input ports, and both directions of
/// mismatch are refused: an unbound placeholder would reach the vendor verbatim, and an input with
/// nowhere to go could never travel.
#[test]
fn a_template_placeholder_must_name_an_input_port() {
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "window" {
            node.kind = template("updated_after:{nope}");
        }
    }
    let error = emit_graph(&vendor(), &sweep).expect_err("an unbound placeholder is not emittable");
    assert!(
        error.to_string().contains("nope"),
        "the refusal must name the placeholder, got: {error}"
    );

    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "window" {
            node.kind = template("updated_after:yesterday");
        }
    }
    let error = emit_graph(&vendor(), &sweep).expect_err("an input with nowhere to go is a defect");
    assert!(
        error.to_string().contains("at"),
        "the refusal must name the port, got: {error}"
    );
}

/// An object assembles a record from ports. With no fields there is no record — and an empty `obj`
/// has no native Flux spelling, so it would leave the module non-canonical too.
#[test]
fn an_object_with_no_fields_is_refused() {
    let mut sweep = nightly_sweep();
    sweep.nodes.push(wired(
        node(
            "empty",
            NodeKind::Object {
                fields: BTreeMap::new(),
            },
        ),
        &[],
        &["out"],
    ));
    sweep.edges.push(edge(("empty", "out"), ("audit", "body")));

    let error = emit_graph(&vendor(), &sweep).expect_err("an empty record assembles nothing");
    assert!(
        error.to_string().contains("empty"),
        "the refusal must name the node, got: {error}"
    );
}

// ---------------------------------------------------------------------------
// The node-path map (C-96)
//
// A diagnostic flux raises against the emitted op carries a `node_path` — `body[3].then[0]` — and
// the map is what turns that back into the graph node an author drew. The spelling is **flux's
// own** (D-139), so these tests read it out of a real `Diagnostic` rather than out of a local
// imitation of the grammar: a path this repository only agreed with itself about would be
// worthless to the canvas the seam exists for.
// ---------------------------------------------------------------------------

/// The statement at a flux node path, walked the way flux's analyzer builds one.
fn statement_at<'a>(body: &'a [flux_lang::ast::Node], path: &str) -> &'a flux_lang::ast::Node {
    use flux_lang::ast::Node;

    let mut block: &[Node] = body;
    let mut current: Option<&Node> = None;
    for segment in path.split('.') {
        let (label, index) = segment
            .split_once('[')
            .unwrap_or_else(|| panic!("`{segment}` of `{path}` is not a `label[index]` segment"));
        let index: usize = index
            .trim_end_matches(']')
            .parse()
            .unwrap_or_else(|error| panic!("`{segment}` of `{path}` carries no index: {error}"));
        if let Some(node) = current {
            block = match (label, node) {
                ("then", Node::When { then, .. }) => then,
                (
                    "body",
                    Node::Retry { body, .. }
                    | Node::Confirm { body, .. }
                    | Node::Throttle { body, .. },
                ) => body,
                _ => panic!("`{path}` opens a `{label}` block on a statement that has none"),
            };
        }
        current = Some(
            block
                .get(index)
                .unwrap_or_else(|| panic!("`{path}` names statement {index} of a shorter block")),
        );
    }
    current.expect("a node path names at least one statement")
}

/// Whether `statement` is the one a node of this kind produces — the table in the emitter's module
/// documentation, read back off the reparsed AST.
fn statement_of_kind(kind: &NodeKind, statement: &flux_lang::ast::Node) -> bool {
    use flux_lang::ast::Node;

    // A node carrying a value binds its statement to a generated symbol; a call whose node declares
    // no output port is the bare statement form, and a region is a statement in its own right.
    let produced = match statement {
        Node::Bind { value, .. } => value.as_ref(),
        other => other,
    };
    match kind {
        NodeKind::Operation { operation } => {
            matches!(produced, Node::Call { op, .. } if op == operation)
        }
        NodeKind::Select { .. } => matches!(produced, Node::Jq { .. }),
        NodeKind::Template { .. } => matches!(produced, Node::Fmt { .. }),
        NodeKind::Object { .. } => matches!(produced, Node::Obj { .. }),
        NodeKind::Literal { .. } => matches!(produced, Node::Lit { .. }),
        NodeKind::Gate { .. } => matches!(produced, Node::When { .. }),
        NodeKind::Approval { .. } => matches!(produced, Node::Confirm { .. }),
        NodeKind::Retry { .. } => matches!(produced, Node::Retry { .. }),
        NodeKind::Throttle { .. } => matches!(produced, Node::Throttle { .. }),
        // A boundary is a parameter of the emitted op and reaches no statement at all.
        NodeKind::Trigger { .. } | NodeKind::Schedule { .. } | NodeKind::Endpoint { .. } => false,
    }
}

/// A catalogue that knows no operation, so every call in the emitted op raises a node-scoped
/// diagnostic. flux's analyzer is the only honest source of a `node_path` to key back.
struct NoOperations;

impl flux_lang::opspec::OpCatalog for NoOperations {
    fn lookup(&self, _name: &str) -> Option<flux_lang::opspec::OpSignature> {
        None
    }
}

/// **Total and correct.** Every node that lowers to a statement appears in the map, the path it
/// names resolves to a statement of that node's own kind, and the only nodes absent are the
/// boundary — a trigger, a schedule and an endpoint are *parameters*, and flux renders no path for
/// one.
#[test]
fn the_map_names_the_statement_every_node_produced() {
    for graph in [autoreply(), nightly_sweep()] {
        let (emitted, paths) = connector_flux::emit_graph_with_paths(&vendor(), &graph)
            .unwrap_or_else(|error| panic!("graph `{}` must lower: {error}", graph.name));
        let body = body_of(&emitted);

        for node in &graph.nodes {
            match paths.path_of(&node.id) {
                Some(path) => assert!(
                    statement_of_kind(&node.kind, statement_at(&body, path)),
                    "`{}` is a `{}` and `{path}` names {:?}",
                    node.id,
                    node.kind.word(),
                    statement_at(&body, path)
                ),
                None => assert!(
                    node.kind.is_boundary(),
                    "the map is total over every node that reaches a statement, and `{}` is a \
                     `{}` rather than a boundary",
                    node.id,
                    node.kind.word()
                ),
            }
        }

        // …and it names nothing else: a stale id would key a diagnostic to a node the author
        // deleted, which is worse than not answering at all.
        for (id, path) in paths.iter() {
            assert!(
                graph.node(id).is_some(),
                "the map names `{id}` at `{path}`, which graph `{}` does not declare",
                graph.name
            );
        }
        assert_eq!(
            paths.iter().count(),
            graph
                .nodes
                .iter()
                .filter(|node| !node.kind.is_boundary())
                .count(),
            "graph `{}`: one path per statement-producing node, no more",
            graph.name
        );
    }
}

/// The worked example, pinned: the reply sits inside the gate's `then` block, and the trigger that
/// wakes the flow sits in no block at all.
#[test]
fn a_node_inside_a_gate_names_the_block_it_sits_in() {
    let (_, paths) = connector_flux::emit_graph_with_paths(&vendor(), &autoreply())
        .expect("the worked example lowers");

    assert_eq!(paths.path_of("guard"), Some("body[3]"));
    assert_eq!(paths.path_of("reply"), Some("body[3].then[0]"));
    assert_eq!(
        paths.path_of("wake"),
        None,
        "a trigger is a parameter of the emitted op, not a statement"
    );
}

/// **Both directions.** A real `Diagnostic.node_path` — flux's own, from flux's own analyzer — keys
/// back to the node that produced the statement it names, and that node's recorded path is the one
/// the diagnostic sits at or inside. flux descends *into* a statement (`body[1].body[0]` is the
/// bind, `.value` is the call it binds), so the answer is the innermost statement the path falls
/// within.
#[test]
fn a_diagnostic_path_keys_back_to_the_node_that_produced_it() {
    let (emitted, paths) = connector_flux::emit_graph_with_paths(&vendor(), &nightly_sweep())
        .expect("the region fixture lowers");

    let diagnostics = flux_lang::analyze::analyze_flow(
        &op_of(&emitted).body,
        &NoOperations,
        &std::collections::HashSet::new(),
    )
    .expect_err("a catalogue that knows no operation must reject every call");

    let mut keyed = 0;
    for diagnostic in &diagnostics {
        let Some(path) = &diagnostic.node_path else {
            continue; // a flow-level finding sits in no node, and flux renders no path for one
        };
        let node = paths.node_at(path).unwrap_or_else(|| {
            panic!(
                "no graph node owns `{path}`: {}\n{emitted}",
                diagnostic.message
            )
        });
        let recorded = paths
            .path_of(node)
            .expect("the node the map answered with is in the map");
        assert!(
            path == recorded || path.starts_with(&format!("{recorded}.")),
            "`{path}` was keyed to `{node}`, whose statement is at `{recorded}`"
        );
        if diagnostic
            .message
            .contains("unknown operation: `vendor-thing-search`")
        {
            assert_eq!(
                node, "fetch",
                "the search runs inside the retry, so its diagnostic belongs to `fetch`"
            );
            keyed += 1;
        }
    }
    assert!(
        keyed > 0,
        "the unknown-operation finding is the one this test keys back; the diagnostics were {:?}",
        diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// A gate comparing against a literal binds that literal as a statement of its **own**, before the
/// `when` — so every path in the block from there on shifts by one. The map is recorded from the
/// statements as they are pushed, which is what keeps it from drifting off an index counted by
/// hand.
#[test]
fn a_gates_bound_literal_shifts_the_paths_after_it() {
    let mut sweep = nightly_sweep();
    for node in &mut sweep.nodes {
        if node.id == "guard" {
            node.kind = NodeKind::Gate {
                condition: Condition {
                    left: PortRef {
                        node: "read".to_string(),
                        port: "result".to_string(),
                    },
                    op: Compare::Eq,
                    right: Some(json!("ok")),
                },
            };
        }
    }

    let (emitted, paths) =
        connector_flux::emit_graph_with_paths(&vendor(), &sweep).expect("a comparison lowers");
    let body = body_of(&emitted);

    assert_eq!(
        paths.path_of("guard"),
        Some("body[5]"),
        "the expected literal is bound at body[4], so the gate follows it"
    );
    assert_eq!(paths.path_of("audit"), Some("body[5].then[0]"));
    assert!(
        matches!(
            statement_at(&body, "body[4]"),
            flux_lang::ast::Node::Bind { .. }
        ),
        "the statement the gate is offset by is the bound literal"
    );
    assert!(matches!(
        statement_at(&body, "body[5]"),
        flux_lang::ast::Node::When { .. }
    ));
}

/// The map is generated output like the module beside it, so it is committed and drift-checked the
/// same way. Nothing in `providers/` declares a `[[graphs]]` yet, so these two fixtures are the
/// only graphs in the repository — and the goldens are where their maps are pinned.
#[test]
fn golden_node_paths() {
    for (graph, golden) in [
        (autoreply(), "graph-message-autoreply.paths.json"),
        (nightly_sweep(), "graph-nightly-sweep.paths.json"),
    ] {
        let (_, paths) = connector_flux::emit_graph_with_paths(&vendor(), &graph)
            .unwrap_or_else(|error| panic!("graph `{}` must lower: {error}", graph.name));
        assert_golden(golden, &paths.to_json());
    }
}

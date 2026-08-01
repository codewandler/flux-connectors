//! One IR [`Graph`] → one formatted Flux **composite `op`**.
//!
//! The operation emitter next door turns one vendor endpoint into one `op`. This turns a *flow* —
//! that operation, then that gate, then that operation — into one `op` as well, built the same way:
//! [`flux_lang`] AST nodes handed to flux-lang's own formatter, never a string template.
//!
//! # A projection of Flux, not a layer over it
//!
//! `flux_lang::ast::Node` has 43 kinds and this module constructs nine. Every graph node names the
//! Flux node it *is*, which is the whole argument that this is a view onto the language rather than
//! a second one in front of it:
//!
//! ```text
//! Operation → Node::Call       Gate     → Node::When
//! Select    → Node::Jq         Approval → Node::Confirm
//! Template  → Node::Fmt        Retry    → Node::Retry
//! Object    → Node::Obj        Throttle → Node::Throttle
//! Literal   → Node::Lit
//! ```
//!
//! [`NodeKind::Trigger`], [`NodeKind::Schedule`] and [`NodeKind::Endpoint`] are the graph's
//! **boundary**: flux lifts only `op` declarations from `~/.flux/flows`, while `channel` and
//! `trigger` are Program members an operator writes. So a boundary node becomes a *parameter* of the
//! emitted op and reaches no statement — the operator writes the two-line program that binds it.
//!
//! # Edges are symbols the compiler owns
//!
//! An author never sees or names a symbol this module binds, which is what makes action-proxy's
//! silent `$emit` shadowing unrepresentable here. Concretely:
//!
//! - A symbol is generated **per edge, keyed on the edge's source port** — `<node id>_<port>`,
//!   normalized to flux's identifier grammar. Two edges out of one port therefore share one symbol:
//!   a fan-out binds the value once rather than binding a copy of it, which is the difference between
//!   a diamond and two statements that look independent and are not.
//! - Allocation walks the graph in **declaration order** — `inputs`, then boundary nodes, then every
//!   other node — so a regenerated module does not churn when an edge is added or an editor
//!   re-orders a canvas. Node ids are author-stable for the same reason.
//! - The allocator vetoes **flux's own reserved words**, through `flux_lang::ast::is_reserved_word`
//!   rather than a list transcribed from flux's parser. Under flux-lang 0.39 a local binding is
//!   spelled *without* the `$` sigil unless it collides with a keyword, so a node an author called
//!   `retry` would otherwise generate a name that reads as a statement keyword — and a transcribed
//!   list is wrong the moment flux adds a word.
//!
//! # Control flow must nest; data flow need not
//!
//! Flux has no `goto`. A statement may read any bound symbol, so **data convergence is free** and a
//! diamond is a legal graph — but a branch is a nested block, so **control must nest**. Four rules
//! follow, and this module refuses every violation rather than guessing:
//!
//! 1. **No cycles** ([`Error::GraphCycle`]) — neither in the data edges nor in the collapsed
//!    per-region ordering.
//! 2. **No edge leaves a region except through a port the region declares**
//!    ([`Error::RegionCrossingEdge`]). A value may *enter* freely; the escaping direction is where a
//!    symbol might not be bound when the block closes.
//! 3. **A gate exports nothing** ([`Error::GateExportsAValue`]). It lowers to `when`, which has no
//!    else branch here, so a symbol bound inside is *unbound* on the false path.
//! 4. **A region's declared output port is bound by its body** ([`Error::UnboundRegionOutput`]) —
//!    the phi node Flux does not have, made explicit.
//!
//! # How an output port becomes a symbol
//!
//! `retry` is the one region kind flux gives a result bind, so a `retry`'s declared output port
//! becomes the block's `-> $symbol` and the block ends in a bare reference to the producing
//! statement, which is what that bind captures. `throttle` and `confirm` have no bind — and need
//! none: both always run their body or fail, so the symbol the body already bound is still bound when
//! the block closes, and the region's port resolves straight to it. That contrast is why rule 3 is
//! about Flux's semantics rather than a blanket ban on exporting from a block.
//!
//! # The second blocker, upstream this time
//!
//! **flux-lang 0.39's two formatters disagree about how to spell a duration.** Its AST formatter
//! (what this crate emits through) writes `delay 250ms` / `per 1m`; its CST formatter (what a human
//! editing the generated file runs) accepts only bare milliseconds and declines to re-print the
//! suffixed form at all. Both spellings parse to the same AST, so nothing here is ambiguous — it is
//! an upstream defect. It bites **every `throttle`** (no window value avoids a suffix) and **every
//! `retry` carrying a delay**; a `retry` without one is unaffected and lowers normally. Both are
//! [refused](Error::UnspellableDuration) rather than emitted, because the alternative is shipping a
//! module flux's own formatter cannot format, and rewriting the token after the fact would be the
//! string surgery on generated Flux that AGENTS.md exists to prevent. See
//! [C-95](../../../docs/stories/C-95-graph-lowering.md)'s progress note.
//!
//! # The blocker this lowering states rather than papers over
//!
//! **`http.request` returns one flat string** — `HTTP {status}\n{headers}\n{body}` — not a record.
//! Flux's `jq` parses a *whole* string as JSON before extracting, so a path applied to an operation's
//! response resolves to `null` on every response, success or failure. A
//! [`Select`](NodeKind::Select) wired to an [`Operation`](NodeKind::Operation) output is therefore
//! the one case that cannot lower, and it is [refused](Error::SelectOnAnOperation) rather than
//! degraded: emitting a selector that always yields null is precisely the plausible-but-wrong output
//! AGENTS.md forbids. It lifts when `http.request` returns a record — a seam story on flux, filed
//! rather than faked. `op.rs` records the same constraint from the response side.
//!
//! # The gate on this module's own output
//!
//! [`emit_graph`] parses what it produced, checks flux's own CST formatter leaves it unchanged, and
//! loads it back as exactly one exposed composite op ([`Error::GraphNotCanonical`]). That is C-11's
//! parse-and-analyze gate applied at the point of emission rather than only in CI: a shape that stops
//! round-tripping becomes a refusal instead of a committed artifact nobody can review a diff of.
//!
//! # Where a diagnostic lands: the node-path map
//!
//! [`emit_graph_with_paths`] returns a [`NodePaths`] beside the module — `"reply"` →
//! `"body[3].then[0]"` — so that a finding flux's analyzer raises about the *generated* op can be
//! shown on the node an author actually drew.
//!
//! **The spelling is flux's own, not this repository's.** `flux_lang::analyze::Diagnostic` carries a
//! typed `node_path` field (flux's D-139) precisely so a downstream canvas keys findings off a field
//! instead of parsing them back out of message text, and this map is the other end of that seam
//! rather than a second attribution mechanism next to it. The paths are therefore generated by
//! *recording the statements as they are pushed*, in the same walk that emits them: an index counted
//! any other way is an index that can drift, and a gate binding its expected literal first — one
//! extra statement, ahead of the `when` — is exactly where hand-counting would.
//!
//! A boundary node has no entry, and that is the map being honest rather than incomplete: a
//! `trigger`, a `schedule` and an `endpoint` become *parameters* of the emitted op, flux renders no
//! path for a parameter, and inventing one (`params[0]`) would be inventing the second mechanism the
//! seam exists to avoid. Everything else appears exactly once.

use std::collections::{BTreeMap, BTreeSet};

use connector_spec::{
    Backoff, Condition, Connector, Graph, GraphNode, Idempotency, NodeKind, Operation, Port,
    PortRef, Risk,
};
use flux_lang::ast::{DraftAst, Node, Param as FluxParam, SymbolName, TypeRef};
use flux_lang::program::{CompositeOpDecl, CompositeOpMeta};

use crate::names::Symbols;
use crate::op::parameter_symbols;
use crate::types::flux_type;
use crate::{Error, Result};

/// Emit `graph` as a formatted Flux composite `op` declaration, ready to concatenate into a module.
///
/// The returned text is canonical: it parses, flux-lang's own formatter leaves it unchanged, and it
/// loads back as exactly one exposed composite op. This function asserts all three rather than
/// trusting them — see the module documentation.
///
/// # Scope
///
/// The nine node kinds listed in the module documentation, nested into `retry` / `throttle` /
/// `confirm` / `when` regions, calling operations the same connector declares. Every shape outside
/// that is a refusal with its reason stated in full; [`Error`] carries them.
pub fn emit_graph(connector: &Connector, graph: &Graph) -> Result<String> {
    Ok(emit_graph_with_paths(connector, graph)?.0)
}

/// [`emit_graph`], plus the [`NodePaths`] map that says where each node's statement ended up.
///
/// The two are produced by one walk and cannot disagree: the path of a statement is recorded at the
/// moment that statement is pushed into its block. See the module documentation for why the path
/// spelling is flux's own and why a boundary node has no entry.
pub fn emit_graph_with_paths(connector: &Connector, graph: &Graph) -> Result<(String, NodePaths)> {
    let mut paths = NodePaths::default();
    let declaration = Lowering::new(connector, graph)?.lower(&mut paths)?;
    let text = flux_lang::format::format_composite_op(&declaration);
    check_canonical(graph, &text)?;
    Ok((text, paths))
}

/// Where each graph node's statement sits inside the emitted op's AST.
///
/// A key is an author-owned node id; a value is a path in **flux's own** node-path spelling —
/// `body[3].then[0]`, the same string `flux_lang::analyze::Diagnostic::node_path` carries. Both
/// directions are answered: [`path_of`](Self::path_of) for "where did this node go", and
/// [`node_at`](Self::node_at) for "which node is this diagnostic about".
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct NodePaths {
    /// Node id → the path of the statement it produced. Ordered, so the serialized map is stable.
    paths: BTreeMap<String, String>,
}

impl NodePaths {
    /// The path of the statement `node` produced, or `None` when it produced none — a boundary node
    /// is a parameter of the emitted op, and flux renders no path for a parameter.
    pub fn path_of(&self, node: &str) -> Option<&str> {
        self.paths.get(node).map(String::as_str)
    }

    /// The graph node a diagnostic's `node_path` belongs to.
    ///
    /// flux descends *into* a statement — `body[1].body[0]` is the bind and `body[1].body[0].value`
    /// is the call it binds — so the answer is the innermost recorded statement the path sits at or
    /// inside, matched on whole segments. A path shallower than any statement belongs to no node,
    /// and so does one from some other flow.
    pub fn node_at(&self, node_path: &str) -> Option<&str> {
        self.paths
            .iter()
            .filter(|(_, path)| {
                node_path == path.as_str()
                    || node_path
                        .strip_prefix(path.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            })
            .max_by_key(|(_, path)| path.len())
            .map(|(node, _)| node.as_str())
    }

    /// Every node that reaches a statement, paired with its path, in node-id order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.paths
            .iter()
            .map(|(node, path)| (node.as_str(), path.as_str()))
    }

    /// The map as the JSON object an artifact holds — one line per node, sorted, newline-terminated.
    ///
    /// Committed beside the module it indexes, so drift shows up in a diff like any other generated
    /// output. Serialization cannot fail: every key and value is a `String`.
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.paths).expect("a map of strings");
        text.push('\n');
        text
    }

    /// Record where the statement `node` is about to produce will sit, and hand the path back so a
    /// region can prefix its children with it.
    fn record(&mut self, node: &GraphNode, block: &Block, index: usize) -> String {
        let path = block.at(index);
        self.paths.insert(node.id.clone(), path.clone());
        path
    }
}

/// The path prefix of one block of statements: `body` at the top level, and an enclosing
/// statement's path followed by the block's own label (`body[1].body`, `body[4].then`) inside a
/// region. The labels are the ones flux's analyzer pushes — a `when`'s branch is `then`, and every
/// other block this emitter produces is a `body`.
struct Block(String);

impl Block {
    /// The op's own body, where flux's analyzer starts a path.
    fn root() -> Self {
        Self("body".to_string())
    }

    /// The path of the statement at `index` of this block.
    fn at(&self, index: usize) -> String {
        format!("{}[{index}]", self.0)
    }

    /// The block labelled `label` that the statement at `path` opens.
    fn inside(path: &str, label: &str) -> Self {
        Self(format!("{path}.{label}"))
    }
}

/// The C-11 parse-and-analyze gate, applied to this module's own output.
///
/// Format-then-reparse is the cheap total round-trip check: `format_module` re-prints the parsed tree
/// and only returns text it has proved re-parses to the same module, so "unchanged" means "already
/// canonical". Loading it back through flux's module loader is the other half — a module that parsed
/// but did not *load* would publish no ops at all, which reaches a consumer as silence.
fn check_canonical(graph: &Graph, text: &str) -> Result<()> {
    let refuse = |reason: String| Error::GraphNotCanonical {
        graph: graph.name.clone(),
        reason,
    };

    let parsed = flux_lang::parser::parse_cst(text);
    if !parsed.errors.is_empty() {
        return Err(refuse(format!("it does not parse: {:?}", parsed.errors)));
    }
    match flux_lang::format_cst::format_module(&parsed) {
        Some(reformatted) if reformatted == text => {}
        Some(_) => return Err(refuse(
            "flux's own formatter would rewrite it, so the emitted module is not a fixed point \
                 of the formatter a human editing it would run"
                .to_string(),
        )),
        None => return Err(refuse("flux's formatter could not re-print it".to_string())),
    }

    let module = flux_lang::program::Module::parse_str(text)
        .map_err(|error| refuse(format!("it does not load: {error}")))?;
    let program = module
        .program()
        .ok_or_else(|| refuse("it does not load as a program".to_string()))?;
    if program.ops.len() != 1 || program.ops[0].name != graph.name {
        return Err(refuse(format!(
            "it publishes {} op(s) rather than one named `{}`",
            program.ops.len(),
            graph.name
        )));
    }
    Ok(())
}

/// One graph's lowering: the generated symbol table, then the statements that read it.
struct Lowering<'a> {
    connector: &'a Connector,
    graph: &'a Graph,
    /// The emitted op's parameters, in declaration order.
    params: Vec<FluxParam>,
    /// A boundary node's output port, or a graph input, and the parameter symbol carrying it.
    /// Keyed by `(node id, port name)`; a graph input uses an empty node id, which no node has.
    inbound: BTreeMap<(String, String), String>,
    /// The symbol a plain node binds its value to, keyed by node id.
    value: BTreeMap<String, String>,
    /// A `retry` region's `-> $bind`: the declared output port and the symbol it binds.
    region_bind: BTreeMap<String, (String, String)>,
    /// A gate's right-hand literal, bound to a symbol rather than spliced into a formula.
    expected: BTreeMap<String, String>,
}

impl<'a> Lowering<'a> {
    /// Allocate every symbol the body will bind, in the graph's own declaration order.
    ///
    /// Allocation happens up front and in one pass so that the names are a function of the graph's
    /// *structure* rather than of the order statements happen to be emitted in — which is what makes
    /// a regenerated module byte-identical.
    fn new(connector: &'a Connector, graph: &'a Graph) -> Result<Self> {
        // `format_composite_op` writes the name verbatim, so an undeclarable one would produce text
        // that does not parse rather than an error.
        if !flux_lang::ast::is_valid_decl_name(&graph.name) {
            return Err(Error::UnspellableGraphName {
                graph: graph.name.clone(),
            });
        }
        // Before anything reads `enclosing`, which is only total on an acyclic graph.
        check_acyclic(graph)?;

        let mut lowering = Self {
            connector,
            graph,
            params: Vec::new(),
            inbound: BTreeMap::new(),
            value: BTreeMap::new(),
            region_bind: BTreeMap::new(),
            expected: BTreeMap::new(),
        };
        let mut symbols = Symbols::guarded(flux_lang::ast::is_reserved_word);

        // 1. The graph's own declared parameters.
        for port in &graph.inputs {
            let symbol = lowering.allocate(&mut symbols, "", &port.name)?;
            lowering
                .inbound
                .insert((String::new(), port.name.clone()), symbol.clone());
            lowering.params.push(FluxParam {
                name: SymbolName(symbol),
                ty: port_type(port),
            });
        }

        // 2. Boundary nodes: what wakes the flow becomes what the op is called with.
        for node in graph.nodes.iter().filter(|n| n.kind.is_boundary()) {
            for port in &node.outputs {
                let base = format!("{}_{}", node.id, port.name);
                let symbol = lowering.allocate(&mut symbols, &node.id, &base)?;
                lowering
                    .inbound
                    .insert((node.id.clone(), port.name.clone()), symbol.clone());
                lowering.params.push(FluxParam {
                    name: SymbolName(symbol),
                    ty: port_type(port),
                });
            }
        }

        // 3. Every other node's value, and a gate's expected literal.
        for node in graph.nodes.iter().filter(|n| !n.kind.is_boundary()) {
            lowering.check_output_shape(node)?;
            lowering.check_durations(node)?;
            if node.kind.is_region() {
                // Only `retry` has a result bind; the other regions export the symbol their body
                // already bound. See the module documentation.
                if supports_bind(&node.kind) {
                    if let Some(port) = node.outputs.first() {
                        let base = format!("{}_{}", node.id, port.name);
                        let symbol = lowering.allocate(&mut symbols, &node.id, &base)?;
                        lowering
                            .region_bind
                            .insert(node.id.clone(), (port.name.clone(), symbol));
                    }
                }
                if let NodeKind::Gate { condition } = &node.kind {
                    if condition.right.is_some() {
                        let base = format!("{}_expected", node.id);
                        let symbol = lowering.allocate(&mut symbols, &node.id, &base)?;
                        lowering.expected.insert(node.id.clone(), symbol);
                    }
                }
                continue;
            }
            if let Some(port) = node.outputs.first() {
                let base = format!("{}_{}", node.id, port.name);
                let symbol = lowering.allocate(&mut symbols, &node.id, &base)?;
                lowering.value.insert(node.id.clone(), symbol);
            }
        }

        Ok(lowering)
    }

    /// One generated symbol, normalized through the emitter's own allocator.
    fn allocate(&self, symbols: &mut Symbols, node: &str, base: &str) -> Result<String> {
        symbols
            .allocate(&self.graph.name, base)
            .map_err(|error| Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.to_string(),
                reason: format!("no Flux symbol can be generated from `{base}`: {error}"),
            })
    }

    /// What a node may declare on its output side, per kind.
    fn check_output_shape(&self, node: &GraphNode) -> Result<()> {
        if let NodeKind::Gate { .. } = &node.kind {
            if let Some(port) = node.outputs.first() {
                return Err(Error::GateExportsAValue {
                    graph: self.graph.name.clone(),
                    node: node.id.clone(),
                    port: port.name.clone(),
                });
            }
            return Ok(());
        }
        if node.kind.is_region() {
            if supports_bind(&node.kind) && node.outputs.len() > 1 {
                return Err(Error::RetryExportsMoreThanOneValue {
                    graph: self.graph.name.clone(),
                    region: node.id.clone(),
                    count: node.outputs.len(),
                });
            }
            // A region's declared ports are promises its body keeps; `producer_of` checks each one.
            for port in &node.outputs {
                self.producer_of(node, &port.name)?;
            }
            return Ok(());
        }
        if node.outputs.len() > 1 {
            return Err(Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                reason: format!(
                    "a `{}` produces one value and declares {} output ports. Flux binds one symbol \
                     per statement; two ports would need the value bound twice under two names",
                    node.kind.word(),
                    node.outputs.len()
                ),
            });
        }
        Ok(())
    }

    /// Refuse a duration flux-lang cannot spell the same way twice.
    ///
    /// See [`Error::UnspellableDuration`]: flux's AST formatter writes `1m`/`1s`/`250ms` and its CST
    /// formatter accepts only bare milliseconds, so a `throttle` (whose window always takes a suffix)
    /// and a `retry` carrying a delay both emit text flux's own formatter then declines to re-print.
    /// A `retry` without a delay is untouched, which is why the check is this narrow.
    fn check_durations(&self, node: &GraphNode) -> Result<()> {
        let refuse = |clause, ms| {
            Err(Error::UnspellableDuration {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                clause,
                ms,
                suffixed: suffixed_duration(ms),
            })
        };
        match &node.kind {
            NodeKind::Retry {
                delay_ms: Some(ms), ..
            } => refuse("delay", *ms),
            NodeKind::Throttle { window_ms, .. } => refuse("per", *window_ms),
            _ => Ok(()),
        }
    }

    /// The one node directly inside `region` that binds its output port `port`.
    fn producer_of(&self, region: &'a GraphNode, port: &str) -> Result<&'a GraphNode> {
        let mut found: Option<&GraphNode> = None;
        for node in self.graph.nodes_in(&region.id) {
            if !node.outputs.iter().any(|p| p.name == port) {
                continue;
            }
            if let Some(first) = found {
                return Err(Error::AmbiguousRegionOutput {
                    graph: self.graph.name.clone(),
                    region: region.id.clone(),
                    port: port.to_string(),
                    first: first.id.clone(),
                    second: node.id.clone(),
                });
            }
            found = Some(node);
        }
        found.ok_or_else(|| Error::UnboundRegionOutput {
            graph: self.graph.name.clone(),
            region: region.id.clone(),
            port: port.to_string(),
        })
    }

    fn node(&self, id: &str) -> Result<&'a GraphNode> {
        self.graph
            .node(id)
            .ok_or_else(|| Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: id.to_string(),
                reason: "the graph declares no such node".to_string(),
            })
    }

    /// The symbol carrying `reference`'s value **as seen inside the scope that binds it**.
    fn local_symbol(&self, reference: &PortRef) -> Result<String> {
        let node = self.node(&reference.node)?;
        if !node.outputs.iter().any(|p| p.name == reference.port) {
            return Err(Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                reason: format!("it declares no output port `{}`", reference.port),
            });
        }
        if node.kind.is_boundary() {
            return Ok(self.inbound[&(node.id.clone(), reference.port.clone())].clone());
        }
        if node.kind.is_region() {
            // A `retry`'s port is the block's own `-> $bind`, which lives in the enclosing scope.
            if let Some((port, symbol)) = self.region_bind.get(&node.id) {
                if port == &reference.port {
                    return Ok(symbol.clone());
                }
            }
            // Otherwise the region exports what its body bound, so keep walking inwards.
            let producer = self.producer_of(node, &reference.port)?;
            return self.local_symbol(&PortRef {
                node: producer.id.clone(),
                port: reference.port.clone(),
            });
        }
        Ok(self.value[&node.id].clone())
    }

    /// The symbol a consumer sitting inside `consumer_regions` reads `from` through.
    ///
    /// This is where rule 2 is enforced: for every region the value has to *leave*, the region must
    /// declare an output port of that name, and a `retry`'s bind replaces the symbol on the way out.
    fn symbol_for(&self, from: &PortRef, consumer_regions: &[&str]) -> Result<String> {
        let mut symbol = self.local_symbol(from)?;
        let source_regions = self
            .graph
            .enclosing(&from.node)
            .ok_or_else(|| Error::GraphCycle {
                graph: self.graph.name.clone(),
                whereabouts: format!(" in the regions containing `{}`", from.node),
            })?;

        for region_id in source_regions {
            if consumer_regions.contains(&region_id) {
                break; // the consumer is inside this region; nothing escapes it
            }
            let region = self.node(region_id)?;
            if !region.outputs.iter().any(|p| p.name == from.port) {
                return Err(Error::RegionCrossingEdge {
                    graph: self.graph.name.clone(),
                    node: from.node.clone(),
                    port: from.port.clone(),
                    region: region_id.to_string(),
                });
            }
            if let Some((port, bind)) = self.region_bind.get(region_id) {
                if port == &from.port {
                    symbol = bind.clone();
                }
            }
        }
        Ok(symbol)
    }

    /// Every input port of `node` that an edge feeds, mapped to the symbol carrying it.
    fn inputs_of(&self, node: &GraphNode) -> Result<BTreeMap<String, String>> {
        let regions = self
            .graph
            .enclosing(&node.id)
            .ok_or_else(|| Error::GraphCycle {
                graph: self.graph.name.clone(),
                whereabouts: format!(" in the regions containing `{}`", node.id),
            })?;

        let mut bound: BTreeMap<String, String> = BTreeMap::new();
        for edge in self.graph.edges.iter().filter(|e| e.to.node == node.id) {
            if !node.inputs.iter().any(|p| p.name == edge.to.port) {
                return Err(Error::UnlowerableGraphNode {
                    graph: self.graph.name.clone(),
                    node: node.id.clone(),
                    reason: format!(
                        "an edge feeds `{}`, which is not a declared input port",
                        edge.to.port
                    ),
                });
            }
            let symbol = self.symbol_for(&edge.from, &regions)?;
            if let Some(existing) = bound.insert(edge.to.port.clone(), symbol.clone()) {
                if existing != symbol {
                    return Err(Error::UnlowerableGraphNode {
                        graph: self.graph.name.clone(),
                        node: node.id.clone(),
                        reason: format!(
                            "two edges feed input port `{}`, so which value it carries is \
                             undecided",
                            edge.to.port
                        ),
                    });
                }
            }
        }
        // A required port nothing feeds would emit a call missing an argument — refused by flux at
        // analysis if the argument is declared, and silently absent if it is not.
        for port in node.inputs.iter().filter(|p| p.required) {
            if !bound.contains_key(&port.name) {
                return Err(Error::UnlowerableGraphNode {
                    graph: self.graph.name.clone(),
                    node: node.id.clone(),
                    reason: format!("no edge feeds its required input port `{}`", port.name),
                });
            }
        }
        Ok(bound)
    }

    // -----------------------------------------------------------------------
    // Emission
    // -----------------------------------------------------------------------

    fn lower(&self, paths: &mut NodePaths) -> Result<CompositeOpDecl> {
        let mut body = self.emit_region(None, &Block::root(), paths)?;
        if let Some(output) = &self.graph.output {
            body.push(Node::Return {
                value: Box::new(symbol(&self.symbol_for(output, &[])?)),
            });
        }
        Ok(CompositeOpDecl {
            name: self.graph.name.clone(),
            params: self.params.clone(),
            returns: Some(TypeRef::Any),
            meta: self.metadata()?,
            body: DraftAst {
                body,
                ..DraftAst::default()
            },
        })
    }

    /// The statements of one block: the nodes directly inside `region`, in dependency order.
    ///
    /// `block` is the path prefix those statements are addressed by, threaded down so each node can
    /// record where it landed as it lands there.
    fn emit_region(
        &self,
        region: Option<&str>,
        block: &Block,
        paths: &mut NodePaths,
    ) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for node in self.order(region)? {
            self.emit_node(node, &mut out, block, paths)?;
        }
        Ok(out)
    }

    /// The nodes directly inside `region`, ordered so every value is bound before it is read.
    ///
    /// The ordering is over a **collapsed** graph: an edge between two nodes counts as a dependency
    /// between whichever children of this level *contain* them, so a statement feeding something deep
    /// inside a region still lands before the region's block. Declaration order breaks ties, which is
    /// what keeps a regenerated module stable.
    fn order(&self, region: Option<&str>) -> Result<Vec<&'a GraphNode>> {
        let children: Vec<&GraphNode> = self
            .graph
            .nodes
            .iter()
            .filter(|node| !node.kind.is_boundary() && node.region.as_deref() == region)
            .collect();

        // Which child of this level owns each node of the graph, if any.
        let mut owner: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, child) in children.iter().enumerate() {
            owner.insert(child.id.as_str(), index);
            for node in &self.graph.nodes {
                let enclosing =
                    self.graph
                        .enclosing(&node.id)
                        .ok_or_else(|| Error::GraphCycle {
                            graph: self.graph.name.clone(),
                            whereabouts: format!(" in the regions containing `{}`", node.id),
                        })?;
                if enclosing.contains(&child.id.as_str()) {
                    owner.insert(node.id.as_str(), index);
                }
            }
        }

        let mut incoming = vec![0usize; children.len()];
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); children.len()];
        for edge in &self.graph.edges {
            let (Some(&from), Some(&to)) = (
                owner.get(edge.from.node.as_str()),
                owner.get(edge.to.node.as_str()),
            ) else {
                continue; // one end sits outside this level entirely
            };
            if from == to {
                continue; // both ends inside one child; its own block orders them
            }
            adjacency[from].push(to);
            incoming[to] += 1;
        }

        // Kahn's algorithm, taking the earliest-declared ready node so the result is deterministic.
        let mut ready: BTreeSet<usize> =
            (0..children.len()).filter(|i| incoming[*i] == 0).collect();
        let mut ordered: Vec<&GraphNode> = Vec::new();
        while let Some(&index) = ready.iter().next() {
            ready.remove(&index);
            ordered.push(children[index]);
            for &next in &adjacency[index] {
                incoming[next] -= 1;
                if incoming[next] == 0 {
                    ready.insert(next);
                }
            }
        }
        if ordered.len() != children.len() {
            return Err(Error::GraphCycle {
                graph: self.graph.name.clone(),
                whereabouts: match region {
                    Some(region) => format!(" among the nodes of region `{region}`"),
                    None => " among its top-level nodes".to_string(),
                },
            });
        }
        Ok(ordered)
    }

    fn emit_node(
        &self,
        node: &GraphNode,
        out: &mut Vec<Node>,
        block: &Block,
        paths: &mut NodePaths,
    ) -> Result<()> {
        let refuse = |reason: String| Error::UnlowerableGraphNode {
            graph: self.graph.name.clone(),
            node: node.id.clone(),
            reason,
        };
        let inputs = self.inputs_of(node)?;

        // A plain node pushes exactly one statement and pushes it last, so the path it will occupy
        // is this block's current end. A region records its own inside its arm instead — it needs
        // the path to prefix its children with, and a gate has to bind its expected literal first,
        // which moves it along by one.
        if !node.kind.is_region() && !node.kind.is_boundary() {
            paths.record(node, block, out.len());
        }

        match &node.kind {
            NodeKind::Operation { operation } => {
                let target = self.operation(node, operation)?;
                let declared = parameter_symbols(target)?;
                let mut args: BTreeMap<String, Box<Node>> = BTreeMap::new();
                for (port, value) in &inputs {
                    let name = declared.get(port).ok_or_else(|| {
                        refuse(format!(
                            "its input port `{port}` names no parameter of operation \
                             `{operation}`, so the call would carry an argument the operation does \
                             not declare"
                        ))
                    })?;
                    args.insert(name.clone(), Box::new(symbol(value)));
                }
                let call = Node::Call {
                    op: target.id.clone(),
                    args: if args.is_empty() {
                        Vec::new()
                    } else {
                        vec![Node::Obj { fields: args }]
                    },
                };
                // A node declaring no output port has no value to carry, so the call is a statement
                // whose result is discarded — which is exactly what a terminal notify is.
                match self.value.get(&node.id) {
                    Some(name) => out.push(bind(name, call)),
                    None => out.push(call),
                }
            }

            NodeKind::Select { path } => {
                let (port, source) = self.sole_input(node, &inputs)?;
                self.refuse_select_on_an_operation(node, path, port)?;
                let jq = format!(".{path}");
                // flux spells a `jq` natively only as field-access sugar over a plain symbol; a path
                // it cannot spell falls back to `@json`, which its own formatter re-spaces.
                if !jq[1..]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                {
                    return Err(refuse(format!(
                        "it selects `{path}`, which Flux cannot spell as field access — a path \
                         carrying anything but ASCII letters, digits, `_` and `.` falls back to an \
                         `@json` escape that flux's own formatter would rewrite"
                    )));
                }
                out.push(bind(
                    self.value_of(node)?,
                    Node::Jq {
                        path: jq,
                        input: Box::new(symbol(&source)),
                        // Strict: a typo'd field name fails loudly rather than reading empty.
                        optional: false,
                    },
                ));
            }

            NodeKind::Template { format } => {
                let template = self.template(node, format, &inputs)?;
                out.push(bind(self.value_of(node)?, Node::Fmt { template }));
            }

            NodeKind::Object { fields } => {
                if fields.is_empty() {
                    return Err(refuse(
                        "it assembles a record with no fields. An empty `obj` has no native Flux \
                         spelling, so it would leave the module non-canonical as well as empty"
                            .to_string(),
                    ));
                }
                let mut record: BTreeMap<String, Box<Node>> = BTreeMap::new();
                for (field, port) in fields {
                    let value = inputs.get(port).ok_or_else(|| {
                        refuse(format!(
                            "it builds field `{field}` from port `{port}`, which no edge feeds"
                        ))
                    })?;
                    record.insert(field.clone(), Box::new(symbol(value)));
                }
                out.push(bind(self.value_of(node)?, Node::Obj { fields: record }));
            }

            NodeKind::Literal { value } => {
                check_scalar(self.graph, node, value)?;
                out.push(bind(
                    self.value_of(node)?,
                    Node::Lit {
                        value: value.clone(),
                    },
                ));
            }

            NodeKind::Gate { condition } => {
                // The condition binds its expected literal first, when it has one, so the `when`
                // lands at whatever index the block has reached by now — not the one it started at.
                let cond = self.condition(node, condition, out)?;
                let here = paths.record(node, block, out.len());
                out.push(Node::When {
                    cond: Box::new(cond),
                    then: self.emit_region(Some(&node.id), &Block::inside(&here, "then"), paths)?,
                    // Flux's `when` has no else branch here — which is exactly why a gate exports
                    // nothing. See `Error::GateExportsAValue`.
                    otherwise: Vec::new(),
                });
            }

            NodeKind::Approval { message, risk } => {
                let here = paths.record(node, block, out.len());
                out.push(Node::Confirm {
                    message: message.clone(),
                    risk: Some(risk_tag(*risk).to_string()),
                    body: self.emit_region(Some(&node.id), &Block::inside(&here, "body"), paths)?,
                })
            }

            NodeKind::Retry {
                max,
                backoff,
                delay_ms,
            } => {
                let here = paths.record(node, block, out.len());
                let mut body =
                    self.emit_region(Some(&node.id), &Block::inside(&here, "body"), paths)?;
                let bound = match self.region_bind.get(&node.id) {
                    Some((port, name)) => {
                        // `-> $bind` captures the block's *final* result, so the block ends in a
                        // reference to the statement that produced the exported value.
                        let producer = self.producer_of(node, port)?;
                        body.push(symbol(&self.local_symbol(&PortRef {
                            node: producer.id.clone(),
                            port: port.clone(),
                        })?));
                        Some(SymbolName(name.clone()))
                    }
                    None => None,
                };
                out.push(retry_node(*max, *backoff, *delay_ms, body, bound));
            }

            NodeKind::Throttle { max, window_ms } => {
                let here = paths.record(node, block, out.len());
                out.push(throttle_node(
                    &self.graph.name,
                    &node.id,
                    *max,
                    *window_ms,
                    self.emit_region(Some(&node.id), &Block::inside(&here, "body"), paths)?,
                ))
            }

            // A boundary declares what wakes the flow and is emitted nowhere — it is already a
            // parameter, and `order` never yields one.
            NodeKind::Trigger { .. } | NodeKind::Schedule { .. } | NodeKind::Endpoint { .. } => {}
        }
        Ok(())
    }

    /// The operation a call node names, in this connector.
    fn operation(&self, node: &GraphNode, id: &str) -> Result<&'a Operation> {
        self.connector
            .operation(id)
            .ok_or_else(|| Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                reason: format!("it names operation `{id}`, which this connector does not declare"),
            })
    }

    /// The symbol a node binds its value to, or a refusal when it declared no output port to carry
    /// one. Only a call has a statement form that discards its result.
    fn value_of(&self, node: &GraphNode) -> Result<&str> {
        self.value
            .get(&node.id)
            .map(String::as_str)
            .ok_or_else(|| Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                reason: format!(
                    "a `{}` computes a value and declares no output port to carry it",
                    node.kind.word()
                ),
            })
    }

    /// The one input a `select` reads from.
    fn sole_input(
        &self,
        node: &GraphNode,
        inputs: &BTreeMap<String, String>,
    ) -> Result<(String, String)> {
        let mut entries = inputs.iter();
        match (entries.next(), entries.next()) {
            (Some((port, symbol)), None) => Ok((port.clone(), symbol.clone())),
            _ => Err(Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                reason: format!(
                    "a `{}` reads one value and {} edges feed it",
                    node.kind.word(),
                    inputs.len()
                ),
            }),
        }
    }

    /// The node that actually **binds** the value at `reference`, looking through region ports.
    ///
    /// A region's output port is an alias for whatever inside it produced the value, so `read.result`
    /// and the `fetch` statement inside `read` name one symbol. Anything asking *what produced this*
    /// — the blocker below, above all — has to see through the alias, or a value laundered through
    /// one region port would answer differently from the same value read directly.
    fn producing_node(&self, reference: &PortRef) -> Result<&'a GraphNode> {
        let node = self.node(&reference.node)?;
        if node.kind.is_region() {
            let producer = self.producer_of(node, &reference.port)?;
            return self.producing_node(&PortRef {
                node: producer.id.clone(),
                port: reference.port.clone(),
            });
        }
        Ok(node)
    }

    /// **The blocker.** A `select` reading an operation's response cannot lower — see the module
    /// documentation and [`Error::SelectOnAnOperation`].
    fn refuse_select_on_an_operation(
        &self,
        node: &GraphNode,
        path: &str,
        port: String,
    ) -> Result<()> {
        let Some(edge) = self
            .graph
            .edges
            .iter()
            .find(|e| e.to.node == node.id && e.to.port == port)
        else {
            return Ok(());
        };
        // Through region ports: a response handed out of a `retry` is still a response.
        let source = self.producing_node(&edge.from)?;
        let NodeKind::Operation { operation } = &source.kind else {
            return Ok(());
        };
        Err(Error::SelectOnAnOperation {
            graph: self.graph.name.clone(),
            select: node.id.clone(),
            path: path.to_string(),
            from: source.id.clone(),
            operation: operation.clone(),
        })
    }

    /// A template's `{port}` placeholders rewritten to the symbols carrying them.
    ///
    /// Both directions of mismatch are refused, for the reason `op.rs` states about URLs: flux's
    /// interpolator leaves an unbound `{name}` *verbatim* in the string, so an unresolvable
    /// placeholder would travel to the vendor as literal braces rather than failing.
    fn template(
        &self,
        node: &GraphNode,
        format: &str,
        inputs: &BTreeMap<String, String>,
    ) -> Result<String> {
        let refuse = |reason: String| Error::UnlowerableGraphNode {
            graph: self.graph.name.clone(),
            node: node.id.clone(),
            reason,
        };
        let mut out = String::new();
        let mut used: BTreeSet<&str> = BTreeSet::new();
        let mut rest = format;

        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else {
                return Err(refuse(format!(
                    "its format opens `{{` at `{after}` and never closes it"
                )));
            };
            let port = &after[..close];
            let symbol = inputs.get(port).ok_or_else(|| {
                refuse(format!(
                    "its format references `{{{port}}}`, which is not an input port an edge feeds. \
                     Flux leaves an unbound placeholder verbatim in the string, so it would travel \
                     as literal braces"
                ))
            })?;
            used.insert(port);
            out.push_str(&format!("{{{symbol}}}"));
            rest = &after[close + 1..];
        }
        out.push_str(rest);

        if let Some(port) = node.inputs.iter().find(|p| !used.contains(p.name.as_str())) {
            return Err(refuse(format!(
                "its input port `{}` never appears in its format, so nothing it carries could \
                 travel",
                port.name
            )));
        }
        Ok(out)
    }

    /// The Flux expression a [`Condition`] generates.
    ///
    /// **The author writes no formula.** A condition is a port reference, one of seven operators and
    /// a literal; this is where that structure becomes Flux. The literal is *bound to a symbol*
    /// first, so nothing an author typed is ever spliced into expression text — which is both the
    /// north-star rule and the only spelling that survives flux's formatter.
    fn condition(
        &self,
        node: &GraphNode,
        condition: &Condition,
        out: &mut Vec<Node>,
    ) -> Result<Node> {
        let regions = self
            .graph
            .enclosing(&node.id)
            .ok_or_else(|| Error::GraphCycle {
                graph: self.graph.name.clone(),
                whereabouts: format!(" in the regions containing `{}`", node.id),
            })?;
        let left = self.symbol_for(&condition.left, &regions)?;

        let Some(operator) = condition.op.operator() else {
            // `exists` is a presence check, and flux's `when` is truthiness — the same guard `op.rs`
            // uses for an unsupplied query filter, with the same documented caveat: a deliberate
            // `0`, `false` or `""` reads as absent.
            if condition.right.is_some() {
                return Err(Error::UnlowerableGraphNode {
                    graph: self.graph.name.clone(),
                    node: node.id.clone(),
                    reason:
                        "its condition is an `exists` check carrying a right-hand literal, and \
                             a presence check compares against nothing"
                            .to_string(),
                });
            }
            return Ok(symbol(&left));
        };

        let expected = condition
            .right
            .as_ref()
            .ok_or_else(|| Error::UnlowerableGraphNode {
                graph: self.graph.name.clone(),
                node: node.id.clone(),
                reason: format!(
                    "its condition compares with `{operator}` and declares no right-hand literal"
                ),
            })?;
        check_scalar(self.graph, node, expected)?;
        let right = self.expected[&node.id].clone();
        out.push(bind(
            &right,
            Node::Lit {
                value: expected.clone(),
            },
        ));

        Ok(Node::Expr {
            formula: format!("{left} {operator} {right}"),
            // Each variable maps to the symbol of the same name, which is what lets flux's formatter
            // invert the expression back to `$left == $right` instead of an `@json` escape.
            vars: BTreeMap::from([
                (left.clone(), Box::new(symbol(&left))),
                (right.clone(), Box::new(symbol(&right))),
            ]),
        })
    }

    /// The `description`/`risk`/`idempotency`/`effects`/`expose` block.
    ///
    /// **Derived from the operations the flow actually calls, never defaulted.** flux's approval gate
    /// reads `risk` and `idempotency`, so a flow that deletes must not inherit the `low` of the reads
    /// it also makes, and one non-idempotent call makes the whole flow unsafe to retry.
    fn metadata(&self) -> Result<CompositeOpMeta> {
        let called: Vec<&Operation> = self
            .graph
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                NodeKind::Operation { operation } => Some(self.operation(node, operation)),
                _ => None,
            })
            .collect::<Result<_>>()?;

        let risk = called
            .iter()
            .map(|operation| operation.risk)
            .max_by_key(|risk| risk_rank(*risk))
            .unwrap_or(Risk::Low);
        let idempotency = if called
            .iter()
            .any(|o| o.idempotency == Idempotency::NonIdempotent)
        {
            Idempotency::NonIdempotent
        } else if called
            .iter()
            .any(|o| o.idempotency == Idempotency::Conditional)
        {
            Idempotency::Conditional
        } else {
            Idempotency::Idempotent
        };

        Ok(CompositeOpMeta {
            description: self.graph.description.clone(),
            risk: crate::op::from_tag(risk_tag(risk))?,
            idempotency: crate::op::from_tag(idempotency_tag(idempotency))?,
            // Every call the flow makes is an HTTP request; a flow that makes none has no effect
            // flux tracks, and saying `network` anyway would be a claim nothing supports.
            effects: if called.is_empty() {
                Vec::new()
            } else {
                vec![crate::op::from_tag("network")?]
            },
            // Authored, unlike the `risk` and `idempotency` above it: those are derived from the
            // called set because a flow that deletes must not inherit the `low` of its reads, but a
            // curated flow over uncurated operations is exactly the shape C-413 exists to allow, so
            // deriving exposure from the called set would forbid it.
            expose: self.graph.expose,
            ..CompositeOpMeta::default()
        })
    }
}

/// Neither the data edges nor the region containment may cycle.
fn check_acyclic(graph: &Graph) -> Result<()> {
    for node in &graph.nodes {
        if graph.enclosing(&node.id).is_none() {
            return Err(Error::GraphCycle {
                graph: graph.name.clone(),
                whereabouts: format!(" in the regions containing `{}`", node.id),
            });
        }
    }
    if graph.topological_order().is_none() {
        return Err(Error::GraphCycle {
            graph: graph.name.clone(),
            whereabouts: " in its edges".to_string(),
        });
    }
    Ok(())
}

/// A literal must be a scalar.
///
/// An object or array `lit` renders as compact JSON, which flux's own CST formatter re-spaces
/// (`{"a":1}` becomes `{ "a": 1 }`) — so the emitted module stops being a fixed point of the
/// formatter a human editing it would run. There is no second spelling to fall back to: a record
/// whose values are all literals is not "dynamic" to the formatter either, and lands on the same
/// `@json` escape. Refused rather than emitted, exactly as `op.rs` binds every literal it
/// contributes for the same reason.
fn check_scalar(graph: &Graph, node: &GraphNode, value: &serde_json::Value) -> Result<()> {
    if value.is_object() || value.is_array() {
        return Err(Error::UnlowerableGraphNode {
            graph: graph.name.clone(),
            node: node.id.clone(),
            reason: format!(
                "it carries a composite literal ({}), and flux's own formatter re-spaces a JSON \
                 object or array, so the emitted module would stop being a fixed point of the \
                 formatter. Assemble a record with an `object` node instead",
                if value.is_object() {
                    "an object"
                } else {
                    "an array"
                }
            ),
        });
    }
    Ok(())
}

/// Whether flux's node for this region kind carries a `-> $bind`. Only `retry` does.
fn supports_bind(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Retry { .. })
}

/// Flux's `retry` node — **the one construction for "bound this and try again"**.
///
/// [C-12](../../../docs/stories/C-12-quirks-as-control-flow.md) turns a declared quirk (a rate
/// limit, a paginated read) into `retry`/`throttle`/a bounded loop, and a `Retry` graph node is the
/// same intent stated by hand. Two code paths emitting different Flux for one intent is the failure
/// to avoid, so both go through here: C-12 calls this rather than rebuilding the node.
pub(crate) fn retry_node(
    max: u32,
    backoff: Backoff,
    delay_ms: Option<u64>,
    body: Vec<Node>,
    bind: Option<SymbolName>,
) -> Node {
    Node::Retry {
        max,
        backoff: backoff_tag(backoff).map(str::to_string),
        delay_ms,
        body,
        bind,
    }
}

/// Flux's `throttle` node, with its bucket name **generated** rather than authored.
///
/// flux keys the token bucket by name and holds it in the session store, so two buckets spelled
/// alike collide silently and one caller's rate limit quietly becomes another's. `owner` is whatever
/// declares the limit — a graph, or the operation C-12 derives one from — and the pair is what makes
/// the name unique without an author ever choosing it.
pub(crate) fn throttle_node(
    owner: &str,
    node: &str,
    max: u32,
    window_ms: u64,
    body: Vec<Node>,
) -> Node {
    Node::Throttle {
        name: format!("{owner}#{node}"),
        max,
        window_ms,
        body,
    }
}

/// The Flux type a port declares, falling back to `Any` where the IR states no schema.
fn port_type(port: &Port) -> TypeRef {
    port.schema.as_ref().map(flux_type).unwrap_or(TypeRef::Any)
}

/// The spelling flux's **AST** formatter gives a duration, reproduced here only so a refusal can
/// quote it back. Mirrors `flux_lang::format::fmt_duration`, which is private.
fn suffixed_duration(ms: u64) -> String {
    if ms != 0 && ms.is_multiple_of(60_000) {
        format!("{}m", ms / 60_000)
    } else if ms != 0 && ms.is_multiple_of(1_000) {
        format!("{}s", ms / 1_000)
    } else {
        format!("{ms}ms")
    }
}

/// The backoff word flux's `retry` reads, or `None` for a fixed delay, which takes no clause.
fn backoff_tag(backoff: Backoff) -> Option<&'static str> {
    match backoff {
        Backoff::None => None,
        Backoff::Linear => Some("linear"),
        Backoff::Exponential => Some("exponential"),
    }
}

/// Riskiest wins. [`Risk`] carries no ordering of its own — deliberately, since it is flux's
/// vocabulary rather than a scale — so the ranking is stated here where it is used.
fn risk_rank(risk: Risk) -> u8 {
    match risk {
        Risk::Low => 0,
        Risk::Medium => 1,
        Risk::High => 2,
        Risk::Destructive => 3,
    }
}

fn risk_tag(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "low",
        Risk::Medium => "medium",
        Risk::High => "high",
        Risk::Destructive => "destructive",
    }
}

fn idempotency_tag(idempotency: Idempotency) -> &'static str {
    match idempotency {
        Idempotency::Idempotent => "idempotent",
        Idempotency::NonIdempotent => "non_idempotent",
        Idempotency::Conditional => "conditional",
    }
}

/// `$name` in expression position.
fn symbol(name: &str) -> Node {
    Node::Var {
        name: SymbolName(name.to_string()),
    }
}

/// `$name = <expr>`.
fn bind(name: &str, value: Node) -> Node {
    Node::Bind {
        name: SymbolName(name.to_string()),
        value: Box::new(value),
        ty: None,
        effect: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The allocator asks **flux** what is reserved rather than carrying its own copy of the list,
    /// so a word flux adds is dodged the moment the pin moves. Under 0.39 a bare `retry = …` reads
    /// as a statement keyword, so this is the check that keeps a node id an author chose from
    /// generating one.
    #[test]
    fn a_name_flux_reserves_is_never_handed_out() {
        for keyword in [
            "retry", "throttle", "when", "return", "do", "op", "true", "channel",
        ] {
            assert!(
                flux_lang::ast::is_reserved_word(keyword),
                "`{keyword}` is expected to be reserved by the flux-lang pin"
            );
            let mut symbols = Symbols::guarded(flux_lang::ast::is_reserved_word);
            let allocated = symbols.allocate("g", keyword).expect("a spellable base");
            assert_ne!(
                allocated, keyword,
                "`{keyword}` must not be handed out as a generated symbol"
            );
            assert!(
                flux_lang::ast::is_bare_symbol_name(&allocated),
                "`{allocated}` must be spellable without the sigil"
            );
        }
    }

    /// Only `retry` carries a result bind in flux, which is the whole reason a region's output port
    /// lowers two different ways.
    #[test]
    fn retry_is_the_only_region_kind_flux_gives_a_bind() {
        assert!(supports_bind(&NodeKind::Retry {
            max: 1,
            backoff: Backoff::None,
            delay_ms: None
        }));
        assert!(!supports_bind(&NodeKind::Throttle {
            max: 1,
            window_ms: 1
        }));
        assert!(!supports_bind(&NodeKind::Approval {
            message: String::new(),
            risk: Risk::Low
        }));
    }

    /// A fixed delay takes no `backoff` clause; the other two name themselves.
    #[test]
    fn a_fixed_delay_emits_no_backoff_clause() {
        assert_eq!(backoff_tag(Backoff::None), None);
        assert_eq!(backoff_tag(Backoff::Linear), Some("linear"));
        assert_eq!(backoff_tag(Backoff::Exponential), Some("exponential"));
    }

    /// A diagnostic path is matched on **whole segments**, and the innermost statement wins.
    ///
    /// The failure this rules out is the cheap one: `body[1]` is a textual prefix of `body[10]`, so
    /// a plain `starts_with` would key the eleventh statement of a flow to whatever produced the
    /// second. `graph_emitter.rs` holds the other half — the same lookup against paths flux's own
    /// analyzer produced.
    #[test]
    fn a_node_path_is_matched_on_whole_segments() {
        let mut paths = NodePaths::default();
        paths
            .paths
            .insert("read".to_string(), "body[1]".to_string());
        paths
            .paths
            .insert("fetch".to_string(), "body[1].body[0]".to_string());
        paths
            .paths
            .insert("late".to_string(), "body[10]".to_string());

        assert_eq!(paths.node_at("body[1]"), Some("read"));
        assert_eq!(paths.node_at("body[10]"), Some("late"));
        // Deeper than a statement: flux descends into the bind, and the answer is still the node.
        assert_eq!(paths.node_at("body[1].body[0].value"), Some("fetch"));
        assert_eq!(paths.node_at("body[1].body[0]"), Some("fetch"));
        // Neither a statement of this flow nor inside one.
        assert_eq!(paths.node_at("body"), None);
        assert_eq!(paths.node_at("body[2]"), None);
        assert_eq!(paths.node_at("body[100]"), None);
    }

    /// The map survives the round trip an artifact makes: emitted to JSON, read back identical.
    #[test]
    fn the_map_round_trips_through_its_artifact_form() {
        let mut paths = NodePaths::default();
        paths
            .paths
            .insert("notify".to_string(), "body[2].then[0]".to_string());

        let json = paths.to_json();
        assert!(json.ends_with('\n'), "an artifact ends in a newline");
        assert_eq!(
            serde_json::from_str::<NodePaths>(&json).expect("the artifact form parses"),
            paths
        );
    }

    #[test]
    fn risk_ranks_from_low_to_destructive() {
        assert!(risk_rank(Risk::Low) < risk_rank(Risk::Medium));
        assert!(risk_rank(Risk::Medium) < risk_rank(Risk::High));
        assert!(risk_rank(Risk::High) < risk_rank(Risk::Destructive));
    }
}

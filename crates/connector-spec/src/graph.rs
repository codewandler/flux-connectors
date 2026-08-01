//! A **flow graph**: connector members composed into one Flux `op`.
//!
//! Four waves built this vocabulary without naming it. An [`Operation`](crate::Operation) is a call
//! node, an [`EventDecl`](crate::EventDecl) is a source node, a
//! [`ChannelBinding`](crate::ChannelBinding) already describes itself as *"the composition of the
//! two"*, an [`Oip`](crate::Oip) is a global node id, and the dotted payload grammar is an edge. What
//! was missing is the graph — a way to say *this operation, then that gate, then that operation*, and
//! a schema an editor can render.
//!
//! # Why this is not the second language the north star forbids
//!
//! The vision says *"No homegrown DSL… We never invent a second little language to sit in front of
//! it."* This repository has ruled on that four times, and the rulings are consistent: every
//! **rejection** was an *expression* language — a template DSL, JSONPath, a vendor's remote expression
//! evaluator — and every **acceptance** was declarative structure that compiles to Flux: inbound
//! events, channel bindings, configuration fields.
//!
//! So the line is exact, and it is the one thing this module must never cross: **no node carries a
//! user-typed formula.** [`Condition`] is a closed comparison over declared ports, and the Flux
//! expression it becomes is *generated here* — the author never writes one. That is the entire
//! difference between this and action-proxy's `retry: ':params.disable_retry ? -1 : 2'`.
//!
//! [`NodeKind::free_text`] is the tripwire that keeps it true: it destructures every variant
//! exhaustively, so a field added later fails to compile until somebody states what kind of text it
//! is.
//!
//! # A view onto Flux, not a layer over it
//!
//! `flux_lang::ast::Node` has 43 kinds. This repository constructs nine. Everything here already
//! exists in the language and is merely unexposed:
//!
//! ```text
//! Throttle   → Node::Throttle {name, max, window_ms}
//! Approval   → Node::Confirm  {message, risk}
//! Retry      → Node::Retry    {max, backoff, delay_ms}
//! Gate       → Node::When
//! Select     → Node::Jq        Template → Node::Fmt
//! Object     → Node::Obj       Literal  → Node::Lit
//! ```
//!
//! # The boundary: two node kinds are not Flux nodes at all
//!
//! flux lifts **only `op` declarations** from `~/.flux/flows`; `channel` and `trigger` are Program
//! members an operator writes. So [`NodeKind::Trigger`], [`NodeKind::Schedule`] and
//! [`NodeKind::Endpoint`] are the graph's **boundary**: they declare what wakes it, become the op's
//! parameters, and are **emitted nowhere**. The operator writes the two-line program that binds them.
//! Same strict split channel bindings already hold.
//!
//! # Control flow must nest; data flow need not
//!
//! Flux has no `goto`. A statement may read any bound symbol, so data convergence is free — but a
//! branch is a nested block, so **control** must nest. Hence [`Graph`]'s four structural rules, of
//! which the third is the one with teeth:
//!
//! 1. No cycles.
//! 2. No edge crosses a region boundary except through a port the region declares.
//! 3. **A [`NodeKind::Gate`] may declare no outputs.** Flux's `when` has no `else` here, so a symbol
//!    bound inside it is *unbound* on the false path, and reading it afterwards is a runtime error.
//!    Exporting a value out of a conditional needs a `match` with a default — a later story.
//! 4. A region's own output ports are safe for the kinds that always run their body or fail
//!    ([`Retry`](NodeKind::Retry), [`Throttle`](NodeKind::Throttle),
//!    [`Approval`](NodeKind::Approval)).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ir::{default_service, is_default_service, JsonSchema};

/// An author-stable node identity.
///
/// **Deliberately unlike `flux_lang::ast::NodeId`**, which is a positional index into a flow's body
/// and is invalidated by any edit. A saved graph has to survive re-ordering, so this is a name the
/// author owns and the compiler never renumbers.
pub type NodeId = String;

/// One end of an edge: a node and one of its named ports.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortRef {
    /// The node's [`NodeId`].
    pub node: NodeId,
    /// The port name on that node.
    pub port: String,
}

/// A named, optionally-typed connection point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    /// The port name, unique among its node's ports on the same side.
    pub name: String,
    /// What the port carries, when it is known.
    ///
    /// This is where a graph can be **stricter than Flux**. flux's checker is deliberately lenient —
    /// `String` versus `Number` conflicts, `Ticket` versus `Invoice` never does, because a `Named`
    /// type never conflicts. A graph has JSON Schema on both ends, so it can see more. It refuses only
    /// a *concrete* conflict for now, mirroring flux's own rule; richer checking waits on better
    /// response modelling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<JsonSchema>,
    /// Whether a value must reach this port.
    #[serde(default = "default_required", skip_serializing_if = "is_true")]
    pub required: bool,
}

/// One connection.
///
/// Lowering generates a `$symbol` per edge, satisfying flux's identifier grammar and avoiding its
/// reserved words. **An author never sees or names one** — which is what makes action-proxy's silent
/// `$emit` shadowing unrepresentable here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    /// Where the value comes from.
    pub from: PortRef,
    /// Where it goes.
    pub to: PortRef,
}

/// How a [`Condition`] compares. Closed, and small on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compare {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// The value is present and not null. Takes no right-hand side.
    Exists,
}

impl Compare {
    /// The Flux operator this generates, or `None` for [`Exists`](Self::Exists), which lowers to a
    /// presence check rather than a comparison.
    pub fn operator(self) -> Option<&'static str> {
        match self {
            Self::Eq => Some("=="),
            Self::Ne => Some("!="),
            Self::Lt => Some("<"),
            Self::Lte => Some("<="),
            Self::Gt => Some(">"),
            Self::Gte => Some(">="),
            Self::Exists => None,
        }
    }
}

/// When a [`NodeKind::Gate`] opens.
///
/// **Structured, never a formula.** This is [`EventDecl::when`](crate::EventDecl::when)'s
/// field-equality idea generalised by exactly one closed operator set. The Flux expression is
/// generated from it; nothing an author types is evaluated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    /// The port whose value is compared.
    pub left: PortRef,
    /// How.
    pub op: Compare,
    /// The literal it is compared against. Absent for [`Compare::Exists`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<JsonSchema>,
}

/// The retry schedule, mirroring flux's own vocabulary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backoff {
    /// A fixed delay between attempts.
    #[default]
    None,
    /// The delay multiplied by the attempt number.
    Linear,
    /// The delay doubled each attempt.
    Exponential,
}

/// What a node does.
///
/// Every variant either names a Flux AST node it lowers to, or is a **boundary** declaration that is
/// emitted nowhere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeKind {
    /// Call a declared operation of this connector — the call node. Lowers to `Node::Call`.
    Operation {
        /// The [`Operation::id`](crate::Operation::id).
        operation: String,
    },
    /// Read a value out of another value by dotted path. Lowers to `Node::Jq`.
    ///
    /// The grammar is [`Param::wire`](crate::Param::wire)'s, as everywhere else in this repository —
    /// a path *selects*, it does not compute.
    Select {
        /// The dotted path, as in `event.thread_ts`.
        path: String,
    },
    /// Interpolate bound values into a string. Lowers to `Node::Fmt`.
    Template {
        /// The template, with `{port}` placeholders naming this node's input ports.
        format: String,
    },
    /// Assemble a record. Lowers to `Node::Obj`.
    Object {
        /// Field name → the input port carrying its value.
        fields: BTreeMap<String, String>,
    },
    /// A constant. Lowers to `Node::Lit`.
    Literal {
        /// The value.
        value: JsonSchema,
    },
    /// Run the contained region only when the condition holds. Lowers to `Node::When`.
    ///
    /// **A gate exports nothing** — see the module docs.
    Gate {
        /// When it opens.
        condition: Condition,
    },
    /// Ask a human before running the contained region. Lowers to `Node::Confirm`.
    Approval {
        /// What the person is asked.
        message: String,
        /// How much is at stake.
        risk: crate::Risk,
    },
    /// Retry the contained region on failure. Lowers to `Node::Retry`.
    Retry {
        /// Attempts. Required — flux's analyzer rejects unbounded loops.
        max: u32,
        /// The schedule.
        #[serde(default, skip_serializing_if = "is_no_backoff")]
        backoff: Backoff,
        /// The base delay.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delay_ms: Option<u64>,
    },
    /// Rate-limit the contained region. Lowers to `Node::Throttle`.
    ///
    /// The bucket name is **generated** from the graph and node ids: flux keys the bucket by name and
    /// buckets collide silently if two are spelled alike, so this is not an author's to choose.
    Throttle {
        /// Dispatches allowed per window.
        max: u32,
        /// The window.
        window_ms: u64,
    },
    /// **Boundary.** An event of this service wakes the graph. Emitted nowhere.
    Trigger {
        /// The [`EventDecl::name`](crate::EventDecl::name).
        event: String,
    },
    /// **Boundary.** A schedule wakes the graph. Emitted nowhere — the operator writes a
    /// `channel schedule`, and flux's cron is best-effort, so nothing here may assume a tick ran.
    Schedule {
        /// A cron expression, in flux's own accepted spelling.
        cron: String,
    },
    /// **Boundary.** A channel binding of this connector wakes the graph. Emitted nowhere.
    Endpoint {
        /// The [`ChannelBinding::name`](crate::ChannelBinding::name).
        binding: String,
    },
}

impl NodeKind {
    /// The word this variant serializes as, for error text.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Operation { .. } => "operation",
            Self::Select { .. } => "select",
            Self::Template { .. } => "template",
            Self::Object { .. } => "object",
            Self::Literal { .. } => "literal",
            Self::Gate { .. } => "gate",
            Self::Approval { .. } => "approval",
            Self::Retry { .. } => "retry",
            Self::Throttle { .. } => "throttle",
            Self::Trigger { .. } => "trigger",
            Self::Schedule { .. } => "schedule",
            Self::Endpoint { .. } => "endpoint",
        }
    }

    /// Whether this kind contains other nodes.
    pub fn is_region(&self) -> bool {
        matches!(
            self,
            Self::Gate { .. } | Self::Approval { .. } | Self::Retry { .. } | Self::Throttle { .. }
        )
    }

    /// Whether this kind is a boundary declaration that reaches no emitted Flux.
    pub fn is_boundary(&self) -> bool {
        matches!(
            self,
            Self::Trigger { .. } | Self::Schedule { .. } | Self::Endpoint { .. }
        )
    }

    /// Every free-text field this variant carries, paired with what kind of text it is.
    ///
    /// **This is the tripwire that keeps the north star's promise.** The match is exhaustive and
    /// destructures every field, so adding one fails to compile here until somebody classifies it —
    /// and the only admissible classifications are in [`TextRole`]. There is no `Formula` role, and
    /// adding one would be the moment this module became the second language the vision forbids.
    ///
    /// `tests/graphs.rs` asserts the classification over every constructible variant.
    pub fn free_text(&self) -> Vec<(&'static str, TextRole)> {
        match self {
            Self::Operation { operation } => {
                let _ = operation;
                vec![("operation", TextRole::Reference)]
            }
            Self::Select { path } => {
                let _ = path;
                vec![("path", TextRole::Path)]
            }
            Self::Template { format } => {
                let _ = format;
                vec![("format", TextRole::Template)]
            }
            Self::Object { fields } => {
                let _ = fields;
                vec![("fields", TextRole::Reference)]
            }
            Self::Literal { value } => {
                let _ = value;
                vec![("value", TextRole::Data)]
            }
            Self::Gate { condition } => {
                let _ = condition;
                // A condition is a *structure* — a port reference, a closed operator and a literal.
                // It carries no text an author writes and this crate evaluates.
                vec![]
            }
            Self::Approval { message, risk } => {
                let _ = risk;
                let _ = message;
                vec![("message", TextRole::Prose)]
            }
            Self::Retry {
                max,
                backoff,
                delay_ms,
            } => {
                let _ = (max, backoff, delay_ms);
                vec![]
            }
            Self::Throttle { max, window_ms } => {
                let _ = (max, window_ms);
                vec![]
            }
            Self::Trigger { event } => {
                let _ = event;
                vec![("event", TextRole::Reference)]
            }
            Self::Schedule { cron } => {
                let _ = cron;
                vec![("cron", TextRole::Schedule)]
            }
            Self::Endpoint { binding } => {
                let _ = binding;
                vec![("binding", TextRole::Reference)]
            }
        }
    }
}

/// What a piece of free text in a [`NodeKind`] *is*.
///
/// **There is deliberately no `Formula`.** Every role here is either a name this crate resolves, a
/// selection this crate validates, or text nobody evaluates. If a future node needs a role that is
/// not one of these, that is the signal to stop and re-read the north star rather than add a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    /// Names a declared member; the loader resolves it, and a dangling one is refused.
    Reference,
    /// A dotted selection path, validated by the same grammar `wire` uses.
    Path,
    /// A `{port}` interpolation template. Substitution only — no operators, no calls.
    Template,
    /// Shown to a human and evaluated by nobody.
    Prose,
    /// A literal value carried through untouched.
    Data,
    /// A cron expression, handed to flux verbatim.
    Schedule,
}

/// One node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    /// The author-stable id, unique within the graph.
    pub id: NodeId,
    /// What it does.
    pub kind: NodeKind,
    /// The region node containing this one, if any.
    ///
    /// The containment is recorded on the *child* so there is one source of truth — a region listing
    /// its members and a member naming its region could disagree, and nothing would say which was
    /// right.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<NodeId>,
    /// The ports values arrive on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Port>,
    /// The ports values leave on.
    ///
    /// For a region kind these are the values that **escape** it, and they are the phi node Flux does
    /// not have, made explicit. A [`NodeKind::Gate`] may declare none — see the module docs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<Port>,
}

/// A flow: connector members composed into one Flux `op`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    /// The graph name, which becomes the emitted `op`'s name — so it must be a legal Flux
    /// declaration name, the narrower grammar `connector-flux` already enforces for operation ids.
    pub name: String,
    /// The [`Service`](crate::Service) this graph belongs to.
    #[serde(
        default = "default_service",
        skip_serializing_if = "is_default_service"
    )]
    pub service: String,
    /// What the flow does, in one line.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The emitted op's parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Port>,
    /// What the emitted op returns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<PortRef>,
    /// **Whether the emitted op reaches a model as an LLM tool** — see
    /// [`Operation::expose`](crate::Operation::expose), which this mirrors exactly, default included.
    ///
    /// **Authored, not derived**, which is where it parts company with the `risk` and `idempotency`
    /// the same emitted metadata block carries. Those two *are* derived from the operations the flow
    /// calls, because a flow that deletes must not inherit the `low` of the reads it also makes.
    /// Exposure has no such floor to respect, and deriving it would make the shape this field exists
    /// for inexpressible: a **curated flow over uncurated operations** — one tool a model can see,
    /// composed from members that individually stay callable and unexposed — which is the single most
    /// valuable use of the whole distinction.
    #[serde(
        default = "crate::ir::exposed",
        skip_serializing_if = "crate::ir::is_exposed"
    )]
    pub expose: bool,
    /// The nodes.
    pub nodes: Vec<GraphNode>,
    /// The connections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
}

impl Graph {
    /// A node by id.
    pub fn node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The nodes directly inside `region`, in declaration order.
    pub fn nodes_in<'a>(&'a self, region: &'a str) -> impl Iterator<Item = &'a GraphNode> {
        self.nodes
            .iter()
            .filter(move |node| node.region.as_deref() == Some(region))
    }

    /// The chain of regions containing `id`, innermost first. Empty at the top level.
    ///
    /// Returns `None` when containment cycles — which the loader refuses, so on a loaded graph this
    /// is always `Some`.
    pub fn enclosing(&self, id: &str) -> Option<Vec<&str>> {
        let mut chain: Vec<&str> = Vec::new();
        let mut at = self.node(id)?;
        while let Some(region) = at.region.as_deref() {
            if chain.contains(&region) || region == id {
                return None; // containment cycle
            }
            chain.push(region);
            at = self.node(region)?;
        }
        Some(chain)
    }

    /// A topological order over the data edges, or `None` when the graph has a cycle.
    ///
    /// Kahn's algorithm over node ids. Flux has no `goto`, so a cyclic graph has no lowering at all —
    /// this is the check that says so before anyone tries.
    pub fn topological_order(&self) -> Option<Vec<&str>> {
        let mut incoming: BTreeMap<&str, usize> =
            self.nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
        for edge in &self.edges {
            if let Some(count) = incoming.get_mut(edge.to.node.as_str()) {
                *count += 1;
            }
        }

        // Declaration order among ready nodes, so the result is deterministic.
        let mut ready: Vec<&str> = self
            .nodes
            .iter()
            .filter(|n| incoming.get(n.id.as_str()) == Some(&0))
            .map(|n| n.id.as_str())
            .collect();
        let mut ordered: Vec<&str> = Vec::new();

        while let Some(id) = ready.first().copied() {
            ready.remove(0);
            ordered.push(id);
            for edge in self.edges.iter().filter(|e| e.from.node == id) {
                let Some(count) = incoming.get_mut(edge.to.node.as_str()) else {
                    continue;
                };
                *count -= 1;
                if *count == 0 {
                    ready.push(edge.to.node.as_str());
                    // Keep declaration order among the newly ready.
                    ready.sort_by_key(|candidate| {
                        self.nodes
                            .iter()
                            .position(|n| n.id == *candidate)
                            .unwrap_or(usize::MAX)
                    });
                }
            }
        }

        (ordered.len() == self.nodes.len()).then_some(ordered)
    }
}

fn default_required() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_no_backoff(backoff: &Backoff) -> bool {
    *backoff == Backoff::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: NodeKind) -> GraphNode {
        GraphNode {
            id: id.to_owned(),
            kind,
            region: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    fn edge(from: (&str, &str), to: (&str, &str)) -> Edge {
        Edge {
            from: PortRef {
                node: from.0.to_owned(),
                port: from.1.to_owned(),
            },
            to: PortRef {
                node: to.0.to_owned(),
                port: to.1.to_owned(),
            },
        }
    }

    fn graph(nodes: Vec<GraphNode>, edges: Vec<Edge>) -> Graph {
        Graph {
            name: "g".to_owned(),
            service: crate::DEFAULT_SERVICE.to_owned(),
            description: String::new(),
            inputs: Vec::new(),
            output: None,
            expose: true,
            nodes,
            edges,
        }
    }

    #[test]
    fn a_chain_orders_topologically() {
        let g = graph(
            vec![
                node("a", NodeKind::Literal { value: 1.into() }),
                node(
                    "b",
                    NodeKind::Select {
                        path: "x".to_owned(),
                    },
                ),
                node(
                    "c",
                    NodeKind::Select {
                        path: "y".to_owned(),
                    },
                ),
            ],
            vec![
                edge(("a", "out"), ("b", "in")),
                edge(("b", "out"), ("c", "in")),
            ],
        );
        assert_eq!(g.topological_order(), Some(vec!["a", "b", "c"]));
    }

    /// Data convergence is free in Flux — a statement may read any bound symbol — so a diamond is a
    /// legal graph even though its *control* flow is linear.
    #[test]
    fn a_diamond_converges_because_data_edges_need_no_nesting() {
        let g = graph(
            vec![
                node("src", NodeKind::Literal { value: 1.into() }),
                node(
                    "l",
                    NodeKind::Select {
                        path: "a".to_owned(),
                    },
                ),
                node(
                    "r",
                    NodeKind::Select {
                        path: "b".to_owned(),
                    },
                ),
                node(
                    "join",
                    NodeKind::Object {
                        fields: BTreeMap::new(),
                    },
                ),
            ],
            vec![
                edge(("src", "out"), ("l", "in")),
                edge(("src", "out"), ("r", "in")),
                edge(("l", "out"), ("join", "left")),
                edge(("r", "out"), ("join", "right")),
            ],
        );
        let order = g.topological_order().expect("a diamond is acyclic");
        assert_eq!(order.first(), Some(&"src"));
        assert_eq!(order.last(), Some(&"join"));
    }

    #[test]
    fn a_cycle_has_no_order_at_all() {
        let g = graph(
            vec![
                node(
                    "a",
                    NodeKind::Select {
                        path: "x".to_owned(),
                    },
                ),
                node(
                    "b",
                    NodeKind::Select {
                        path: "y".to_owned(),
                    },
                ),
            ],
            vec![
                edge(("a", "out"), ("b", "in")),
                edge(("b", "out"), ("a", "in")),
            ],
        );
        assert_eq!(g.topological_order(), None);
    }

    #[test]
    fn enclosing_walks_outwards_and_detects_a_containment_cycle() {
        let mut inner = node("inner", NodeKind::Literal { value: 1.into() });
        inner.region = Some("outer".to_owned());
        let outer = node(
            "outer",
            NodeKind::Retry {
                max: 3,
                backoff: Backoff::Exponential,
                delay_ms: None,
            },
        );
        let g = graph(vec![outer, inner], Vec::new());
        assert_eq!(g.enclosing("inner"), Some(vec!["outer"]));
        assert_eq!(g.enclosing("outer"), Some(Vec::new()));

        let mut a = node("a", NodeKind::Literal { value: 1.into() });
        let mut b = node("b", NodeKind::Literal { value: 2.into() });
        a.region = Some("b".to_owned());
        b.region = Some("a".to_owned());
        assert_eq!(graph(vec![a, b], Vec::new()).enclosing("a"), None);
    }

    #[test]
    fn region_and_boundary_kinds_are_classified() {
        assert!(NodeKind::Retry {
            max: 1,
            backoff: Backoff::None,
            delay_ms: None
        }
        .is_region());
        assert!(NodeKind::Gate {
            condition: Condition {
                left: PortRef {
                    node: "a".into(),
                    port: "out".into()
                },
                op: Compare::Exists,
                right: None,
            }
        }
        .is_region());
        assert!(!NodeKind::Operation {
            operation: "x".into()
        }
        .is_region());

        assert!(NodeKind::Trigger { event: "e".into() }.is_boundary());
        assert!(NodeKind::Schedule {
            cron: "0 9 * * *".into()
        }
        .is_boundary());
        assert!(!NodeKind::Select { path: "a".into() }.is_boundary());
    }

    /// `Exists` is the one comparison with no right-hand side, so it has no operator either.
    #[test]
    fn comparisons_map_to_flux_operators() {
        assert_eq!(Compare::Eq.operator(), Some("=="));
        assert_eq!(Compare::Gte.operator(), Some(">="));
        assert_eq!(Compare::Exists.operator(), None);
    }
}

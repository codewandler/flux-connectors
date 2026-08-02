//! OpenAPI 3.x ingest: a vendored vendor document in, [`Ingested`] out — C-4.
//!
//! This is the second front-end the pipeline was designed around
//! (`docs/designs/connector-pipeline.md`, "Two front-ends, one IR"). A provider file that points at
//! a document under `specs/` shrinks to a pointer plus patches, and this module is what turns the
//! pointer into something a patch can select from.
//!
//! # Ingest makes everything available; it selects nothing
//!
//! [`ingest`] returns **every** operation the document declares, keyed by `operationId`. It does not
//! decide which of them a connector publishes, does not mint op ids, and does not invent a
//! [`Risk`](crate::Risk) or an [`Idempotency`](crate::Idempotency) that no specification carries.
//! Selection is opt-in and is the patch set's job (`crate::provider`, and C-411 to widen it): a
//! 398-operation document with no patch yields a connector with **no operations at all**, which is
//! the property that stops a vendor catalogue from becoming 398 LLM tools by default.
//!
//! # No network, no filesystem
//!
//! [`ingest`] takes the document's bytes as text. Fetching is `connector-cli`'s (C-14's), and this
//! crate reaches no network at all — see the crate docs and `AGENTS.md`'s ownership table.
//!
//! # A real vendor document is never fully well-formed
//!
//! Which is why the diagnostic path is the important half of this module rather than an afterthought.
//! Two failure grades, and the difference between them is deliberate:
//!
//! - **The document is not an OpenAPI 3.x document at all** — unparseable bytes, no `openapi` key, a
//!   version this ingest does not read. That is an [`Err`], because nothing useful can follow.
//! - **One endpoint is wrong** — a parameter with no schema, a `$ref` that points nowhere, a
//!   `$ref` cycle in a request, a body in a media type the IR cannot express. That is a
//!   [`Diagnostic`] naming the
//!   offending path and method, and the operation is **skipped**. One bad endpoint must not cost the
//!   other 397, and an operation that was skipped cannot be selected — so the failure surfaces as a
//!   patch naming an operation that is not there, which the loader already refuses loudly.
//!
//! A skipped operation is never a silently degraded one. There is no "ingest it without its body"
//! path, because a `PUT` that quietly stopped sending a body is exactly the kind of plausible-wrong
//! output this repository refuses.
//!
//! # JSON and YAML
//!
//! Vendors publish both, and the spec cache is extension-agnostic already — `discover_specs` takes
//! the version from the file stem, not from the suffix. So [`ingest`] sniffs: JSON first, then YAML
//! through `serde_norway`. YAML mapping keys are coerced to strings on the way in, because
//! `responses: {200: …}` is an *integer* key in YAML and a JSON object has no such thing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Map, Value};

use crate::{BodyEncoding, HttpMethod, JsonSchema, Param, ParamSet};

/// The media types a request body may arrive in, in the order [`ingest`] prefers them.
///
/// Preference, not tolerance: a vendor that publishes both a JSON and a form encoding for one
/// endpoint is describing one request two ways, and picking the one the IR expresses best is a
/// decision worth making once here rather than per document. Anything not on this list is a
/// [`Diagnostic`] — `multipart/form-data` most of all, which [`BodyEncoding`] cannot spell and which
/// `docs/designs/spec-front-end.md` records as a known blocker rather than a surprise.
const READABLE_BODIES: [(&str, BodyEncoding); 2] = [
    ("application/json", BodyEncoding::Json),
    ("application/x-www-form-urlencoded", BodyEncoding::Form),
];

/// The response media type an operation's shape is read from.
///
/// Only JSON: a `response_schema` is what reaches `web/public/catalog.json` and a model's tool
/// contract, and neither has anything to do with a media type nothing here parses.
const RESPONSE_BODY: &str = "application/json";

/// The `openapi` versions this ingest reads.
///
/// Matched on the major/minor prefix, so `3.0.3` and `3.1.1` both qualify. The two differ in their
/// schema dialect and not in the structure walked here, and every schema travels **verbatim**
/// ([`JsonSchema`]) — so 3.1's JSON Schema 2020-12 keywords survive precisely because nothing here
/// interprets them. A `2.0` (Swagger) or a `4.x` document is refused outright rather than
/// half-read.
const READABLE_VERSIONS: [&str; 2] = ["3.0", "3.1"];

/// The HTTP methods a path item may declare, in the order they are walked.
///
/// `trace` is deliberately absent: [`HttpMethod`] does not carry it, and an operation this pipeline
/// cannot spell must not be ingested into a form that implies it could be emitted.
const METHODS: [(&str, HttpMethod); 6] = [
    ("get", HttpMethod::Get),
    ("post", HttpMethod::Post),
    ("put", HttpMethod::Put),
    ("patch", HttpMethod::Patch),
    ("delete", HttpMethod::Delete),
    ("head", HttpMethod::Head),
];

/// The path-item keys that name an operation this pipeline cannot emit.
///
/// [`HttpMethod`] carries neither, so an operation declared under one has no representation in the
/// IR at all. It earns a **diagnostic** rather than being passed over in silence, for the reason
/// every other unrepresentable construct here does: an author who selected it would otherwise get
/// "names no `operationId`" about an operation they can see in the document, with nothing saying the
/// method is the problem. Zero occurrences across the babelforce corpus, which is why this is a
/// diagnostic and not a reason to widen `HttpMethod`.
const UNREPRESENTABLE_METHODS: [&str; 2] = ["options", "trace"];

/// The ceiling on nodes produced while expanding one schema's `$ref`s.
///
/// Cycle detection alone bounds *depth*, not *size*: a document whose schemas reference each other
/// in a diamond expands multiplicatively without ever repeating a pointer on one path, and 848
/// component schemas is enough for that to matter. This is the second bound, and it fails the
/// operation with a diagnostic rather than consuming the machine — the same trade the cycle refusal
/// makes, for the same reason.
///
/// **The diagnostic does not diagnose.** Hitting this says the expansion was too large and says
/// nothing about why, because the two causes are indistinguishable from here: a genuinely enormous
/// legitimate schema and a pathological reference graph both arrive as the same count. Measured
/// headroom on the babelforce corpus is 14x — the largest real expansion is 3,580 nodes — so a
/// document that reaches this is far outside anything observed, which is the reason to report it
/// neutrally rather than assert a cause.
const EXPANSION_BUDGET: usize = 50_000;

/// What to do when a schema reaches a `$ref` already on its current expansion path.
///
/// Requests are executable contracts and must resolve exactly, so a cycle still refuses the
/// operation. Responses are descriptive and may be recursive by nature (trees, quoted messages,
/// threaded replies). They retain one concrete occurrence of every schema on the path and replace
/// only the back-edge with JSON Schema's `true` schema. That is an explicit over-approximation of
/// the recursive continuation: it invents no finite depth while keeping the useful bounded prefix.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CyclePolicy {
    Reject,
    BoundResponse,
}

/// One thing wrong with a document that did not stop the ingest.
///
/// The `where` is a deliberate part of the value rather than something prepended to the message:
/// "a real vendor spec is never fully well-formed" is only actionable if the report names the
/// endpoint, and a caller collecting a hundred of these wants to sort them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Where in the document the problem is — `GET /api/v2/agents`, or `paths` for a whole-section
    /// problem.
    pub location: String,
    /// What is wrong, in one sentence, and what it cost.
    pub problem: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.location, self.problem)
    }
}

/// One `servers[]` entry, with its templating **preserved rather than resolved**.
///
/// Zendesk's `https://{subdomain}.zendesk.com` is the motivating case: the variable is a per-tenant
/// value a host substitutes at call time, so an ingest that helpfully filled in the `default`
/// (`demo`) would bake one vendor's example account into every connector. The URL is carried with
/// its braces intact and the variables are carried beside it, which is the same shape
/// `Connector::base_url` already uses and what `[[config]]`'s `binds = "endpoint.<var>"` targets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Server {
    /// The URL as the document writes it, `{variables}` and all.
    pub url: String,
    /// What the server is, when the document says.
    pub description: String,
    /// The declared variables, by name.
    pub variables: BTreeMap<String, ServerVariable>,
}

/// One `{variable}` of a [`Server`] URL.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerVariable {
    /// The vendor's default. Recorded, never substituted — see [`Server`].
    pub default: String,
    /// What the variable is, when the document says.
    pub description: String,
    /// The closed set of values, when the document publishes one (`enum`).
    pub allowed: Vec<String>,
}

/// One operation the document declares, in the IR's own vocabulary.
///
/// Everything an [`Operation`](crate::Operation) needs **except the three things no specification
/// carries**: the op id (a public contract, so it is stated by a patch and never derived from the
/// volatile `operationId` — see `docs/designs/connector-pipeline.md`), the
/// [`Risk`](crate::Risk) and the [`Idempotency`](crate::Idempotency). Those have no `Default` in
/// this IR precisely so that they cannot be decided by silence, and ingest inventing them would be
/// the silence.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecOperation {
    /// The document's `operationId` — what a `[[patch.operations]] select` names.
    pub operation_id: String,
    /// The HTTP method.
    pub method: HttpMethod,
    /// The path template, exactly as the document's `paths` key spells it.
    ///
    /// Verbatim, including any API-version prefix. A document's `servers` URL and its paths are two
    /// halves of one address and the split between them is the vendor's; re-cutting it here would
    /// make an operation's path disagree with the document a reviewer is diffing against.
    pub path: String,
    /// The vendor's `summary`, falling back to its `description`. Reaches a model as the tool
    /// contract, which is why the shorter, sentence-shaped field is preferred.
    pub description: String,
    /// The request parameters, grouped by position, each carrying its **resolved** JSON Schema.
    pub params: ParamSet,
    /// The 2xx response body's schema, when the document publishes one.
    pub response_schema: Option<JsonSchema>,
    /// Whether the vendor marks the operation `deprecated`.
    pub deprecated: bool,
}

/// Everything one vendor document says, normalized.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ingested {
    /// The `openapi` version string the document declares.
    pub version: String,
    /// `info.title`.
    pub title: String,
    /// `info.version` — the vendor's own version for the API, which
    /// [`SpecSource::upstream_version`](crate::SpecSource) records independently.
    pub upstream_version: String,
    /// The declared servers, templating preserved.
    pub servers: Vec<Server>,
    /// Every operation the document declares that this ingest could read, ordered by path and then
    /// by method. Deterministic and independent of the document's own key order, which is what lets
    /// two runs over one document produce byte-identical IR.
    pub operations: Vec<SpecOperation>,
    /// Everything wrong with the document that did not stop the ingest.
    pub diagnostics: Vec<Diagnostic>,
}

impl Ingested {
    /// The operation a patch's `select` names, if the document declares one.
    pub fn operation(&self, operation_id: &str) -> Option<&SpecOperation> {
        self.operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
    }

    /// Every `operationId` available to select, in ingest order — what a refusal lists when a patch
    /// names one that is not there.
    pub fn operation_ids(&self) -> Vec<&str> {
        self.operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect()
    }

    /// The first declared server's URL — the base URL the document implies.
    ///
    /// **Advisory, and deliberately not authoritative.** `providers/<name>.toml` states its own
    /// `base_url` even when a spec is present, and the loader refuses an empty one, because
    /// `servers[0]` is where a vendor puts whichever environment it happened to list first — the
    /// babelforce manager document leads with staging. So this is what a *reviewer* compares
    /// against, not what a build silently adopts.
    pub fn base_url(&self) -> Option<&str> {
        self.servers.first().map(|server| server.url.as_str())
    }
}

/// Turn one vendored OpenAPI 3.x document into [`Ingested`].
///
/// Pure: bytes in, IR out, no IO of any kind. JSON or YAML — see the module docs.
///
/// # Errors
///
/// [`Error::ParseSpec`](crate::Error::ParseSpec) when the bytes are neither JSON nor YAML, when the
/// document is not a mapping, when it declares no `openapi` version, or when that version is not one
/// this ingest reads. Everything narrower than "this is not a document I can read" is a
/// [`Diagnostic`] instead.
pub fn ingest(document: &str) -> crate::Result<Ingested> {
    let root = parse(document)?;
    let root = root
        .as_object()
        .ok_or_else(|| invalid("the document's top level is not a mapping"))?;

    let version = string(root, "openapi").ok_or_else(|| {
        invalid(
            "the document declares no `openapi` version, so it is not an OpenAPI document. A \
             Swagger 2.0 file declares `swagger` instead and must be converted before it is \
             vendored",
        )
    })?;
    if !READABLE_VERSIONS
        .iter()
        .any(|readable| version.starts_with(readable))
    {
        return Err(invalid(format!(
            "the document declares `openapi = \"{version}\"`; this ingest reads {}",
            READABLE_VERSIONS.join(" and ")
        )));
    }

    let info = root.get("info").and_then(Value::as_object);
    let mut ingested = Ingested {
        version: version.to_owned(),
        title: info
            .and_then(|info| string(info, "title"))
            .unwrap_or_default()
            .to_owned(),
        upstream_version: info
            .and_then(|info| string(info, "version"))
            .unwrap_or_default()
            .to_owned(),
        ..Ingested::default()
    };

    read_servers(root, &mut ingested);
    read_paths(root, &mut ingested);
    Ok(ingested)
}

/// JSON first, then YAML.
///
/// The order is not arbitrary: YAML 1.2 is nominally a JSON superset, but the two parsers disagree
/// about duplicate keys and about large integers, and a document published as JSON deserves its own
/// parser's answer. The YAML error is the one reported when both fail, because a file that is
/// neither is much more often a malformed YAML than a malformed JSON.
fn parse(document: &str) -> crate::Result<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(document) {
        return Ok(value);
    }
    let yaml: serde_norway::Value = serde_norway::from_str(document)
        .map_err(|error| invalid(format!("the document is neither JSON nor YAML: {error}")))?;
    from_yaml(yaml).map_err(invalid)
}

/// YAML's data model into JSON's.
///
/// The one lossy step is the mapping key, and it has to be: YAML keys are values, JSON keys are
/// strings, and `responses: {200: {…}}` — an *integer* key — is how every OpenAPI YAML document in
/// existence spells a status code. Coercing scalars is therefore not a convenience, it is what makes
/// YAML ingest work at all. A non-scalar key is refused rather than stringified, because there is no
/// spelling of it a JSON Pointer could ever name.
fn from_yaml(value: serde_norway::Value) -> Result<Value, String> {
    Ok(match value {
        serde_norway::Value::Null => Value::Null,
        serde_norway::Value::Bool(bool) => Value::Bool(bool),
        // A YAML number JSON cannot hold — an infinity, a NaN — becomes its own text rather than
        // vanishing. Nothing in an OpenAPI document should reach that arm, and a schema silently
        // losing a bound would be worse than one carrying an odd string.
        serde_norway::Value::Number(number) => {
            number_to_json(&number).unwrap_or_else(|| Value::String(number.to_string()))
        }
        serde_norway::Value::String(string) => Value::String(string),
        serde_norway::Value::Sequence(sequence) => Value::Array(
            sequence
                .into_iter()
                .map(from_yaml)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        serde_norway::Value::Mapping(mapping) => {
            let mut object = Map::new();
            for (key, value) in mapping {
                object.insert(yaml_key(key)?, from_yaml(value)?);
            }
            Value::Object(object)
        }
        // A `!Tag` has no OpenAPI meaning; the tagged value is what the document is about.
        serde_norway::Value::Tagged(tagged) => from_yaml(tagged.value)?,
    })
}

/// A YAML number as JSON's, preserving integer-ness so `200` does not become `200.0`.
fn number_to_json(number: &serde_norway::Number) -> Option<Value> {
    if let Some(unsigned) = number.as_u64() {
        return Some(Value::Number(unsigned.into()));
    }
    if let Some(signed) = number.as_i64() {
        return Some(Value::Number(signed.into()));
    }
    number
        .as_f64()
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
}

/// One YAML mapping key as a JSON object key — see [`from_yaml`].
fn yaml_key(key: serde_norway::Value) -> Result<String, String> {
    Ok(match key {
        serde_norway::Value::String(string) => string,
        serde_norway::Value::Number(number) => number.to_string(),
        serde_norway::Value::Bool(bool) => bool.to_string(),
        serde_norway::Value::Null => "null".to_owned(),
        other => {
            return Err(format!(
                "a YAML mapping key is {}, which no JSON object can carry",
                match other {
                    serde_norway::Value::Sequence(_) => "a sequence",
                    serde_norway::Value::Mapping(_) => "a mapping",
                    _ => "not a scalar",
                }
            ))
        }
    })
}

/// The `servers` block, or a diagnostic saying there is none.
///
/// An absent `servers` is legal OpenAPI — the specification says it defaults to `/` — and it is
/// worth a diagnostic anyway: a relative default tells a reviewer comparing the document against
/// `base_url` precisely nothing, so silence here would read as agreement.
fn read_servers(root: &Map<String, Value>, ingested: &mut Ingested) {
    let Some(servers) = root.get("servers") else {
        ingested.diagnostics.push(Diagnostic {
            location: "servers".to_owned(),
            problem: "the document declares no `servers`, so it implies no base URL; \
                      `providers/<name>.toml` states its own and nothing here can corroborate it"
                .to_owned(),
        });
        return;
    };
    let Some(servers) = servers.as_array() else {
        ingested.diagnostics.push(Diagnostic {
            location: "servers".to_owned(),
            problem: "`servers` is not a list, so no base URL could be read from it".to_owned(),
        });
        return;
    };

    for (index, server) in servers.iter().enumerate() {
        let Some(server) = server.as_object() else {
            ingested.diagnostics.push(Diagnostic {
                location: format!("servers[{index}]"),
                problem: "the entry is not a mapping and was skipped".to_owned(),
            });
            continue;
        };
        let Some(url) = string(server, "url") else {
            ingested.diagnostics.push(Diagnostic {
                location: format!("servers[{index}]"),
                problem: "the entry declares no `url` and was skipped".to_owned(),
            });
            continue;
        };
        ingested.servers.push(Server {
            url: url.to_owned(),
            description: string(server, "description").unwrap_or_default().to_owned(),
            variables: read_server_variables(server),
        });
    }
}

/// A server's `variables` block. An unreadable entry is dropped rather than diagnosed: the URL it
/// belongs to is already carried with its braces intact, so the substitution target survives.
fn read_server_variables(server: &Map<String, Value>) -> BTreeMap<String, ServerVariable> {
    let mut variables = BTreeMap::new();
    let Some(declared) = server.get("variables").and_then(Value::as_object) else {
        return variables;
    };
    for (name, variable) in declared {
        let Some(variable) = variable.as_object() else {
            continue;
        };
        variables.insert(
            name.clone(),
            ServerVariable {
                default: string(variable, "default").unwrap_or_default().to_owned(),
                description: string(variable, "description")
                    .unwrap_or_default()
                    .to_owned(),
                allowed: variable
                    .get("enum")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            },
        );
    }
    variables
}

/// Every operation under `paths`, walked in sorted path order and then in [`METHODS`] order.
fn read_paths(root: &Map<String, Value>, ingested: &mut Ingested) {
    let Some(paths) = root.get("paths") else {
        ingested.diagnostics.push(Diagnostic {
            location: "paths".to_owned(),
            problem: "the document declares no `paths`, so it publishes no operations".to_owned(),
        });
        return;
    };
    let Some(paths) = paths.as_object() else {
        ingested.diagnostics.push(Diagnostic {
            location: "paths".to_owned(),
            problem: "`paths` is not a mapping, so no operation could be read from it".to_owned(),
        });
        return;
    };

    // `serde_json::Map` is a `BTreeMap`, so this is sorted by path and stable across runs — the
    // determinism `connectors.lock` rests on, obtained without sorting anything here.
    let resolver = Resolver { root };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (path, item) in paths {
        let item = match resolver.resolve(item) {
            Ok(item) => item,
            Err(problem) => {
                ingested.diagnostics.push(Diagnostic {
                    location: path.clone(),
                    problem: format!("the path item could not be resolved: {problem}"),
                });
                continue;
            }
        };
        let Some(item) = item.as_object() else {
            ingested.diagnostics.push(Diagnostic {
                location: path.clone(),
                problem: "the path item is not a mapping and was skipped".to_owned(),
            });
            continue;
        };

        for word in UNREPRESENTABLE_METHODS {
            if item.contains_key(word) {
                ingested.diagnostics.push(Diagnostic {
                    location: format!("{} {path}", word.to_uppercase()),
                    problem: format!(
                        "`{word}` is not a method this pipeline can emit, so the operation was \
                         skipped and cannot be selected"
                    ),
                });
            }
        }

        let shared = item.get("parameters").cloned().unwrap_or(Value::Null);
        for (word, method) in METHODS {
            let Some(operation) = item.get(word).and_then(Value::as_object) else {
                continue;
            };
            read_operation(
                &resolver,
                Located {
                    path,
                    word,
                    method,
                    shared: &shared,
                },
                operation,
                &mut seen,
                ingested,
            );
        }
    }
}

/// Where in the document one operation sits, and what its path item lends it.
///
/// A struct rather than four arguments because every helper below needs all four to build its own
/// diagnostic, and a diagnostic that cannot name the endpoint is the one thing this module must not
/// produce.
struct Located<'a> {
    path: &'a str,
    word: &'a str,
    method: HttpMethod,
    /// The path item's own `parameters`, which every operation under it inherits.
    shared: &'a Value,
}

impl Located<'_> {
    /// `GET /api/v2/agents` — how every diagnostic about this operation names it.
    fn location(&self) -> String {
        format!("{} {}", self.word.to_uppercase(), self.path)
    }
}

/// One operation, or a diagnostic saying why it was skipped.
fn read_operation(
    resolver: &Resolver<'_>,
    at: Located<'_>,
    operation: &Map<String, Value>,
    seen: &mut BTreeSet<String>,
    ingested: &mut Ingested,
) {
    let location = at.location();
    let Some(operation_id) = string(operation, "operationId").filter(|id| !id.trim().is_empty())
    else {
        ingested.diagnostics.push(Diagnostic {
            location,
            problem: "the operation declares no `operationId`, so no `[[patch.operations]]` could \
                      ever select it; it was skipped"
                .to_owned(),
        });
        return;
    };
    if !seen.insert(operation_id.to_owned()) {
        ingested.diagnostics.push(Diagnostic {
            location,
            problem: format!(
                "`operationId = {operation_id:?}` is declared more than once in this document; \
                 only the first was ingested, because a `select` naming it would otherwise mean \
                 two different requests"
            ),
        });
        return;
    }

    let mut params = ParamSet::default();
    if let Err(problem) = read_parameters(resolver, &at, operation, &mut params) {
        ingested.diagnostics.push(Diagnostic { location, problem });
        return;
    }
    if let Err(problem) = read_request_body(resolver, operation, &mut params) {
        ingested.diagnostics.push(Diagnostic { location, problem });
        return;
    }
    let response_schema = match read_response(resolver, operation) {
        Ok(schema) => schema,
        Err(problem) => {
            ingested.diagnostics.push(Diagnostic { location, problem });
            return;
        }
    };
    // **A vendor's placeholder does not become this repository's coverage** — C-417.
    //
    // `response_schema_coverage.rs` has refused an authored `{}` or a bare `{"type": "object"}`
    // since C-126, on the grounds that a schema admitting every document tells a consumer no more
    // than absence does while counting as a declaration. Everything it guarded was hand-written
    // until babelforce started deriving 392 schemas from five documents — and 24 of those
    // documents' 2xx bodies are exactly that placeholder, almost all of them deletes answering
    // `{"type": "object"}` with no properties.
    //
    // Carrying one through would launder a vendor's non-statement into a coverage number, which is
    // the precise defect that gate exists to catch; it would just have been committed by ingest
    // rather than by an author. So the schema is dropped and the operation publishes an **honest
    // absence**, which is a reviewable answer, and the drop is diagnosed rather than silent.
    //
    // Zero shipped operations were affected when this landed — the gate was green — so this changes
    // no existing provider's output. What it changes is the direction a future one can drift.
    let response_schema = match response_schema {
        Some(schema) if crate::constrains_nothing(&schema) => {
            ingested.diagnostics.push(Diagnostic {
                location,
                problem: format!(
                    "the lowest 2xx response declares `{schema}`, which admits every document and \
                     so states nothing the absence of a schema does not. It was dropped rather \
                     than published, because a schema that constrains nothing counts as coverage \
                     while carrying none"
                ),
            });
            None
        }
        schema => schema,
    };

    ingested.operations.push(SpecOperation {
        operation_id: operation_id.to_owned(),
        method: at.method,
        path: at.path.to_owned(),
        description: string(operation, "summary")
            .filter(|summary| !summary.trim().is_empty())
            .or_else(|| string(operation, "description"))
            .unwrap_or_default()
            .trim()
            .to_owned(),
        params,
        response_schema,
        deprecated: operation
            .get("deprecated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    });
}

/// The path item's parameters and the operation's, merged, with the operation's winning.
///
/// The precedence is OpenAPI's own: a parameter is identified by the pair `(name, in)`, and an
/// operation redeclaring one it inherits replaces it rather than sending it twice.
fn read_parameters(
    resolver: &Resolver<'_>,
    at: &Located<'_>,
    operation: &Map<String, Value>,
    params: &mut ParamSet,
) -> Result<(), String> {
    let mut declared: Vec<&Value> = Vec::new();
    for source in [
        at.shared,
        operation.get("parameters").unwrap_or(&Value::Null),
    ] {
        match source {
            Value::Null => {}
            Value::Array(entries) => declared.extend(entries),
            _ => return Err("`parameters` is not a list".to_owned()),
        }
    }

    // Keyed by `(in, name)` so the operation's own declaration replaces the path item's; insertion
    // order is preserved, which keeps the emitted parameter order the document's.
    let mut merged: Vec<(String, String, Param)> = Vec::new();
    for entry in declared {
        let resolved = resolver.resolve(entry)?;
        let entry = resolved
            .as_object()
            .ok_or_else(|| "a `parameters` entry is not a mapping".to_owned())?;

        let name = string(entry, "name")
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| "a parameter declares no `name`".to_owned())?
            .to_owned();
        let position = string(entry, "in")
            .ok_or_else(|| format!("parameter `{name}` declares no `in`"))?
            .to_owned();
        if position == "cookie" {
            // **Skips the operation, exactly as an unrepresentable body does.** `ParamSet` has no
            // cookie position, so carrying the operation without the parameter would publish a
            // request that quietly stopped sending something the vendor declared — the same failure
            // shape `read_request_body` refuses for `multipart/form-data`, and the reason it is
            // refused there applies here unchanged. Zero occurrences across the babelforce corpus.
            return Err(format!(
                "parameter `{name}` travels in a cookie, which this IR has no request position \
                 for. The operation was skipped rather than published without it"
            ));
        }
        if !matches!(position.as_str(), "path" | "query" | "header") {
            return Err(format!(
                "parameter `{name}` declares `in = \"{position}\"`, which is not a request position"
            ));
        }

        let Some(schema) = parameter_schema(resolver, entry) else {
            return Err(format!(
                "parameter `{name}` carries no `schema`, so its type is unknown. An untyped \
                 parameter has already collapsed to a string, which is the failure this pipeline \
                 exists to prevent, so the whole operation was skipped"
            ));
        };
        let schema = schema?;

        let param = Param {
            name: name.clone(),
            wire: None,
            description: string(entry, "description")
                .unwrap_or_default()
                .trim()
                .to_owned(),
            // A path parameter is required by specification; a vendor that says otherwise is
            // describing `PUT /tickets/` and is corrected by a `[[patch.operations.params]]`.
            required: entry
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(position == "path"),
            schema,
        };

        match merged
            .iter_mut()
            .find(|(had_position, had_name, _)| *had_position == position && *had_name == name)
        {
            Some(slot) => slot.2 = param,
            None => merged.push((position, name, param)),
        }
    }

    for (position, _, param) in merged {
        match position.as_str() {
            "path" => params.path.push(param),
            "query" => params.query.push(param),
            _ => params.header.push(param),
        }
    }
    Ok(())
}

/// A parameter's schema, from `schema` or from a `content` media type.
///
/// The `content` form is OpenAPI's escape hatch for a parameter whose value is a serialized
/// document (a JSON-encoded filter in a query string). It is one parameter either way, so it is read
/// into one [`Param::schema`] rather than modelled separately.
fn parameter_schema(
    resolver: &Resolver<'_>,
    entry: &Map<String, Value>,
) -> Option<Result<JsonSchema, String>> {
    if let Some(schema) = entry.get("schema") {
        return Some(resolver.expand(schema));
    }
    let content = entry.get("content")?.as_object()?;
    let (_, media) = content.iter().next()?;
    let schema = media.as_object()?.get("schema")?;
    Some(resolver.expand(schema))
}

/// The request body, as named fields when the vendor describes an object and as a whole schema
/// otherwise.
///
/// **Named fields where they exist**, because that is what reaches a model as separate arguments
/// and what `Operation::input_schema` composes. Only the *top level* is expanded: a property whose
/// own schema is an object stays one parameter carrying that object, because flattening it would
/// need a `wire` path per leaf and would present a vendor's nesting as if it were the caller's.
fn read_request_body(
    resolver: &Resolver<'_>,
    operation: &Map<String, Value>,
    params: &mut ParamSet,
) -> Result<(), String> {
    let Some(body) = operation.get("requestBody") else {
        return Ok(());
    };
    let body = resolver.resolve(body)?;
    let Some(body) = body.as_object() else {
        return Err("`requestBody` is not a mapping".to_owned());
    };
    let Some(content) = body.get("content").and_then(Value::as_object) else {
        return Err(
            "`requestBody` declares no `content`, so nothing says what it sends".to_owned(),
        );
    };

    let Some((media_type, encoding)) = READABLE_BODIES
        .into_iter()
        .find(|(media_type, _)| content.contains_key(*media_type))
    else {
        return Err(format!(
            "the request body is declared as {}, and none of those is a body this IR can express \
             (`body_encoding` is `json` or `form`); the operation was skipped rather than emitted \
             without its body",
            content
                .keys()
                .map(|media_type| format!("`{media_type}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };

    let Some(schema) = content
        .get(media_type)
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
    else {
        return Err(format!(
            "the `{media_type}` request body declares no `schema`, so its shape is unknown"
        ));
    };
    let schema = resolver.expand(schema)?;
    params.body_encoding = encoding;

    let object = schema.as_object().filter(|schema| {
        schema.get("type").and_then(Value::as_str) == Some("object")
            && schema
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| !properties.is_empty())
    });
    let Some(object) = object else {
        // A free-form body, a non-object body, or an object with no properties. `body_schema` is
        // exactly the IR field for "the body *is* this schema", and losing it would emit a write
        // with no body at all — indistinguishable from a legitimately bodiless one.
        params.body_schema = Some(schema);
        return Ok(());
    };

    let required: BTreeSet<&str> = object
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .expect("checked above");
    for (name, property) in properties {
        params.body.push(Param {
            name: name.clone(),
            wire: None,
            description: property
                .as_object()
                .and_then(|property| string(property, "description"))
                .unwrap_or_default()
                .trim()
                .to_owned(),
            required: required.contains(name.as_str()),
            schema: property.clone(),
        });
    }
    Ok(())
}

/// The schema of the lowest 2xx JSON response, when the document publishes one.
///
/// The *lowest* rather than "the first the document lists": an endpoint declaring both `200` and
/// `201` describes one success shape twice far more often than two, and a rule that depended on key
/// order would make the answer depend on how the vendor typed the file.
fn read_response(
    resolver: &Resolver<'_>,
    operation: &Map<String, Value>,
) -> Result<Option<JsonSchema>, String> {
    let Some(responses) = operation.get("responses").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(status) = responses
        .keys()
        .filter(|status| status.starts_with('2') && status.len() == 3)
        .min()
    else {
        return Ok(None);
    };

    let response = resolver.resolve(&responses[status])?;
    let Some(schema) = response
        .as_object()
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get(RESPONSE_BODY))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
    else {
        // Honest absence, not a problem: 35 of babelforce's 398 operations publish no 2xx schema,
        // and `response_schema_coverage.rs` measures exactly that rather than pretending otherwise.
        return Ok(None);
    };
    resolver.expand_response(schema).map(Some)
}

// ---------------------------------------------------------------------------------------------
// `$ref` resolution
// ---------------------------------------------------------------------------------------------

/// Resolves `$ref`s against the document that declares them.
///
/// **Within the document only.** An external `$ref` (`common.yaml#/components/schemas/Error`) is
/// refused rather than followed, because following one would mean reading a second file — and this
/// crate touches no filesystem at all. C-410 is where a connector gains several documents, and it
/// gains them as *services*, not as a resolution scope.
struct Resolver<'a> {
    root: &'a Map<String, Value>,
}

impl Resolver<'_> {
    /// One level of `$ref`, for a structural node — a path item, a parameter, a request body, a
    /// response. Their nested schemas are expanded separately by [`expand`](Self::expand), because
    /// only a schema position is allowed to contain arbitrarily deep references.
    fn resolve(&self, value: &Value) -> Result<Value, String> {
        let mut seen = Vec::new();
        self.follow(value, &mut seen)
    }

    /// A schema with every `$ref` inside it replaced by what it points at.
    ///
    /// This is what "each carrying its **resolved** JSON Schema" means: a consumer reading a
    /// `Param::schema` never has to know the document it came from, which is the whole reason the
    /// IR carries schemas verbatim rather than by reference.
    fn expand(&self, value: &Value) -> Result<JsonSchema, String> {
        let mut seen = Vec::new();
        let mut budget = 0;
        self.walk(value, &mut seen, &mut budget, CyclePolicy::Reject)
    }

    /// A response schema with recursive continuations represented by an explicit, bounded prefix.
    ///
    /// Unlike a request, a response is not material the connector constructs. Recursive response
    /// models therefore remain useful when their repeated tail is admitted by `true`, the JSON
    /// Schema spelling of any value. The ordinary node budget still applies to the retained prefix.
    fn expand_response(&self, value: &Value) -> Result<JsonSchema, String> {
        let mut seen = Vec::new();
        let mut budget = 0;
        self.walk(value, &mut seen, &mut budget, CyclePolicy::BoundResponse)
    }

    /// Follows a `$ref` chain one node deep, leaving the target's own children alone.
    ///
    /// No expansion budget here, unlike [`walk`](Self::walk), and the asymmetry is real rather than
    /// an oversight: this recurses along a *chain* — a reference to a reference — so every hop
    /// pushes a distinct pointer onto `seen`, the document has finitely many, and cycle detection
    /// alone bounds it. `walk` recurses into a *tree* and can multiply, which is what needs the
    /// second bound.
    fn follow(&self, value: &Value, seen: &mut Vec<String>) -> Result<Value, String> {
        let Some(pointer) = reference(value) else {
            return Ok(value.clone());
        };
        let target = self.target(pointer, seen)?;
        let resolved = self.follow(&target, seen)?;
        seen.pop();
        Ok(merge_siblings(value, resolved))
    }

    /// Walks a value, expanding every `$ref` it meets at any depth.
    fn walk(
        &self,
        value: &Value,
        seen: &mut Vec<String>,
        budget: &mut usize,
        cycle_policy: CyclePolicy,
    ) -> Result<Value, String> {
        *budget += 1;
        if *budget > EXPANSION_BUDGET {
            return Err(format!(
                "resolving its `$ref`s produced more than {EXPANSION_BUDGET} nodes, so the \
                 operation was skipped rather than expanded further. This bounds the expansion and \
                 does not explain it: a very large legitimate schema and a reference graph that \
                 multiplies are the same count from here. For scale, the largest expansion measured \
                 across babelforce's 398 operations is 3,580 nodes"
            ));
        }

        match value {
            Value::Array(entries) => Ok(Value::Array(
                entries
                    .iter()
                    .map(|entry| self.walk(entry, seen, budget, cycle_policy))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Value::Object(object) => {
                if let Some(pointer) = reference(value) {
                    if cycle_policy == CyclePolicy::BoundResponse
                        && seen.iter().any(|had| had == pointer)
                    {
                        return Ok(bound_recursive_reference(value));
                    }
                    let target = self.target(pointer, seen)?;
                    let expanded = self.walk(&target, seen, budget, cycle_policy)?;
                    seen.pop();
                    return Ok(merge_siblings(value, expanded));
                }
                let mut expanded = Map::new();
                for (key, child) in object {
                    expanded.insert(key.clone(), self.walk(child, seen, budget, cycle_policy)?);
                }
                Ok(Value::Object(expanded))
            }
            scalar => Ok(scalar.clone()),
        }
    }

    /// What one `$ref` points at, refusing a cycle and pushing the pointer onto the trail.
    ///
    /// The caller pops. A cycle is an error rather than a truncation because there is no honest
    /// truncation of one: a schema that contains itself has no finite verbatim form, and emitting a
    /// depth-limited approximation would publish a type contract the vendor never stated.
    fn target(&self, pointer: &str, seen: &mut Vec<String>) -> Result<Value, String> {
        if let Some(at) = seen.iter().position(|had| had == pointer) {
            let mut cycle: Vec<&str> = seen[at..].iter().map(String::as_str).collect();
            cycle.push(pointer);
            return Err(format!(
                "`$ref` cycle: {}. A schema that contains itself has no finite resolved form, so \
                 the operation was skipped rather than expanded forever",
                cycle.join(" -> ")
            ));
        }
        let target = self.at(pointer)?;
        seen.push(pointer.to_owned());
        Ok(target)
    }

    /// The node a local JSON Pointer names.
    fn at(&self, pointer: &str) -> Result<Value, String> {
        let Some(path) = pointer.strip_prefix("#/") else {
            if pointer == "#" {
                return Err("`$ref: \"#\"` points at the whole document".to_owned());
            }
            return Err(format!(
                "`$ref: {pointer:?}` is not a pointer into this document. Ingest reads one \
                 document and touches no filesystem, so an external reference cannot be followed"
            ));
        };

        let mut node: Option<&Value> = None;
        for segment in path.split('/') {
            // RFC 6901's escapes, in the order the standard mandates: `~1` before `~0`, or a
            // literal `~1` in a key would decode to `/`.
            let segment = segment.replace("~1", "/").replace("~0", "~");
            node = Some(match node {
                None => self.root.get(&segment).ok_or_else(|| {
                    format!("`$ref: {pointer:?}` points at nothing — no `{segment}` there")
                })?,
                Some(Value::Object(object)) => object.get(&segment).ok_or_else(|| {
                    format!("`$ref: {pointer:?}` points at nothing — no `{segment}` there")
                })?,
                Some(Value::Array(entries)) => segment
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| entries.get(index))
                    .ok_or_else(|| {
                        format!("`$ref: {pointer:?}` points past the end of a list at `{segment}`")
                    })?,
                Some(_) => {
                    return Err(format!(
                        "`$ref: {pointer:?}` walks into a scalar at `{segment}`"
                    ))
                }
            });
        }
        node.cloned()
            .ok_or_else(|| format!("`$ref: {pointer:?}` names no node"))
    }
}

/// Drop only a recursive `$ref` back-edge, retaining any sibling constraints or annotations.
///
/// With no siblings the result is `true`, JSON Schema's explicit unconstrained schema. With
/// siblings, those remain a sound over-approximation because OpenAPI 3.1 combines them with the
/// referenced target; dropping one conjunct broadens the accepted set and never fabricates a
/// narrower finite recursive type. OpenAPI 3.0 forbids siblings, so its ordinary result is `true`.
fn bound_recursive_reference(reference: &Value) -> Value {
    let Some(object) = reference.as_object() else {
        return Value::Bool(true);
    };
    let mut siblings = object.clone();
    siblings.remove("$ref");
    if siblings.is_empty() {
        Value::Bool(true)
    } else {
        Value::Object(siblings)
    }
}

/// The pointer an object's `$ref` names, if it has one.
fn reference(value: &Value) -> Option<&str> {
    value.as_object()?.get("$ref")?.as_str()
}

/// OpenAPI 3.1 allows keywords beside a `$ref`, and they annotate the target rather than replacing
/// it — a `description` on the reference is the common case. 3.0 forbids the sibling; carrying it
/// anyway costs nothing and loses nothing.
fn merge_siblings(reference: &Value, mut resolved: Value) -> Value {
    let (Some(siblings), Some(target)) = (reference.as_object(), resolved.as_object_mut()) else {
        return resolved;
    };
    for (key, value) in siblings {
        if key != "$ref" {
            target.insert(key.clone(), value.clone());
        }
    }
    resolved
}

/// A string field of a mapping, when it is a string.
fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key)?.as_str()
}

/// The whole-document refusal — see [`ingest`]'s errors.
fn invalid(reason: impl Into<String>) -> crate::Error {
    crate::Error::ParseSpec {
        reason: reason.into(),
    }
}

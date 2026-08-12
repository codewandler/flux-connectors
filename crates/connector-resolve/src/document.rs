//! The canonical document, typed.
//!
//! One deterministic JSON document per provider (`catalog/<id>.catalog.json`, C-536), served from
//! the embedded pack by the dependency-free reader (C-537). This module turns the parts of it a
//! **request** is made of into types: the services' base URLs, and each operation's request
//! template, declared parameters and endpoint slots.
//!
//! Everything else the document carries — response schemas, quirks, events, the OAuth2 spec — is
//! skipped rather than modelled. Interpreting those is somebody else's job, and a struct that
//! claimed them would have to be kept in step with a schema this crate does not own.
//!
//! # The one thing the document does not publish, and what this module does about it
//!
//! A caller addresses a parameter by the name the operation's **contract** advertises, and that name
//! is a *Flux symbol*: `time.start` is declared `time_start`, `$top` is `_top`, and a parameter
//! called `response` becomes `response_2` because the emitter binds `response` itself. The document
//! publishes the IR name (`time.start`) and the wire name, and **not** the symbol — so the mapping
//! between the two lives only in `connector-flux`'s `names.rs`, which this crate must not link.
//!
//! [`Symbols`] therefore reproduces that allocation, and the reproduction is held to the same
//! standard Decision 0022 sets for everything else in this migration: the whole-catalogue
//! differential gate (`connector-pack/tests/main/catalogue_differential.rs`) asserts, for all 835
//! operations, that the names derived here are exactly the names the emitted declaration declares.
//! A divergence is a red gate rather than a request sent under the wrong parameter name.
//!
//! **This is the gap C-540 has to close before it deletes the emitter**, and it is recorded here
//! rather than in a story only: either the document grows a `symbol` beside `name`, or the pack's
//! caller-facing names become the IR's. Until one of those happens, the reproduction below is load
//! bearing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use serde_json::Value;

use crate::slot::Slot;

/// The symbols the emitter binds inside an op body, reserved unconditionally.
///
/// Transcribed from `connector-flux`'s `names.rs::RESERVED`. Reserved everywhere rather than per
/// operation, exactly as there: a `GET` binds neither `payload` nor `content_type`, but reserving
/// them everywhere keeps a parameter's symbol independent of which other parameters its operation
/// happens to declare.
const RESERVED: &[&str] = &[
    "base",
    "url",
    "content_type",
    "payload",
    "form_sep",
    "response",
];

/// The parameter name a free-form body is declared under.
const FREE_FORM_BODY: &str = "body";

/// Hands out a unique Flux symbol name for each vendor parameter name, in declaration order.
///
/// A reproduction of `connector-flux`'s `Symbols`. Stateful rather than a pure function because
/// `time.start` and `time_start` are distinct names that normalize identically, and collapsing them
/// would send one parameter's value under the other's name.
struct Symbols {
    taken: BTreeSet<String>,
}

impl Symbols {
    fn new() -> Self {
        Self {
            taken: RESERVED.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    /// The Flux symbol for `wire`, reserving it so no later parameter can collide with it.
    ///
    /// 1. every character outside `[A-Za-z0-9_]` becomes `_` (`time.start` → `time_start`);
    /// 2. a name that would start with a digit is prefixed with `p_`;
    /// 3. a name already taken gains a `_2`, `_3`, … suffix.
    fn allocate(&mut self, wire: &str) -> String {
        let mut symbol: String = wire
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        if symbol.starts_with(|c: char| c.is_ascii_digit()) {
            symbol.insert_str(0, "p_");
        }
        if self.taken.contains(&symbol) {
            let base = symbol.clone();
            let mut n = 2;
            while self.taken.contains(&symbol) {
                symbol = format!("{base}_{n}");
                n += 1;
            }
        }
        self.taken.insert(symbol.clone());
        symbol
    }
}

// ---------------------------------------------------------------------------------------------
// The document, as JSON
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawDocument {
    connector: String,
    #[serde(default)]
    services: Vec<RawService>,
    #[serde(default)]
    operations: Vec<RawOperation>,
}

#[derive(Debug, Deserialize)]
struct RawService {
    name: String,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct RawOperation {
    id: String,
    service: String,
    #[serde(default)]
    expose: bool,
    #[serde(default)]
    params: Vec<RawParam>,
    request: RequestTemplate,
    #[serde(default)]
    endpoint: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawParam {
    name: String,
    position: String,
}

/// One operation's request, in the document's closed template vocabulary.
#[derive(Debug, Clone, Deserialize)]
pub struct RequestTemplate {
    /// The HTTP method, stated rather than defaulted.
    pub method: String,
    /// The URL, always `{base}/…`, interpolating endpoint slots and caller parameters.
    pub url: String,
    /// The headers, keyed by wire name. A `BTreeMap`, which is also the order they travel in.
    #[serde(default)]
    pub headers: BTreeMap<String, ValueTemplate>,
    /// The structured query, in the order the document states.
    #[serde(default)]
    pub query: Vec<QueryEntry>,
    /// The body, when the operation has one.
    #[serde(default)]
    pub body: Option<BodyTemplate>,
}

/// A literal (which may interpolate endpoint slots) or a whole-parameter splice.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ValueTemplate {
    /// `{"$param": "name"}` — the whole value of the named caller parameter.
    Splice {
        /// The parameter's **document** name, which is not its caller-facing symbol.
        #[serde(rename = "$param")]
        param: String,
    },
    /// A literal, which may carry `{slot}` placeholders.
    Literal(String),
}

/// One structured-query pair.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryEntry {
    /// The wire name, as the vendor spells it.
    pub name: String,
    /// The value.
    pub value: ValueTemplate,
}

/// The request body, by encoding.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "encoding", rename_all = "lowercase")]
pub enum BodyTemplate {
    /// A JSON body: literals, nested objects and arrays, `$param` splices at caller leaves.
    Json {
        /// The template.
        template: Value,
    },
    /// A form-encoded body.
    Form {
        /// The fields, in the order they travel.
        fields: Vec<FormField>,
    },
}

/// One form-encoded body field.
#[derive(Debug, Clone, Deserialize)]
pub struct FormField {
    /// The wire name.
    pub name: String,
    /// The value.
    pub value: ValueTemplate,
    /// Whether the field always travels, or only when its value is truthy.
    pub required: bool,
}

// ---------------------------------------------------------------------------------------------
// The resolved view
// ---------------------------------------------------------------------------------------------

/// One provider's canonical document, resolved into the shape a request is derived from.
#[derive(Debug)]
pub struct Document {
    /// The connector id.
    pub connector: String,
    /// Each service's base URL, keyed by service name.
    services: BTreeMap<String, String>,
    /// Each operation, keyed by id.
    operations: BTreeMap<String, Operation>,
}

impl Document {
    /// Parse one provider document from its canonical JSON text.
    ///
    /// # Errors
    ///
    /// The parse failure, as `serde_json` states it. Unreachable for a document this repository
    /// generated — `connector-cli`'s `build` writes it and the pack's reader verifies the bytes —
    /// and reported rather than unwrapped, because the alternative is a panic inside a host's
    /// registration call.
    pub fn parse(text: &str) -> Result<Document, String> {
        let raw: RawDocument = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let services = raw
            .services
            .into_iter()
            .map(|service| (service.name, service.base_url))
            .collect();
        let operations = raw
            .operations
            .into_iter()
            .map(|operation| (operation.id.clone(), Operation::resolve(operation)))
            .collect();
        Ok(Document {
            connector: raw.connector,
            services,
            operations,
        })
    }

    /// The base URL template of `service`.
    pub fn base_url(&self, service: &str) -> Option<&str> {
        self.services.get(service).map(String::as_str)
    }

    /// One operation, by id.
    pub fn operation(&self, id: &str) -> Option<&Operation> {
        self.operations.get(id)
    }

    /// Every operation this document carries, in id order.
    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.operations.values()
    }
}

/// One operation's document: its request template, and everything derived from it once.
#[derive(Debug)]
pub struct Operation {
    /// The operation id.
    pub id: String,
    /// The service it belongs to — `default` for a connector with a single API surface.
    pub service: String,
    /// Whether the operation reaches a model as a tool (C-413).
    pub expose: bool,
    /// The request template.
    pub request: RequestTemplate,
    /// The configuration variables the request needs, in stable order.
    variables: Vec<String>,
    /// Where each of them lands.
    slots: BTreeMap<String, Slot>,
    /// The document name of each declared parameter, in declaration order.
    parameters: Vec<String>,
    /// Document name → the caller-facing symbol. See this module's documentation.
    symbols: BTreeMap<String, String>,
    /// Caller parameters the template places in a URL path segment (C-478), by symbol.
    caller_path_parameters: BTreeSet<String>,
}

impl Operation {
    fn resolve(raw: RawOperation) -> Operation {
        let mut allocator = Symbols::new();
        let mut symbols = BTreeMap::new();
        let mut parameters = Vec::new();
        let mut caller_path_parameters = BTreeSet::new();
        for param in &raw.params {
            // The free-form body is allocated *after* every positional group, exactly as
            // `connector-flux`'s `bind_parameters` does, and the document already lists it last.
            let symbol = allocator.allocate(&param.name);
            if param.position == "path" {
                caller_path_parameters.insert(symbol.clone());
            }
            parameters.push(param.name.clone());
            symbols.insert(param.name.clone(), symbol);
        }

        let slots: BTreeMap<String, Slot> = raw
            .endpoint
            .iter()
            .map(|(variable, positions)| {
                // One declared position is that slot; more than one is `Unplaced`, which is the
                // intersection of every rule rather than the absence of any (C-229).
                let slot = match positions.as_slice() {
                    [only] => Slot::from_document(only),
                    _ => Slot::Unplaced,
                };
                (variable.clone(), slot)
            })
            .collect();

        Operation {
            variables: slots.keys().cloned().collect(),
            slots,
            parameters,
            symbols,
            caller_path_parameters,
            id: raw.id,
            service: raw.service,
            expose: raw.expose,
            request: raw.request,
        }
    }

    /// The configuration variables this operation's request needs, in stable order.
    pub fn endpoint_variables(&self) -> &[String] {
        &self.variables
    }

    /// Where each of those variables lands on the request.
    pub fn endpoint_slots(&self) -> &BTreeMap<String, Slot> {
        &self.slots
    }

    /// Caller-visible parameters the request template places in the URL path.
    pub fn caller_path_parameters(&self) -> &BTreeSet<String> {
        &self.caller_path_parameters
    }

    /// The caller-facing name of each declared parameter, in declaration order.
    pub fn caller_parameters(&self) -> Vec<&str> {
        self.parameters
            .iter()
            .map(|name| self.symbols[name].as_str())
            .collect()
    }

    /// The caller-facing symbol of the document parameter `name`.
    pub(crate) fn symbol(&self, name: &str) -> Option<&str> {
        self.symbols.get(name).map(String::as_str)
    }

    /// Whether this operation declares a free-form body — one parameter carrying the whole payload,
    /// which travels through `parse(…, as: "json")` and is therefore validated rather than sent
    /// verbatim.
    pub(crate) fn has_free_form_body(&self) -> bool {
        matches!(&self.request.body, Some(BodyTemplate::Json { template })
            if matches!(splice_of(template), Some(name) if name == FREE_FORM_BODY))
    }
}

/// The parameter a `{"$param": name}` splice names, or `None` for anything else.
pub(crate) fn splice_of(value: &Value) -> Option<&str> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    object.get("$param").and_then(Value::as_str)
}

// ---------------------------------------------------------------------------------------------
// The embedded catalogue
// ---------------------------------------------------------------------------------------------

/// Documents parsed on demand and kept for the life of the process, keyed by connector id.
///
/// Leaked rather than reference-counted for the same reason `catalog::Operation` is `&'static`: a
/// catalogue is process-lifetime data, and a resolved operation is held by every projected tool.
/// One provider's document is parsed the first time an operation of that provider is resolved, so a
/// host that installs one connector does not pay for fifty-five.
fn cache() -> &'static Mutex<BTreeMap<String, &'static Document>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, &'static Document>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// One provider's canonical document, from the embedded pack.
///
/// `None` when the pack carries no such provider. A document that does not parse is also `None`:
/// the reader has already verified the container's digest and schema version, so a parse failure
/// here is a corrupt build rather than an input a caller can act on, and the callers all report
/// their own refusal naming the operation.
pub fn provider(id: &str) -> Option<&'static Document> {
    let mut cache = cache().lock().expect("the document cache is not poisoned");
    if let Some(document) = cache.get(id) {
        return Some(document);
    }
    let text = catalog_reader::provider(id)?.document();
    let document: &'static Document = Box::leak(Box::new(Document::parse(text).ok()?));
    cache.insert(id.to_owned(), document);
    Some(document)
}

/// One operation's document, by operation id.
pub fn operation(id: &str) -> Option<&'static Operation> {
    let record = catalog_reader::operation(id)?;
    provider(record.provider())?.operation(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_symbol_allocation_reproduces_the_emitters() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("per_page"), "per_page");
        assert_eq!(symbols.allocate("time.start"), "time_start");
        assert_eq!(symbols.allocate("time_start"), "time_start_2");
        assert_eq!(symbols.allocate("$top"), "_top");
        assert_eq!(symbols.allocate("2fa"), "p_2fa");
    }

    #[test]
    fn the_emitters_own_symbols_are_reserved() {
        let mut symbols = Symbols::new();
        assert_eq!(symbols.allocate("url"), "url_2");
        assert_eq!(symbols.allocate("base"), "base_2");
        assert_eq!(symbols.allocate("payload"), "payload_2");
        assert_eq!(symbols.allocate("response"), "response_2");
        assert_eq!(symbols.allocate("content_type"), "content_type_2");
    }

    #[test]
    fn a_shipped_document_parses_into_its_services_and_operations() {
        let document = provider("zendesk").expect("the pack carries zendesk");
        assert_eq!(
            document.base_url("default"),
            Some("https://{subdomain}.zendesk.com")
        );
        let operation = document
            .operation("zendesk-ticket-update")
            .expect("the document carries it");
        assert_eq!(operation.request.method, "PUT");
        assert_eq!(operation.request.url, "{base}/api/v2/tickets/{ticket_id}");
        assert_eq!(operation.endpoint_variables(), ["subdomain"]);
        assert_eq!(operation.endpoint_slots()["subdomain"], Slot::Host);
        assert_eq!(operation.caller_parameters(), ["ticket_id", "ticket"]);
        assert!(operation.caller_path_parameters().contains("ticket_id"));
    }

    #[test]
    fn an_operation_resolves_through_the_embedded_pack_without_naming_its_provider() {
        let operation = operation("zendesk-ticket-show").expect("the pack carries it");
        assert_eq!(operation.service, "default");
        assert!(operation.expose);
    }

    #[test]
    fn an_unknown_provider_or_operation_is_absent_rather_than_a_panic() {
        assert!(provider("no-such-vendor").is_none());
        assert!(operation("no-such-operation").is_none());
    }
}

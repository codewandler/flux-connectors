//! `flux-connectors scaffold` — write the patch set from the document, so referencing a spec is
//! cheaper than hand-authoring it (C-419).
//!
//! C-411, C-412 and C-414 reduced how much has to be *said* about 397 operations. They did not
//! reduce who has to say it, and at that size that is the binding constraint. This module reads the
//! vendored documents a connector points at and emits the provider TOML that references them — the
//! `[[spec]]` blocks, the `[patch.naming]` rule and its pins, the `[[patch.select]]` statements, and
//! the `[[patch.operations]]` blocks a selector cannot cover.
//!
//! # Three rules, and everything here follows from them
//!
//! **To stdout, never over a file in place.** The author diffs and pastes, so a bad run costs
//! nothing and the reviewed artifact stays a human's. Nothing in this module opens a file for
//! writing, and `tests/scaffold.rs::scaffold_writes_no_file` holds that.
//!
//! **What nobody has claimed comes out as a hole, not a guess.** No OpenAPI document publishes
//! direction, `risk` or `idempotency`, so where this helper cannot read the complete claim off an
//! operation the connector *already publishes*, it states none of them and leaves a `TODO` — and
//! the loader's required direction makes that a build failure rather than a comment. A scaffold
//! that silently called every GET a read or declared 54 DELETEs `low` would have manufactured
//! unreviewed safety claims. See [`Claim`].
//!
//! **What it could not carry is reported, per operation and by count.** A dropped operation that
//! produces no output reads as "the vendor does not offer that", so every drop is named in the
//! emitted file. See [`Notes`].
//!
//! # It carries a human's claim forward, and only a human's
//!
//! Scaffolding is most valuable over a connector that already exists — C-420 rebuilds 52 of them —
//! and the `risk` those files state was reviewed by somebody. Throwing it away would make a rebuild
//! a re-authoring job, which is the cost this whole epic exists to remove. So a claim is carried
//! **only** from an operation the current connector publishes, and a `[[patch.select]]` may restate
//! one only when *every* operation it matches is already published and all of them agree. The moment
//! a selector reaches one operation nobody has claimed, it states nothing and every claimed sibling
//! keeps its own `[[patch.operations]]` block — so the hole lands exactly on the gap and no reviewed
//! claim is lost to it.
//!
//! # Offline, like everything else here
//!
//! It reads committed bytes: `providers/<name>.toml` and the documents under `specs/<name>/`. There
//! is no fetch, and `tests/no_network.rs`'s source audit covers this file with the rest of the crate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use connector_spec::{
    AuthMethod, AuthRequirement, ChannelBinding, ConfigField, Connector, EventDecl, Graph,
    HttpMethod, Idempotency, Ingested, Operation, OperationDirection, ParamPosition, Risk, Runtime,
    Service, SpecOperation, SpecSource, DEFAULT_SERVICE,
};

use crate::seam::{self, ProviderInputs, SpecInput};
use crate::workspace::Workspace;

/// Conventionally read-shaped methods. This class structures review output; it supplies no
/// direction or safety metadata.
const READ_METHODS: &[HttpMethod] = &[HttpMethod::Get, HttpMethod::Head, HttpMethod::Options];

/// Conventionally write-shaped methods. One selector makes a compact review set, but the emitted
/// direction still comes only from already published connector truth.
const WRITE_METHODS: &[HttpMethod] = &[HttpMethod::Post, HttpMethod::Put, HttpMethod::Patch];

/// The methods that destroy.
const DELETE_METHODS: &[HttpMethod] = &[HttpMethod::Delete];

/// The method classes, in the order they are emitted. Total over [`HttpMethod`], so no method can
/// fall out of a selection by not being classified.
const METHOD_CLASSES: [&[HttpMethod]; 3] = [READ_METHODS, WRITE_METHODS, DELETE_METHODS];

/// Whole path segments that mark an operation as part of the **authentication flow**, which this
/// helper never proposes selecting.
///
/// `AGENTS.md`, owner-stated 2026-08-01: *"An authentication endpoint is never a connector
/// operation… A vendor document that publishes them as paths does not make them operations, and
/// ingest selecting them is a selection error rather than a coverage win."* The same section rules
/// out the obvious workaround in advance — **`expose = false` is not the mechanism**, because
/// `connector_pack::resolve` admits any named operation regardless of exposure, so the operation
/// must not be *selected*.
///
/// A whole segment rather than a substring or a guess about the last one: `/oauth/token` and
/// `/oauth2/authorize` are unambiguous, and a path such as `/api/v2/agents/{id}/token` is not this
/// rule's business. The ambiguous middle is [`AUTH_FLOW_SUSPECTS`] — reported to a human, never
/// decided by a heuristic.
const AUTH_FLOW_SEGMENTS: [&str; 4] = ["oauth", "oauth2", "openid", ".well-known"];

/// Final path segments that *may* name an authentication endpoint outside an
/// [`AUTH_FLOW_SEGMENTS`] prefix.
///
/// **Reported, never excluded.** Withholding on this would be a heuristic deciding what a connector
/// offers, and this repository's posture on an ambiguous case is to say so rather than to choose.
const AUTH_FLOW_SUSPECTS: [&str; 4] = ["token", "authorize", "revoke", "introspect"];

/// Whether a path is part of the authentication flow rather than a call anyone makes.
fn is_auth_flow(path: &str) -> bool {
    path.split('/')
        .any(|segment| AUTH_FLOW_SEGMENTS.contains(&segment.to_ascii_lowercase().as_str()))
}

/// Whether a path's last segment is one an authentication endpoint often ends on.
fn looks_like_auth_flow(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|segment| AUTH_FLOW_SUSPECTS.contains(&segment.to_ascii_lowercase().as_str()))
}

// ---------------------------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------------------------

/// Emit the provider TOML that references `provider`'s vendored documents.
///
/// `selects` are the `--select` arguments in the order they were written; empty means every document
/// whole, split by method class.
pub fn render(workspace: &Workspace, provider: &str, selects: &[String]) -> Result<String> {
    Plan::read(workspace, provider, selects)?.to_toml()
}

/// Report the vendored documents against the connector as it stands — `--diff`.
///
/// This is the half that makes a **re-build** repeatable rather than a one-time migration, and it is
/// what C-420 runs 52 times: what upstream added, what it removed, and where a published operation's
/// parameters no longer agree with the document.
pub fn render_diff(workspace: &Workspace, provider: &str) -> Result<String> {
    Plan::read(workspace, provider, &[])?.to_diff()
}

// ---------------------------------------------------------------------------------------------
// Reading the inputs
// ---------------------------------------------------------------------------------------------

/// One vendored document, ingested, with the provenance the provider file already recorded for it.
struct Document {
    /// `specs/<provider>/<file>`, spelled as `[[spec]] path` spells it.
    path: String,
    /// The service this document's operations join. [`DEFAULT_SERVICE`] when there is one document
    /// and the file names none.
    service: String,
    /// Whether the emitted `[[spec]]` entry states `service`.
    ///
    /// False for the single-document case, where absent already means [`DEFAULT_SERVICE`] and where
    /// naming it explicitly is refused by the loader. It also decides how a document operation is
    /// matched against a published one: with no service stated there is one document and one
    /// surface, so method and path are the whole key.
    names_service: bool,
    /// SHA-256 of the vendored bytes, computed here rather than copied from the file — the emitted
    /// declaration has to be a fact about the bytes this run read.
    sha256: String,
    /// Carried from the file's `[[spec]]` entry when it had one; otherwise the document's own
    /// `info.version`.
    upstream_version: Option<String>,
    /// Carried from the file's `[[spec]]` entry. Nothing in a document states when it was pulled.
    fetched_at: Option<String>,
    /// Carried from the file's `[[spec]]` entry. Deliberately absent for babelforce — see AGENTS.md,
    /// "the pulled bytes come here, the configuration that pulled them does not".
    source_url: Option<String>,
    /// Everything the document declares.
    ingested: Ingested,
}

/// Everything one scaffold run reads, resolved.
struct Plan {
    provider: String,
    /// The connector as it stands, when `providers/<name>.toml` exists and compiles. `None` for a
    /// provider that does not exist yet, which is the case with no claims to carry at all.
    existing: Option<Connector>,
    /// The parameters the current file already omits, by (service, `operationId`) — subtracted from
    /// `--diff` so a decision somebody wrote down does not read as upstream drift (C-422).
    ///
    /// Keyed by the position's *word* rather than by [`ParamPosition`], which is not `Ord`: this is
    /// a report's index and widening a published enum's derives to serve one would be the tail
    /// wagging the IR.
    omitted: BTreeMap<(String, String), BTreeSet<(&'static str, String)>>,
    documents: Vec<Document>,
    /// The selectors to emit, already split into method classes and already matched.
    selections: Vec<Selection>,
    /// Everything this run could not carry.
    notes: Notes,
}

impl Plan {
    fn read(workspace: &Workspace, provider: &str, selects: &[String]) -> Result<Self> {
        let mut notes = Notes::default();
        let inputs = read_inputs(workspace, provider)?;

        // A file that does not compile is reported rather than fatal: the reason to scaffold may be
        // that the connector is broken, and refusing here would withhold the tool at the moment it
        // is most useful. What is lost is only the claims it would have carried, so saying so is the
        // whole obligation.
        let loaded = match inputs.definition.is_empty() {
            true => None,
            false => match seam::load_full(&inputs) {
                Ok(loaded) => Some(loaded),
                Err(error) => {
                    notes.blocked.push(format!(
                        "providers/{provider}.toml does not compile, so no `risk`, no \
                         `idempotency`, no op id and no credential was carried forward from it: {}",
                        one_line(&format!("{error:#}"))
                    ));
                    None
                }
            },
        };

        let documents = documents(&inputs, loaded.as_ref(), &mut notes)?;
        if documents.is_empty() {
            bail!(
                "no vendored document to scaffold from: `{}/{provider}/` holds nothing this ingest \
                 can read. `scaffold` writes the TOML that *references* a document, so there has to \
                 be one",
                crate::workspace::SPECS_DIR
            );
        }

        let mut omitted = BTreeMap::new();
        if let Some(loaded) = &loaded {
            for patch in &loaded.patch.operations {
                let service = patch.service.clone().unwrap_or_else(|| {
                    documents
                        .first()
                        .map_or_else(|| DEFAULT_SERVICE.to_owned(), |first| first.service.clone())
                });
                omitted.insert(
                    (service, patch.select.clone()),
                    patch
                        .omit
                        .entries()
                        .map(|(position, name)| (position_word(position), name.to_owned()))
                        .collect(),
                );
            }
        }

        let existing = loaded.map(|loaded| loaded.connector);
        let selections = selections(&documents, selects, &mut notes)?;

        Ok(Self {
            provider: provider.to_owned(),
            existing,
            omitted,
            documents,
            selections,
            notes,
        })
    }

    /// The operations one selector matched, in ingest order.
    fn matched<'a>(&'a self, selection: &'a Selection) -> impl Iterator<Item = &'a SpecOperation> {
        let operations = &self.documents[selection.document].ingested.operations;
        selection.matched.iter().map(|index| &operations[*index])
    }

    /// The operation the current connector publishes for one document operation, matched on method
    /// and path.
    ///
    /// **Method and path rather than op id**, deliberately: an op id is this repository's name for
    /// the call and an `operationId` is the vendor's, and neither is a key into the other. The
    /// request itself is the only thing both descriptions agree about.
    fn published(&self, document: &Document, operation: &SpecOperation) -> Option<&Operation> {
        self.existing.as_ref().and_then(|connector| {
            connector.operations.iter().find(|published| {
                // With no service stated there is exactly one document, so the service is not part
                // of the key — and it must not be, or a single-document connector that declares
                // named services would match nothing and every reviewed claim it holds would be
                // thrown away as though nobody had made it.
                (!document.names_service || published.service == document.service)
                    && published.method == operation.method
                    && published.path == operation.path
            })
        })
    }
}

/// Read `providers/<name>.toml` and the whole spec cache for one provider.
///
/// A missing definition is not an error — scaffolding a connector that does not exist yet is the
/// case the helper is *for* — so it arrives as an empty `definition`, which is the one value
/// [`Plan::read`] reads as "there is nothing here to carry forward".
fn read_inputs(workspace: &Workspace, provider: &str) -> Result<ProviderInputs> {
    let definition_path = workspace.providers_dir().join(format!("{provider}.toml"));
    let definition = match definition_path.is_file() {
        true => crate::artifact::read(&definition_path)?,
        false => String::new(),
    };

    let dir = workspace.spec_dir(provider);
    let mut files: Vec<std::path::PathBuf> = match dir.is_dir() {
        true => std::fs::read_dir(&dir)
            .with_context(|| format!("cannot read {}", dir.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(|| format!("cannot read {}", dir.display()))?,
        false => Vec::new(),
    };
    // `read_dir` yields filesystem order, which is not one. Every emitted byte downstream is ordered
    // by this list, so sorting is what makes two runs byte-identical.
    files.sort();

    let mut specs = Vec::new();
    for path in files {
        if path.is_dir() {
            continue;
        }
        let Some(version) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if version.starts_with('.') {
            continue;
        }
        specs.push(SpecInput {
            version: version.to_owned(),
            path: crate::workspace::spec_path(provider, &path),
            document: crate::artifact::read(&path)?,
        });
    }

    // Built here rather than through `discovery::discover`, which requires `providers/<name>.toml`
    // to exist. Scaffolding a connector that does not exist yet is the case this helper is for, so
    // the one input discovery insists on is the one input that may be missing.
    Ok(ProviderInputs {
        name: provider.to_owned(),
        definition,
        specs,
    })
}

/// Every document in the cache, in the order the provider file pins them and then in cache order.
fn documents(
    inputs: &ProviderInputs,
    loaded: Option<&connector_spec::LoadedProvider>,
    notes: &mut Notes,
) -> Result<Vec<Document>> {
    let pinned: Vec<&SpecSource> = loaded
        .map(|loaded| loaded.specs.iter().collect())
        .unwrap_or_default();
    let single = inputs.specs.len() == 1;
    // "Declares a **named** service", which is not the same as "declares a `[[services]]` entry".
    // Since C-153 a single-surface provider carries one entry for the reserved `default` service to
    // hold its `tags`, and reading that as a named service would send all 47 of them down the
    // one-document-many-services branch below — emitting a blocked note claiming a named service
    // that does not exist. `is_default_only` is the predicate that survives the tag.
    let declares_services = loaded.is_some_and(|loaded| !loaded.connector.is_default_only());

    // The file's order first, because that is the order a reviewer is diffing against.
    let mut order: Vec<&SpecInput> = Vec::new();
    for spec in &pinned {
        if let Some(input) = inputs
            .specs
            .iter()
            .find(|input| input.path == spec.path.trim())
        {
            order.push(input);
        }
    }
    for input in &inputs.specs {
        if !order.iter().any(|already| already.path == input.path) {
            order.push(input);
        }
    }

    let mut documents = Vec::new();
    for input in order {
        let declared = pinned
            .iter()
            .find(|spec| spec.path.trim() == input.path)
            .copied();

        // The pinned documents are already ingested by the load; re-ingesting them would be the same
        // work twice and could disagree with what the connector was actually compiled from.
        let ingested = match loaded.and_then(|loaded| {
            loaded
                .ingested
                .iter()
                .find(|document| document.path == input.path)
        }) {
            Some(document) => document.ingested.clone(),
            None => match seam::ingest(&input.document) {
                Ok(ingested) => ingested,
                Err(error) => {
                    notes.blocked.push(format!(
                        "{} could not be ingested and is not referenced by the emitted file: {}",
                        input.path,
                        one_line(&format!("{error:#}"))
                    ));
                    continue;
                }
            },
        };

        let (service, names_service) = match declared.and_then(|spec| spec.service.clone()) {
            Some(service) => (service, true),
            // One document and no named services: absent already means `default`, and naming it
            // explicitly is refused by the loader.
            None if single && !declares_services => (DEFAULT_SERVICE.to_owned(), false),
            // **One document and several declared services, which one document cannot join.** A
            // document becomes exactly one service (C-410), so this connector needs either a
            // document per service or a decision about which one this is — and neither is a
            // decision to make from a file name. The key is left out, the loader refuses naming the
            // services it declares, and the report says why.
            None if single => {
                notes.blocked.push(format!(
                    "{} is the only document and this connector declares {} named service(s), so \
                     the `[[spec]]` entry states no `service` and the emitted file will refuse. A \
                     document joins exactly one service (C-410): either vendor a document per \
                     service, or state which one this is",
                    input.path,
                    loaded.map_or(0, |loaded| loaded.connector.services.len())
                ));
                (DEFAULT_SERVICE.to_owned(), false)
            }
            None => (service_name_of(&input.path), true),
        };

        documents.push(Document {
            path: input.path.clone(),
            service,
            names_service,
            sha256: connector_spec::sha256_hex(input.document.as_bytes()),
            upstream_version: declared
                .and_then(|spec| spec.upstream_version.clone())
                .or_else(|| {
                    Some(ingested.upstream_version.clone()).filter(|v| !v.trim().is_empty())
                }),
            fetched_at: declared.and_then(|spec| spec.fetched_at.clone()),
            source_url: declared.and_then(|spec| spec.source_url.clone()),
            ingested,
        });
    }

    // Two documents joining one service would silently merge two API surfaces under one name, which
    // is exactly what C-410 made impossible in the loader. Say so here rather than emitting a file
    // that is refused three layers down with less context.
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for document in &documents {
        *seen.entry(document.service.as_str()).or_default() += 1;
    }
    for (service, count) in seen {
        if count > 1 {
            notes.blocked.push(format!(
                "{count} documents derive the service name {service:?} from their file names. A \
                 service is joined by exactly one document, so name them apart in the `[[spec]]` \
                 entries below before pasting"
            ));
        }
    }

    Ok(documents)
}

/// The service name a document's file path suggests: `manager-2026-07-10.openapi.yaml` -> `manager`.
///
/// **A suggestion, and the emitted file says so.** C-415 names a vendored document by its pull date
/// because three of babelforce's five publish `info.version = "0.0.0-dev"`, so the stem carries the
/// date and the service name in one string and this is the only place they can be told apart without
/// asking. The cut is at the first `-` followed by a digit, which is a date and never a word.
fn service_name_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.split('.').next().unwrap_or(file);
    let bytes: Vec<char> = stem.chars().collect();
    for (index, ch) in bytes.iter().enumerate() {
        if *ch == '-' && bytes.get(index + 1).is_some_and(char::is_ascii_digit) {
            return stem[..index].to_owned();
        }
    }
    stem.to_owned()
}

// ---------------------------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------------------------

/// One `[[patch.select]]` statement, already matched against its document.
struct Selection {
    /// Index into [`Plan::documents`].
    document: usize,
    path_prefix: Option<String>,
    /// Never empty: a class with no matching method is not emitted at all, because a selector that
    /// matches nothing is a loud error in the loader and would make the scaffold's own output refuse.
    methods: Vec<HttpMethod>,
    /// Indices into the document's ingested operations, in ingest order.
    matched: Vec<usize>,
}

/// A `--select` argument, parsed.
struct SelectArg {
    service: Option<String>,
    path_prefix: Option<String>,
    methods: Vec<HttpMethod>,
}

/// `<service>:<path_prefix>:<METHOD,METHOD>`, with fields droppable from the right and any field
/// allowed to be empty.
///
/// Positional and fixed rather than inferred: a grammar that guessed whether `manager` was a service
/// or a path would select by spelling accident, which is the opposite of a statement. An OpenAPI
/// path template cannot hold a `:`, so the separator is unambiguous.
fn parse_select(raw: &str) -> Result<SelectArg> {
    let fields: Vec<&str> = raw.split(':').collect();
    if fields.len() > 3 {
        bail!(
            "`--select {raw}` has {} `:`-separated fields; the grammar is \
             `<service>:<path_prefix>:<METHOD,METHOD>`, and a path template cannot hold a `:`",
            fields.len()
        );
    }

    let field = |index: usize| {
        fields
            .get(index)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };

    let mut methods = Vec::new();
    for word in field(2).unwrap_or_default().split(',') {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        let method = method_of(word).ok_or_else(|| {
            anyhow::anyhow!(
                "`--select {raw}` names the method {word:?}, which is not an HTTP method this IR \
                 has: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS"
            )
        })?;
        if !methods.contains(&method) {
            methods.push(method);
        }
    }

    Ok(SelectArg {
        service: field(0),
        path_prefix: field(1),
        methods,
    })
}

/// Turn the `--select` arguments — or their absence — into the selectors to emit.
fn selections(
    documents: &[Document],
    selects: &[String],
    notes: &mut Notes,
) -> Result<Vec<Selection>> {
    let mut requested: Vec<(usize, SelectArg)> = Vec::new();
    if selects.is_empty() {
        // Every document whole. `path_prefix` is left absent rather than derived from a longest
        // common prefix: absent is a statement the grammar already has ("every path in this
        // document"), and a derived prefix would be this helper deciding a boundary the author has
        // not looked at yet.
        for index in 0..documents.len() {
            requested.push((
                index,
                SelectArg {
                    service: None,
                    path_prefix: None,
                    methods: Vec::new(),
                },
            ));
        }
    } else {
        for raw in selects {
            let arg = parse_select(raw)?;
            let index = match &arg.service {
                Some(service) => documents
                    .iter()
                    .position(|document| &document.service == service)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "`--select {raw}` names the service {service:?}, and the documents here \
                             join {}",
                            list(documents.iter().map(|document| document.service.clone()))
                        )
                    })?,
                None if documents.len() == 1 => 0,
                None => bail!(
                    "`--select {raw}` names no service, and this connector has {} documents ({}). \
                     A path prefix is no more unique across documents than an `operationId` is",
                    documents.len(),
                    list(documents.iter().map(|document| document.service.clone()))
                ),
            };
            requested.push((index, arg));
        }
    }

    let mut selections = Vec::new();
    for (index, arg) in requested {
        let document = &documents[index];
        for class in METHOD_CLASSES {
            // The class narrowed to what the argument asked for, so an author who wrote
            // `methods = ["GET","DELETE"]` gets the two statements those are, not one that claims
            // a delete's risk over a read.
            let methods: Vec<HttpMethod> = class
                .iter()
                .copied()
                .filter(|method| arg.methods.is_empty() || arg.methods.contains(method))
                .collect();
            if methods.is_empty() {
                continue;
            }
            let reached: Vec<usize> = document
                .ingested
                .operations
                .iter()
                .enumerate()
                .filter(|(_, operation)| {
                    methods.contains(&operation.method)
                        && path_has_prefix(&operation.path, arg.path_prefix.as_deref())
                })
                .map(|(index, _)| index)
                .collect();

            // **An authentication endpoint is never a connector operation** (AGENTS.md,
            // owner-stated). It is withheld whether the author asked for it or not — the rule is
            // about what a connector *is*, not about what a command line requested — and it is
            // withheld by not being selected, because that section rules out `expose = false` as
            // the mechanism by name. Every one is reported, so an exclusion reads as a decision
            // with a reason rather than as a coverage gap.
            let (withheld, matched): (Vec<usize>, Vec<usize>) = reached
                .into_iter()
                .partition(|index| is_auth_flow(&document.ingested.operations[*index].path));
            for index in withheld {
                let operation = &document.ingested.operations[index];
                notes.auth_flow.push(format!(
                    "{}: {} {} ({}) is authentication-flow material rather than an operation — \
                     AGENTS.md, \"an authentication endpoint is never a connector operation\". It \
                     is withheld by not being selected, because `expose = false` withholds the \
                     tool and not the call",
                    document.service,
                    method_word(operation.method),
                    operation.path,
                    operation.operation_id
                ));
            }

            // A selector that matches nothing is a loud error in the loader, for the same reason a
            // `select` naming an absent `operationId` is. Emitting one would hand the author a file
            // that refuses; dropping it silently would hide that they asked for something the
            // document does not have. So it is dropped and reported.
            if matched.is_empty() {
                notes.unmatched.push(format!(
                    "{}: no operation matches {}",
                    document.service,
                    describe_selector(arg.path_prefix.as_deref(), &methods)
                ));
                continue;
            }
            selections.push(Selection {
                document: index,
                path_prefix: arg.path_prefix.clone(),
                methods,
                matched,
            });
        }
    }

    Ok(selections)
}

/// Whether `path` lies under `prefix`, matched on **whole segments**.
///
/// `/api/v2/agents` covers `/api/v2/agents/{id}` and not `/api/v2/agentsummary`. Restated from
/// `connector_spec::provider`'s private rule rather than shared: the emitted file is checked against
/// *that* copy by the loader, so the obligation here is to agree with it, and a test that scaffolds
/// and then loads is what proves the two agree.
fn path_has_prefix(path: &str, prefix: Option<&str>) -> bool {
    let Some(prefix) = prefix else {
        return true;
    };
    let prefix = prefix.trim().trim_end_matches('/');
    if prefix.is_empty() {
        return true;
    }
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

// ---------------------------------------------------------------------------------------------
// The claim: what a selector may say about risk and idempotency, and what it may not
// ---------------------------------------------------------------------------------------------

/// What one selector is allowed to state about the set it matched.
///
/// This type is the safety rule, so it is deliberately narrow: there is no constructor that invents
/// a value, and the only way to reach [`Claim::Stated`] is for every matched operation to be one a
/// human already published a claim for, with all of them agreeing.
enum Claim {
    /// Every matched operation has read-shaped transport, so the existing C-414 risk/idempotency
    /// rule applies. Direction is emitted separately by stable operation identity.
    ReadOnly,
    /// Every matched operation is already published with these values, and they all agree. One line
    /// instead of 54, restating a claim a human made rather than making one.
    Stated(Risk, Idempotency),
    /// Somebody has to decide. The count is how many matched operations carry no claim at all, which
    /// is what the emitted `TODO` names.
    Todo { unclaimed: usize },
}

impl Claim {
    /// The `(risk, idempotency)` a matched operation ends up with when its own block says nothing.
    ///
    /// This is what makes a `[[patch.operations]]` block hold the *exceptions and only the
    /// exceptions*: a block restating what the selector already gives is 391 blocks nobody reads.
    fn effective(&self) -> Option<(Risk, Idempotency)> {
        match self {
            Claim::ReadOnly => Some((Risk::Low, Idempotency::Idempotent)),
            Claim::Stated(risk, idempotency) => Some((*risk, *idempotency)),
            Claim::Todo { .. } => None,
        }
    }
}

impl Plan {
    fn claim(&self, selection: &Selection) -> Claim {
        let document = &self.documents[selection.document];
        if selection
            .methods
            .iter()
            .all(|method| READ_METHODS.contains(method))
        {
            return Claim::ReadOnly;
        }

        let mut agreed: Option<(Risk, Idempotency)> = None;
        let mut unclaimed = 0;
        for operation in self.matched(selection) {
            match self.published(document, operation) {
                // C-186: `conditional` owes a stated `repeatable_because`, and no `[[patch.select]]`
                // and no `[[patch.operations]]` has a field to put one in. So an operation claiming
                // it cannot be carried by any construct this file has, and it is a hole rather than
                // a value silently downgraded to `non_idempotent`.
                Some(published) if published.idempotency == Idempotency::Conditional => {
                    unclaimed += 1;
                }
                Some(published) => match agreed {
                    Some(claim) if claim != (published.risk, published.idempotency) => {
                        // Disagreement is not a hole — every one of these was claimed by a human —
                        // but it is not one statement either, so each keeps its own block below.
                        return Claim::Todo { unclaimed: 0 };
                    }
                    Some(_) => {}
                    None => agreed = Some((published.risk, published.idempotency)),
                },
                None => unclaimed += 1,
            }
        }

        match (agreed, unclaimed) {
            (Some((risk, idempotency)), 0) => Claim::Stated(risk, idempotency),
            _ => Claim::Todo { unclaimed },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Naming
// ---------------------------------------------------------------------------------------------

/// `listReportingCalls` -> `list-reporting-calls`, the derivation `[patch.naming] rule = "kebab"`
/// applies.
///
/// Restated from `connector_spec::provider`'s private `kebab` for the same reason
/// [`path_has_prefix`] is: this helper has to predict what the loader will derive in order to know
/// which op ids need pinning, and the round-trip test is what proves the prediction is right.
fn kebab(operation_id: &str) -> String {
    let chars: Vec<char> = operation_id.chars().collect();
    let mut out = String::with_capacity(operation_id.len() + 8);
    for (index, ch) in chars.iter().copied().enumerate() {
        if !ch.is_ascii_uppercase() {
            out.push(ch);
            continue;
        }
        let follows_a_word = index > 0
            && (chars[index - 1].is_ascii_lowercase() || chars[index - 1].is_ascii_digit());
        let ends_an_acronym = index > 0
            && chars[index - 1].is_ascii_uppercase()
            && chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
        if follows_a_word || ends_an_acronym {
            out.push('-');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// The op id the emitted `[patch.naming]` rule will derive for one `operationId`.
fn derived_id(provider: &str, operation_id: &str) -> String {
    format!("{provider}-{}", kebab(operation_id))
}

/// How a selected operation gets the public name it must keep.
struct Naming {
    /// `operationId` -> op id, for `[patch.naming.pin]`. Only ids the rule would *not* derive, and
    /// only where the `operationId` is unique across the selected documents.
    pins: BTreeMap<String, String>,
    /// `(service, operationId)` -> op id, for the `[[patch.operations]] rename` a pin cannot express
    /// because two documents declare the same `operationId`.
    renames: BTreeMap<(String, String), String>,
}

impl Plan {
    /// Work out which shipped op ids the naming rule would move, and how to hold each of them still.
    ///
    /// **An op id is a public contract** (`docs/designs/connector-pipeline.md`), so a rule arriving
    /// underneath a connector that already publishes 254 of them must not move one. Every pin here
    /// is read off an operation this connector publishes today; nothing is invented.
    fn naming(&self, notes: &mut Notes) -> Naming {
        // Which `operationId`s more than one selected document declares — the only case a pin, which
        // is keyed by `operationId` alone, cannot express.
        let mut declared_by: BTreeMap<&str, usize> = BTreeMap::new();
        for selection in &self.selections {
            for operation in self.matched(selection) {
                *declared_by
                    .entry(operation.operation_id.as_str())
                    .or_default() += 1;
            }
        }

        let mut naming = Naming {
            pins: BTreeMap::new(),
            renames: BTreeMap::new(),
        };
        let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for selection in &self.selections {
            let document = &self.documents[selection.document];
            for operation in self.matched(selection) {
                let derived = derived_id(&self.provider, &operation.operation_id);
                let published = self.published(document, operation);
                let wanted = published.map_or(derived.clone(), |published| published.id.clone());

                if wanted != derived {
                    if declared_by
                        .get(operation.operation_id.as_str())
                        .is_some_and(|count| *count > 1)
                    {
                        naming.renames.insert(
                            (document.service.clone(), operation.operation_id.clone()),
                            wanted.clone(),
                        );
                    } else {
                        naming
                            .pins
                            .insert(operation.operation_id.clone(), wanted.clone());
                    }
                }

                ids.entry(wanted).or_default().push(format!(
                    "{} {} ({})",
                    method_word(operation.method),
                    operation.path,
                    operation.operation_id
                ));
            }
        }

        // Two `operationId`s deriving one op id is an error in the loader, never last-write-wins,
        // because the loser would silently become unreachable under a name a user still calls. This
        // helper cannot break the tie — a public name is not something to invent — so it names both
        // and leaves the rename for a human.
        for (id, holders) in &ids {
            if holders.len() > 1 {
                notes.collisions.push(format!(
                    "{} operations derive the op id {id:?}: {}. A `[[patch.operations]] rename` is \
                     the only construct that can tell them apart, and choosing the name is not this \
                     helper's to do",
                    holders.len(),
                    holders.join(", ")
                ));
            }
        }

        naming
    }
}

// ---------------------------------------------------------------------------------------------
// Per-operation blocks
// ---------------------------------------------------------------------------------------------

/// One `[[patch.operations]]` block: the exceptions, and only the exceptions.
#[derive(Default)]
struct Block {
    /// Absent when the file declares exactly one document, where `select` is already unambiguous —
    /// which is the only case the loader allows it to be absent in.
    service: Option<String>,
    select: String,
    rename: Option<String>,
    risk: Option<Risk>,
    idempotency: Option<Idempotency>,
    expose: Option<bool>,
    description: Option<String>,
    /// Rendered as comments above the block — what it could not carry about this one operation.
    todos: Vec<String>,
}

impl Block {
    fn is_bare(&self) -> bool {
        self.rename.is_none()
            && self.risk.is_none()
            && self.idempotency.is_none()
            && self.expose.is_none()
            && self.description.is_none()
            && self.todos.is_empty()
    }
}

impl Plan {
    /// The blocks the selectors cannot cover, in selector order and then in document order.
    fn blocks(&self, naming: &Naming, notes: &mut Notes) -> Vec<Block> {
        let mut blocks: Vec<Block> = Vec::new();
        for selection in &self.selections {
            let document = &self.documents[selection.document];
            let claim = self.claim(selection);
            let effective = claim.effective();
            let selector_expose = self.selector_expose(selection);

            for operation in self.matched(selection) {
                let published = self.published(document, operation);
                let mut block = Block {
                    service: document.names_service.then(|| document.service.clone()),
                    select: operation.operation_id.clone(),
                    rename: naming
                        .renames
                        .get(&(document.service.clone(), operation.operation_id.clone()))
                        .cloned(),
                    ..Block::default()
                };

                if let Some(published) = published {
                    if effective != Some((published.risk, published.idempotency)) {
                        if published.idempotency == Idempotency::Conditional {
                            // C-186's condition has nowhere to go in a patch, so the claim is a hole
                            // and says why. Emitting `idempotency = "conditional"` without it would
                            // be refused anyway, with a message about a field this file cannot write.
                            block.todos.push(format!(
                                "TODO(idempotency): {} publishes `conditional` today, and its \
                                 `repeatable_because` — {:?} — has no field in a \
                                 `[[patch.operations]]` block (C-186). State an idempotency here, \
                                 or the build refuses",
                                published.id,
                                published.repeatable_because.as_deref().unwrap_or("")
                            ));
                            block.risk = Some(published.risk);
                        } else {
                            block.risk = Some(published.risk);
                            block.idempotency = Some(published.idempotency);
                        }
                    }
                    if selector_expose.unwrap_or(true) != published.expose {
                        block.expose = Some(published.expose);
                    }
                    // The document's own sentence is preferred wherever it says the same thing; a
                    // stated one is kept only where the connector's carries something the vendor's
                    // does not, because that sentence is the tool contract.
                    if published.description.trim() != operation.description.trim()
                        && !published.description.trim().is_empty()
                    {
                        block.description = Some(published.description.clone());
                    }
                }

                if operation.description.trim().is_empty()
                    && published.is_none_or(|published| published.description.trim().is_empty())
                {
                    notes.nameless.push(format!(
                        "{}: {} {} ({}) has no description",
                        document.service,
                        method_word(operation.method),
                        operation.path,
                        operation.operation_id
                    ));
                }

                if !block.is_bare() {
                    blocks.push(block);
                }
            }
        }
        blocks
    }

    /// The `expose` a selector states, or `None` to leave the field's own default (**exposed**).
    ///
    /// # Why this one field may be decided here, when `risk` may not
    ///
    /// It is the one field where the conservative direction is not also the flattering one.
    /// Withholding the *tool* costs a caller nothing — C-413 separates catalogued-and-callable from
    /// exposed precisely so it can — while `expose = true` over a set nobody has curated is the
    /// denial of service against a model's context that `docs/designs/spec-front-end.md` §3 argues
    /// at length: 391 tools is not a catalogue. A wrong `risk = "low"` is read by a host as a
    /// licence; a wrong `expose = false` is read by nobody, and is one line to widen.
    ///
    /// So the rule is: **the default is only left standing when every matched operation is one this
    /// connector already publishes as a tool.** The moment a selector reaches an operation nobody
    /// has decided about, it states `expose = false` and the tools that already exist keep their own
    /// `expose = true` block.
    fn selector_expose(&self, selection: &Selection) -> Option<bool> {
        let document = &self.documents[selection.document];
        let all_exposed = self.matched(selection).all(|operation| {
            self.published(document, operation)
                .is_some_and(|published| published.expose)
        });
        (!all_exposed).then_some(false)
    }
}

// ---------------------------------------------------------------------------------------------
// What could not be carried
// ---------------------------------------------------------------------------------------------

/// Everything one scaffold run dropped, kept per item so the emitted file can name each one.
///
/// **A dropped operation that produces no output reads as "the vendor does not offer that."** That
/// sentence is why this type exists and why every field is a list of named items rather than a count:
/// the count is the summary, and the summary is not the report.
#[derive(Default)]
struct Notes {
    /// Operations a document declares that ingest could not read at all — an inexpressible body, a
    /// parameter in a position this IR has no slot for, an unresolvable `$ref`.
    dropped: Vec<String>,
    /// Narrower problems that did not cost the operation.
    lesser: Vec<String>,
    /// Selected operations with no sentence in them. A tool contract with no description is not a
    /// tool contract (`docs/designs/spec-front-end.md` §"What retiring manager-sdk requires").
    nameless: Vec<String>,
    /// Op ids two operations both derive.
    collisions: Vec<String>,
    /// `--select` statements that matched nothing and were therefore not emitted.
    unmatched: Vec<String>,
    /// Declarations this run could not carry at all — an unreadable document, a service a document
    /// cannot join, a role claim a narrowed selection may not satisfy.
    blocked: Vec<String>,
    /// Authentication-flow endpoints withheld from the selection, and the ambiguous ones a human has
    /// to look at. Reported so an exclusion reads as a decision rather than as something missing.
    auth_flow: Vec<String>,
    /// Operations a selector reaches that the connector does not publish today, and which will
    /// therefore reach a model as tools unless `expose = false` is stated.
    newly_exposed: usize,
}

impl Plan {
    /// Fill in everything the documents themselves reported, plus what selection then implied.
    ///
    /// **Scoped to what was selected**, and that scoping is the difference between a report and a
    /// wall. A diagnostic about an endpoint no selector reaches is not something this run failed to
    /// carry — nobody asked for it — and 300 such lines would bury the five that matter.
    fn survey(&self, notes: &mut Notes) {
        for (index, document) in self.documents.iter().enumerate() {
            let selectors: Vec<&Selection> = self
                .selections
                .iter()
                .filter(|selection| selection.document == index)
                .collect();
            if selectors.is_empty() {
                notes.unmatched.push(format!(
                    "{} is in the cache and no selector reaches it, so the emitted file does not \
                     reference it at all",
                    document.path
                ));
                continue;
            }

            for diagnostic in &document.ingested.diagnostics {
                let Some((method, path)) = parse_location(&diagnostic.location) else {
                    // A whole-section problem — no `servers`, unreadable `paths` — is about the
                    // document rather than about one endpoint, so no selector can scope it away.
                    notes
                        .lesser
                        .push(format!("{}: {diagnostic}", document.service));
                    continue;
                };
                let reached = selectors.iter().any(|selection| {
                    selection.methods.contains(&method)
                        && path_has_prefix(&path, selection.path_prefix.as_deref())
                });
                if !reached {
                    continue;
                }
                // Whether an operation *survived* its diagnostic is a fact about the ingest rather
                // than a sentence to grep for: the location is `METHOD /path`, so an operation
                // missing from the result at that address is one the document declared, a selector
                // would have matched, and this pipeline could not carry.
                let survived = document
                    .ingested
                    .operations
                    .iter()
                    .any(|operation| operation.method == method && operation.path == path);
                let line = format!("{}: {diagnostic}", document.service);
                if survived {
                    notes.lesser.push(line);
                } else {
                    notes.dropped.push(line);
                }
            }
        }

        for selection in &self.selections {
            let document = &self.documents[selection.document];
            for operation in self.matched(selection) {
                if self.published(document, operation).is_none() {
                    notes.newly_exposed += 1;
                }
                // Selected, and ending on a segment an authentication endpoint often ends on. Not
                // withheld — that judgement belongs to whoever reads the vendor's prose, and a
                // heuristic deciding what a connector offers is exactly what this repository does
                // not do. Named here so the decision is taken rather than defaulted.
                if looks_like_auth_flow(&operation.path) {
                    notes.auth_flow.push(format!(
                        "{}: {} {} ({}) is selected and ends on a segment an authentication \
                         endpoint often ends on. If it mints, exchanges or destroys a credential it \
                         is not an operation (AGENTS.md) and must be deselected — this is reported \
                         rather than decided, because only the vendor's prose settles it",
                        document.service,
                        method_word(operation.method),
                        operation.path,
                        operation.operation_id
                    ));
                }
            }
        }
    }
}

/// `GET /api/v2/agents` -> the pair, for a diagnostic whose location is an endpoint.
fn parse_location(location: &str) -> Option<(HttpMethod, String)> {
    let (method, path) = location.split_once(' ')?;
    Some((method_of(method)?, path.trim().to_owned()))
}

// ---------------------------------------------------------------------------------------------
// Emitting the TOML
// ---------------------------------------------------------------------------------------------

/// The connector's identity, re-serialized rather than re-derived.
///
/// Everything here is a decision somebody already made and no document states: the reverse-DNS
/// authority, the base URL a call actually reaches, the credentials. Scalars first, then tables,
/// because that is the order TOML requires and serde emits fields in declaration order.
#[derive(Serialize)]
struct Identity<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    vendor: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority: Option<&'a str>,
    #[serde(skip_serializing_if = "is_http")]
    runtime: Runtime,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: Option<&'a str>,
    base_url: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    verify: Option<&'a str>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    default_auth: &'a [AuthRequirement],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    services: &'a [Service],
}

/// The connector's declarations that are neither identity nor spec — carried whole, because a
/// scaffold that dropped a channel binding would be a scaffold that deleted an inbound surface.
#[derive(Serialize)]
struct Declarations<'a> {
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    auth: &'a [AuthMethod],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    config: &'a [ConfigField],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    events: &'a [EventDecl],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    channels: &'a [ChannelBinding],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    graphs: &'a [Graph],
}

/// The `[[spec]]` entries.
#[derive(Serialize)]
struct Specs {
    spec: Vec<SpecSource>,
}

fn is_http(runtime: &Runtime) -> bool {
    *runtime == Runtime::Http
}

impl Plan {
    fn to_toml(&self) -> Result<String> {
        let mut notes = Notes {
            dropped: self.notes.dropped.clone(),
            lesser: self.notes.lesser.clone(),
            nameless: self.notes.nameless.clone(),
            collisions: self.notes.collisions.clone(),
            unmatched: self.notes.unmatched.clone(),
            blocked: self.notes.blocked.clone(),
            auth_flow: self.notes.auth_flow.clone(),
            newly_exposed: self.notes.newly_exposed,
        };
        self.survey(&mut notes);
        let naming = self.naming(&mut notes);
        let blocks = self.blocks(&naming, &mut notes);
        let services = self.selected_services(&mut notes);

        let mut out = String::new();
        self.write_header(&mut out);
        self.write_identity(&mut out, &services)?;
        self.write_specs(&mut out)?;
        self.write_declarations(&mut out)?;
        self.write_naming(&mut out, &naming);
        self.write_directions(&mut out);
        self.write_selectors(&mut out);
        write_blocks(&mut out, &blocks);
        write_notes(&mut out, &notes);
        Ok(out)
    }

    fn write_header(&self, out: &mut String) {
        let documents = self.documents.len();
        let noun = if documents == 1 {
            "document"
        } else {
            "documents"
        };
        rule(out);
        let _ = writeln!(
            out,
            "\
# {} — scaffolded from {documents} vendored {noun} by `flux-connectors scaffold` (C-419).
#
# **This is input to a human, not an artifact.** Nothing hashes it, it is not in `connectors.lock`,
# and `flux-connectors diff` says nothing about it. Read it, correct it, and paste it over
# `{}/{}.toml` yourself — the reviewed file stays a human's.
#
# Two rules govern what is here, and they are the same rule twice:
#
#   * **It states no `risk` and no `idempotency` it did not read off an operation this connector
#     already publishes.** No OpenAPI document publishes either, so where nobody has claimed one the
#     selector below is silent, and C-414 refuses the build by name. That refusal is the point: a
#     scaffold that declared every DELETE `low` would have manufactured unreviewed safety claims
#     rather than saved anyone work.
#   * **It drops nothing quietly.** Everything it could not carry is named at the bottom of this
#     file, per operation and by count.",
            self.provider,
            crate::workspace::PROVIDERS_DIR,
            self.provider,
        );
        rule(out);
        out.push('\n');
    }

    fn write_identity(&self, out: &mut String, services: &[Service]) -> Result<()> {
        let empty_connector;
        let connector = match &self.existing {
            Some(connector) => connector,
            None => {
                empty_connector = self.identity_from_documents();
                &empty_connector
            }
        };

        if self.existing.is_none() {
            let _ = writeln!(
                out,
                "\
# No `{}/{}.toml` exists yet, so the six lines below are the ones a document cannot state and
# nobody has stated either. `base_url` is the document's first `servers` entry, which is advisory
# by construction — a vendor puts whichever environment it listed first there. Everything marked
# TODO is a hole on purpose.",
                crate::workspace::PROVIDERS_DIR,
                self.provider
            );
        }

        let verify = connector
            .verify
            .as_deref()
            .filter(|id| self.publishes_op_id(id));
        if connector.verify.is_some() && verify.is_none() {
            let _ = writeln!(
                out,
                "# `verify` is dropped: {:?} is not among the operations this selection publishes.",
                connector.verify.as_deref().unwrap_or_default()
            );
        }

        let identity = Identity {
            id: &connector.id,
            vendor: &connector.vendor,
            authority: connector.authority.as_deref(),
            runtime: connector.runtime,
            api_version: connector.api_version.as_deref(),
            base_url: &connector.base_url,
            description: &connector.description,
            verify,
            default_auth: &connector.default_auth,
            services,
        };
        out.push_str(&toml::to_string(&identity)?);
        out.push('\n');
        Ok(())
    }

    fn write_specs(&self, out: &mut String) -> Result<()> {
        rule(out);
        let _ = writeln!(
            out,
            "\
# The vendored documents — one per service (C-410).
#
# `sha256` is computed from the bytes this run read, and `load_with_spec` checks it against the
# bytes it ingests, so a declaration that disagrees refuses the build rather than travelling as a
# claim the file makes about itself. `source_url` and `fetched_at` are carried from the file that
# already recorded them; nothing in a document states either, and a URL naming an internal host does
# not belong in a public repository (AGENTS.md: the pulled bytes come here, the configuration that
# pulled them does not)."
        );
        rule(out);
        out.push('\n');

        let specs = Specs {
            spec: self
                .selected_documents()
                .map(|document| SpecSource {
                    path: document.path.clone(),
                    service: document.names_service.then(|| document.service.clone()),
                    source_url: document.source_url.clone(),
                    upstream_version: document.upstream_version.clone(),
                    sha256: Some(document.sha256.clone()),
                    fetched_at: document.fetched_at.clone(),
                })
                .collect(),
        };
        out.push_str(&toml::to_string(&specs)?);
        out.push('\n');
        Ok(())
    }

    fn write_declarations(&self, out: &mut String) -> Result<()> {
        let Some(connector) = &self.existing else {
            let _ = writeln!(
                out,
                "\
# TODO(auth): no `[[auth]]` block, because nothing carries one. Ingest reads a document's
# operations and not its `securitySchemes` (C-5 is the story that closes that), so the credentials
# this connector needs are a declaration only a human can make. Every operation below will refuse at
# call time until one is declared and referenced by `default_auth`.\n"
            );
            return Ok(());
        };
        let declarations = Declarations {
            auth: &connector.auth,
            config: &connector.config,
            events: &connector.events,
            channels: &connector.channels,
            graphs: &connector.graphs,
        };
        let rendered = toml::to_string(&declarations)?;
        if !rendered.trim().is_empty() {
            rule(out);
            let _ = writeln!(
                out,
                "\
# Credentials and the rest of the connector's declarations, carried forward verbatim. None of this
# is derived from a document; it is what the current file already says."
            );
            rule(out);
            out.push('\n');
            out.push_str(&rendered);
            out.push('\n');
        }
        Ok(())
    }

    fn write_naming(&self, out: &mut String, naming: &Naming) {
        rule(out);
        let _ = writeln!(
            out,
            "\
# Naming — one rule, {} pin(s) (C-412).
#
# The rule derives `listReportingCalls` into `{}-list-reporting-calls`, so an op id
# exists for every selected operation without a line each saying so. Collisions refuse
# rather than last-write-wins.
#
# **Every pin below is an op id this connector already publishes**, held still because an op id is a
# public contract users and models call by name. None of them was invented here.",
            naming.pins.len(),
            self.provider
        );
        rule(out);
        let _ = writeln!(out, "\n[patch.naming]\nrule = \"kebab\"");
        let _ = writeln!(out, "prefix = {}", quote(&self.provider));
        if !naming.pins.is_empty() {
            let _ = writeln!(out, "\n[patch.naming.pin]");
            for (operation_id, id) in &naming.pins {
                // A bare key is only legal for a bare-key `operationId`; anything else is quoted.
                let key = match operation_id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
                    && !operation_id.is_empty()
                {
                    true => operation_id.clone(),
                    false => quote(operation_id),
                };
                let _ = writeln!(out, "{key} = {}", quote(id));
            }
        }
        out.push('\n');
    }

    fn write_directions(&self, out: &mut String) {
        let mut reviewed = BTreeMap::<String, BTreeMap<String, OperationDirection>>::new();
        let mut holes = BTreeMap::<String, BTreeSet<String>>::new();

        for selection in &self.selections {
            let document = &self.documents[selection.document];
            for operation in self.matched(selection) {
                match self.published(document, operation) {
                    Some(published) => {
                        let previous = reviewed
                            .entry(document.service.clone())
                            .or_default()
                            .insert(operation.operation_id.clone(), published.direction);
                        assert!(
                            previous.is_none_or(|previous| previous == published.direction),
                            "one stable operation identity cannot carry two reviewed directions"
                        );
                    }
                    None => {
                        holes
                            .entry(document.service.clone())
                            .or_default()
                            .insert(operation.operation_id.clone());
                    }
                }
            }
        }

        rule(out);
        let _ = writeln!(
            out,
            "\
# Direction — connector truth keyed by stable service and vendor `operationId` (C-516).
#
# These values are copied only from operations the current connector already publishes. Method,
# path, name, description, risk and idempotency do not choose a value or a grouping membership."
        );
        rule(out);

        for document in &self.documents {
            if let Some(missing) = holes.get(&document.service) {
                let names = list(missing.iter().cloned());
                for line in wrap_comment(&format!(
                    "TODO(direction): state `read` or `write` under \
                     `[patch.directions.{}]` for {} unreviewed operation(s): {names}. The key is \
                     omitted so the loader refuses rather than inferring from HTTP method",
                    document.service,
                    missing.len()
                )) {
                    let _ = writeln!(out, "{line}");
                }
            }
            let Some(directions) = reviewed.get(&document.service) else {
                continue;
            };
            let _ = writeln!(out, "\n[patch.directions.{}]", document.service);
            for (operation_id, direction) in directions {
                let _ = writeln!(
                    out,
                    "{} = {}",
                    quote(operation_id),
                    quote(direction_word(*direction))
                );
            }
        }
        out.push('\n');
    }

    fn write_selectors(&self, out: &mut String) {
        rule(out);
        let _ = writeln!(
            out,
            "\
# Selection — {} statement(s) (C-411, C-414).
#
# Split by method class rather than by resource to make similarly transported operations compact to
# review. Direction is never emitted here; it is carried separately by stable operation identity.
#
# A prefix matches on whole segments, so `/api/v2/agents` reaches `/api/v2/agents/{{id}}` and never
# `/api/v2/agentsummary`.",
            self.selections.len()
        );
        rule(out);

        for selection in &self.selections {
            let document = &self.documents[selection.document];
            let claim = self.claim(selection);
            out.push('\n');
            let _ = writeln!(
                out,
                "# {} operation(s) of `{}`.",
                selection.matched.len(),
                document.path
            );
            match &claim {
                Claim::ReadOnly => {}
                Claim::Stated(_, _) => {
                    let _ = writeln!(
                        out,
                        "# `risk` and `idempotency` below are **read off the operations this \
                         connector already publishes**,\n# all {} of which agree. Nothing here was \
                         derived from the HTTP method.",
                        selection.matched.len()
                    );
                }
                Claim::Todo { unclaimed } => {
                    let _ = writeln!(
                        out,
                        "\
# TODO(risk, idempotency): STATE BOTH, OR THIS SELECTOR DOES NOT BUILD.
#
# {unclaimed} of the {} operations this selector matches carry no claim anybody has made — no
# OpenAPI document publishes `risk` or `idempotency`, and this connector does not publish these
# operations today. The keys are left out and the loader names every unclaimed operation.
#
#   risk = \"low\" | \"medium\" | \"high\" | \"destructive\"
#   idempotency = \"idempotent\" | \"non_idempotent\" | \"conditional\"
#
# Operations below that this connector *does* publish keep their reviewed claim in their own
# `[[patch.operations]]` block, so stating a value here is a decision about the rest.",
                        selection.matched.len()
                    );
                }
            }
            let _ = writeln!(out, "[[patch.select]]");
            if document.names_service {
                let _ = writeln!(out, "service = {}", quote(&document.service));
            }
            if let Some(prefix) = &selection.path_prefix {
                let _ = writeln!(out, "path_prefix = {}", quote(prefix));
            }
            let methods: Vec<String> = selection
                .methods
                .iter()
                .map(|method| quote(method_word(*method)))
                .collect();
            let _ = writeln!(out, "methods = [{}]", methods.join(", "));
            if let Claim::Stated(risk, idempotency) = claim {
                let _ = writeln!(out, "risk = {}", quote(risk_word(risk)));
                let _ = writeln!(
                    out,
                    "idempotency = {}",
                    quote(idempotency_word(idempotency))
                );
            }
            if let Some(expose) = self.selector_expose(selection) {
                let _ = writeln!(out, "expose = {expose}");
            }
        }
        out.push('\n');
    }

    /// The `[[services]]` the emitted file declares.
    ///
    /// **The current file's, whole, whenever it has any** — not just the ones a selector reached.
    /// A `[[config]]` field, an `[[events]]` declaration and a `[[channels]]` binding each name a
    /// service, and those are carried forward verbatim; dropping the entry they name would refuse
    /// the emitted file over a service the author never touched. A declared service that ends up
    /// with no operations is legal and reviewable; a dangling reference is neither.
    ///
    /// `roles` is the one field dropped, and it is dropped rather than carried because it is a claim
    /// the *narrowed* connector may no longer satisfy — the loader checks every role against the
    /// members that implement it, so carrying one into a smaller selection refuses the build over a
    /// line nobody wrote. Re-claiming it is a decision, and the report says so.
    ///
    /// **`tags` is carried, and the asymmetry is the role/tag distinction doing its job** (C-153). A
    /// tag is checked against nothing — it says what *kind* of thing a service is, not what it can
    /// do — so narrowing the selection cannot invalidate one. A `stripe` cut down to three operations
    /// is still `payments`, and dropping the tag would lose a true fact to protect an invariant that
    /// only `roles` has.
    fn selected_services(&self, notes: &mut Notes) -> Vec<Service> {
        if let Some(connector) = &self.existing {
            if !connector.services.is_empty() {
                return connector
                    .services
                    .iter()
                    .map(|service| {
                        if !service.roles.is_empty() {
                            notes.blocked.push(format!(
                                "service {:?} claims the role(s) {} today; the claim is not carried, \
                                 because a role is checked against the members that implement it and \
                                 this selection may hold fewer. Re-state it once you have read the \
                                 selection",
                                service.name,
                                service.roles.len()
                            ));
                        }
                        Service {
                            roles: Vec::new(),
                            ..service.clone()
                        }
                    })
                    .collect();
            }
        }

        self.selected_documents()
            .filter(|document| document.names_service)
            .map(|document| Service {
                name: document.service.clone(),
                legacy: false,
                description: document.ingested.title.clone(),
                base_url: None,
                api_version: None,
                roles: Vec::new(),
                tags: Vec::new(),
            })
            .collect()
    }

    /// The documents at least one selector reaches, in document order.
    fn selected_documents(&self) -> impl Iterator<Item = &Document> {
        self.documents
            .iter()
            .enumerate()
            .filter_map(|(index, doc)| {
                self.selections
                    .iter()
                    .any(|selection| selection.document == index)
                    .then_some(doc)
            })
    }

    /// Whether the emitted selection publishes an operation under this op id.
    fn publishes_op_id(&self, id: &str) -> bool {
        self.selections.iter().any(|selection| {
            let document = &self.documents[selection.document];
            self.matched(selection).any(|operation| {
                self.published(document, operation).map_or_else(
                    || derived_id(&self.provider, &operation.operation_id) == id,
                    |published| published.id == id,
                )
            })
        })
    }

    /// The identity a connector that does not exist yet gets: what the documents say, and holes for
    /// what they cannot.
    fn identity_from_documents(&self) -> Connector {
        let first = self.documents.first();
        Connector {
            id: self.provider.clone(),
            // Not derivable from a document and not guessable from a hostname: a vendor's display
            // name is a decision.
            vendor: "TODO".to_owned(),
            // An empty `base_url` is refused by the loader, which is the correct outcome when no
            // document declares a server — the alternative is a connector that compiles and points
            // at nothing.
            base_url: first
                .and_then(|document| document.ingested.base_url())
                .unwrap_or_default()
                .to_owned(),
            description: first
                .map(|document| document.ingested.title.clone())
                .unwrap_or_default(),
            authority: None,
            runtime: Runtime::Http,
            api_version: None,
            services: Vec::new(),
            auth: Vec::new(),
            default_auth: Vec::new(),
            operations: Vec::new(),
            events: Vec::new(),
            channels: Vec::new(),
            config: Vec::new(),
            graphs: Vec::new(),
            provenance: connector_spec::Provenance::default(),
            verify: None,
        }
    }
}

fn write_blocks(out: &mut String, blocks: &[Block]) {
    rule(out);
    let _ = writeln!(
        out,
        "\
# The exceptions — {} block(s), and every one says something a selector cannot.
#
# A block overrides a selector **field by field**: where it is silent the selector's statement
# stands. Every `risk`, `idempotency` and `expose` below was read off an operation this connector
# already publishes; none of them was derived from a method or a name.",
        blocks.len()
    );
    rule(out);

    for block in blocks {
        out.push('\n');
        for todo in &block.todos {
            for line in wrap_comment(todo) {
                let _ = writeln!(out, "{line}");
            }
        }
        let _ = writeln!(out, "[[patch.operations]]");
        if let Some(service) = &block.service {
            let _ = writeln!(out, "service = {}", quote(service));
        }
        let _ = writeln!(out, "select = {}", quote(&block.select));
        if let Some(rename) = &block.rename {
            let _ = writeln!(out, "rename = {}", quote(rename));
        }
        if let Some(risk) = block.risk {
            let _ = writeln!(out, "risk = {}", quote(risk_word(risk)));
        }
        if let Some(idempotency) = block.idempotency {
            let _ = writeln!(
                out,
                "idempotency = {}",
                quote(idempotency_word(idempotency))
            );
        }
        if let Some(expose) = block.expose {
            let _ = writeln!(out, "expose = {expose}");
        }
        if let Some(description) = &block.description {
            let _ = writeln!(out, "description = {}", quote(description));
        }
    }
    out.push('\n');
}

/// The report, and the reason this command is trustworthy rather than merely convenient.
fn write_notes(out: &mut String, notes: &Notes) {
    let total = notes.dropped.len()
        + notes.lesser.len()
        + notes.nameless.len()
        + notes.collisions.len()
        + notes.unmatched.len()
        + notes.blocked.len()
        + notes.auth_flow.len();

    rule(out);
    let _ = writeln!(
        out,
        "\
# What this scaffold could not carry — {total} item(s).
#
# A dropped operation that produces no output reads as \"the vendor does not offer that\", so every
# one of them is named. Delete this block when you have read it; nothing above depends on it."
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "#   {:>5}  operation(s) the document declares that this pipeline could not read at all",
        notes.dropped.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  narrower problem(s) in a document that did not cost the operation",
        notes.lesser.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  selected operation(s) with no description — a tool contract with no sentence",
        notes.nameless.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  op id collision(s) only a human can break",
        notes.collisions.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  statement(s) or document(s) that selected nothing and were not emitted",
        notes.unmatched.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  declaration(s) this run could not carry at all",
        notes.blocked.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  authentication-flow endpoint(s) withheld, or ambiguous enough to need a look",
        notes.auth_flow.len()
    );
    let _ = writeln!(
        out,
        "#   {:>5}  selected operation(s) this connector does not publish today, which reach a model\n\
         #          as tools unless a selector or a block states `expose = false` (C-413)",
        notes.newly_exposed
    );
    rule(out);

    for (heading, items) in [
        ("not carried, and why", &notes.blocked),
        ("authentication flow, never an operation", &notes.auth_flow),
        ("dropped by ingest", &notes.dropped),
        ("no description", &notes.nameless),
        ("op id collision", &notes.collisions),
        ("selected nothing", &notes.unmatched),
        ("reported, operation kept", &notes.lesser),
    ] {
        if items.is_empty() {
            continue;
        }
        let _ = writeln!(out, "#\n# --- {heading} ({}) ---", items.len());
        for item in items {
            for line in wrap_comment(item) {
                let _ = writeln!(out, "{line}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// `--diff`
// ---------------------------------------------------------------------------------------------

impl Plan {
    fn to_diff(&self) -> Result<String> {
        let mut out = String::new();
        let published = self
            .existing
            .as_ref()
            .map_or(0, |connector| connector.operations.len());
        let _ = writeln!(
            out,
            "{}: {} vendored document(s) against {published} published operation(s)\n",
            self.provider,
            self.documents.len()
        );

        let (mut added, mut removed, mut changed, mut unchanged) = (0, 0, 0, 0);
        for document in &self.documents {
            let _ = writeln!(out, "{} — {}", document.service, document.path);

            let mut document_added = Vec::new();
            let mut document_changed = Vec::new();
            for operation in &document.ingested.operations {
                match self.published(document, operation) {
                    None => document_added.push(format!(
                        "    {:<7} {}  ({})",
                        method_word(operation.method),
                        operation.path,
                        operation.operation_id
                    )),
                    Some(publication) => {
                        let drift = self.drift(document, publication, operation);
                        if drift.is_empty() {
                            unchanged += 1;
                        } else {
                            document_changed.push(format!(
                                "    {:<7} {}  ({}) — {}",
                                method_word(operation.method),
                                operation.path,
                                publication.id,
                                drift.join("; ")
                            ));
                        }
                    }
                }
            }

            let mut document_removed = Vec::new();
            if let Some(connector) = &self.existing {
                for publication in &connector.operations {
                    if publication.service != document.service {
                        continue;
                    }
                    let still_there = document.ingested.operations.iter().any(|operation| {
                        operation.method == publication.method && operation.path == publication.path
                    });
                    if !still_there {
                        document_removed.push(format!(
                            "    {:<7} {}  ({})",
                            method_word(publication.method),
                            publication.path,
                            publication.id
                        ));
                    }
                }
            }

            added += document_added.len();
            removed += document_removed.len();
            changed += document_changed.len();

            let _ = writeln!(
                out,
                "  added     {:>4}   upstream declares it, this connector does not publish it",
                document_added.len()
            );
            for line in &document_added {
                let _ = writeln!(out, "{line}");
            }
            let _ = writeln!(
                out,
                "  removed   {:>4}   this connector publishes it, no document declares it",
                document_removed.len()
            );
            for line in &document_removed {
                let _ = writeln!(out, "{line}");
            }
            let _ = writeln!(
                out,
                "  changed   {:>4}   published, and the document moved underneath it",
                document_changed.len()
            );
            for line in &document_changed {
                let _ = writeln!(out, "{line}");
            }
            out.push('\n');
        }

        let _ = writeln!(
            out,
            "{added} added, {removed} removed, {changed} changed, {unchanged} unchanged"
        );
        if self.existing.is_none() {
            let _ = writeln!(
                out,
                "\nNothing is published yet, so every operation is `added`. Run `scaffold` without \
                 `--diff` to write the file that would publish them."
            );
        }
        Ok(out)
    }

    /// How one published operation and its document entry disagree.
    ///
    /// Parameters and deprecation only, deliberately. A description is overridden on purpose all the
    /// time — nine of babelforce's do — so reporting one as drift would bury the report in decisions
    /// somebody already made. A **parameter** is different: it is what a caller passes, and one that
    /// appeared upstream is an argument this connector does not offer.
    fn drift(
        &self,
        document: &Document,
        publication: &Operation,
        operation: &SpecOperation,
    ) -> Vec<String> {
        let omitted = self
            .omitted
            .get(&(document.service.clone(), operation.operation_id.clone()));
        let declared = param_names(&operation.params);
        let published = param_names(&publication.params);

        let mut drift = Vec::new();
        let missing: Vec<String> = declared
            .difference(&published)
            // A parameter the file *says* it drops is a decision somebody wrote down (C-422), not
            // upstream drift, so subtracting it is what keeps this report about the vendor.
            .filter(|entry| omitted.is_none_or(|omitted| !omitted.contains(*entry)))
            .map(|(position, name)| format!("{position} `{name}`"))
            .collect();
        if !missing.is_empty() {
            drift.push(format!(
                "declared upstream, not published: {}",
                missing.join(", ")
            ));
        }
        let extra: Vec<String> = published
            .difference(&declared)
            .map(|(position, name)| format!("{position} `{name}`"))
            .collect();
        if !extra.is_empty() {
            drift.push(format!(
                "published, no longer declared: {}",
                extra.join(", ")
            ));
        }
        if operation.deprecated {
            drift.push("the vendor marks it deprecated".to_owned());
        }
        drift
    }
}

/// Every parameter of a request, as the pair that identifies one: position **and** name, because a
/// vendor may bind one name in two places.
fn param_names(params: &connector_spec::ParamSet) -> BTreeSet<(&'static str, String)> {
    let mut names = BTreeSet::new();
    for (position, group) in [
        (ParamPosition::Path, &params.path),
        (ParamPosition::Query, &params.query),
        (ParamPosition::Header, &params.header),
        (ParamPosition::Body, &params.body),
    ] {
        for param in group {
            names.insert((position_word(position), param.name.clone()));
        }
    }
    names
}

// ---------------------------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------------------------

/// A TOML basic string. JSON's escaping is a subset of TOML's for every value this emits, and
/// `serde_json` is already in this crate's closure — so the alternative would be a hand-rolled
/// escaper, which is the thing most worth not hand-rolling.
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

fn rule(out: &mut String) {
    out.push_str(
        "# ---------------------------------------------------------------------------------------------\n",
    );
}

/// One report item as `#`-prefixed comment lines, wrapped so the emitted file stays readable at the
/// width every other comment in `providers/` is written to.
fn wrap_comment(text: &str) -> Vec<String> {
    const WIDTH: usize = 96;
    let mut lines = Vec::new();
    let mut current = String::from("#   ");
    for word in text.split_whitespace() {
        if current.chars().count() + 1 + word.chars().count() > WIDTH && current.trim() != "#" {
            lines.push(std::mem::replace(&mut current, "#     ".to_owned()));
        }
        if !current.ends_with(' ') {
            current.push(' ');
        }
        current.push_str(word);
    }
    if current.trim() != "#" {
        lines.push(current);
    }
    lines
}

/// An error rendered onto one line, so a report item stays one item.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn list(values: impl Iterator<Item = String>) -> String {
    let values: Vec<String> = values.map(|value| format!("{value:?}")).collect();
    match values.is_empty() {
        true => "no service".to_owned(),
        false => values.join(", "),
    }
}

fn describe_selector(path_prefix: Option<&str>, methods: &[HttpMethod]) -> String {
    let words: Vec<&str> = methods.iter().copied().map(method_word).collect();
    match path_prefix {
        Some(prefix) => format!("`path_prefix = {prefix:?}, methods = {words:?}`"),
        None => format!("`methods = {words:?}`"),
    }
}

fn method_word(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Head => "HEAD",
        HttpMethod::Options => "OPTIONS",
    }
}

fn method_of(word: &str) -> Option<HttpMethod> {
    Some(match word.trim().to_ascii_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => return None,
    })
}

fn risk_word(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "low",
        Risk::Medium => "medium",
        Risk::High => "high",
        Risk::Destructive => "destructive",
    }
}

fn direction_word(direction: OperationDirection) -> &'static str {
    direction.word()
}

fn idempotency_word(idempotency: Idempotency) -> &'static str {
    match idempotency {
        Idempotency::Idempotent => "idempotent",
        Idempotency::NonIdempotent => "non_idempotent",
        Idempotency::Conditional => "conditional",
    }
}

fn position_word(position: ParamPosition) -> &'static str {
    match position {
        ParamPosition::Path => "path",
        ParamPosition::Query => "query",
        ParamPosition::Header => "header",
        ParamPosition::Body => "body",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_select_argument_drops_fields_from_the_right() {
        let whole = parse_select("manager").unwrap();
        assert_eq!(whole.service.as_deref(), Some("manager"));
        assert_eq!(whole.path_prefix, None);
        assert!(whole.methods.is_empty());

        let full = parse_select("manager:/api/v2:GET,DELETE").unwrap();
        assert_eq!(full.path_prefix.as_deref(), Some("/api/v2"));
        assert_eq!(full.methods, vec![HttpMethod::Get, HttpMethod::Delete]);

        // An empty field states nothing, which is C-411's absent key rather than a wildcard this
        // grammar had to invent.
        let unqualified = parse_select(":/api/v2:GET").unwrap();
        assert_eq!(unqualified.service, None);
        assert_eq!(unqualified.path_prefix.as_deref(), Some("/api/v2"));
    }

    #[test]
    fn a_select_argument_refuses_what_it_cannot_mean() {
        assert!(parse_select("a:b:c:d").is_err());
        assert!(parse_select("manager:/api/v2:FETCH").is_err());
    }

    #[test]
    fn a_prefix_matches_whole_segments() {
        assert!(path_has_prefix(
            "/api/v2/agents/{id}",
            Some("/api/v2/agents")
        ));
        assert!(path_has_prefix("/api/v2/agents", Some("/api/v2/agents")));
        assert!(!path_has_prefix(
            "/api/v2/agentsummary",
            Some("/api/v2/agents")
        ));
        assert!(path_has_prefix("/anything", None));
    }

    #[test]
    fn a_service_name_is_cut_at_the_pull_date() {
        assert_eq!(
            service_name_of("specs/babelforce/manager-2026-07-10.openapi.yaml"),
            "manager"
        );
        assert_eq!(
            service_name_of("specs/babelforce/task-automation-2026-06-25.openapi.yaml"),
            "task-automation"
        );
        assert_eq!(service_name_of("specs/zendesk/support.json"), "support");
    }
}

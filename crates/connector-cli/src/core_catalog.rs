//! Validation and publication of Flux's vendored core-catalogue projection.
//!
//! Flux owns these records. This crate checks and republishes the inert JSON; it does not register,
//! execute, reinterpret, or mirror the built-in operations in `connector-catalog`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifact;
use crate::workspace::Workspace;

const VERSION: u32 = 1;
const HOST_PREFIX: &str = "https://flux.codewandler.org/";
const CORE_PREFIX: &str = "https://flux.codewandler.org/v1/core/";
const ENTRY_SCHEMA: &str = "https://flux.codewandler.org/v1/schema/core-entry.schema.json";
const CATALOG_SCHEMA: &str = "https://flux.codewandler.org/v1/schema/core-catalog.schema.json";
const AST_SCHEMA: &str = "https://flux.codewandler.org/v1/schema/flux-ast.schema.json";
const CATALOG_ID: &str = "https://flux.codewandler.org/v1/core/index.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Planned,
}

impl Availability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Planned => "planned",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Operation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Capability,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreOperation {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "$id")]
    pub id: String,
    pub schema_version: u32,
    pub kind: OperationKind,
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: Vec<String>,
    pub availability: Availability,
    pub tool_spec: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreNode {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "$id")]
    pub id: String,
    pub schema_version: u32,
    pub kind: NodeKind,
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: Vec<String>,
    pub availability: Availability,
    pub schema_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCapability {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "$id")]
    pub id: String,
    pub schema_version: u32,
    pub kind: CapabilityKind,
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: Vec<String>,
    pub availability: Availability,
    pub callable: bool,
    pub operation_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum CoreEntry {
    Operation(CoreOperation),
    Node(CoreNode),
    Capability(CoreCapability),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreSchemas {
    pub catalog: Value,
    pub entry: Value,
    pub flux_ast: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreCatalog {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "$id")]
    pub id: String,
    pub schema_version: u32,
    pub generator: String,
    pub operations: Vec<CoreOperation>,
    pub nodes: Vec<CoreNode>,
    pub capabilities: Vec<CoreCapability>,
    pub schemas: CoreSchemas,
}

/// Read the optional snapshot. Test fixtures without it still build a connector-only catalogue;
/// the real repository commits it and therefore always takes the validated path.
pub fn read_optional(workspace: &Workspace) -> Result<Option<CoreCatalog>> {
    let path = workspace.core_catalog_source_path();
    artifact::read_if_exists(&path)?
        .map(|source| {
            parse(&source).with_context(|| {
                format!(
                    "vendored Flux core catalogue `{}`",
                    workspace.display_path(&path).display()
                )
            })
        })
        .transpose()
}

/// Parse, schema-check and validate the cross-record invariants of a Flux core bundle.
pub fn parse(source: &str) -> Result<CoreCatalog> {
    let raw: Value = serde_json::from_str(source).context("core catalogue is not JSON")?;
    let catalog: CoreCatalog = serde_json::from_value(raw.clone())
        .context("core catalogue does not match its versioned wire shape")?;

    if catalog.schema_version != VERSION {
        bail!(
            "unsupported core catalogue schema_version {} (expected {VERSION})",
            catalog.schema_version
        );
    }
    if catalog.schema != CATALOG_SCHEMA || catalog.id != CATALOG_ID {
        bail!("core catalogue has a non-canonical schema or `$id`");
    }
    expect_schema_id(&catalog.schemas.catalog, CATALOG_SCHEMA, "catalog")?;
    expect_schema_id(&catalog.schemas.entry, ENTRY_SCHEMA, "entry")?;
    expect_schema_id(&catalog.schemas.flux_ast, AST_SCHEMA, "Flux AST")?;

    validate_json(&catalog.schemas.catalog, &raw, "core catalogue")?;
    // Compile the other two schemas even before an instance uses them. A malformed committed schema
    // is a build failure, not a broken URL published successfully.
    jsonschema::validator_for(&catalog.schemas.entry).context("invalid core-entry JSON Schema")?;
    jsonschema::validator_for(&catalog.schemas.flux_ast).context("invalid Flux AST JSON Schema")?;

    let mut ids = BTreeSet::new();
    let mut operation_ids = BTreeSet::new();
    for operation in &catalog.operations {
        validate_entry_common(
            &operation.id,
            &operation.schema,
            operation.schema_version,
            &operation.name,
            &operation.category,
            &mut ids,
        )?;
        let spec_name = operation.tool_spec["name"]
            .as_str()
            .context("core operation ToolSpec has no string `name`")?;
        if spec_name != operation.name {
            bail!(
                "core operation `{}` disagrees with ToolSpec name `{spec_name}`",
                operation.name
            );
        }
        for field in [
            "description",
            "input_schema",
            "effects",
            "risk",
            "idempotency",
            "access",
        ] {
            if operation.tool_spec.get(field).is_none() {
                bail!(
                    "core operation `{}` ToolSpec lacks `{field}`",
                    operation.name
                );
            }
        }
        validate_json(
            &catalog.schemas.entry,
            &serde_json::to_value(CoreEntry::Operation(operation.clone()))?,
            &format!("core operation `{}`", operation.name),
        )?;
        operation_ids.insert(operation.id.as_str());
    }

    let anchors = schema_anchors(&catalog.schemas.flux_ast);
    for node in &catalog.nodes {
        validate_entry_common(
            &node.id,
            &node.schema,
            node.schema_version,
            &node.name,
            &node.category,
            &mut ids,
        )?;
        let (schema_id, anchor) = node
            .schema_ref
            .split_once('#')
            .with_context(|| format!("node `{}` schema_ref has no anchor", node.name))?;
        if schema_id != AST_SCHEMA {
            bail!("node `{}` has a non-canonical schema_ref", node.name);
        }
        if !anchors.contains(anchor) {
            bail!(
                "node `{}` references missing Flux AST anchor `{anchor}`",
                node.name
            );
        }
        let expected_anchor = format!("node-{}", node.name);
        if anchor != expected_anchor {
            bail!("node `{}` has a non-canonical schema_ref anchor", node.name);
        }
        validate_json(
            &catalog.schemas.entry,
            &serde_json::to_value(CoreEntry::Node(node.clone()))?,
            &format!("core node `{}`", node.name),
        )?;
    }

    for capability in &catalog.capabilities {
        validate_entry_common(
            &capability.id,
            &capability.schema,
            capability.schema_version,
            &capability.name,
            &capability.category,
            &mut ids,
        )?;
        match capability.availability {
            Availability::Available
                if !capability.callable || capability.operation_ids.is_empty() =>
            {
                bail!(
                    "available capability `{}` has no callable operation",
                    capability.name
                )
            }
            Availability::Planned
                if capability.callable || !capability.operation_ids.is_empty() =>
            {
                bail!(
                    "planned capability `{}` must not be callable or name operations",
                    capability.name
                )
            }
            _ => {}
        }
        for id in &capability.operation_ids {
            if !operation_ids.contains(id.as_str()) {
                bail!(
                    "capability `{}` references missing operation `{id}`",
                    capability.name
                );
            }
        }
        validate_json(
            &catalog.schemas.entry,
            &serde_json::to_value(CoreEntry::Capability(capability.clone()))?,
            &format!("core capability `{}`", capability.name),
        )?;
    }

    Ok(catalog)
}

/// `web/public/v<VERSION>` — the directory this module publishes into, and nothing else does.
///
/// The **artifact root** of the core-catalogue family (C-429). One entry per record in the vendored
/// snapshot, so the set shrinks whenever Flux retires a record; the whole point of a root is that
/// the retired record's document is then reported rather than served forever. [`public_path`]
/// refuses an `$id` that would land outside it, which is what makes the root exhaustive rather than
/// merely usual.
pub fn public_root(workspace: &Workspace) -> PathBuf {
    workspace.site_public_path(format!("v{VERSION}"))
}

/// The public index, per-entry documents and their validating schemas.
pub fn public_artifacts(
    workspace: &Workspace,
    catalog: &CoreCatalog,
) -> Result<Vec<(PathBuf, String)>> {
    let mut artifacts = vec![
        document_artifact(workspace, &catalog.id, catalog)?,
        schema_artifact(workspace, CATALOG_SCHEMA, &catalog.schemas.catalog)?,
        schema_artifact(workspace, ENTRY_SCHEMA, &catalog.schemas.entry)?,
        schema_artifact(workspace, AST_SCHEMA, &catalog.schemas.flux_ast)?,
    ];
    for operation in &catalog.operations {
        artifacts.push(document_artifact(workspace, &operation.id, operation)?);
    }
    for node in &catalog.nodes {
        artifacts.push(document_artifact(workspace, &node.id, node)?);
    }
    for capability in &catalog.capabilities {
        artifacts.push(document_artifact(workspace, &capability.id, capability)?);
    }
    artifacts.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(artifacts)
}

fn document_artifact<T: Serialize>(
    workspace: &Workspace,
    id: &str,
    document: &T,
) -> Result<(PathBuf, String)> {
    Ok((
        public_path(workspace, id)?,
        format!("{}\n", serde_json::to_string_pretty(document)?),
    ))
}

fn schema_artifact(workspace: &Workspace, id: &str, schema: &Value) -> Result<(PathBuf, String)> {
    document_artifact(workspace, id, schema)
}

fn public_path(workspace: &Workspace, id: &str) -> Result<PathBuf> {
    let relative = id.strip_prefix(HOST_PREFIX).with_context(|| {
        format!("public specification id `{id}` is not on flux.codewandler.org")
    })?;
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains("//")
        || relative.contains("..")
        || relative.contains(['\\', '?', '#'])
    {
        bail!("public specification id `{id}` cannot map to a safe output path");
    }
    // Inside the versioned root or nowhere: [`public_root`] is what `build` scans for documents no
    // snapshot claims any more, and a record published beside it rather than under it would be
    // outside every root and so unremovable by the only mechanism that looks.
    if !relative.starts_with(&format!("v{VERSION}/")) {
        bail!("public specification id `{id}` is outside the published v{VERSION} tree");
    }
    Ok(workspace.site_public_path(relative))
}

fn validate_entry_common<'a>(
    id: &'a str,
    schema: &str,
    version: u32,
    name: &str,
    category: &[String],
    ids: &mut BTreeSet<&'a str>,
) -> Result<()> {
    if schema != ENTRY_SCHEMA || version != VERSION {
        bail!("core entry `{name}` has the wrong schema contract");
    }
    if category.is_empty()
        || category
            .iter()
            .any(|part| part.is_empty() || part.contains(['/', '.', '\\']))
    {
        bail!("core entry `{name}` has an invalid category path");
    }
    let leaf = name.rsplit('.').next().unwrap_or(name);
    let expected = format!("{CORE_PREFIX}{}/{leaf}.json", category.join("/"));
    if !ids.insert(id) {
        bail!("duplicate core entry `$id` `{id}`");
    }
    if id != expected {
        bail!("core entry `{name}` has non-canonical `$id` `{id}`; expected `{expected}`");
    }
    Ok(())
}

fn expect_schema_id(schema: &Value, expected: &str, label: &str) -> Result<()> {
    if schema["$id"] != expected {
        bail!("{label} JSON Schema has non-canonical `$id`");
    }
    Ok(())
}

fn validate_json(schema: &Value, instance: &Value, label: &str) -> Result<()> {
    let validator = jsonschema::validator_for(schema)
        .with_context(|| format!("invalid JSON Schema for {label}"))?;
    if let Err(error) = validator.validate(instance) {
        bail!("{label} does not validate: {error}");
    }
    Ok(())
}

fn schema_anchors(schema: &Value) -> BTreeSet<&str> {
    fn walk<'a>(value: &'a Value, into: &mut BTreeSet<&'a str>) {
        match value {
            Value::Object(object) => {
                if let Some(anchor) = object.get("$anchor").and_then(Value::as_str) {
                    into.insert(anchor);
                }
                for value in object.values() {
                    walk(value, into);
                }
            }
            Value::Array(values) => {
                for value in values {
                    walk(value, into);
                }
            }
            _ => {}
        }
    }
    let mut anchors = BTreeSet::new();
    walk(schema, &mut anchors);
    anchors
}

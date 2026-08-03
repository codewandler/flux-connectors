//! The native-plugin migration inventory and conformance ratchet (C-505).
//!
//! This module reads committed bytes only. It never starts a plugin, contacts Exchange, or reaches
//! the network: wave-owned harnesses capture both observations and this code compares them. The
//! release check is deliberately cross-repository but not cross-crate — callers pass an explicit
//! Flux checkout, which keeps the compiler dependency graph hermetic.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

const INVENTORY_PATH: &str = "migration/native-plugins.toml";
const INVENTORY_SCHEMA: u32 = 1;
const CONFORMANCE_FORMAT: &str = "flux-connectors-conformance/v1";
const CONFORMANCE_SCHEMA: &str = include_str!("../../../migration/conformance-v1.schema.json");
const PUBLICATION_FORMAT: &str = "flux-connectors-publication/v1";
const WAVES: [&str; 5] = ["C-499", "C-500", "C-501", "C-502", "C-503"];

/// The retained inventory. Adapter rows are tombstones: a migrated adapter stays here after its
/// Flux manifest disappears.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    /// Inventory format version.
    pub schema: u32,
    /// Fixed migration-wave order.
    pub waves: Vec<String>,
    /// Official integration adapters.
    pub adapters: Vec<Adapter>,
    /// Plugin-workspace members that support distribution or execution but are not integrations.
    #[serde(default)]
    pub support: Vec<SupportCrate>,
}

/// One official Flux integration and its connector-owned migration destination.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adapter {
    /// Stable legacy adapter id.
    pub id: String,
    /// Flux-repository-relative Cargo manifest.
    pub flux_manifest: PathBuf,
    /// The packaged `flux-plugin-*` binary target.
    pub flux_binary: String,
    /// Flux-repository-relative source that constructs the legacy public contract.
    pub legacy_contract: PathBuf,
    /// Connector id replacing this adapter.
    pub connector: String,
    /// The one fixed migration wave that owns it.
    pub wave: String,
    /// Connector-repository-relative retained conformance evidence.
    pub conformance: PathBuf,
    /// Connector-repository-relative immutable publication receipt.
    pub publication: PathBuf,
}

/// A Flux plugin-workspace member that is deliberately not an official integration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportCrate {
    /// Stable workspace-member id.
    pub id: String,
    /// Flux-repository-relative Cargo manifest.
    pub flux_manifest: PathBuf,
    /// Cargo package name.
    pub package: String,
    /// Exact `bin:<name>` / `lib:<name>` target set.
    pub targets: Vec<String>,
    /// Human-readable reason this is support rather than an integration.
    pub role: String,
}

/// The cross-repository checklist result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    /// Retained adapter rows.
    pub inventoried: usize,
    /// Adapter manifests still present in Flux.
    pub legacy_present: usize,
    /// Missing adapter manifests admitted by both evidence gates.
    pub retired_with_evidence: usize,
    /// Live support/distribution crates classified separately.
    pub support_present: usize,
}

/// Load and validate `<connectors>/migration/native-plugins.toml`.
pub fn load_inventory(connectors_root: &Path) -> Result<Inventory> {
    let path = connectors_root.join(INVENTORY_PATH);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("cannot read native-plugin inventory `{}`", path.display()))?;
    let inventory: Inventory = toml::from_str(&text)
        .with_context(|| format!("invalid native-plugin inventory `{}`", path.display()))?;
    validate_inventory(&inventory)?;
    Ok(inventory)
}

fn validate_inventory(inventory: &Inventory) -> Result<()> {
    if inventory.schema != INVENTORY_SCHEMA {
        bail!(
            "native-plugin inventory schema {} is unsupported; expected {INVENTORY_SCHEMA}",
            inventory.schema
        );
    }
    if inventory
        .waves
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        != WAVES
    {
        bail!(
            "native-plugin waves must remain in fixed order: {}",
            WAVES.join(", ")
        );
    }
    if inventory.adapters.is_empty() {
        bail!("native-plugin inventory declares no adapters");
    }

    let mut ids = BTreeSet::new();
    let mut manifests = BTreeSet::new();
    let mut binaries = BTreeSet::new();
    for adapter in &inventory.adapters {
        require_identifier("adapter id", &adapter.id)?;
        require_identifier("connector id", &adapter.connector)?;
        if !ids.insert(adapter.id.as_str()) {
            bail!(
                "native-plugin adapter `{}` is declared more than once",
                adapter.id
            );
        }
        require_relative("flux_manifest", &adapter.flux_manifest)?;
        require_relative("legacy_contract", &adapter.legacy_contract)?;
        require_relative("conformance", &adapter.conformance)?;
        require_relative("publication", &adapter.publication)?;
        if !manifests.insert(&adapter.flux_manifest) {
            bail!(
                "Flux manifest `{}` is assigned to more than one adapter",
                adapter.flux_manifest.display()
            );
        }
        if !adapter.flux_binary.starts_with("flux-plugin-") {
            bail!(
                "adapter `{}` binary `{}` is not a packaged `flux-plugin-*` target",
                adapter.id,
                adapter.flux_binary
            );
        }
        if !binaries.insert(adapter.flux_binary.as_str()) {
            bail!(
                "Flux binary `{}` is assigned more than once",
                adapter.flux_binary
            );
        }
        if !WAVES.contains(&adapter.wave.as_str()) {
            bail!(
                "adapter `{}` names unknown migration wave `{}`",
                adapter.id,
                adapter.wave
            );
        }
        let expected_manifest = PathBuf::from(format!("plugins/{}/Cargo.toml", adapter.id));
        if adapter.flux_manifest != expected_manifest {
            bail!(
                "adapter `{}` manifest must be `{}`, got `{}`",
                adapter.id,
                expected_manifest.display(),
                adapter.flux_manifest.display()
            );
        }
        let expected_conformance =
            PathBuf::from(format!("migration/conformance/{}.json", adapter.id));
        if adapter.conformance != expected_conformance {
            bail!(
                "adapter `{}` conformance path must be `{}`",
                adapter.id,
                expected_conformance.display()
            );
        }
        let expected_publication =
            PathBuf::from(format!("migration/publications/{}.json", adapter.id));
        if adapter.publication != expected_publication {
            bail!(
                "adapter `{}` publication path must be `{}`",
                adapter.id,
                expected_publication.display()
            );
        }
    }

    for support in &inventory.support {
        require_identifier("support id", &support.id)?;
        if !ids.insert(support.id.as_str()) {
            bail!(
                "workspace member `{}` is classified more than once",
                support.id
            );
        }
        require_relative("support flux_manifest", &support.flux_manifest)?;
        if !manifests.insert(&support.flux_manifest) {
            bail!(
                "Flux manifest `{}` is classified more than once",
                support.flux_manifest.display()
            );
        }
        if support.package.trim().is_empty()
            || support.role.trim().is_empty()
            || support.targets.is_empty()
        {
            bail!(
                "support crate `{}` has an incomplete classification",
                support.id
            );
        }
        if support
            .targets
            .iter()
            .any(|target| !target.starts_with("bin:") && !target.starts_with("lib:"))
        {
            bail!(
                "support crate `{}` targets must use `bin:` or `lib:` prefixes",
                support.id
            );
        }
    }
    Ok(())
}

fn require_identifier(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("{label} `{value}` must use lowercase ASCII letters, digits and `-`");
    }
    Ok(())
}

fn require_relative(label: &str, path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        bail!(
            "{label} `{}` must be a contained relative path",
            path.display()
        );
    }
    Ok(())
}

/// Run the offline cross-repository release checklist.
pub fn check(connectors_root: &Path, flux_root: &Path) -> Result<Report> {
    let inventory = load_inventory(connectors_root)?;
    let live = load_flux_workspace(flux_root)?;

    let adapters_by_manifest = inventory
        .adapters
        .iter()
        .map(|adapter| (adapter.flux_manifest.as_path(), adapter))
        .collect::<BTreeMap<_, _>>();
    let support_by_manifest = inventory
        .support
        .iter()
        .map(|support| (support.flux_manifest.as_path(), support))
        .collect::<BTreeMap<_, _>>();

    let mut live_adapter_ids = BTreeSet::new();
    let mut live_support_ids = BTreeSet::new();
    for member in &live {
        if member.plugin_binary.is_some() {
            let adapter = adapters_by_manifest
                .get(member.manifest.as_path())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Flux integration `{}` at `{}` is absent from the native-plugin inventory",
                        member.id,
                        member.manifest.display()
                    )
                })?;
            if member.plugin_binary.as_deref() != Some(adapter.flux_binary.as_str()) {
                bail!(
                    "Flux integration `{}` target changed: inventory `{}`, live `{}`",
                    adapter.id,
                    adapter.flux_binary,
                    member.plugin_binary.as_deref().unwrap_or("<none>")
                );
            }
            if !live_adapter_ids.insert(adapter.id.as_str()) {
                bail!("Flux integration `{}` appears more than once", adapter.id);
            }
        } else {
            let support = support_by_manifest
                .get(member.manifest.as_path())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                    "Flux support/distribution member `{}` at `{}` is absent from the inventory",
                    member.id,
                    member.manifest.display()
                )
                })?;
            if support.package != member.package || support.targets != member.targets {
                bail!(
                    "Flux support crate `{}` changed: inventory package/targets `{} {:?}`, live `{} {:?}`",
                    support.id,
                    support.package,
                    support.targets,
                    member.package,
                    member.targets
                );
            }
            live_support_ids.insert(support.id.as_str());
        }
    }

    for support in &inventory.support {
        if !live_support_ids.contains(support.id.as_str()) {
            bail!(
                "Flux support crate `{}` disappeared; support/distribution removal belongs to the final Flux cleanup, not an adapter wave",
                support.id
            );
        }
    }

    let mut retired_with_evidence = 0;
    for adapter in &inventory.adapters {
        let conformance_path = connectors_root.join(&adapter.conformance);
        let publication_path = connectors_root.join(&adapter.publication);
        let evidence = conformance_path
            .is_file()
            .then(|| load_conformance(&conformance_path))
            .transpose()?;
        if let Some(document) = &evidence {
            ensure_document_identity(adapter, document)?;
        }
        let publication = publication_path
            .is_file()
            .then(|| load_publication(&publication_path))
            .transpose()?;
        if let Some(receipt) = &publication {
            ensure_publication_identity(adapter, receipt)?;
            if !matches!(
                evidence.as_ref().map(conformance_verdict),
                Some(Verdict::Conformant)
            ) {
                bail!(
                    "adapter `{}` has a publication receipt but its computed conformance verdict is not conformant",
                    adapter.id
                );
            }
        }

        if live_adapter_ids.contains(adapter.id.as_str()) {
            let legacy_contract = flux_root.join(&adapter.legacy_contract);
            if !legacy_contract.is_file() {
                bail!(
                    "Flux adapter `{}` is present but its declared contract source `{}` is missing",
                    adapter.id,
                    adapter.legacy_contract.display()
                );
            }
            continue;
        }

        let Some(document) = evidence else {
            bail!(
                "Flux adapter `{}` disappeared before conformance evidence `{}` existed",
                adapter.id,
                adapter.conformance.display()
            );
        };
        match conformance_verdict(&document) {
            Verdict::Conformant => {}
            verdict => bail!(
                "Flux adapter `{}` disappeared with nonconformant evidence: {verdict:?}",
                adapter.id
            ),
        }
        if publication.is_none() {
            bail!(
                "Flux adapter `{}` disappeared before publication receipt `{}` existed",
                adapter.id,
                adapter.publication.display()
            );
        }
        retired_with_evidence += 1;
    }

    Ok(Report {
        inventoried: inventory.adapters.len(),
        legacy_present: live_adapter_ids.len(),
        retired_with_evidence,
        support_present: live_support_ids.len(),
    })
}

#[derive(Debug)]
struct LiveMember {
    id: String,
    manifest: PathBuf,
    package: String,
    targets: Vec<String>,
    plugin_binary: Option<String>,
}

fn load_flux_workspace(flux_root: &Path) -> Result<Vec<LiveMember>> {
    let workspace_path = flux_root.join("plugins/Cargo.toml");
    let workspace_text = fs::read_to_string(&workspace_path).with_context(|| {
        format!(
            "cannot read Flux plugin workspace `{}`",
            workspace_path.display()
        )
    })?;
    let workspace: toml::Value = workspace_text.parse().with_context(|| {
        format!(
            "invalid Flux plugin workspace `{}`",
            workspace_path.display()
        )
    })?;
    let members = workspace
        .get("workspace")
        .and_then(|value| value.get("members"))
        .and_then(toml::Value::as_array)
        .context("Flux plugins/Cargo.toml has no [workspace].members array")?;

    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in members {
        let id = value
            .as_str()
            .context("Flux plugin workspace member is not a string")?;
        require_identifier("Flux plugin workspace member", id)?;
        if !seen.insert(id) {
            bail!("Flux plugin workspace member `{id}` is repeated");
        }
        let manifest = PathBuf::from(format!("plugins/{id}/Cargo.toml"));
        let path = flux_root.join(&manifest);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("cannot read Flux member manifest `{}`", path.display()))?;
        let parsed: toml::Value = text
            .parse()
            .with_context(|| format!("invalid Flux member manifest `{}`", path.display()))?;
        let package = parsed
            .get("package")
            .and_then(|value| value.get("name"))
            .and_then(toml::Value::as_str)
            .with_context(|| format!("Flux member `{id}` has no [package].name"))?
            .to_owned();

        let mut targets = Vec::new();
        if let Some(bins) = parsed.get("bin").and_then(toml::Value::as_array) {
            for bin in bins {
                let name = bin
                    .get("name")
                    .and_then(toml::Value::as_str)
                    .with_context(|| format!("Flux member `{id}` has a [[bin]] without a name"))?;
                targets.push(format!("bin:{name}"));
            }
        }
        let autobins = parsed
            .get("package")
            .and_then(|value| value.get("autobins"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        if targets.iter().all(|target| !target.starts_with("bin:"))
            && autobins
            && path
                .parent()
                .is_some_and(|parent| parent.join("src/main.rs").is_file())
        {
            // Cargo's implicit binary target is the package name. Vault and 1Password use this
            // spelling, so classifying only explicit `[[bin]]` tables silently calls two official
            // integrations support crates.
            targets.push(format!("bin:{package}"));
        }
        if let Some(lib) = parsed.get("lib") {
            let name = lib
                .get("name")
                .and_then(toml::Value::as_str)
                .unwrap_or(&package);
            targets.push(format!("lib:{name}"));
        }
        targets.sort();
        let plugin_binaries = targets
            .iter()
            .filter_map(|target| target.strip_prefix("bin:flux-plugin-"))
            .collect::<Vec<_>>();
        if plugin_binaries.len() > 1 {
            bail!("Flux member `{id}` exposes more than one `flux-plugin-*` binary");
        }
        let plugin_binary = plugin_binaries
            .first()
            .map(|suffix| format!("flux-plugin-{suffix}"));
        result.push(LiveMember {
            id: id.to_owned(),
            manifest,
            package,
            targets,
            plugin_binary,
        });
    }
    Ok(result)
}

/// A frozen connector migration contract plus captured legacy and Exchange observations.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceDocument {
    format: String,
    adapter: String,
    connector: String,
    surface: Surface,
    evidence: Evidence,
    cases: Vec<Case>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Surface {
    operations: Vec<OperationContract>,
    events: Vec<EventContract>,
    lifecycle: Lifecycle,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationContract {
    legacy_id: String,
    replacement_id: String,
    input_schema: Value,
    #[serde(deserialize_with = "present_option")]
    output_schema: Option<Value>,
    errors: Vec<ErrorContract>,
    host_effects: Vec<String>,
    semantic_effects: Vec<String>,
    #[serde(rename = "risk")]
    _risk: Risk,
    #[serde(rename = "idempotency")]
    _idempotency: Idempotency,
    capability_subjects: Vec<AuthorityRequirement>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventContract {
    legacy_id: String,
    replacement_id: String,
    payload_schema: Value,
    host_effects: Vec<String>,
    semantic_effects: Vec<String>,
    capability_subjects: Vec<AuthorityRequirement>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorContract {
    code: String,
    schema: Value,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Risk {
    Low,
    Medium,
    High,
    Destructive,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Idempotency {
    Idempotent,
    Repeatable,
    NonRepeatable,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lifecycle {
    mode: LifecycleMode,
    cancellation: Cancellation,
    #[serde(deserialize_with = "present_option")]
    stream: Option<StreamContract>,
    #[serde(deserialize_with = "present_option")]
    lease: Option<LeaseContract>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LifecycleMode {
    OneShot,
    Stream,
    Lease,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Cancellation {
    NotApplicable,
    Bounded,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamContract {
    item_schema: Value,
    #[serde(deserialize_with = "present_option")]
    terminal_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseContract {
    acquire: String,
    renew: String,
    release: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    #[serde(deserialize_with = "present_option")]
    legacy: Option<EvidenceIdentity>,
    #[serde(deserialize_with = "present_option")]
    exchange: Option<EvidenceIdentity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceIdentity {
    runner: String,
    source_commit: String,
    captured_at: String,
    raw_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    member: MemberRef,
    input: Value,
    exchange_execution: Execution,
    #[serde(deserialize_with = "present_option")]
    legacy: Option<Observation>,
    #[serde(deserialize_with = "present_option")]
    exchange: Option<Observation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MemberRef {
    Operation {
        legacy_id: String,
        replacement_id: String,
    },
    Event {
        legacy_id: String,
        replacement_id: String,
    },
}

impl MemberRef {
    fn key(&self) -> (&str, &str, &'static str) {
        match self {
            Self::Operation {
                legacy_id,
                replacement_id,
            } => (legacy_id, replacement_id, "operation"),
            Self::Event {
                legacy_id,
                replacement_id,
            } => (legacy_id, replacement_id, "event"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Execution {
    runtime: String,
    topology: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Observation {
    host_effects: Vec<String>,
    semantic_effects: Vec<String>,
    capability_subjects: Vec<AuthorityRequirement>,
    transcript: Vec<Outcome>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AuthorityRequirement {
    action: String,
    resource: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Outcome {
    Returned {
        value: Value,
    },
    DeclaredError {
        code: String,
        error: Value,
    },
    Refused {
        class: RefusalClass,
        code: String,
        status: u16,
        sent: Sent,
        retryable: bool,
    },
    Event {
        payload: Value,
    },
    StreamItem {
        sequence: u64,
        value: Value,
    },
    Cancelled {
        code: String,
    },
    LeaseAcquired {
        lease: String,
    },
    LeaseRenewed {
        lease: String,
    },
    LeaseReleased {
        lease: String,
    },
    Terminal {
        status: String,
        #[serde(deserialize_with = "present_option")]
        value: Option<Value>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Sent {
    No,
    Maybe,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RefusalClass {
    Runtime,
    Topology,
    Authority,
    Input,
    Policy,
}

fn present_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// A verdict is computed from paired observations; no authored `conformant` boolean exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// One side has not been captured yet.
    MissingEvidence { detail: String },
    /// Exchange explicitly refused the required runtime or topology.
    Unsupported {
        case: String,
        code: String,
        runtime: String,
        topology: String,
    },
    /// Both sides exist but their normalized public observations differ.
    Diverged { case: String },
    /// Every frozen case has equal legacy and Exchange observations.
    Conformant,
}

/// Parse and semantically validate a conformance document.
pub fn parse_conformance(text: &str) -> Result<ConformanceDocument> {
    let value: Value =
        serde_json::from_str(text).context("invalid native-plugin conformance JSON")?;
    let schema: Value = serde_json::from_str(CONFORMANCE_SCHEMA)
        .context("the embedded native-plugin conformance schema is invalid JSON")?;
    let validator = jsonschema::validator_for(&schema)
        .context("the embedded native-plugin conformance schema does not compile")?;
    if let Err(error) = validator.validate(&value) {
        bail!("native-plugin conformance document does not match the published schema: {error}");
    }
    let document: ConformanceDocument =
        serde_json::from_value(value).context("invalid native-plugin conformance document")?;
    validate_conformance(&document)?;
    Ok(document)
}

/// Load and validate one conformance document.
pub fn load_conformance(path: &Path) -> Result<ConformanceDocument> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("cannot read conformance document `{}`", path.display()))?;
    parse_conformance(&text).with_context(|| format!("invalid conformance `{}`", path.display()))
}

fn validate_conformance(document: &ConformanceDocument) -> Result<()> {
    if document.format != CONFORMANCE_FORMAT {
        bail!(
            "conformance format `{}` is unsupported; expected `{CONFORMANCE_FORMAT}`",
            document.format
        );
    }
    require_identifier("conformance adapter", &document.adapter)?;
    require_identifier("conformance connector", &document.connector)?;
    if document.surface.operations.is_empty() && document.surface.events.is_empty() {
        bail!("conformance surface declares no operations or events");
    }
    if document.cases.is_empty() {
        bail!("conformance document declares no cases");
    }
    validate_lifecycle(&document.surface.lifecycle)?;
    validate_evidence(document.evidence.legacy.as_ref(), "legacy")?;
    validate_evidence(document.evidence.exchange.as_ref(), "exchange")?;

    let mut members = BTreeMap::new();
    for operation in &document.surface.operations {
        validate_json_schema(&operation.input_schema, &operation.legacy_id, "input")?;
        if let Some(schema) = &operation.output_schema {
            validate_json_schema(schema, &operation.legacy_id, "output")?;
        }
        for error in &operation.errors {
            if error.code.trim().is_empty() {
                bail!(
                    "operation `{}` has an empty declared error code",
                    operation.legacy_id
                );
            }
            validate_json_schema(&error.schema, &operation.legacy_id, "error")?;
        }
        let key = (
            operation.legacy_id.as_str(),
            operation.replacement_id.as_str(),
            "operation",
        );
        if members
            .insert(key, ContractRef::Operation(operation))
            .is_some()
        {
            bail!(
                "conformance surface repeats operation `{}`",
                operation.legacy_id
            );
        }
    }
    for event in &document.surface.events {
        validate_json_schema(&event.payload_schema, &event.legacy_id, "event payload")?;
        let key = (
            event.legacy_id.as_str(),
            event.replacement_id.as_str(),
            "event",
        );
        if members.insert(key, ContractRef::Event(event)).is_some() {
            bail!("conformance surface repeats event `{}`", event.legacy_id);
        }
    }

    let mut case_ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for case in &document.cases {
        if case.id.trim().is_empty() || !case_ids.insert(case.id.as_str()) {
            bail!("conformance case ids must be non-empty and unique");
        }
        if case.exchange_execution.runtime.trim().is_empty()
            || case.exchange_execution.topology.trim().is_empty()
        {
            bail!(
                "case `{}` has incomplete Exchange execution metadata",
                case.id
            );
        }
        let key = case.member.key();
        let contract = members.get(&key).with_context(|| {
            format!(
                "case `{}` names undeclared {} `{}` -> `{}`",
                case.id, key.2, key.0, key.1
            )
        })?;
        covered.insert(key);
        validate_case_input(case, contract)?;
        if let Some(observation) = &case.legacy {
            validate_observation(case, observation, contract, &document.surface.lifecycle)?;
        }
        if let Some(observation) = &case.exchange {
            validate_observation(case, observation, contract, &document.surface.lifecycle)?;
        }
    }
    let uncovered = members
        .keys()
        .filter(|key| !covered.contains(*key))
        .map(|key| format!("{}:{}", key.2, key.0))
        .collect::<Vec<_>>();
    if !uncovered.is_empty() {
        bail!(
            "conformance surface members have no frozen case: {}",
            uncovered.join(", ")
        );
    }
    Ok(())
}

enum ContractRef<'a> {
    Operation(&'a OperationContract),
    Event(&'a EventContract),
}

fn validate_lifecycle(lifecycle: &Lifecycle) -> Result<()> {
    match lifecycle.mode {
        LifecycleMode::OneShot => {
            if !matches!(lifecycle.cancellation, Cancellation::NotApplicable)
                || lifecycle.stream.is_some()
                || lifecycle.lease.is_some()
            {
                bail!("one-shot lifecycle requires no cancellation, stream or lease contract");
            }
        }
        LifecycleMode::Stream => {
            if !matches!(lifecycle.cancellation, Cancellation::Bounded)
                || lifecycle.stream.is_none()
                || lifecycle.lease.is_some()
            {
                bail!("stream lifecycle requires bounded cancellation and one stream contract");
            }
        }
        LifecycleMode::Lease => {
            if !matches!(lifecycle.cancellation, Cancellation::Bounded) || lifecycle.lease.is_none()
            {
                bail!("lease lifecycle requires bounded cancellation and one lease contract");
            }
        }
    }
    if let Some(stream) = &lifecycle.stream {
        validate_json_schema(&stream.item_schema, "lifecycle", "stream item")?;
        if let Some(schema) = &stream.terminal_schema {
            validate_json_schema(schema, "lifecycle", "stream terminal")?;
        }
    }
    if let Some(lease) = &lifecycle.lease {
        if lease.acquire.trim().is_empty()
            || lease.renew.trim().is_empty()
            || lease.release.trim().is_empty()
        {
            bail!("lease contract must name acquire, renew and release semantics");
        }
    }
    Ok(())
}

fn validate_evidence(evidence: Option<&EvidenceIdentity>, side: &str) -> Result<()> {
    let Some(evidence) = evidence else {
        return Ok(());
    };
    if evidence.runner.trim().is_empty() || evidence.captured_at.trim().is_empty() {
        bail!("{side} evidence identity is incomplete");
    }
    require_hex(
        &format!("{side} source_commit"),
        &evidence.source_commit,
        40,
    )?;
    require_hex(&format!("{side} raw_sha256"), &evidence.raw_sha256, 64)
}

fn validate_json_schema(schema: &Value, member: &str, label: &str) -> Result<()> {
    jsonschema::validator_for(schema)
        .with_context(|| format!("{member} has an invalid {label} JSON Schema"))?;
    Ok(())
}

fn validate_instance(schema: &Value, value: &Value, context: &str) -> Result<()> {
    let validator = jsonschema::validator_for(schema)
        .with_context(|| format!("{context} schema is invalid"))?;
    if let Err(error) = validator.validate(value) {
        bail!("{context} does not match its frozen schema: {error}");
    }
    Ok(())
}

fn validate_case_input(case: &Case, contract: &ContractRef<'_>) -> Result<()> {
    match contract {
        ContractRef::Operation(operation) => validate_instance(
            &operation.input_schema,
            &case.input,
            &format!("case `{}` input", case.id),
        ),
        ContractRef::Event(event) => validate_instance(
            &event.payload_schema,
            &case.input,
            &format!("case `{}` event payload", case.id),
        ),
    }
}

fn validate_observation(
    case: &Case,
    observation: &Observation,
    contract: &ContractRef<'_>,
    lifecycle: &Lifecycle,
) -> Result<()> {
    if observation.transcript.is_empty() {
        bail!("case `{}` has an empty observation transcript", case.id);
    }
    let (host_effects, semantic_effects, capability_subjects) = match contract {
        ContractRef::Operation(operation) => (
            &operation.host_effects,
            &operation.semantic_effects,
            &operation.capability_subjects,
        ),
        ContractRef::Event(event) => (
            &event.host_effects,
            &event.semantic_effects,
            &event.capability_subjects,
        ),
    };
    if &observation.host_effects != host_effects
        || &observation.semantic_effects != semantic_effects
        || &observation.capability_subjects != capability_subjects
    {
        bail!(
            "case `{}` observation disagrees with its frozen effects or capability subjects",
            case.id
        );
    }

    for outcome in &observation.transcript {
        match outcome {
            Outcome::Returned { value } => {
                let ContractRef::Operation(operation) = contract else {
                    bail!("case `{}` event returned an operation result", case.id);
                };
                if let Some(schema) = &operation.output_schema {
                    validate_instance(
                        schema,
                        value,
                        &format!("case `{}` returned value", case.id),
                    )?;
                }
            }
            Outcome::DeclaredError { code, error } => {
                let ContractRef::Operation(operation) = contract else {
                    bail!(
                        "case `{}` event emitted a declared operation error",
                        case.id
                    );
                };
                let declared = operation
                    .errors
                    .iter()
                    .find(|declared| declared.code == *code)
                    .with_context(|| {
                        format!("case `{}` observed undeclared error `{code}`", case.id)
                    })?;
                validate_instance(
                    &declared.schema,
                    error,
                    &format!("case `{}` declared error `{code}`", case.id),
                )?;
            }
            Outcome::Event { payload } => {
                let ContractRef::Event(event) = contract else {
                    bail!("case `{}` operation transcript contains an event", case.id);
                };
                validate_instance(
                    &event.payload_schema,
                    payload,
                    &format!("case `{}` delivered event", case.id),
                )?;
            }
            Outcome::StreamItem { value, .. } => {
                let stream = lifecycle.stream.as_ref().with_context(|| {
                    format!(
                        "case `{}` observed a stream item without a stream contract",
                        case.id
                    )
                })?;
                validate_instance(
                    &stream.item_schema,
                    value,
                    &format!("case `{}` stream item", case.id),
                )?;
            }
            Outcome::LeaseAcquired { .. }
            | Outcome::LeaseRenewed { .. }
            | Outcome::LeaseReleased { .. } => {
                if lifecycle.lease.is_none() {
                    bail!(
                        "case `{}` observed a lease without a lease contract",
                        case.id
                    );
                }
            }
            Outcome::Cancelled { .. } => {
                if !matches!(lifecycle.cancellation, Cancellation::Bounded) {
                    bail!(
                        "case `{}` observed cancellation for a one-shot member",
                        case.id
                    );
                }
            }
            Outcome::Terminal { value, .. } => {
                let stream = lifecycle.stream.as_ref().with_context(|| {
                    format!(
                        "case `{}` observed a terminal stream outcome without a stream contract",
                        case.id
                    )
                })?;
                if let Some(schema) = &stream.terminal_schema {
                    validate_instance(
                        schema,
                        value.as_ref().unwrap_or(&Value::Null),
                        &format!("case `{}` stream terminal value", case.id),
                    )?;
                }
            }
            Outcome::Refused { .. } => {}
        }
    }
    Ok(())
}

/// Compute the result of comparing every captured legacy and Exchange observation.
pub fn conformance_verdict(document: &ConformanceDocument) -> Verdict {
    if document.evidence.legacy.is_none() || document.evidence.exchange.is_none() {
        return Verdict::MissingEvidence {
            detail: "legacy and Exchange evidence identities are both required".to_owned(),
        };
    }
    for case in &document.cases {
        let (Some(legacy), Some(exchange)) = (&case.legacy, &case.exchange) else {
            return Verdict::MissingEvidence {
                detail: format!("case `{}` needs both observations", case.id),
            };
        };
        for outcome in &exchange.transcript {
            if let Outcome::Refused {
                class: RefusalClass::Runtime | RefusalClass::Topology,
                code,
                ..
            } = outcome
            {
                return Verdict::Unsupported {
                    case: case.id.clone(),
                    code: code.clone(),
                    runtime: case.exchange_execution.runtime.clone(),
                    topology: case.exchange_execution.topology.clone(),
                };
            }
        }
        if legacy != exchange {
            return Verdict::Diverged {
                case: case.id.clone(),
            };
        }
    }
    Verdict::Conformant
}

fn ensure_document_identity(adapter: &Adapter, document: &ConformanceDocument) -> Result<()> {
    if document.adapter != adapter.id || document.connector != adapter.connector {
        bail!(
            "adapter `{}` conformance identity is `{}` -> `{}`, expected `{}` -> `{}`",
            adapter.id,
            document.adapter,
            document.connector,
            adapter.id,
            adapter.connector
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationReceipt {
    format: String,
    adapter: String,
    connector: String,
    release: String,
    connector_commit: String,
    artifact: PublishedArtifact,
    replacement_addresses: Vec<String>,
    migration_notes: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedArtifact {
    identity: String,
    sha256: String,
}

fn load_publication(path: &Path) -> Result<PublicationReceipt> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("cannot read publication receipt `{}`", path.display()))?;
    let receipt: PublicationReceipt = serde_json::from_str(&text)
        .with_context(|| format!("invalid publication receipt `{}`", path.display()))?;
    if receipt.format != PUBLICATION_FORMAT {
        bail!(
            "publication format `{}` is unsupported; expected `{PUBLICATION_FORMAT}`",
            receipt.format
        );
    }
    require_identifier("publication adapter", &receipt.adapter)?;
    require_identifier("publication connector", &receipt.connector)?;
    if !receipt.release.starts_with('v') || receipt.release.len() < 2 {
        bail!(
            "publication release `{}` is not a version tag",
            receipt.release
        );
    }
    require_hex(
        "publication connector_commit",
        &receipt.connector_commit,
        40,
    )?;
    require_hex("publication artifact sha256", &receipt.artifact.sha256, 64)?;
    if receipt.artifact.identity.trim().is_empty() || receipt.replacement_addresses.is_empty() {
        bail!("publication receipt has no artifact identity or replacement addresses");
    }
    if receipt
        .replacement_addresses
        .iter()
        .any(|address| address.trim().is_empty())
    {
        bail!("publication receipt carries an empty replacement address");
    }
    require_relative("publication migration_notes", &receipt.migration_notes)?;
    Ok(receipt)
}

fn ensure_publication_identity(adapter: &Adapter, receipt: &PublicationReceipt) -> Result<()> {
    if receipt.adapter != adapter.id || receipt.connector != adapter.connector {
        bail!(
            "adapter `{}` publication identity is `{}` -> `{}`, expected `{}` -> `{}`",
            adapter.id,
            receipt.adapter,
            receipt.connector,
            adapter.id,
            adapter.connector
        );
    }
    Ok(())
}

fn require_hex(label: &str, value: &str, length: usize) -> Result<()> {
    if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("{label} must be exactly {length} hexadecimal characters");
    }
    Ok(())
}

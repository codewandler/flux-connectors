//! What the host holds between requests.

use std::sync::Arc;

use connector_pack::{
    CredentialRef, Credentials, Egress, MemoryStore, Secret, SecretStore, StoreError,
};
use flux_runtime::ToolContext;
use flux_system::{System, Workspace};
use flux_web::http::HttpRequestTool;
use flux_web::WebOptions;

use crate::auth::oidc::{self, JwksCache, Setup};
use crate::auth::session::{Accounts, Sessions};
use crate::config::Settings;

/// Everything sign-in needs, once it is configured.
///
/// Absent — `App::oidc()` returns `None` — when the operator has not registered an OAuth client
/// yet. That is a first-run state, not an error state, and it is why this is an `Option` rather
/// than a reason to refuse to start.
pub struct Oidc {
    pub settings: oidc::Settings,
    /// Google's signing keys, fetched lazily and refreshed across a rotation.
    pub jwks: JwksCache,
}

/// The host.
///
/// Cheap to clone — every field is an `Arc` — because axum hands a clone to each request, and the
/// ports must be *the same* instances across requests. A transport constructed per request would
/// give every call a fresh connection pool; a secret store constructed per request would forget
/// what an operator just pasted.
#[derive(Clone)]
pub struct App {
    /// Sign-in, if an operator has configured it.
    oidc: Option<Arc<Oidc>>,
    /// What to tell an operator when they have not.
    setup_message: Option<Arc<String>>,
    /// Live sessions, and the sign-ins on their way to becoming one.
    sessions: Arc<Sessions>,
    /// Every account this host has seen, keyed by OIDC subject.
    accounts: Arc<Accounts>,
    /// flux's `http.request`, configured once. Every operation delegates to this instance, so
    /// connectors inherit one egress policy rather than each inventing its own.
    egress: Egress,
    /// Where this host's tenants' credentials live.
    ///
    /// In memory, deliberately, for now: the process exiting is the cleanup, and this is the first
    /// component in the repository that holds a plaintext credential at runtime. A file-backed
    /// `0600` store and the existing `VaultStore` are both drop-in — the port is
    /// `Arc<dyn SecretStore>` precisely so that swapping it is a one-line change at this call site.
    secrets: Arc<MemoryStore>,
    /// Non-secret connection settings — the `{subdomain}` in a templated base URL.
    settings: Arc<Settings>,
    /// The workspace every dispatch happens under. Nothing here reaches the filesystem through it;
    /// `System` requires a root that exists, and a `ToolContext` is the only way to reach the
    /// redactor.
    system: Arc<System>,
}

impl App {
    /// Build the host.
    ///
    /// # Errors
    ///
    /// If `root` is not a directory that exists — `System` requires one, and failing here makes a
    /// misconfiguration a startup error rather than a surprise at the first dispatch.
    #[allow(clippy::missing_panics_doc)]
    pub fn new(root: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        // `WebOptions::default()` carries `PrivateNetAllow::None` — the full SSRF guard, where
        // private, loopback, link-local and internal hosts are all refused. That is the right
        // default for a host whose connectors call public SaaS APIs, and widening it is a deliberate
        // act through [`App::with_web_options`] rather than a state a reader finds by accident.
        Self::with_web_options(root, WebOptions::default())
    }

    /// The host, with an egress policy chosen by the caller.
    ///
    /// # The one seam a test uses, and why it is not a stub
    ///
    /// An integration test needs a vendor it controls, which means a loopback address, which
    /// [`App::new`]'s SSRF guard refuses — correctly. So a test widens `private_net` to the one host
    /// it started, and everything else stays real: the same `HttpRequestTool`, the same `Egress`,
    /// the same pack, the same bytes.
    ///
    /// That distinction matters more than the convenience. `Egress`'s own documentation warns that
    /// *"a stand-in that ignores `body`, or that resolves `url` against some base of its own, is not
    /// a substitute — it is a different connector"*, and every `connector-pack` test passes a stub
    /// for want of a transport. This crate is where that stops being true, and it would be a poor
    /// trade to arrive here and then assert against a stub of our own.
    ///
    /// # Errors
    ///
    /// If `root` is not a directory that exists.
    pub fn with_web_options(
        root: impl AsRef<std::path::Path>,
        options: WebOptions,
    ) -> anyhow::Result<Self> {
        let workspace = Workspace::new(root.as_ref())
            .map_err(|error| anyhow::anyhow!("workspace root {:?}: {error}", root.as_ref()))?;

        let http = HttpRequestTool::new(&options);

        // Sign-in is read from the environment here, once, so that a misconfiguration is a startup
        // fact rather than something discovered on a click. **A missing registration does not stop
        // the host**: it starts, serves its page, and says which variables are unset. Panicking
        // would turn a first run into a stack trace, and starting silently would turn it into a
        // button that leads nowhere.
        let (oidc, setup_message) = match oidc::Settings::from_env() {
            Setup::Configured(settings) => {
                let jwks = JwksCache::new(settings.jwks_url.clone());
                (Some(Arc::new(Oidc { settings, jwks })), None)
            }
            Setup::Missing(missing) => (None, Some(Arc::new(Setup::explain(&missing)))),
        };

        Ok(Self {
            oidc,
            setup_message,
            sessions: Arc::new(Sessions::new()),
            accounts: Arc::new(Accounts::new()),
            egress: Egress::new(Arc::new(http)),
            secrets: Arc::new(MemoryStore::new()),
            settings: Arc::new(Settings::new()),
            system: Arc::new(System::new(workspace)),
        })
    }

    /// Sign-in, if it is configured.
    pub fn oidc(&self) -> Option<&Arc<Oidc>> {
        self.oidc.as_ref()
    }

    /// What an operator must still set up, if anything.
    pub fn setup_message(&self) -> Option<String> {
        self.setup_message
            .as_ref()
            .map(|message| message.to_string())
    }

    /// Live sessions.
    pub fn sessions(&self) -> &Arc<Sessions> {
        &self.sessions
    }

    /// Known accounts.
    pub fn accounts(&self) -> &Arc<Accounts> {
        &self.accounts
    }

    /// The transport every operation delegates to.
    pub fn egress(&self) -> Egress {
        self.egress.clone()
    }

    /// This host's connection settings.
    pub fn settings(&self) -> &Arc<Settings> {
        &self.settings
    }

    /// The credential port, bound for one tenant.
    ///
    /// # Errors
    ///
    /// [`connector_pack::Error::Tenant`] when the tenant id is not a usable path segment. Refused
    /// here rather than at the first call: a tenant id ends up in a store path, and the cautionary
    /// precedent is close to home — action-proxy puts two client-supplied headers straight into a
    /// Vault path with no validation at all.
    pub fn credentials(&self, tenant: &str) -> Result<Credentials, connector_pack::Error> {
        Credentials::new(self.secrets.clone(), tenant)
    }

    /// Store one credential value at its address.
    ///
    /// The value is moved in and never returned. Nothing in this type has a getter for a secret,
    /// which is the shape rather than the habit: a route cannot echo back what it cannot read.
    pub async fn put_secret(
        &self,
        reference: &CredentialRef,
        value: Secret,
    ) -> Result<(), StoreError> {
        self.secrets.put(reference, &value).await
    }

    /// Forget one credential.
    pub async fn delete_secret(&self, reference: &CredentialRef) -> Result<(), StoreError> {
        self.secrets.delete(reference).await
    }

    /// **Whether** a credential is stored at an address — never what.
    ///
    /// This is what lets the UI show a connector as connected without any surface ever holding the
    /// value. `StoreError::NotFound` is the "no" answer; any other error is reported as "unknown"
    /// rather than collapsed into "no", because "unreachable" and "not configured" want opposite
    /// responses from an operator and collapsing them is the gap `connector-secrets`' error type
    /// exists to close.
    pub async fn has_secret(&self, reference: &CredentialRef) -> Result<bool, StoreError> {
        match self.secrets.get(reference).await {
            Ok(_) => Ok(true),
            Err(error) if error.is_not_found() => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// A fresh dispatch context.
    ///
    /// One per request, because the redactor is per context and a credential registered for one
    /// call must not outlive it into another tenant's.
    pub fn context(&self) -> ToolContext {
        ToolContext::new(self.system.clone())
    }
}

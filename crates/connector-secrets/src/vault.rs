//! A [`SecretStore`] over HashiCorp Vault's **KV version 2** engine.
//!
//! Behind the `vault` feature, off by default, so that a consumer wanting only the trait, the
//! addressing and [`MemoryStore`](crate::MemoryStore) links no HTTP client at all.
//!
//! # KV v2, and why that is a decision worth recording
//!
//! action-proxy — the closest existing deployment in this ecosystem — keeps its credentials under
//! **KV v1** (`customer/<accountUuid>/integrations/<integrationUuid>`). This store speaks **v2**,
//! so importing that data is a **migration, not a rename**: the two engines differ in the URL
//! (`/v1/<mount>/data/<path>` versus `/v1/<mount>/<path>`), in the response envelope (v2 nests the
//! secret under `data.data`), in delete semantics (v2 versions everything, and `DELETE` on a *data*
//! path only soft-deletes the current version), and in whether a mount can serve both at once (it
//! cannot). v2 is chosen anyway, because versioning and undelete are exactly what one wants of a
//! credential store, and the vendor's own internal secret store — the precedent for the `tenants/` prefix this crate's
//! default [`Layout`] renders — is already on v2.
//!
//! # The transport is a seam
//!
//! [`VaultStore`] is generic over a [`VaultTransport`]. Everything that is *Vault* — the KV v2 URL
//! shape, the response envelope, the mapping from status code to [`StoreError`] — is in this module
//! and is tested against a **recorded transcript**, offline, with no server of any kind. The
//! reqwest-backed [`HttpTransport`] is the small commodity remainder.
//!
//! There is also a live leg, `tests/vault_live.rs`, which runs against a real dev server when
//! `CONNECTOR_SECRETS_VAULT_ADDR` and `CONNECTOR_SECRETS_VAULT_TOKEN` are set, and when they are not
//! is compiled `#[ignore]`d with the reason attached — so a run without a server reports
//! `0 passed; 1 ignored` and prints why, rather than the `ok` it used to print (C-149). It never
//! simulates success, and now the output says which of the two happened.
//!
//! # No session handling
//!
//! Static token only. flux's `VaultCredentialStore` already does Kubernetes auth, a 60s renew
//! buffer, `renew-self` and one retry on 401/403 — including the re-read-the-projected-JWT-on-every-
//! login fix that three codebases here have each had to learn, because kubelet rotates that file
//! roughly hourly. Reimplementing any of it badly is worse than not having it, and expiry, refresh
//! and rotation are out of scope for this crate by instruction. A host that needs a renewed token
//! supplies one by constructing a new store, or wraps this one.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    CredentialRef, CredentialScope, Layout, Secret, SecretBatch, SecretStore, StoreError,
    TenantLayout,
};

/// Vault's default KV v2 mount point.
pub const DEFAULT_MOUNT: &str = "secret";

/// The field a secret's single value is written to inside the KV entry.
///
/// A KV v2 secret is a map, but a [`CredentialRef`] already names one credential — the leaf of the
/// path *is* `api_token` — so nesting a second name under it would say the same thing twice and
/// give two places to disagree. One conventional field, overridable for a store that inherited a
/// different convention.
pub const DEFAULT_FIELD: &str = "value";

/// The HTTP methods this client uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    /// Read a secret.
    Get,
    /// Write a secret.
    Post,
    /// Remove a secret and all of its versions.
    Delete,
}

impl Method {
    /// The method's wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }
}

/// One request to Vault.
///
/// The token is a [`Secret`] and stays one all the way to the transport, which is the point: it is
/// never a `String` in a header map that some `Debug` could print.
#[derive(Debug)]
pub struct VaultRequest<'a> {
    /// The method.
    pub method: Method,
    /// The absolute URL, already including `/v1/`.
    pub url: String,
    /// The value for the `X-Vault-Token` header.
    pub token: &'a Secret,
    /// A JSON body, for [`Method::Post`].
    pub body: Option<String>,
}

/// Vault's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultResponse {
    /// The HTTP status.
    pub status: u16,
    /// The response body, which Vault always renders as JSON when it renders anything.
    pub body: String,
}

/// A transport failed to obtain any answer at all.
///
/// Deliberately narrow: it means *the request did not complete*, which is what
/// [`StoreError::Unreachable`] is for. A `403` is an answer and is not this.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct TransportError(String);

impl TransportError {
    /// Wrap whatever the underlying client said.
    pub fn new(reason: impl std::fmt::Display) -> Self {
        Self(reason.to_string())
    }
}

/// How a [`VaultStore`] talks to a server.
///
/// A seam rather than a hard-wired client, so the KV v2 logic above it is testable against a
/// recorded transcript, and so a host that already has a configured HTTP client — with its own
/// proxy, mTLS or tracing — can hand it over instead of getting a second one.
#[async_trait]
pub trait VaultTransport: Send + Sync {
    /// Perform one request.
    ///
    /// # Errors
    ///
    /// Only when no response was obtained. Any status Vault returned, including an error status, is
    /// an `Ok(VaultResponse)`.
    async fn send(&self, request: VaultRequest<'_>) -> Result<VaultResponse, TransportError>;
}

/// A [`SecretStore`] backed by Vault's KV v2 engine.
///
/// Generic over its [`Layout`] — the decorator this design exists for. The layout decides the
/// logical path; this store decides only how a logical path becomes a KV v2 URL, which is
/// `<base>/v1/<mount>/data/<path>` to read and write, and `<base>/v1/<mount>/metadata/<path>` to
/// delete.
pub struct VaultStore<T, L = TenantLayout> {
    transport: T,
    layout: L,
    base_url: String,
    mount: String,
    field: String,
    token: Secret,
}

impl<T: VaultTransport> VaultStore<T, TenantLayout> {
    /// A store against `base_url` using the blessed [`TenantLayout`], the default mount and the
    /// default field.
    ///
    /// `base_url` is the server root — `https://vault.internal:8200` — without the `/v1` prefix,
    /// which this store adds. A trailing slash is tolerated.
    pub fn new(transport: T, base_url: impl Into<String>, token: Secret) -> Self {
        Self::with_layout(transport, base_url, token, TenantLayout)
    }
}

impl<T: VaultTransport, L: Layout> VaultStore<T, L> {
    /// A store rendering logical paths through `layout`.
    pub fn with_layout(
        transport: T,
        base_url: impl Into<String>,
        token: Secret,
        layout: L,
    ) -> Self {
        Self {
            transport,
            layout,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            mount: DEFAULT_MOUNT.to_owned(),
            field: DEFAULT_FIELD.to_owned(),
            token,
        }
    }

    /// Use a KV v2 mount other than [`DEFAULT_MOUNT`].
    #[must_use]
    pub fn with_mount(mut self, mount: impl Into<String>) -> Self {
        self.mount = mount.into().trim_matches('/').to_owned();
        self
    }

    /// Read and write the value under a field other than [`DEFAULT_FIELD`].
    #[must_use]
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = field.into();
        self
    }

    /// The logical path `reference` resolves to under this store's layout — the path an operator
    /// sees, without the `/v1/<mount>/data/` machinery.
    pub fn path(&self, reference: &CredentialRef) -> String {
        self.layout.render(reference)
    }

    /// The address a logical path resolves back to — the inverse of [`path`](Self::path).
    ///
    /// The caller this is for is an operator holding a path they read out of Vault, asking whose
    /// credential is at it. It talks to nothing: a path is not a lookup.
    ///
    /// # Errors
    ///
    /// [`StoreError::Layout`], carrying the layout's own explanation, when the path is not one this
    /// layout writes. A layout is entitled to refuse rather than guess, and this is how that refusal
    /// reaches a caller.
    pub fn reference(&self, path: &str) -> Result<CredentialRef, StoreError> {
        self.layout
            .parse(path)
            .map_err(|reason| StoreError::Layout { reason })
    }

    /// The layout this store renders through.
    pub fn layout(&self) -> &L {
        &self.layout
    }

    /// The URL for reading or writing a secret.
    fn data_url(&self, path: &str) -> String {
        format!("{}/v1/{}/data/{path}", self.base_url, self.mount)
    }

    /// The URL for removing a secret and every version of it.
    fn metadata_url(&self, path: &str) -> String {
        format!("{}/v1/{}/metadata/{path}", self.base_url, self.mount)
    }

    /// Send, and turn "no answer" into [`StoreError::Unreachable`].
    async fn send(
        &self,
        method: Method,
        url: String,
        path: &str,
        body: Option<String>,
    ) -> Result<VaultResponse, StoreError> {
        self.transport
            .send(VaultRequest {
                method,
                url,
                token: &self.token,
                body,
            })
            .await
            .map_err(|error| StoreError::Unreachable {
                path: path.to_owned(),
                reason: error.to_string(),
            })
    }
}

/// Map a status Vault answered with onto the error it means.
///
/// Only reached for statuses the caller did not already handle as success or as a meaningful `404`.
fn status_error(status: u16, path: &str, body: &str) -> StoreError {
    let reason = vault_errors(body).unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        // Vault answers 400 for a malformed request, which for this client means the path or body
        // it built was wrong — retrying will not help.
        400 => StoreError::Backend {
            path: path.to_owned(),
            reason,
        },
        401 | 403 => StoreError::Denied {
            path: path.to_owned(),
            reason,
        },
        // 503 is a sealed or standby Vault, and 502/504 a proxy in front of one. All are "ask
        // again later", which is what `Unreachable` means.
        429 | 500 | 502 | 503 | 504 => StoreError::Unreachable {
            path: path.to_owned(),
            reason,
        },
        _ => StoreError::Backend {
            path: path.to_owned(),
            reason,
        },
    }
}

/// Vault renders its failures as `{"errors": ["…"]}`. Recover that text when it is there, so an
/// operator reads the server's own words rather than a status number.
fn vault_errors(body: &str) -> Option<String> {
    let document: Value = serde_json::from_str(body).ok()?;
    let errors = document.get("errors")?.as_array()?;
    let joined: Vec<&str> = errors.iter().filter_map(Value::as_str).collect();
    (!joined.is_empty()).then(|| joined.join("; "))
}

#[async_trait]
impl<T: VaultTransport, L: Layout + Send + Sync> SecretStore for VaultStore<T, L> {
    async fn get(&self, reference: &CredentialRef) -> Result<Secret, StoreError> {
        let path = self.layout.render(reference);
        let url = self.data_url(&path);
        let response = self.send(Method::Get, url, &path, None).await?;

        // A KV v2 read answers 404 both for "never written" and for "deleted but not destroyed".
        // Both are "nothing is stored here", which is the distinction the caller cares about.
        if response.status == 404 {
            return Err(StoreError::NotFound { path });
        }
        if response.status != 200 {
            return Err(status_error(response.status, &path, &response.body));
        }

        let document: Value =
            serde_json::from_str(&response.body).map_err(|error| StoreError::Backend {
                path: path.clone(),
                reason: format!("the response is not JSON: {error}"),
            })?;

        // A version that was deleted but not destroyed answers 200 with `data.data: null` and a
        // `deletion_time` in the metadata. Reading that as an empty secret would hand a host a
        // credential that is not there.
        let value = document.pointer("/data/data").ok_or_else(|| {
            let deleted = document.pointer("/data/metadata/deletion_time").is_some();
            if deleted {
                StoreError::NotFound { path: path.clone() }
            } else {
                StoreError::Backend {
                    path: path.clone(),
                    reason: "the response carries no `data.data`; is this mount KV v1?".to_owned(),
                }
            }
        })?;
        if value.is_null() {
            return Err(StoreError::NotFound { path });
        }

        match value.get(&self.field).and_then(Value::as_str) {
            Some(secret) => Ok(Secret::new(secret)),
            // The entry exists but carries no `value` field, or carries one that is not a string.
            // Both are a store somebody else wrote in a shape this client does not understand, and
            // both are worth naming rather than reporting as "not found".
            None => Err(StoreError::Backend {
                path,
                reason: format!(
                    "the secret exists but has no string field {:?}; \
                     construct the store with `with_field` if it uses another name",
                    self.field
                ),
            }),
        }
    }

    async fn put(&self, reference: &CredentialRef, secret: &Secret) -> Result<(), StoreError> {
        let path = self.layout.render(reference);
        let url = self.data_url(&path);
        // The one place a value is serialised, and it goes straight out on the wire. Built as a map
        // rather than through a literal, because the field name is configurable.
        let mut fields = serde_json::Map::new();
        fields.insert(
            self.field.clone(),
            Value::String(secret.expose_secret().to_owned()),
        );
        let body = json!({ "data": Value::Object(fields) }).to_string();
        let response = self.send(Method::Post, url, &path, Some(body)).await?;

        match response.status {
            200 | 204 => Ok(()),
            status => Err(status_error(status, &path, &response.body)),
        }
    }

    async fn delete(&self, reference: &CredentialRef) -> Result<(), StoreError> {
        let path = self.layout.render(reference);
        // `metadata`, not `data`: a `DELETE` on the data path soft-deletes only the current
        // version, so the credential would still be readable at the version below it. "Clear this
        // tenant's credential" means every version.
        let url = self.metadata_url(&path);
        let response = self.send(Method::Delete, url, &path, None).await?;

        match response.status {
            // Idempotent by the trait's contract, and by Vault's: a metadata delete answers 204
            // whether or not anything was there. 404 is folded in for the same reason.
            200 | 204 | 404 => Ok(()),
            status => Err(status_error(status, &path, &response.body)),
        }
    }

    async fn references(&self, _scope: &CredentialScope) -> Result<Vec<CredentialRef>, StoreError> {
        Err(StoreError::Unsupported {
            operation: "references".to_owned(),
            reason: "Vault KV v2 listing and policy semantics are not implemented".to_owned(),
        })
    }

    async fn apply(&self, _batch: &SecretBatch) -> Result<(), StoreError> {
        Err(StoreError::Unsupported {
            operation: "atomic batch".to_owned(),
            reason: "Vault KV v2 provides no multi-path transaction used by this adapter"
                .to_owned(),
        })
    }
}

#[async_trait]
impl<T: VaultTransport, L: Layout + Send + Sync> crate::PreparedSecretStore for VaultStore<T, L> {}

/// `Debug` without the token. Derived would print it.
impl<T, L> std::fmt::Debug for VaultStore<T, L> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VaultStore")
            .field("base_url", &self.base_url)
            .field("mount", &self.mount)
            .field("field", &self.field)
            .field("token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// The reqwest-backed [`VaultTransport`].
///
/// The commodity part: it sets `X-Vault-Token`, sends the body, and reports a status and a string.
/// Every decision about what those mean is above it, in [`VaultStore`], where it can be tested
/// without a socket.
#[derive(Debug, Clone)]
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    /// A transport with a default client and a request timeout.
    ///
    /// # Errors
    ///
    /// [`TransportError`] if the client cannot be built — a TLS backend that will not initialise,
    /// most likely.
    pub fn new(timeout: std::time::Duration) -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(TransportError::new)?;
        Ok(Self { client })
    }

    /// A transport over a client the host already configured — its own proxy, its own root store,
    /// its own instrumentation.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl VaultTransport for HttpTransport {
    async fn send(&self, request: VaultRequest<'_>) -> Result<VaultResponse, TransportError> {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = self
            .client
            .request(method, &request.url)
            .header("X-Vault-Token", request.token.expose_secret());
        if let Some(body) = request.body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body);
        }

        let response = builder.send().await.map_err(TransportError::new)?;
        let status = response.status().as_u16();
        // A body that cannot be read is still an answer with a status; report the status rather
        // than losing it to a transport error that would be read as "unreachable".
        let body = response.text().await.unwrap_or_default();
        Ok(VaultResponse { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// Obviously not a credential. Nothing in this repository commits a value shaped like a real
    /// token — a plausible placeholder has tripped GitHub push protection here before.
    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";
    const SENTINEL_TOKEN: &str = "SENTINEL-NOT-A-REAL-VAULT-TOKEN";

    const BASE: &str = "https://vault.invalid:8200";
    const DATA_URL: &str =
        "https://vault.invalid:8200/v1/secret/data/tenants/9f3a4b2c/com.zendesk.api/support/api_token";
    const METADATA_URL: &str =
        "https://vault.invalid:8200/v1/secret/metadata/tenants/9f3a4b2c/com.zendesk.api/support/api_token";

    /// A [`VaultTransport`] that answers from a script and records what it was asked.
    ///
    /// This is the "recorded transcript" the story calls for: the bodies below are Vault's own KV
    /// v2 response envelopes, so the parsing, the URL shape and the status mapping are all
    /// exercised, and none of it needs a server.
    #[derive(Default)]
    struct Recorded {
        replies: BTreeMap<(Method, String), VaultResponse>,
        seen: Mutex<Vec<Exchange>>,
        /// When set, every request fails to complete — the "backend unreachable" leg.
        offline: Option<String>,
    }

    /// One request the transcript saw, as the assertions want to read it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Exchange {
        method: Method,
        url: String,
        body: Option<String>,
        /// The token the store handed the transport, which is what would become
        /// `X-Vault-Token`. A `String` here rather than a `Secret` so a failing assertion can
        /// actually show what went out — this is a sentinel, in a test.
        token: String,
    }

    impl Recorded {
        fn new() -> Self {
            Self::default()
        }

        fn offline(reason: &str) -> Self {
            Self {
                offline: Some(reason.to_owned()),
                ..Self::default()
            }
        }

        fn reply(mut self, method: Method, url: &str, status: u16, body: &str) -> Self {
            self.replies.insert(
                (method, url.to_owned()),
                VaultResponse {
                    status,
                    body: body.to_owned(),
                },
            );
            self
        }

        fn requests(&self) -> Vec<Exchange> {
            self.seen.lock().expect("the transcript lock").clone()
        }
    }

    #[async_trait]
    impl VaultTransport for Recorded {
        async fn send(&self, request: VaultRequest<'_>) -> Result<VaultResponse, TransportError> {
            self.seen
                .lock()
                .expect("the transcript lock")
                .push(Exchange {
                    method: request.method,
                    url: request.url.clone(),
                    body: request.body.clone(),
                    token: request.token.expose_secret().to_owned(),
                });
            if let Some(reason) = &self.offline {
                return Err(TransportError::new(reason));
            }
            self.replies
                .get(&(request.method, request.url.clone()))
                .cloned()
                .ok_or_else(|| {
                    TransportError::new(format!(
                        "the transcript has no {} {}",
                        request.method.as_str(),
                        request.url
                    ))
                })
        }
    }

    /// Vault's KV v2 read envelope, with the secret nested under `data.data`.
    fn read_envelope(value: &str) -> String {
        json!({
            "request_id": "00000000-0000-0000-0000-000000000000",
            "lease_id": "",
            "renewable": false,
            "lease_duration": 0,
            "data": {
                "data": { "value": value },
                "metadata": {
                    "created_time": "2026-07-30T00:00:00.000000000Z",
                    "custom_metadata": null,
                    "deletion_time": "",
                    "destroyed": false,
                    "version": 1
                }
            },
            "warnings": null
        })
        .to_string()
    }

    fn reference() -> CredentialRef {
        CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token").expect("valid")
    }

    fn store<T: VaultTransport>(transport: T) -> VaultStore<T> {
        VaultStore::new(transport, BASE, Secret::new(SENTINEL_TOKEN))
    }

    #[tokio::test]
    async fn a_read_takes_the_kv_v2_data_path_and_unwraps_the_envelope() {
        let store =
            store(Recorded::new().reply(Method::Get, DATA_URL, 200, &read_envelope(SENTINEL)));

        let secret = store.get(&reference()).await.expect("get");

        assert_eq!(secret.expose_secret(), SENTINEL);
        let requests = store.transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, Method::Get);
        assert_eq!(requests[0].url, DATA_URL, "KV v2 reads go through `/data/`");
        assert_eq!(
            requests[0].token, SENTINEL_TOKEN,
            "the token reaches the transport for `X-Vault-Token`"
        );
    }

    #[tokio::test]
    async fn a_write_posts_the_value_under_one_field() {
        let store = store(Recorded::new().reply(Method::Post, DATA_URL, 200, "{}"));

        store
            .put(&reference(), &Secret::new(SENTINEL))
            .await
            .expect("put");

        let requests = store.transport.requests();
        assert_eq!(requests[0].method, Method::Post);
        assert_eq!(requests[0].url, DATA_URL);
        let body: Value = serde_json::from_str(requests[0].body.as_deref().expect("a body"))
            .expect("the body is JSON");
        assert_eq!(body, json!({ "data": { "value": SENTINEL } }));
    }

    /// `--clear` must remove every version, so the delete goes to `metadata`, not `data`. A `data`
    /// delete would soft-delete only the newest version and leave the credential readable below it.
    #[tokio::test]
    async fn a_delete_removes_every_version_and_is_idempotent() {
        let transport = Recorded::new()
            .reply(Method::Delete, METADATA_URL, 204, "")
            .reply(Method::Get, DATA_URL, 404, r#"{"errors":[]}"#);
        let store = store(transport);

        store.delete(&reference()).await.expect("delete");
        assert_eq!(store.transport.requests()[0].url, METADATA_URL);

        // Vault answers 204 whether or not anything was there, so a second delete succeeds too.
        store.delete(&reference()).await.expect("second delete");
        assert!(store.get(&reference()).await.unwrap_err().is_not_found());
    }

    /// The distinction flux's `Option`-returning `load` cannot make.
    #[tokio::test]
    async fn not_stored_and_unreachable_are_told_apart() {
        let missing = store(Recorded::new().reply(Method::Get, DATA_URL, 404, r#"{"errors":[]}"#))
            .get(&reference())
            .await
            .expect_err("404 is not found");
        assert!(missing.is_not_found(), "got {missing:?}");

        let down = store(Recorded::offline("connection refused"))
            .get(&reference())
            .await
            .expect_err("a transport failure is not a missing secret");
        assert!(
            matches!(down, StoreError::Unreachable { ref reason, .. } if reason.contains("connection refused")),
            "got {down:?}"
        );
        assert!(!down.is_not_found());
    }

    /// A sealed Vault is `Unreachable`, not `NotFound` — the failure mode that would otherwise
    /// look to a whole fleet like every tenant simultaneously disconnecting their integration.
    #[tokio::test]
    async fn a_sealed_vault_is_unreachable_and_a_refusal_is_denied() {
        let sealed = store(Recorded::new().reply(
            Method::Get,
            DATA_URL,
            503,
            r#"{"errors":["Vault is sealed"]}"#,
        ))
        .get(&reference())
        .await
        .expect_err("503");
        assert!(
            matches!(sealed, StoreError::Unreachable { ref reason, .. } if reason == "Vault is sealed"),
            "got {sealed:?}"
        );

        let denied = store(Recorded::new().reply(
            Method::Get,
            DATA_URL,
            403,
            r#"{"errors":["permission denied"]}"#,
        ))
        .get(&reference())
        .await
        .expect_err("403");
        assert!(
            matches!(denied, StoreError::Denied { ref reason, .. } if reason == "permission denied"),
            "got {denied:?}"
        );
    }

    /// A version deleted but not destroyed answers 200 with `data.data: null`. Reading that as an
    /// empty secret would hand a host a credential that is not there.
    #[tokio::test]
    async fn a_soft_deleted_version_is_not_an_empty_secret() {
        let body = json!({
            "data": {
                "data": null,
                "metadata": {
                    "created_time": "2026-07-30T00:00:00.000000000Z",
                    "deletion_time": "2026-07-30T00:00:01.000000000Z",
                    "destroyed": false,
                    "version": 1
                }
            }
        })
        .to_string();

        let error = store(Recorded::new().reply(Method::Get, DATA_URL, 200, &body))
            .get(&reference())
            .await
            .expect_err("a soft-deleted version holds nothing");
        assert!(error.is_not_found(), "got {error:?}");
    }

    /// A `200` whose envelope has no `data.data` is **named**, not reported as a missing credential.
    ///
    /// The flat `{"data": {…}}` fed here is a KV **v1** body, but naming this after v1 claimed more
    /// than it proved (C-149): it is reachable from a v1 mount only in the sub-case where a literal
    /// key called `data/<path>` happens to exist there, because v1 has no `data/` indirection and
    /// this store's URL therefore addresses that name. The *ordinary* v1 outcome is the 404 in the
    /// test below. Both are worth keeping, and worth keeping apart — the difference between them is
    /// whether a migration gets a message or a shrug.
    #[tokio::test]
    async fn a_two_hundred_without_data_data_is_named_rather_than_read_as_missing() {
        let body = json!({ "data": { "value": SENTINEL } }).to_string();

        let error = store(Recorded::new().reply(Method::Get, DATA_URL, 200, &body))
            .get(&reference())
            .await
            .expect_err("an envelope with no `data.data` is not readable here");
        assert!(
            matches!(error, StoreError::Backend { ref reason, .. } if reason.contains("KV v1")),
            "got {error:?}"
        );
    }

    /// Pointing this store at a **real** KV v1 mount reads as `NotFound`, not as the message above.
    ///
    /// v1 serves `/v1/<mount>/<path>` with no `data/` segment, so the URL this store builds asks a
    /// v1 mount for a literal key named `data/tenants/…`, which is not there, and Vault answers 404.
    /// action-proxy's credentials live on v1, so this — not the envelope message — is what a
    /// migration actually meets: every tenant reads as "has not connected that integration". The
    /// store is not wrong to say it, and the recorded reason it *is* worth asserting is that nobody
    /// debugging that should be looking for a transport fault.
    #[tokio::test]
    async fn a_real_kv_v1_mount_reads_as_not_found_because_the_data_prefix_is_a_literal_key() {
        let error = store(Recorded::new().reply(Method::Get, DATA_URL, 404, r#"{"errors":[]}"#))
            .get(&reference())
            .await
            .expect_err("a v1 mount has no `data/<path>` key");

        assert!(
            error.is_not_found(),
            "a v1 mount's 404 is indistinguishable from an unwritten address, got {error:?}"
        );
    }

    #[tokio::test]
    async fn a_secret_stored_under_another_field_is_reported_not_silently_missing() {
        let body = json!({ "data": { "data": { "token": SENTINEL }, "metadata": {} } }).to_string();

        let error = store(Recorded::new().reply(Method::Get, DATA_URL, 200, &body))
            .get(&reference())
            .await
            .expect_err("the default field is absent");
        assert!(
            matches!(error, StoreError::Backend { ref reason, .. } if reason.contains("with_field")),
            "got {error:?}"
        );
    }

    #[tokio::test]
    async fn a_custom_mount_and_field_reach_the_url_and_the_body() {
        let url = "https://vault.invalid:8200/v1/kv/data/tenants/9f3a4b2c/com.zendesk.api/support/api_token";
        let store = VaultStore::new(
            Recorded::new().reply(Method::Post, url, 204, ""),
            BASE,
            Secret::new(SENTINEL_TOKEN),
        )
        .with_mount("kv")
        .with_field("token");

        store
            .put(&reference(), &Secret::new(SENTINEL))
            .await
            .expect("put");

        let requests = store.transport.requests();
        assert_eq!(requests[0].url, url);
        let body: Value = serde_json::from_str(requests[0].body.as_deref().expect("a body"))
            .expect("the body is JSON");
        assert_eq!(body, json!({ "data": { "token": SENTINEL } }));
    }

    /// The decorator this epic exists for: swapping the layout moves the path and changes nothing
    /// else about the exchange.
    #[tokio::test]
    async fn a_non_default_layout_changes_the_path_and_nothing_else() {
        // The same stand-in `tests/layout_composition.rs` uses: a different root and a different
        // segment order, still lossless and still eliding `default`.
        struct Flat;
        impl Layout for Flat {
            fn render(&self, reference: &CredentialRef) -> String {
                format!(
                    "flux/{}/{}/{}/{}",
                    reference.authority(),
                    reference.tenant(),
                    reference.service(),
                    reference.credential()
                )
            }
            fn parse(&self, path: &str) -> Result<CredentialRef, String> {
                match path.split('/').collect::<Vec<_>>()[..] {
                    ["flux", authority, tenant, service, credential] => {
                        CredentialRef::new(tenant, authority, service, credential)
                    }
                    _ => Err(format!("{path:?} is not a flat path")),
                }
            }
        }

        let flat_url = "https://vault.invalid:8200/v1/secret/data/flux/com.zendesk.api/9f3a4b2c/support/api_token";
        let store = VaultStore::with_layout(
            Recorded::new().reply(Method::Get, flat_url, 200, &read_envelope(SENTINEL)),
            BASE,
            Secret::new(SENTINEL_TOKEN),
            Flat,
        );

        let secret = store.get(&reference()).await.expect("get");

        // Everything but the path is unchanged: same method, same envelope handling, same value.
        assert_eq!(secret.expose_secret(), SENTINEL);
        let requests = store.transport.requests();
        assert_eq!(requests[0].method, Method::Get);
        assert_eq!(requests[0].url, flat_url);
        assert_eq!(requests[0].token, SENTINEL_TOKEN);
        assert_eq!(
            store.path(&reference()),
            "flux/com.zendesk.api/9f3a4b2c/support/api_token"
        );
    }

    #[test]
    fn debug_does_not_print_the_token() {
        let rendered = format!("{:?}", store(Recorded::new()));
        assert!(!rendered.contains(SENTINEL_TOKEN), "{rendered}");
        assert!(rendered.contains("<redacted>"), "{rendered}");
    }

    #[test]
    fn a_base_url_with_a_trailing_slash_does_not_double_the_separator() {
        let store = VaultStore::new(
            Recorded::new(),
            "https://vault.invalid:8200/",
            Secret::new(SENTINEL_TOKEN),
        );
        assert_eq!(
            store.data_url("tenants/t/com.acme.api/token"),
            "https://vault.invalid:8200/v1/secret/data/tenants/t/com.acme.api/token"
        );
        assert_eq!(
            store.metadata_url("tenants/t/com.acme.api/token"),
            "https://vault.invalid:8200/v1/secret/metadata/tenants/t/com.acme.api/token"
        );
    }
}

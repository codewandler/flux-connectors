//! **The engine-free credential assembler** (C-557): a bound secret store in, the assembled
//! credentials and the set a redactor must hold out.
//!
//! This is the logic that lived in `connector-pack`'s `Credentials::resolve`/`resolve_mechanism`,
//! relocated so a consumer that never touches `connector-pack` and holds no flux `ToolContext` can run
//! it. What moved: the alternative-selection rule (the first mechanism whose credentials all resolve
//! wins), the acquisition axis ([`acquire`](crate::auth::acquire)), the placement, and the C-159
//! rule for **which forms must be redacted** — the stored value, the acquired value when it differs,
//! and the placed form when the placement transforms it.
//!
//! # What is *not* here, and is deliberately the consumer's
//!
//! This path **touches no redactor**. It returns the redaction set as data — a `Vec<`[`Redaction`]`>`,
//! each a [`SensitiveText`] and the credential it came from — and the consumer registers them with
//! its own redactor before using the plan. `connector-pack`'s wrapper registers them with the flux
//! redactor it holds and enforces flux's own "will this value actually be held" check (its
//! `UnredactableCredential`); a bare consumer holds whatever redactor it owns. The library resolves
//! the secret through the port and assembles it; where it is scrubbed is the host's.
//!
//! The secret port is [`SecretStore`] itself — the trait a host already binds — so a store failure
//! is that store's own [`StoreError`] rather than a re-spelling of it, and "not stored" stays apart
//! from "unreachable" exactly as `connector-pack` kept them.

use connector_secrets::{CredentialRef, InstanceId, SecretStore, StoreError};

use crate::auth::{self, Assembled};
use crate::config::{ConfigField, ConfigPort};
use crate::plan::SensitiveText;
use crate::Error;

/// The reserved service a provider-level credential addresses. `connector-address` owns the
/// definition and this crate depends on it, so this is the definition rather than a mirror.
use connector_address::DEFAULT_SERVICE;

/// **The credentials one operation's chosen mechanism resolves to, and everything to redact.**
///
/// `credentials` is the `Vec<Assembled>` [`resolve`](crate::resolve()) places on the request;
/// `redactions` is every credential-derived string that must be registered with a redactor **before**
/// the plan is used — the stored value, the acquired value and the placed form, per credential and in
/// that order.
///
/// Its `Debug` reveals no value: [`Assembled`] and [`SensitiveText`] both redact themselves.
#[derive(Debug)]
pub struct Assembly {
    /// The placed credentials, ready to hand to [`resolve`](crate::resolve()).
    pub credentials: Vec<Assembled>,
    /// The strings a redactor must hold before the plan is used, each tagged with its credential.
    pub redactions: Vec<Redaction>,
}

/// One credential-derived string a consumer must register with its redactor, and the credential it
/// came from.
///
/// The credential name is carried so a consumer that enforces its own "can this be held" check can
/// name the offending credential — which is exactly what `connector-pack`'s `UnredactableCredential`
/// does. The value is a [`SensitiveText`], so it does not reveal itself through `Debug`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Redaction {
    credential: String,
    value: SensitiveText,
}

impl Redaction {
    fn new(credential: &str, value: &str) -> Self {
        Self {
            credential: credential.to_owned(),
            value: SensitiveText::new(value),
        }
    }

    /// The flat-namespace name of the credential this string derives from.
    pub fn credential(&self) -> &str {
        &self.credential
    }

    /// The string to redact, still guarded.
    pub fn value(&self) -> &SensitiveText {
        &self.value
    }

    /// **The string to redact, exposed.** Named `expose_*` for the same reason
    /// [`SensitiveText::expose_secret`] is: every call site is a place a reviewer should stop at.
    pub fn expose(&self) -> &str {
        self.value.expose_secret()
    }
}

/// **Resolve, assemble and collect the redactions for one operation's credentials** (C-557).
///
/// `operation`'s declared mechanisms are tried in order; the first whose credentials all resolve
/// wins. `secrets` is the port a `CredentialRef` is resolved through, and `config` supplies the Basic
/// user half (non-secret, so it is config rather than a stored secret). `tenant` and `instance`
/// address the credential.
///
/// # Errors
///
/// [`Error::MissingCredential`] when no mechanism's credentials are all stored;
/// [`Error::CredentialStore`] when the store could not answer; [`Error::UndeclaredCredential`],
/// [`Error::InboundCredential`], [`Error::EmptyMechanism`], [`Error::NoCredentialAddress`],
/// [`Error::CredentialAddress`] and [`Error::MissingCredentialConfig`] as the composition demands.
/// None sends anything, and none carries a credential value.
pub async fn assemble_credentials(
    operation: &'static catalog::Operation,
    provider: &'static catalog::Provider,
    tenant: &str,
    instance: Option<&InstanceId>,
    secrets: &dyn SecretStore,
    config: &dyn ConfigPort,
) -> Result<Assembly, Error> {
    // An explicitly unauthenticated operation. Distinct from "nothing resolved", and the IR keeps the
    // two apart precisely so this branch can exist.
    if operation.credentials.is_empty() {
        return Ok(Assembly {
            credentials: Vec::new(),
            redactions: Vec::new(),
        });
    }

    let mut unmet: Vec<String> = Vec::new();
    for mechanism in operation.credentials {
        match assemble_mechanism(
            operation, provider, tenant, instance, secrets, config, mechanism,
        )
        .await
        {
            Ok(assembly) => return Ok(assembly),
            // Only "this tenant has not connected it" moves on to the next alternative.
            Err(Error::MissingCredential { path, .. }) => unmet.push(path),
            Err(other) => return Err(other),
        }
    }

    Err(Error::MissingCredential {
        operation: operation.id.to_owned(),
        path: unmet
            .first()
            .cloned()
            .unwrap_or_else(|| "<no address>".to_owned()),
        alternatives: unmet.len(),
    })
}

/// One mechanism: every credential in it, all or nothing.
#[allow(clippy::too_many_arguments)]
async fn assemble_mechanism(
    operation: &'static catalog::Operation,
    provider: &'static catalog::Provider,
    tenant: &str,
    instance: Option<&InstanceId>,
    secrets: &dyn SecretStore,
    config: &dyn ConfigPort,
    mechanism: &'static [&'static str],
) -> Result<Assembly, Error> {
    // A mechanism naming nothing would authenticate nothing while looking satisfied. The loader
    // refuses a degenerate empty mechanism; this is the second lock.
    if mechanism.is_empty() {
        return Err(Error::EmptyMechanism {
            operation: operation.id.to_owned(),
        });
    }

    let mut credentials = Vec::with_capacity(mechanism.len());
    let mut redactions = Vec::new();
    for name in mechanism {
        let credential = provider
            .credential(name)
            .ok_or_else(|| Error::UndeclaredCredential {
                operation: operation.id.to_owned(),
                credential: (*name).to_owned(),
                provider: provider.id.to_owned(),
            })?;

        // A signing secret never leaves, so reading one only to discover that is a round trip whose
        // sole outcome is this error. Refused before the store is touched.
        if matches!(credential.place, catalog::Placement::Inbound) {
            return Err(Error::InboundCredential {
                operation: operation.id.to_owned(),
                credential: credential.name.to_owned(),
            });
        }

        let reference = reference(operation.id, provider, tenant, instance, credential)?;
        let secret = match secrets.get(&reference).await {
            Ok(secret) => secret,
            Err(source) if source.is_not_found() => {
                return Err(Error::MissingCredential {
                    operation: operation.id.to_owned(),
                    path: not_found_path(&source),
                    alternatives: 1,
                });
            }
            Err(source) => {
                return Err(Error::CredentialStore {
                    operation: operation.id.to_owned(),
                    credential: credential.name.to_owned(),
                    source,
                });
            }
        };

        // **The stored value** — registered on its own terms because it is the form a vendor's 401
        // echoes, and a redactor holding it already covers a header prefix around it.
        redactions.push(Redaction::new(credential.name, secret.expose_secret()));

        let user = user_half(operation.id, credential, tenant, config)?;
        let value = auth::acquire(credential, secret.expose_secret(), user.as_deref());

        // **The acquired value**, when acquisition transformed the stored one — `base64(user:secret)`
        // is as good as the secret and shares no substring with either half.
        if value != secret.expose_secret() {
            redactions.push(Redaction::new(credential.name, &value));
        }

        // **The placed form**, when the placement transforms it too — a query placement
        // percent-encodes, and a base64 credential's `+`, `/`, `=` do not survive that (C-159).
        if let Some(travelling) = auth::placed_form(credential.place, &value) {
            redactions.push(Redaction::new(credential.name, &travelling));
        }

        credentials.push(Assembled::new(credential.name, value, credential.place));
    }

    Ok(Assembly {
        credentials,
        redactions,
    })
}

/// **Resolve one channel handshake's declared credential alternatives** (C-558).
///
/// The channel twin of [`assemble_credentials`], relocated from `connector-pack`'s
/// `Credentials::resolve_channel`: the first mechanism whose credentials all resolve wins, the
/// acquisition axis is applied, and the placed forms are handed back as [`Assembled`] for the channel
/// producer to put on the URL or the headers. It collects **no** redaction set of its own — the
/// channel plan carries its secret-bearing fields as [`SensitiveText`], so the redaction posture is
/// the plan's, not a returned `Vec`.
///
/// `operation` is the `provider#binding` label every refusal names; `requirements` is the channel's
/// `connect.auth` mechanisms; `secrets` is the port the credential is resolved through and `config`
/// supplies the Basic user half (with the channel's declared defaults already overlaid by the
/// caller).
///
/// # Errors
///
/// The same refusals [`assemble_credentials`] raises, for the channel's declared alternatives; none
/// sends anything, and none carries a credential value.
pub(crate) async fn resolve_channel(
    operation: &str,
    provider: &'static catalog::Provider,
    requirements: &'static [&'static [&'static str]],
    tenant: &str,
    instance: Option<&InstanceId>,
    secrets: &dyn SecretStore,
    config: &dyn ConfigPort,
) -> Result<Vec<Assembled>, Error> {
    // An explicitly unauthenticated handshake. Distinct from "nothing resolved".
    if requirements.is_empty() {
        return Ok(Vec::new());
    }
    let mut unmet: Vec<String> = Vec::new();
    for mechanism in requirements {
        let mut assembled = Vec::with_capacity(mechanism.len());
        let mut missing = None;
        for name in *mechanism {
            let credential =
                provider
                    .credential(name)
                    .ok_or_else(|| Error::UndeclaredCredential {
                        operation: operation.to_owned(),
                        credential: (*name).to_owned(),
                        provider: provider.id.to_owned(),
                    })?;
            // A signing secret never leaves — refused before the store is touched.
            if matches!(credential.place, catalog::Placement::Inbound) {
                return Err(Error::InboundCredential {
                    operation: operation.to_owned(),
                    credential: credential.name.to_owned(),
                });
            }
            let reference = reference(operation, provider, tenant, instance, credential)?;
            let secret = match secrets.get(&reference).await {
                Ok(secret) => secret,
                // Only "this tenant has not connected it" moves on to the next alternative.
                Err(source) if source.is_not_found() => {
                    missing = Some(not_found_path(&source));
                    break;
                }
                Err(source) => {
                    return Err(Error::CredentialStore {
                        operation: operation.to_owned(),
                        credential: credential.name.to_owned(),
                        source,
                    });
                }
            };
            let user = user_half(operation, credential, tenant, config)?;
            assembled.push(Assembled::new(
                credential.name,
                auth::acquire(credential, secret.expose_secret(), user.as_deref()),
                credential.place,
            ));
        }
        if let Some(path) = missing {
            unmet.push(path);
        } else {
            return Ok(assembled);
        }
    }

    Err(Error::MissingCredential {
        operation: operation.to_owned(),
        path: unmet
            .first()
            .cloned()
            .unwrap_or_else(|| "<no address>".to_owned()),
        alternatives: unmet.len(),
    })
}

/// Where `credential` of `provider` is kept for `tenant`'s selected connection.
fn reference(
    operation: &str,
    provider: &catalog::Provider,
    tenant: &str,
    instance: Option<&InstanceId>,
    credential: &catalog::Credential,
) -> Result<CredentialRef, Error> {
    let authority = provider
        .authority
        .ok_or_else(|| Error::NoCredentialAddress {
            operation: operation.to_owned(),
            provider: provider.id.to_owned(),
            credential: credential.name.to_owned(),
        })?;
    // Always the elided default service: a credential is declared at provider level, so it belongs
    // to the connector rather than to one of its surfaces.
    let reference = match instance {
        Some(instance) => CredentialRef::for_instance(
            tenant,
            authority,
            instance.as_str(),
            DEFAULT_SERVICE,
            credential.leaf,
        ),
        None => CredentialRef::new(tenant, authority, DEFAULT_SERVICE, credential.leaf),
    };
    reference.map_err(|reason| Error::CredentialAddress {
        operation: operation.to_owned(),
        credential: credential.name.to_owned(),
        reason,
    })
}

/// The user half of a Basic join, with its literal suffix, or `None` for every other acquisition.
///
/// The user half is **config, not a gated secret** — an email address or an account name — so it
/// comes through the configuration port. The suffix is the connector's declared data (zendesk's
/// `/token`), appended here so a host cannot get the join wrong.
fn user_half(
    operation: &str,
    credential: &'static catalog::Credential,
    tenant: &str,
    config: &dyn ConfigPort,
) -> Result<Option<String>, Error> {
    let catalog::Acquisition::BasicJoin {
        user_env,
        user_suffix,
    } = credential.acquire
    else {
        return Ok(None);
    };

    let user = config
        .resolve(ConfigField::Username(credential.name))
        .filter(|value| !value.value().trim().is_empty())
        .map(|value| value.value().to_owned())
        .ok_or_else(|| Error::MissingCredentialConfig {
            operation: operation.to_owned(),
            credential: credential.name.to_owned(),
            tenant: tenant.to_owned(),
            env: user_env.join(", "),
        })?;
    Ok(Some(format!("{user}{user_suffix}")))
}

/// The path a [`StoreError::NotFound`] names, taken from the error rather than re-rendered: a
/// reference renders differently under each layout, so quoting our own would send an operator to a
/// path their store does not use.
fn not_found_path(error: &StoreError) -> String {
    match error {
        StoreError::NotFound { path } => path.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use connector_secrets::{MemoryStore, Secret};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::config::ConfigValue;

    const TENANT: &str = "t-c557";
    const SENTINEL: &str = "SENTINEL-NOT-A-REAL-CREDENTIAL";

    /// A `ConfigPort` over a fixed map, keyed by the field's own name.
    struct MapPort(BTreeMap<String, String>);

    impl ConfigPort for MapPort {
        fn resolve(&self, field: ConfigField<'_>) -> Option<ConfigValue> {
            let name = match field {
                ConfigField::Endpoint(name) | ConfigField::Username(name) => name,
                // No operation-path credential reads a channel query; the assembler never asks.
                ConfigField::ChannelQuery { parameter, .. } => parameter,
            };
            self.0.get(name).map(ConfigValue::proposed)
        }
    }

    fn empty_port() -> MapPort {
        MapPort(BTreeMap::new())
    }

    /// Seed the sentinel at every address `entry`'s provider declares, and hand back the store.
    async fn seeded(
        entry: &'static catalog::Operation,
        provider: &'static catalog::Provider,
    ) -> Arc<MemoryStore> {
        let store = Arc::new(MemoryStore::new());
        for credential in provider.auth {
            if let Ok(reference) = reference(entry.id, provider, TENANT, None, credential) {
                store
                    .put(&reference, &Secret::new(SENTINEL))
                    .await
                    .expect("an in-memory put cannot fail");
            }
        }
        store
    }

    fn operation(id: &str) -> (&'static catalog::Operation, &'static catalog::Provider) {
        let entry = catalog::operation(catalog::OperationKey::id(id)).expect("a shipped operation");
        let provider =
            catalog::provider(catalog::ProviderKey::id(entry.provider)).expect("its provider");
        (entry, provider)
    }

    #[tokio::test]
    async fn a_bearer_credential_assembles_and_lists_its_redaction() {
        let (entry, provider) = operation("slack-chat-post-message");
        let store = seeded(entry, provider).await;
        let assembly =
            assemble_credentials(entry, provider, TENANT, None, store.as_ref(), &empty_port())
                .await
                .expect("the store holds the token");

        assert_eq!(assembly.credentials.len(), 1);
        assert_eq!(assembly.credentials[0].expose_value(), SENTINEL);
        // A header prefix does not transform the value, so the one redaction is the stored value.
        assert_eq!(
            assembly
                .redactions
                .iter()
                .map(Redaction::expose)
                .collect::<Vec<_>>(),
            vec![SENTINEL]
        );
        assert!(assembly.redactions[0].credential().contains("slack"));
    }

    #[tokio::test]
    async fn an_unstored_credential_refuses_and_names_the_alternatives() {
        let (entry, provider) = operation("slack-chat-post-message");
        let store = Arc::new(MemoryStore::new());
        let error =
            assemble_credentials(entry, provider, TENANT, None, store.as_ref(), &empty_port())
                .await
                .expect_err("nothing is stored");
        assert!(
            matches!(error, Error::MissingCredential { alternatives, .. } if alternatives >= 1),
            "{error}"
        );
        assert!(!error.to_string().contains(SENTINEL), "{error}");
    }

    #[tokio::test]
    async fn a_basic_join_reads_its_user_half_from_the_config_port() {
        let (entry, provider) = operation("zendesk-ticket-show");
        let store = seeded(entry, provider).await;
        let port = MapPort(BTreeMap::from([(
            "zendesk.api_token".to_string(),
            "ops@acme.test".to_string(),
        )]));
        let assembly = assemble_credentials(entry, provider, TENANT, None, store.as_ref(), &port)
            .await
            .expect("the store holds the token and the port holds the user half");

        // The stored secret and the assembled base64 pair are both redacted; the base64 is what
        // travels.
        let assembled = assembly.credentials[0].expose_value().to_string();
        assert_ne!(assembled, SENTINEL, "a Basic join transforms the value");
        let exposed: Vec<&str> = assembly.redactions.iter().map(Redaction::expose).collect();
        assert!(exposed.contains(&SENTINEL), "the stored value is redacted");
        assert!(
            exposed.contains(&assembled.as_str()),
            "the assembled pair is redacted"
        );
    }

    #[tokio::test]
    async fn a_basic_join_with_no_user_half_refuses_by_name() {
        let (entry, provider) = operation("zendesk-ticket-show");
        let store = seeded(entry, provider).await;
        let error =
            assemble_credentials(entry, provider, TENANT, None, store.as_ref(), &empty_port())
                .await
                .expect_err("no user half is bound");
        assert!(
            matches!(error, Error::MissingCredentialConfig { ref credential, .. } if credential == "zendesk.api_token"),
            "{error}"
        );
    }
}

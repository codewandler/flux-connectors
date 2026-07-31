//! **The credential port**: where a connector's credential *address* becomes a *value*.
//!
//! This is the adapter [the design](../../../docs/designs/connector-tool-pack.md) lists first under
//! "ports the host binds", and it is what finally gives C-90's addressing a consumer.
//!
//! # Bound at construction, never looked up
//!
//! [`Credentials`] is an argument to [`crate::pack`], and every operation it installs holds the one
//! that was handed in. There is no global, no `OnceLock`, no ambient default and no environment
//! fallback for the secret itself: a host that has not bound a store has connectors that refuse,
//! which is the correct posture for a thing that decides what authenticates an outgoing request.
//!
//! `Arc<dyn SecretStore>` is what makes that possible, and it is why C-91 made the trait object-safe
//! — *"the intended binding is injection at construction, not a global lookup."*
//!
//! # The address is C-90's, and it is not re-derived here
//!
//! `tenants/<tenant>/<authority>/<credential>` comes from
//! [`connector_secrets::CredentialRef`], which re-exports `connector-spec`'s. This module composes
//! one from facts the catalogue already carries — the provider's authority, the credential's leaf —
//! and the *store* renders it through whichever [`Layout`](connector_secrets::Layout) the host
//! configured. So a deployment with its own secret layout keeps it, and nothing here has an opinion
//! about paths.
//!
//! **A tenant id is untrusted input, and validating it is not vouching for it.**
//! [`Credentials::new`] refuses one that would traverse, exactly as `CredentialRef::new` does.
//! Deriving the tenant from an authenticated principal — never from request input — stays the
//! host's job, and no signature here can do it for them.
//!
//! # Alternatives are resolved in declared order, and a transport failure is never "not configured"
//!
//! An operation's `credentials` are an OR over mechanisms, each an AND over credentials. The rule is
//! the one `unified-auth.md` states: **the first mechanism whose credentials all resolve wins**, so
//! the choice is deterministic and a reader can see why. A [`StoreError::NotFound`] moves on to the
//! next mechanism; anything else — unreachable, denied, unusable — stops immediately and is
//! reported, because presenting a Vault outage as "this tenant has not connected it" is the
//! distinction C-91 built its error type around.
//!
//! # A credential the redactor cannot hold is refused, not sent
//!
//! Every value this module resolves is registered with the host's redactor through [`register`],
//! **before** anything fallible happens to it and long before a request exists. That call is also
//! where the guarantee is *verified*: `Redactor::add_secret` is a documented no-op for a value under
//! six trimmed characters, so registering a five-character credential succeeded and protected
//! nothing, and every surface would have rendered it in the clear. C-152's decision is to refuse such
//! a credential at resolve time with [`Error::UnredactableCredential`], naming the address so an
//! operator can replace the value. `docs/designs/connector-tool-pack.md` records why.
//!
//! # Nothing here holds a value beyond the call
//!
//! No cache, no expiry, no refresh. Out of scope since C-90 and still is: the store hands back a
//! value, and keeping it current is the host's problem. A cache added here would be a second,
//! differently-shaped copy of machinery flux already owns.

use std::sync::Arc;

use connector_secrets::{validate_tenant, CredentialRef, SecretStore, StoreError};
use flux_runtime::ToolContext;

use crate::auth::{self, Assembled};
use crate::config::Field;
use crate::{Configuration, Error};

/// The reserved service name [`CredentialRef::new`] elides, spelled here because a credential is
/// declared at **provider** level and therefore always addresses it.
///
/// `connector-spec` owns the definition (`ir::DEFAULT_SERVICE`), and this crate deliberately does not
/// depend on the loader — the pack's input is the catalogue. So this is a mirror, and a mirror is only
/// safe if drift is *checked* rather than promised. It is:
/// [`the_elided_service_is_the_one_the_addressing_reserves`](tests::the_elided_service_is_the_one_the_addressing_reserves)
/// builds a real [`CredentialRef`] with it and asserts the addressing agrees that it elides, which is
/// a stronger statement than string equality would be — it fails if the reserved name changes *or* if
/// the elision rule does.
pub const DEFAULT_SERVICE: &str = "default";

/// **The credential adapter a host binds when it constructs the pack.**
///
/// Holds a store and the tenant every address is rendered under. Cloning is cheap and shares the
/// store, which is what lets one bound port serve every operation of every provider in a pack.
#[derive(Clone)]
pub struct Credentials {
    store: Arc<dyn SecretStore>,
    tenant: String,
}

/// `Arc<dyn SecretStore>` is not `Debug`, and the tenant is the part worth seeing. The store is
/// deliberately unnamed: there is nothing safe *and* useful to print about it.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("tenant", &self.tenant)
            .finish_non_exhaustive()
    }
}

impl Credentials {
    /// Bind `store` as the place this pack's credentials are resolved from, for `tenant`.
    ///
    /// # Errors
    ///
    /// [`Error::Tenant`] when `tenant` is not a usable path segment — empty, over-long, or a
    /// spelling that would traverse. Refused here rather than at the first call, so a
    /// misconfiguration is a startup failure instead of a runtime one.
    pub fn new(store: Arc<dyn SecretStore>, tenant: &str) -> Result<Self, Error> {
        validate_tenant(tenant).map_err(|reason| Error::Tenant {
            tenant: tenant.to_owned(),
            reason,
        })?;
        Ok(Self {
            store,
            tenant: tenant.to_owned(),
        })
    }

    /// The tenant every address this port renders belongs to.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// Where `credential` of `provider` is kept for this port's tenant.
    ///
    /// # Errors
    ///
    /// [`Error::NoCredentialAddress`] when the connector declares no `authority` — without one the
    /// second segment of the path does not exist, so there is nothing to look up and the only honest
    /// answer is a refusal. [`Error::CredentialAddress`] when the components do not compose into a
    /// valid address.
    pub fn reference(
        &self,
        operation: &str,
        provider: &'static catalog::Provider,
        credential: &'static catalog::Credential,
    ) -> Result<CredentialRef, Error> {
        let authority = provider
            .authority
            .ok_or_else(|| Error::NoCredentialAddress {
                operation: operation.to_owned(),
                provider: provider.id.to_owned(),
                credential: credential.name.to_owned(),
            })?;
        // Always the elided default service: a credential is declared at provider level, so it
        // belongs to the connector rather than to one of its surfaces. `CredentialRef` can carry a
        // service, and that headroom is C-90's, for a vendor whose surfaces authenticate separately.
        CredentialRef::new(&self.tenant, authority, DEFAULT_SERVICE, credential.leaf).map_err(
            |reason| Error::CredentialAddress {
                operation: operation.to_owned(),
                credential: credential.name.to_owned(),
                reason,
            },
        )
    }

    /// Resolve, assemble and **register** every credential one of `operation`'s mechanisms needs.
    ///
    /// The registration is the safety-critical half and it happens here — before the caller has
    /// built a request at all, which is the ordering
    /// [the story](../../../docs/stories/C-116-credential-store-port.md) names and that
    /// `flux-web`'s `http.rs:248` set the precedent for. A failure between construction and dispatch
    /// then cannot surface a value the redactor has not already been told about.
    ///
    /// Both the stored secret **and** the assembled value are registered. For `Bearer` they are the
    /// same string; for Basic the assembled value is `base64(user:secret)`, which is as good as the
    /// secret to anyone holding it and is the one that actually travels. Both go through
    /// [`register`], so both are values the redactor demonstrably holds.
    ///
    /// # Errors
    ///
    /// Every variant refuses and none sends — including [`Error::UnredactableCredential`], for a
    /// value the host's redactor would silently decline to hold. See the module documentation for the
    /// alternative-selection rule and for why a transport failure is never reported as "not
    /// configured".
    pub(crate) async fn resolve(
        &self,
        ctx: &ToolContext,
        operation: &'static catalog::Operation,
        provider: &'static catalog::Provider,
        configuration: &Configuration,
    ) -> Result<Vec<Assembled>, Error> {
        // An explicitly unauthenticated operation — a health check, a ping. Distinct from "nothing
        // resolved", and the IR keeps the two apart precisely so this branch can exist.
        if operation.credentials.is_empty() {
            return Ok(Vec::new());
        }

        let mut unmet: Vec<String> = Vec::new();
        for mechanism in operation.credentials {
            match self
                .resolve_mechanism(ctx, operation, provider, mechanism, configuration)
                .await
            {
                Ok(assembled) => return Ok(assembled),
                // Only "this tenant has not connected it" moves on to the next alternative.
                Err(Error::MissingCredential { path, .. }) => unmet.push(path),
                Err(other) => return Err(other),
            }
        }

        // Every alternative was unmet. The first mechanism's first missing address is quoted, since
        // it is the one a connector's own documentation tells an operator to provision.
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
    async fn resolve_mechanism(
        &self,
        ctx: &ToolContext,
        operation: &'static catalog::Operation,
        provider: &'static catalog::Provider,
        mechanism: &'static [&'static str],
        configuration: &Configuration,
    ) -> Result<Vec<Assembled>, Error> {
        // A mechanism naming nothing would authenticate nothing while looking satisfied. The loader
        // refuses a degenerate empty mechanism; this is the second lock, because "the request went
        // out unauthenticated" is the failure that looks like success.
        if mechanism.is_empty() {
            return Err(Error::EmptyMechanism {
                operation: operation.id.to_owned(),
            });
        }

        let mut assembled = Vec::with_capacity(mechanism.len());
        for name in mechanism {
            let credential =
                provider
                    .credential(name)
                    .ok_or_else(|| Error::UndeclaredCredential {
                        operation: operation.id.to_owned(),
                        credential: (*name).to_owned(),
                        provider: provider.id.to_owned(),
                    })?;

            // Refused before the store is touched. A signing secret never leaves, so reading one in
            // order to discover that is a round trip whose only possible outcome is this error.
            if matches!(credential.place, catalog::Placement::Inbound) {
                return Err(Error::InboundCredential {
                    operation: operation.id.to_owned(),
                    credential: credential.name.to_owned(),
                });
            }

            let reference = self.reference(operation.id, provider, credential)?;
            let secret = self.store.get(&reference).await.map_err(|source| {
                if source.is_not_found() {
                    Error::MissingCredential {
                        operation: operation.id.to_owned(),
                        path: not_found_path(&source),
                        alternatives: 1,
                    }
                } else {
                    Error::CredentialStore {
                        operation: operation.id.to_owned(),
                        credential: credential.name.to_owned(),
                        source,
                    }
                }
            })?;

            // **Before any request exists, and before the fallible step below.** C-116 stated the
            // ordering as "registered before the request is constructed"; registering here rather
            // than after `user_half` — which consults the configuration port and can fail — closes
            // the window in which the value was in memory and the redactor had not been told
            // (C-152, finding 4). Nothing in that window could surface it, and the point is that the
            // code now says so.
            register(
                ctx,
                operation.id,
                credential,
                &reference,
                secret.expose_secret(),
            )?;

            let user = user_half(operation.id, provider, credential, configuration)?;
            let value = auth::acquire(credential, secret.expose_secret(), user.as_deref());

            // The second string: `base64(user:secret)` is as good as the secret to anyone holding it
            // and is the one that actually travels, so it is registered on its own terms.
            if value != secret.expose_secret() {
                register(ctx, operation.id, credential, &reference, &value)?;
            }

            assembled.push(Assembled {
                credential: credential.name,
                value,
                place: credential.place,
            });
        }
        Ok(assembled)
    }

}

/// The user half of a Basic join, with its literal suffix, or `None` for every other acquisition.
///
/// # It comes from the configuration port, not the process environment (C-193)
///
/// This used to read `Acquisition::BasicJoin::user_env` out of `std::env`, on the reasoning that the
/// user half is config rather than a gated secret and that flux resolves its own `AuthMethod` the
/// same way. The first half of that is still right — it is a non-secret, which is exactly why it is
/// not in the store — and the second half is what made it wrong here: **a server's environment holds
/// one value, and this is a per-tenant one.** `ZENDESK_USER` can name one customer's account; a pack
/// serving a second tenant would have signed its requests as the first. Fixing a templated host
/// while leaving this would have been half a migration, so it moves to the same port
/// ([`Field::Username`]), and this crate now reads no environment variable at all.
///
/// `user_env` stays in the catalogue and is quoted in the refusal below, because it is the name the
/// vendor's own documentation and flux's `AuthMethod` use for the same value — the fastest way for
/// an operator to recognise what they are being asked for.
///
/// # Errors
///
/// [`Error::MissingConfig`] when the tenant has not supplied it. Composing `base64(":<secret>")`
/// instead would produce a header the vendor answers with a 401 that says nothing about what is
/// missing.
fn user_half(
    operation: &str,
    provider: &'static catalog::Provider,
    credential: &'static catalog::Credential,
    configuration: &Configuration,
) -> Result<Option<String>, Error> {
    let catalog::Acquisition::BasicJoin {
        user_env,
        user_suffix,
    } = credential.acquire
    else {
        return Ok(None);
    };

    let user = configuration
        .require(operation, provider.id, Field::Username(credential.name))
        .map_err(|error| match error {
            // Re-stated with the vendor's own name for the value. `MissingConfig` alone would say
            // `username.zendesk.api_token`, which is right and is not what a Zendesk operator has
            // ever seen this called.
            Error::MissingConfig { .. } => Error::MissingCredentialConfig {
                operation: operation.to_owned(),
                credential: credential.name.to_owned(),
                tenant: configuration.tenant().to_owned(),
                env: user_env.join(", "),
            },
            other => other,
        })?;
    // The suffix is the connector's declared data — zendesk's `/token` — so it is appended here
    // rather than asked of a host, which cannot get it wrong and cannot be asked to know it.
    Ok(Some(format!("{user}{user_suffix}")))
}

/// **Register `value` with the host's redactor, or refuse the call.**
///
/// Every value this pack puts on a request goes through here, and that is what makes the guarantee
/// in the module documentation above a structural one rather than a promise: a value the redactor
/// does not hold is never assembled into a request, because this returns an error instead.
///
/// # Why it asks the redactor instead of checking a length
///
/// `Redactor::add_secret` **silently drops a value under six characters once trimmed**
/// (`codewandler-flux-secret-1.0.1/src/lib.rs:195-201`) — over-redacting a common English word is
/// the worse failure for it to risk, so the no-op is right for flux and wrong for a caller that
/// reads the call as a guarantee. Registering such a value *succeeds* and redacts nothing.
///
/// Mirroring the six here would be a constant that can rot silently on a flux upgrade, in exactly
/// the way [`DEFAULT_SERVICE`]'s mirror is guarded against. So the value is registered and the
/// redactor is then **asked** whether it holds it: if scrubbing the value returns the value, nothing
/// is protecting it, whatever the threshold happens to be. That also covers the empty and
/// all-whitespace cases without naming them.
fn register(
    ctx: &ToolContext,
    operation: &str,
    credential: &'static catalog::Credential,
    reference: &CredentialRef,
    value: &str,
) -> Result<(), Error> {
    ctx.redactor.add_secret(value.to_owned());
    if ctx.redactor.redact(value) == value {
        return Err(Error::UnredactableCredential {
            operation: operation.to_owned(),
            credential: credential.name.to_owned(),
            tenant: reference.tenant().to_owned(),
            authority: reference.authority().to_owned(),
        });
    }
    Ok(())
}

/// The path a [`StoreError::NotFound`] names.
///
/// Taken from the error rather than re-rendered here: a reference renders differently under each
/// [`Layout`](connector_secrets::Layout), so quoting our own rendering would send an operator to a
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
    use connector_secrets::{Layout, MemoryStore, TenantLayout};

    /// **The guard on [`DEFAULT_SERVICE`]'s mirror.**
    ///
    /// Asserted through the addressing type rather than against `connector_spec`'s constant, because
    /// this crate does not depend on the loader and should not start. That makes it the stronger
    /// check of the two: it fails if the reserved name changes, and it also fails if the *elision
    /// rule* changes — either of which would have this port writing a service segment into every
    /// credential path and looking up values nobody stored.
    #[test]
    fn the_elided_service_is_the_one_the_addressing_reserves() {
        let reference = CredentialRef::new("t-guard", "com.acme.api", DEFAULT_SERVICE, "token")
            .expect("a valid address");
        assert!(
            reference.is_default_service(),
            "`{DEFAULT_SERVICE}` is no longer the service the addressing elides"
        );
        assert!(
            !TenantLayout.render(&reference).contains(DEFAULT_SERVICE),
            "the default service rendered into the path: {}",
            TenantLayout.render(&reference)
        );
    }

    /// A tenant id is untrusted input on its way into a store path, and it is refused when the port
    /// is **bound** rather than at the first call — so a misconfiguration is a startup failure.
    #[test]
    fn a_traversing_tenant_is_refused_when_the_port_is_bound() {
        let error = Credentials::new(Arc::new(MemoryStore::new()), "../../etc")
            .expect_err("a traversing tenant cannot address anything");
        assert!(matches!(error, Error::Tenant { .. }), "{error}");
        assert!(error.to_string().contains("../../etc"), "{error}");
    }
}

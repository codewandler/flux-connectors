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
//! [`connector_secrets::CredentialRef`], which re-exports `connector-address`'s. That address gained an
//! optional instance level in C-406 — `…/<authority>/@instances/<uuid>/…`, for a tenant holding more
//! than one connection to one connector. [`Credentials::new`] composes the sole-connection form,
//! which is the same address it always rendered; [`Credentials::for_instance`] composes C-406's
//! existing UUID form after the host has resolved its own mutable label. What must not happen is a
//! second spelling being invented here. This module composes
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

use connector_secrets::{
    validate_tenant, CredentialRef, InstanceId, Secret, SecretStore, StoreError,
};
use flux_runtime::ToolContext;

use crate::auth::{self, Assembled};
use crate::config::{Field, Snapshot};
use crate::Error;

/// The reserved service name [`CredentialRef::new`] elides, spelled here because a credential is
/// declared at **provider** level and therefore always addresses it.
///
/// `connector-address` owns the definition (`connector_address::DEFAULT_SERVICE`, moved out of the
/// loader by C-407), and this crate deliberately does not depend on either — the pack's input is the
/// catalogue. So this is a mirror, and a mirror is only safe if drift is *checked* rather than
/// promised. It is:
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
    instance: Option<InstanceId>,
}

/// `Arc<dyn SecretStore>` is not `Debug`, and the tenant is the part worth seeing. The store is
/// deliberately unnamed: there is nothing safe *and* useful to print about it.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("tenant", &self.tenant)
            .field("instance", &self.instance)
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
            instance: None,
        })
    }

    /// Bind the credential store for one explicitly selected connection instance.
    ///
    /// The UUID is the existing C-406 address component. A host resolves any mutable label before
    /// calling this constructor; connector-pack never sees or guesses one.
    pub fn for_instance(
        store: Arc<dyn SecretStore>,
        tenant: &str,
        instance: &str,
    ) -> Result<Self, Error> {
        let mut credentials = Self::new(store, tenant)?;
        credentials.instance =
            Some(
                InstanceId::parse(instance).map_err(|reason| Error::Instance {
                    instance: instance.to_owned(),
                    reason,
                })?,
            );
        Ok(credentials)
    }

    /// The tenant every address this port renders belongs to.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// The selected connection UUID, absent for the sole-connection binding.
    pub fn instance(&self) -> Option<&InstanceId> {
        self.instance.as_ref()
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
        let reference = match &self.instance {
            Some(instance) => CredentialRef::for_instance(
                &self.tenant,
                authority,
                instance.as_str(),
                DEFAULT_SERVICE,
                credential.leaf,
            ),
            None => CredentialRef::new(&self.tenant, authority, DEFAULT_SERVICE, credential.leaf),
        };
        reference.map_err(|reason| Error::CredentialAddress {
            operation: operation.to_owned(),
            credential: credential.name.to_owned(),
            reason,
        })
    }

    /// **Divert a freshly minted secret into the bound store, and answer with its address** (C-136).
    ///
    /// The mirror image of [`resolve`](Self::resolve), through the same `Arc<dyn SecretStore>` and
    /// therefore under the same rule: the store is the one the host handed to [`crate::pack`], so an
    /// operation cannot mint into a store the host did not supply. A host that bound none has a
    /// login that refuses rather than a login that writes somewhere.
    ///
    /// # The redactor is told first, and it is the same ordering `resolve` uses
    ///
    /// [`register`] runs **before** the store write — the only fallible step after it — so a failure
    /// between "the value arrived" and "the value is stored" cannot surface it. That is the second
    /// line of defence the story asks for and not the guarantee: the guarantee is that this function
    /// returns a [`CredentialRef`] and there is no code path on which the value itself is returned.
    /// Registering it anyway matters because the value is now a value *this process is holding*,
    /// which is exactly the condition `Redactor` exists for.
    ///
    /// It also inherits C-152's refusal: a secret the host's redactor would silently decline to hold
    /// — under six characters once trimmed — is [`Error::UnredactableCredential`] and **is not
    /// stored**. A vendor answering a login with a five-character token is a misconfiguration long
    /// before it is a credential, and storing it would leave every later request placing a value no
    /// scrub covers.
    ///
    /// # Errors
    ///
    /// [`Error::NoCredentialAddress`] and [`Error::CredentialAddress`] exactly as
    /// [`reference`](Self::reference) raises them, [`Error::UnredactableCredential`] per the above,
    /// and [`Error::CredentialStore`] when the store could not be written. None of them carries the
    /// value: `StoreError` is documented as safe to log, and the two address errors name a path.
    pub(crate) async fn mint(
        &self,
        ctx: &ToolContext,
        operation: &'static catalog::Operation,
        provider: &'static catalog::Provider,
        credential: &'static catalog::Credential,
        secret: &str,
    ) -> Result<CredentialRef, Error> {
        let reference = self.reference(operation.id, provider, credential)?;
        register(ctx, operation.id, credential, &reference, secret)?;
        self.store
            .put(&reference, &Secret::new(secret))
            .await
            .map_err(|source| Error::CredentialStore {
                operation: operation.id.to_owned(),
                credential: credential.name.to_owned(),
                source,
            })?;
        Ok(reference)
    }

    /// Resolve, assemble and **register** every credential one of `operation`'s mechanisms needs.
    ///
    /// The registration is the safety-critical half and it happens here — before the caller has
    /// built a request at all, which is the ordering
    /// [the story](../../../docs/stories/C-116-credential-store-port.md) names and that
    /// `flux-web`'s `http.rs:248` set the precedent for. A failure between construction and dispatch
    /// then cannot surface a value the redactor has not already been told about.
    ///
    /// **Every form that travels is registered, not only the one that was stored.** For `Bearer`
    /// they are the same string. For Basic the assembled value is `base64(user:secret)`, which is as
    /// good as the secret to anyone holding it. For a query placement the *placed* form is
    /// percent-encoded, and for a base64 credential that shares no substring with either of the
    /// other two (C-159). Each goes through [`register`], so each is a value the redactor
    /// demonstrably holds; a header prefix asks for nothing extra, because it surrounds the value
    /// rather than transforming it.
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
        settings: &Snapshot,
    ) -> Result<Vec<Assembled>, Error> {
        // An explicitly unauthenticated operation — a health check, a ping. Distinct from "nothing
        // resolved", and the IR keeps the two apart precisely so this branch can exist.
        if operation.credentials.is_empty() {
            return Ok(Vec::new());
        }

        let mut unmet: Vec<String> = Vec::new();
        for mechanism in operation.credentials {
            match self
                .resolve_mechanism(ctx, operation, provider, mechanism, settings)
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

    /// Resolve one channel handshake's declared alternatives without constructing transport state.
    pub(crate) async fn resolve_channel(
        &self,
        channel: &str,
        provider: &'static catalog::Provider,
        requirements: &'static [&'static [&'static str]],
        settings: &Snapshot,
    ) -> Result<Vec<Assembled>, Error> {
        if requirements.is_empty() {
            return Ok(Vec::new());
        }
        let mut unmet = Vec::new();
        for mechanism in requirements {
            let mut assembled = Vec::with_capacity(mechanism.len());
            let mut missing = None;
            for name in *mechanism {
                let credential =
                    provider
                        .credential(name)
                        .ok_or_else(|| Error::UndeclaredCredential {
                            operation: channel.to_owned(),
                            credential: (*name).to_owned(),
                            provider: provider.id.to_owned(),
                        })?;
                if matches!(credential.place, catalog::Placement::Inbound) {
                    return Err(Error::InboundCredential {
                        operation: channel.to_owned(),
                        credential: credential.name.to_owned(),
                    });
                }
                let reference = self.reference(channel, provider, credential)?;
                let secret = match self.store.get(&reference).await {
                    Ok(secret) => secret,
                    Err(source) if source.is_not_found() => {
                        missing = Some(not_found_path(&source));
                        break;
                    }
                    Err(source) => {
                        return Err(Error::CredentialStore {
                            operation: channel.to_owned(),
                            credential: credential.name.to_owned(),
                            source,
                        });
                    }
                };
                let user = user_half(channel, credential, settings)?;
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
            operation: channel.to_owned(),
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
        settings: &Snapshot,
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

            let user = user_half(operation.id, credential, settings)?;
            let value = auth::acquire(credential, secret.expose_secret(), user.as_deref());

            // The second string: `base64(user:secret)` is as good as the secret to anyone holding it
            // and is the one that actually travels, so it is registered on its own terms.
            if value != secret.expose_secret() {
                register(ctx, operation.id, credential, &reference, &value)?;
            }

            // **And the third, when the placement transforms the value too** (C-159, finding 2).
            // A query placement percent-encodes on its way onto the URL, and `+`, `/` and `=` — a
            // base64 credential's whole alphabet — do not survive that, so the string on the wire
            // shared no substring with either string above. [`auth::placed_form`] is the single
            // answer to "does this placement transform"; a header prefix answers `None`, because it
            // surrounds the value and a redactor holding the bare form already covers it.
            if let Some(travelling) = auth::placed_form(credential.place, &value) {
                register(ctx, operation.id, credential, &reference, &travelling)?;
            }

            assembled.push(Assembled::new(credential.name, value, credential.place));
        }
        Ok(assembled)
    }
}

#[cfg(test)]
mod instance_tests {
    use super::*;

    #[test]
    fn an_instance_binding_renders_the_existing_instance_address() {
        let instance = "0d3f79ae-b6df-4f77-8f77-438436c3b2ef";
        let credentials = Credentials::for_instance(
            Arc::new(connector_secrets::MemoryStore::new()),
            "tenant-a",
            instance,
        )
        .expect("valid binding");
        let provider =
            catalog::provider(catalog::ProviderKey::id("github")).expect("shipped provider");
        let operation = provider.operations.first().expect("operation");
        let credential = provider.auth.first().expect("credential");
        let reference = credentials
            .reference(operation.id, provider, credential)
            .expect("address");

        assert_eq!(reference.instance().map(InstanceId::as_str), Some(instance));
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
    credential: &'static catalog::Credential,
    settings: &Snapshot,
) -> Result<Option<String>, Error> {
    let catalog::Acquisition::BasicJoin {
        user_env,
        user_suffix,
    } = credential.acquire
    else {
        return Ok(None);
    };

    let user = settings
        .require(operation, Field::Username(credential.name))
        .map_err(|error| match error {
            // Re-stated with the vendor's own name for the value. `MissingConfig` alone would say
            // `username.zendesk.api_token`, which is right and is not what a Zendesk operator has
            // ever seen this called.
            Error::MissingConfig { .. } => Error::MissingCredentialConfig {
                operation: operation.to_owned(),
                credential: credential.name.to_owned(),
                tenant: settings.tenant().to_owned(),
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
/// Every string this pack puts on a request either goes through here or **contains** one that did,
/// and that is what makes the guarantee in the module documentation above a structural one rather
/// than a promise: a value the redactor does not hold is never assembled into a request, because
/// this returns an error instead.
///
/// The distinction in that first sentence is C-159's correction, and it is not pedantry. What
/// travels is not always what was resolved: acquisition can transform the value (`base64(user:pass)`
/// contains neither half), and so can *placement* (a query parameter is percent-encoded, and a
/// header prefix is not). The rule that covers all three is the one
/// [`crate::auth`] states — **the redactor holds every form that is not recoverable from a form it
/// already holds** — and [`Credentials::resolve_mechanism`] registers each such form. The older
/// claim, "every value this pack puts on a request goes through here", was true of the door and
/// false of the bytes: it read as though a placement could only surround.
///
/// # Registering twice is a no-op, and it is verified rather than remembered
///
/// `Redactor::add_secret` pushes onto a `Vec` and dedupes nothing, while `redact` walks that set for
/// every scrub — so a process resolving the same credential on every call grew the set by an entry
/// per call, forever. This asks [`holds`] first and tells the redactor only what it does not already
/// have (C-159, finding 3).
///
/// **Asking is what makes it safe as well as idempotent.** A memo of `(address, value)` pairs kept
/// on this side would be a memory of some *earlier* redactor: `ExecutionEnvironment::new` constructs
/// one per environment, and whether that is per turn or per process is decided by a binding in flux
/// rather than here. A remembered registration against a redactor that never received the value is
/// precisely a credential travelling unheld — the failure this whole module exists to prevent — so
/// the question is put to the redactor in hand, every time.
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
    if !holds(ctx, value) {
        ctx.redactor.add_secret(value.to_owned());
    }
    if !holds(ctx, value) {
        return Err(Error::UnredactableCredential {
            operation: operation.to_owned(),
            credential: credential.name.to_owned(),
            tenant: reference.tenant().to_owned(),
            authority: reference.authority().to_owned(),
        });
    }
    Ok(())
}

/// **Whether the host's redactor already scrubs `value` by *registration*.**
///
/// Not `redact(value) != value`, and the difference is the whole point of the function existing.
/// `Redactor::redact` runs two passes: an exact-substring replacement over the registered set, and
/// then a scrub of credential-*shaped* tokens it was never told about — `sk-ant-…`, `xoxb-…`,
/// `ghp_…`. So the plain comparison answers "yes" for a value nobody registered, and a caller using
/// it to decide whether to register would skip precisely the tokens that look most like credentials.
///
/// [`PROBE`] is what separates the passes. Glued to the front of the value it leaves the shape rule
/// inapplicable — the run no longer begins with a known prefix, and a prefix is the only thing that
/// pass matches on — while the exact-substring pass is unaffected, because a registered value is
/// still a substring of the probed string. What changes the probe is therefore registration and
/// nothing else.
///
/// `tests/credentials.rs` keeps a sentinel carrying none of flux's known prefixes for the same
/// reason: a pass that came from shape rather than from registration would prove nothing.
///
/// # The condition this answer has, stated rather than assumed
///
/// `flux-secret` 1.0.1 exposes no membership test and no count, so what is observable is *coverage*,
/// not *identity*: this returns `true` when a registered value is a substring of `value`, which
/// includes but is wider than "`value` itself is registered". The two differ in exactly one case —
/// a **proper** substring of `value` is registered and the rest of `value` is not — and there the
/// skip would leave the surrounding fragment rendering in the clear where an unconditional
/// `add_secret` would not.
///
/// That case cannot arise from the three forms this port registers, which is why the trade is taken
/// rather than argued about. They are the stored value `S`, the acquired value `A` (either `S` or
/// `base64(user:S)`, which does not contain `S`), and the placed form `P` (either `A` or a
/// percent-encoding of it, which either equals `A` or escapes a character and so does not contain
/// it). Whitespace is the one real containment, and it is the case where skipping is *correct*:
/// `add_secret` stores values **trimmed**, so a stored value and its newline-terminated twin are one
/// entry either way. What remains is two distinct vendor secrets in one mechanism where one embeds
/// the other, which is a store holding a truncated copy of its own credential.
fn holds(ctx: &ToolContext, value: &str) -> bool {
    let probe = format!("{PROBE}{value}");
    ctx.redactor.redact(&probe) != probe
}

/// A byte that is neither a token boundary nor a line marker in `flux_secret`'s tokenizer, so
/// prefixing it defeats shape-based redaction without splitting the value. See [`holds`].
const PROBE: char = '\u{1}';

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
    use connector_secrets::{Layout, MemoryStore, Secret, TenantLayout};
    use flux_system::{System, Workspace};

    use crate::config::{Configuration, MemoryConfig};
    use crate::request::Request;

    /// The tenant the query-placement fixture below is addressed under.
    const QUERY_TENANT: &str = "t-c159";

    /// **A base64-shaped non-credential**, and the shape is the whole point: `+`, `/` and `=` are
    /// the three characters `query_encode` escapes, and they are exactly the alphabet a base64
    /// credential is made of.
    const QUERY_SENTINEL: &str = "SENTINEL+NOT/A+REAL+SECRET=C159";

    /// [`QUERY_SENTINEL`] as it appears on the URL, written out rather than computed by the
    /// encoder under test — an expectation derived the same way as the code it checks would pass
    /// against any encoder, correct or not.
    const QUERY_SENTINEL_ENCODED: &str = "SENTINEL%2BNOT%2FA%2BREAL%2BSECRET%3DC159";

    /// A `ToolContext` with its own redactor. Nothing here reaches the filesystem through it; the
    /// workspace root is this crate's own directory because `System` requires one that exists.
    fn context() -> ToolContext {
        let workspace = Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("the crate root exists");
        ToolContext::new(Arc::new(System::new(workspace)))
    }

    /// **The shipped slack provider with its bot token moved to a query placement.**
    ///
    /// Doctored deliberately, and this is the one branch where doctoring is the only option: the
    /// committed catalogue is 18 `Placement::Header` and 2 `Placement::Inbound` and **zero**
    /// `Placement::Query`, so nothing shipped reaches this code. That makes the path *more* worth a
    /// test rather than less — it is a fail-closed guarantee with no accidental coverage. Everything
    /// except the placement is real catalogue data, including the `com.slack.api` authority the
    /// address below is composed from.
    fn slack_with_a_query_placed_token() -> &'static catalog::Provider {
        let mut provider = *catalog::provider(catalog::ProviderKey::id("slack"))
            .expect("the shipped catalogue carries slack");
        provider.auth = Box::leak(Box::new([catalog::Credential {
            name: "slack.bot_token",
            leaf: "bot_token",
            acquire: catalog::Acquisition::Static,
            place: catalog::Placement::Query { name: "token" },
            subject: catalog::Subject::App,
            hazard: None,
        }]));
        Box::leak(Box::new(provider))
    }

    /// **The string that travels is a string the redactor holds** (C-159, finding 2).
    ///
    /// `auth::place` percent-encodes a query-placed credential onto the URL, and for a base64
    /// credential the encoded form shares no substring with the registered one — `+`, `/` and `=`
    /// all change. So the value that reached the wire was one the redactor had never been told
    /// about, while `register`'s own documentation claimed *"every value this pack puts on a request
    /// goes through here"*. The claim was true of the door and false of the bytes.
    ///
    /// The control is the first assertion: the encoded form really is what lands on the URL, so the
    /// scrub below is being asked about the string that actually travels.
    #[tokio::test]
    async fn a_query_placed_credential_registers_the_form_that_travels() {
        let provider = slack_with_a_query_placed_token();
        let operation = catalog::operation(catalog::OperationKey::id("slack-chat-post-message"))
            .expect("the shipped catalogue carries slack-chat-post-message");

        let store = MemoryStore::new();
        store
            .put(
                &CredentialRef::new(QUERY_TENANT, "com.slack.api", DEFAULT_SERVICE, "bot_token")
                    .expect("a valid address"),
                &Secret::new(QUERY_SENTINEL),
            )
            .await
            .expect("an in-memory put cannot fail");

        let ctx = context();
        let credentials = Credentials::new(Arc::new(store), QUERY_TENANT).expect("a valid tenant");
        let settings = Configuration::new(Arc::new(MemoryConfig::new()), QUERY_TENANT)
            .expect("a valid tenant")
            .snapshot(provider.id, operation.service, Vec::<Field>::new());

        let assembled = credentials
            .resolve(&ctx, operation, provider, &settings)
            .await
            .expect("the store holds the token");

        let mut request = Request {
            method: "POST".to_string(),
            url: "https://slack.com/api/chat.postMessage".to_string(),
            headers: std::collections::BTreeMap::new(),
            body: None,
        };
        for credential in &assembled {
            auth::place(operation.id, credential, &mut request).expect("a query placement");
        }

        assert!(
            request.url.contains(QUERY_SENTINEL_ENCODED),
            "the encoded form is what travels, or the scrub below asserts nothing: {}",
            request.url
        );
        assert!(
            !ctx.redactor
                .redact(&request.url)
                .contains(QUERY_SENTINEL_ENCODED),
            "the credential travelled in a form the redactor was never told about: {}",
            ctx.redactor.redact(&request.url)
        );
        // And the raw form stays registered. It is the form a vendor's 401 echoes and the one an
        // operator pastes into a shell, so the encoded registration is an addition rather than a
        // replacement.
        assert_ne!(
            ctx.redactor.redact(QUERY_SENTINEL),
            QUERY_SENTINEL,
            "the stored value must stay registered on its own terms"
        );
    }

    /// **Shape is not registration** (C-159, finding 3).
    ///
    /// The idempotence check has to key on the redactor's *registered set*, and the obvious spelling
    /// — `redact(value) != value` — does not: `redact` also scrubs credential-shaped tokens it was
    /// never told about, so that spelling reports a `sk-ant-…` value as already held and skips the
    /// one registration this port is responsible for. The value below is such a token, and the first
    /// assertion is the control that makes the second mean something.
    ///
    /// Skipping would not be harmless. flux's shape pass replaces a whole token, and a token ends at
    /// a boundary character it does not agree with this pack about — so a value the pack registers
    /// is scrubbed wherever it appears, and a value left to its shape is scrubbed wherever flux's
    /// tokenizer happens to agree.
    #[test]
    fn a_token_shape_the_redactor_scrubs_is_not_a_registration() {
        const SHAPED: &str = "sk-ant-SENTINEL-NOT-A-REAL-SECRET-C159";

        let ctx = context();
        assert_ne!(
            ctx.redactor.redact(SHAPED),
            SHAPED,
            "flux no longer scrubs this shape, so the assertion below proves nothing"
        );
        assert!(
            !holds(&ctx, SHAPED),
            "a value nobody registered was reported as held"
        );

        ctx.redactor.add_secret(SHAPED.to_owned());
        assert!(
            holds(&ctx, SHAPED),
            "a registered value must be reported as held, or nothing is ever idempotent"
        );
    }

    /// **The guard on [`DEFAULT_SERVICE`]'s mirror.**
    ///
    /// Asserted through the addressing type rather than against `connector_address`'s constant,
    /// because this crate reaches the addressing only through `connector-secrets`. That makes it the stronger
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

    // The Basic user half used to be driven from here, over a `Box::leak`ed zendesk doctored with an
    // authority, because no shipped connector had one and `reference` refused before the
    // configuration port was ever consulted. C-92 gave zendesk `com.zendesk.api`, so the doctoring
    // has no purpose left: both tests now live in `tests/credentials.rs` and drive the real
    // catalogue entry through the public `Operation::build_authenticated_request`.
}

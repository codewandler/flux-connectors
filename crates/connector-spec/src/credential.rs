//! Where a tenant's credential for a connector lives — the address, not the value and not the store.
//!
//! The [`config`](crate::config) module says *what to ask a human for* and where the answer goes on a
//! request. This one says where the answer is **kept**: a stable, tenant-scoped path a secret store
//! can be wrapped around.
//!
//! # This is a convention, not a client
//!
//! `docs/vision.md`'s non-goal is load-bearing — *"A runtime. This repo compiles; flux executes."* —
//! so nothing here opens a socket, holds a value, or knows what Vault is. What this repository is
//! uniquely placed to own is the **naming**: it already owns [`Pid`](crate::Pid),
//! [`Gid`](crate::Gid) and [`Oip`](crate::Oip), validates every component of them, and refuses an
//! address it cannot spell. A credential path is one more address derived from the same facts.
//!
//! The store itself is a host library's job, and a [`Layout`] is the seam: "wrap a simple Vault store
//! with some conventions" is exactly a decorator, where the client is commodity and the convention is
//! the part worth owning. [`TenantLayout`] is the blessed default so that two deployments do not
//! quietly diverge.
//!
//! ```text
//! tenants/<tenant>/<authority>/<service>/<credential>
//!
//! tenants/9f3a…/com.slack.api/signing_secret         ← `default` service elided
//! tenants/9f3a…/com.zendesk.api/support/api_token
//! tenants/9f3a…/com.amazonaws/s3/access_key
//! ```
//!
//! # The API version is deliberately absent
//!
//! A [`Gid`](crate::Gid) is `authority/service:version`. A credential path uses the **`pid` plus the
//! service** and drops the version, because **a token must survive the vendor's v2 migration**.
//! Putting the version in the path would force every tenant to re-provision the day Zendesk ships a
//! new API version — which is backwards, since the credential is precisely the thing that did *not*
//! change.
//!
//! A useful consequence: a path needs only an `authority`, not an `api_version`, so a provider is
//! half as far from having one.
//!
//! # A tenant id is untrusted input
//!
//! Every segment here reaches a filesystem-like path in a secret store, and the cautionary precedent
//! is close to home: action-proxy takes `x-babelforce-customer-id` and `x-babelforce-integration-id`
//! straight from client headers into a Vault path with no validation at all.
//!
//! So [`CredentialRef::new`] returns a `Result` and there is **no way to construct one that renders a
//! traversing path**. What it cannot do is vouch for *provenance*: validating a tenant id does not
//! make an attacker-supplied one safe to act on. Deriving the tenant server-side — from an
//! authenticated principal, never from request input — is the host's job and stays the host's job.

use crate::ir::DEFAULT_SERVICE;

/// The first segment of every path [`TenantLayout`] renders.
pub const TENANTS_ROOT: &str = "tenants";

/// The longest tenant id this will accept.
///
/// A UUID is 36 characters; this leaves generous room for a prefixed or composite id while refusing
/// the pathological. A bound exists at all because the value is untrusted and ends up in a path.
pub const MAX_TENANT: usize = 128;

/// Where one tenant's credential for one connector lives.
///
/// An **address**, not a value: there is deliberately no secret field on this type, so it can be
/// logged, compared and stored freely. The thing it points at is the host's to hold.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialRef {
    tenant: String,
    authority: String,
    service: String,
    credential: String,
}

impl CredentialRef {
    /// Build a reference, validating every component.
    ///
    /// `service` may be [`DEFAULT_SERVICE`], which renders as nothing — the same elision
    /// [`Gid`](crate::Gid) performs, and for the same reason.
    ///
    /// # Errors
    ///
    /// A reason string naming the offending component. The authority, service and credential are
    /// re-checked even though a loaded [`Connector`](crate::Connector) already validated them,
    /// because a reference can be built from outside one — a host resolving a path it was handed is
    /// exactly the case that matters.
    pub fn new(
        tenant: &str,
        authority: &str,
        service: &str,
        credential: &str,
    ) -> Result<Self, String> {
        validate_tenant(tenant)?;
        crate::address::validate_authority(authority)
            .map_err(|reason| format!("authority: {reason}"))?;
        if service != DEFAULT_SERVICE {
            crate::address::validate_service_name(service)
                .map_err(|reason| format!("service: {reason}"))?;
        }
        crate::address::validate_member_name(credential)
            .map_err(|reason| format!("credential: {reason}"))?;
        // The member grammar admits `.`, which is right for an event name (`issues.opened`) and wrong
        // here: a credential leaf is a single path segment, and a dotted one would read as a nested
        // path under a layout that split on it.
        if credential.contains('.') {
            return Err(format!(
                "credential {credential:?} contains `.`; the leaf of a credential path is one \
                 segment, and the vendor prefix belongs to the authority above it"
            ));
        }
        Ok(Self {
            tenant: tenant.to_owned(),
            authority: authority.to_owned(),
            service: service.to_owned(),
            credential: credential.to_owned(),
        })
    }

    /// The tenant this credential belongs to.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// The provider's reverse-DNS authority.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// The service, which is [`DEFAULT_SERVICE`] when the provider has one surface.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// The credential's local name — `api_token`, not `zendesk.api_token`.
    pub fn credential(&self) -> &str {
        &self.credential
    }

    /// Whether this addresses the reserved default service, and therefore renders with no service
    /// segment.
    pub fn is_default_service(&self) -> bool {
        self.service == DEFAULT_SERVICE
    }

    /// The provider this credential belongs to.
    pub fn pid(&self) -> crate::address::Pid {
        crate::address::Pid::new(&self.authority)
    }
}

/// How a [`CredentialRef`] becomes a path in some store.
///
/// A trait rather than one function, because a deployment that already has a secret layout should be
/// able to keep it — the point of this module is that the *convention* is pluggable while the
/// *address* is not. [`TenantLayout`] is the default; anything else is a deliberate choice a
/// deployment makes once.
///
/// # Contract
///
/// `parse(render(r)) == r` for every reference the layout can render. That law is what makes a path
/// usable as an identifier rather than only as a destination — a host reading a store back must be
/// able to say what it found. [`TenantLayout`] holds it through the `default`-service elision, and a
/// custom layout that cannot should say so by returning an error from [`parse`](Self::parse) rather
/// than guessing.
pub trait Layout {
    /// Render a reference into a store path.
    fn render(&self, reference: &CredentialRef) -> String;

    /// Recover a reference from a path this layout rendered.
    ///
    /// # Errors
    ///
    /// A reason string when the path is not one this layout produces.
    fn parse(&self, path: &str) -> Result<CredentialRef, String>;
}

/// The default layout: `tenants/<tenant>/<authority>/<service>/<credential>`.
///
/// The tenant leads because it is the segment a store's access control is most likely to be written
/// against — a Vault policy scoping a token to one customer is a prefix rule, and a prefix rule wants
/// the tenant first. `sbf/secrets` reached the same shape independently
/// (`tenants/<tenantID>/credentials/<id>`), which is the closest real precedent in this ecosystem;
/// action-proxy's `customer/<uuid>/integrations/<uuid>` is the same idea with the vendor identity
/// replaced by an opaque row id, so nothing about the path says which API it opens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TenantLayout;

impl Layout for TenantLayout {
    fn render(&self, reference: &CredentialRef) -> String {
        if reference.is_default_service() {
            format!(
                "{TENANTS_ROOT}/{}/{}/{}",
                reference.tenant, reference.authority, reference.credential
            )
        } else {
            format!(
                "{TENANTS_ROOT}/{}/{}/{}/{}",
                reference.tenant, reference.authority, reference.service, reference.credential
            )
        }
    }

    fn parse(&self, path: &str) -> Result<CredentialRef, String> {
        let segments: Vec<&str> = path.split('/').collect();
        if segments.first() != Some(&TENANTS_ROOT) {
            return Err(format!("{path:?} does not start with `{TENANTS_ROOT}/`"));
        }
        // Exactly one optional middle segment, which is what keeps the elision unambiguous — the same
        // property `Gid::parse` relies on.
        match segments.len() {
            4 => CredentialRef::new(segments[1], segments[2], DEFAULT_SERVICE, segments[3]),
            // Writing the reserved service out is a **second spelling of the elided form**, and two
            // paths for one address is how a store ends up holding the same credential twice with
            // nothing to say which is current. `Gid::parse` refuses it for the same reason.
            5 if segments[3] == DEFAULT_SERVICE => Err(format!(
                "{path:?} spells out the reserved {DEFAULT_SERVICE:?} service, which is elided — the \
                 path is `{TENANTS_ROOT}/{}/{}/{}`",
                segments[1], segments[2], segments[4]
            )),
            5 => CredentialRef::new(segments[1], segments[2], segments[3], segments[4]),
            n => Err(format!(
                "{path:?} has {n} segments; a credential path is \
                 `{TENANTS_ROOT}/<tenant>/<authority>/<credential>` or \
                 `{TENANTS_ROOT}/<tenant>/<authority>/<service>/<credential>`"
            )),
        }
    }
}

/// Whether `tenant` is safe to place in a path.
///
/// **Public, because a host validating an id before it builds a reference is the point.** The rules
/// are about what a path segment may contain, not about what a tenant id means — this crate has no
/// opinion on whether it is a UUID, and refusing a shape a deployment legitimately uses would be
/// worse than useless.
pub fn validate_tenant(tenant: &str) -> Result<(), String> {
    if tenant.is_empty() {
        return Err("a tenant id must not be empty".to_owned());
    }
    if tenant.len() > MAX_TENANT {
        return Err(format!(
            "a tenant id is at most {MAX_TENANT} characters, and this one is {}",
            tenant.len()
        ));
    }
    // `.` is admitted because real ids carry it; `..` never is, and neither is a leading or trailing
    // one — those are the spellings that traverse.
    if tenant == "." || tenant.contains("..") || tenant.starts_with('.') || tenant.ends_with('.') {
        return Err(format!(
            "tenant id {tenant:?} would traverse: a path segment may not be `.`, contain `..`, or \
             begin or end with `.`"
        ));
    }
    if let Some(bad) = tenant.chars().find(|c| !is_tenant_char(*c)) {
        return Err(format!(
            "tenant id {tenant:?} contains {bad:?}; a tenant id is ASCII letters, digits, `-`, `_` \
             and `.` — anything else could change which secret a path addresses"
        ));
    }
    Ok(())
}

/// The character class a tenant id admits. Both cases, because a UUID may arrive either way.
fn is_tenant_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> CredentialRef {
        CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token").expect("valid")
    }

    #[test]
    fn a_path_renders_and_parses_back() {
        let r = reference();
        let rendered = TenantLayout.render(&r);
        assert_eq!(
            rendered,
            "tenants/9f3a4b2c/com.zendesk.api/support/api_token"
        );
        assert_eq!(TenantLayout.parse(&rendered), Ok(r));
    }

    #[test]
    fn the_default_service_is_elided_and_still_round_trips() {
        let r = CredentialRef::new("9f3a", "com.slack.api", DEFAULT_SERVICE, "signing_secret")
            .expect("valid");
        let rendered = TenantLayout.render(&r);
        assert_eq!(rendered, "tenants/9f3a/com.slack.api/signing_secret");
        assert!(
            !rendered.contains(DEFAULT_SERVICE),
            "`default` never reaches a path"
        );
        assert_eq!(TenantLayout.parse(&rendered), Ok(r));
    }

    /// The rule the whole module exists to make unnecessary to remember.
    #[test]
    fn a_tenant_id_cannot_traverse() {
        for hostile in [
            "..",
            "../../etc",
            "a/b",
            "",
            ".",
            ".hidden",
            "trailing.",
            "a..b",
            "with space",
            "new\nline",
            "nul\0byte",
        ] {
            assert!(
                validate_tenant(hostile).is_err(),
                "tenant {hostile:?} must be refused"
            );
            assert!(
                CredentialRef::new(hostile, "com.acme.api", DEFAULT_SERVICE, "token").is_err(),
                "a reference must not be constructible from tenant {hostile:?}"
            );
        }
    }

    #[test]
    fn a_realistic_tenant_id_is_admitted() {
        for good in [
            "9f3a4b2c-1d5e-4f60-8a7b-2c3d4e5f6071",
            "9F3A4B2C1D5E4F608A7B2C3D4E5F6071",
            "acme_corp",
            "tenant.eu-west-1",
            "1",
        ] {
            assert!(
                validate_tenant(good).is_ok(),
                "tenant {good:?} must be admitted"
            );
        }
    }

    #[test]
    fn an_over_long_tenant_id_is_refused() {
        assert!(validate_tenant(&"a".repeat(MAX_TENANT)).is_ok());
        assert!(validate_tenant(&"a".repeat(MAX_TENANT + 1)).is_err());
    }

    #[test]
    fn every_other_component_is_validated_too() {
        // An authority that is not reverse-DNS.
        assert!(CredentialRef::new("t", "acme", DEFAULT_SERVICE, "token").is_err());
        // An authority carrying a separator would render a path that means something else.
        assert!(CredentialRef::new("t", "com.acme/x", DEFAULT_SERVICE, "token").is_err());
        // A service that is not spellable.
        assert!(CredentialRef::new("t", "com.acme.api", "../etc", "token").is_err());
        // An empty credential.
        assert!(CredentialRef::new("t", "com.acme.api", DEFAULT_SERVICE, "").is_err());
    }

    /// The leaf is one segment. `zendesk.api_token` is the *flat namespace* name; the path already
    /// carries the authority, so the vendor prefix would be said twice — and a layout splitting on
    /// `.` would read it as a nesting that was never intended.
    #[test]
    fn a_dotted_credential_leaf_is_refused() {
        assert!(
            CredentialRef::new("t", "com.zendesk.api", DEFAULT_SERVICE, "zendesk.api_token")
                .is_err()
        );
        assert!(CredentialRef::new("t", "com.zendesk.api", DEFAULT_SERVICE, "api_token").is_ok());
    }

    #[test]
    fn a_path_that_is_not_ours_is_refused_rather_than_guessed_at() {
        for foreign in [
            "customer/9f3a/integrations/abcd",     // action-proxy's shape
            "cloud/google/gemini",                 // the Go credentials-store shape
            "secret/data/flux/plugin/slack/token", // flux's Vault path
            "tenants/9f3a",                        // too short
            "tenants/9f3a/com.acme.api/a/b/c",     // too long
            "",
        ] {
            assert!(
                TenantLayout.parse(foreign).is_err(),
                "path {foreign:?} must not parse as ours"
            );
        }
    }
}

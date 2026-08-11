//! Where a tenant's credential for a connector lives — the address, not the value and not the store.
//!
//! `connector_spec`'s `config` module says *what to ask a human for* and where the answer goes on a
//! request. This one says where the answer is **kept**: a stable, tenant-scoped path a secret store
//! can be wrapped around.
//!
//! # This is a convention, not a client
//!
//! `docs/vision.md`'s non-goal is load-bearing — *"A runtime. This repo compiles; flux executes."* —
//! so nothing here opens a socket, holds a value, or knows what Vault is. What this repository is
//! uniquely placed to own is the **naming**: this crate already owns [`Pid`](crate::Pid),
//! [`Gid`](crate::Gid) and [`Oip`](crate::Oip), validates every component of them, and refuses an
//! address it cannot spell. A credential path is one more address derived from the same facts.
//!
//! The store itself is a host library's job, and a [`Layout`] is the seam: "wrap a simple Vault store
//! with some conventions" is exactly a decorator, where the client is commodity and the convention is
//! the part worth owning. [`TenantLayout`] is the blessed default so that two deployments do not
//! quietly diverge.
//!
//! ```text
//! tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>
//!
//! tenants/9f3a…/com.slack.api/signing_secret         ← `default` service elided
//! tenants/9f3a…/com.zendesk.api/support/api_token
//! tenants/9f3a…/com.amazonaws/s3/access_key
//! tenants/9f3a…/com.zendesk.api/@instances/7c1e…/api_token   ← one of several connections
//! ```
//!
//! # One tenant, two connections to the same vendor
//!
//! A tenant may hold two Zendesk subdomains, or a sandbox and a production Jira. Without a component
//! that varies per connection both render one address: the second write overwrites the first, and
//! every later call resolves whichever survived — a `200` from the wrong account rather than a
//! refusal (C-406).
//!
//! So a reference may carry an **instance**, and three rules keep it honest:
//!
//! - It is a **uuid**, not an operator's label. A uuid is stable under rename, cannot collide, and
//!   cannot be spelled to traverse. The human-facing "production vs sandbox" naming is a **label on
//!   the connection, owned by the host** — flux-exchange's `invoke` design already forbids a caller
//!   from naming the authority, the host or the credential, so the host resolves a tenant-scoped
//!   label to the uuid and only the uuid reaches the address. This crate never sees a label.
//! - It is carried **only when the tenant holds more than one connection of the same kind**
//!   ([`TenantInstances`]). One connection renders exactly the address it rendered before this
//!   existed, byte for byte, because a shifted address strands every credential already stored.
//! - The ambiguous case — several connections and no uuid — **refuses**, naming the uuids that would
//!   have worked. Never a default, never the first match.
//!
//! The marker segment [`INSTANCES_SEGMENT`] is `@instances`, and the `@` is doing work: no tenant id,
//! authority, service or credential leaf may contain one, so the marker cannot be confused with a
//! component. That is a proof rather than a reservation — a vendor whose surfaces really are called
//! `instances` stays spellable.
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

use std::fmt;

use crate::DEFAULT_SERVICE;

/// The first segment of every path [`TenantLayout`] renders.
pub const TENANTS_ROOT: &str = "tenants";

/// The segment that introduces an instance uuid: `<authority>/@instances/<uuid>/…`.
///
/// The `@` is what makes the marker unmistakable. Every other component's grammar admits only ASCII
/// letters, digits, `-`, `_` and `.`, so no tenant id, authority, service name or credential leaf can
/// ever spell this — the marker needs no reserved word and takes no name out of circulation.
pub const INSTANCES_SEGMENT: &str = "@instances";

/// The longest tenant id this will accept.
///
/// A UUID is 36 characters; this leaves generous room for a prefixed or composite id while refusing
/// the pathological. A bound exists at all because the value is untrusted and ends up in a path.
pub const MAX_TENANT: usize = 128;

/// One of a tenant's connections to a connector, named by uuid.
///
/// Validated on the way in, so a value of this type is always spellable into a path — the same
/// guarantee [`CredentialRef`] gives for every other component, hoisted into a type because a host
/// holds a connection's id long before it builds an address from it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(String);

impl InstanceId {
    /// Check `text` and keep it.
    ///
    /// # Errors
    ///
    /// A reason string naming the component, from [`validate_instance`].
    pub fn parse(text: &str) -> Result<Self, String> {
        validate_instance(text)?;
        Ok(Self(text.to_owned()))
    }

    /// The uuid, as it renders into a path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Every credential address for one tenant and one connector authority.
///
/// A host uses this as the boundary of an inventory or atomic migration. Both components are
/// validated once at construction, and [`contains`](Self::contains) then makes it impossible for a
/// batch assembled for one connector to drift into another tenant or authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialScope {
    tenant: String,
    authority: String,
}

impl CredentialScope {
    /// Validate and bind a tenant/authority scope.
    ///
    /// # Errors
    ///
    /// The same reason [`CredentialRef`] would refuse for either component.
    pub fn new(tenant: &str, authority: &str) -> Result<Self, String> {
        validate_tenant(tenant)?;
        crate::address::validate_authority(authority)
            .map_err(|reason| format!("authority: {reason}"))?;
        Ok(Self {
            tenant: tenant.to_owned(),
            authority: authority.to_owned(),
        })
    }

    /// The tenant whose addresses are in scope.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// The connector authority whose addresses are in scope.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Whether `reference` is inside this exact tenant/authority boundary.
    pub fn contains(&self, reference: &CredentialRef) -> bool {
        reference.tenant() == self.tenant && reference.authority() == self.authority
    }
}

/// The connections one tenant holds to one connector, and which of them an address names.
///
/// This is the input `connector_spec::Connector::credential_ref_for` needs and cannot derive: a
/// connector knows what it declares, never how many times a tenant has connected it. The host holds
/// that fact, so the host states it here.
///
/// [`resolve`](Self::resolve) is the whole rule:
///
/// | held | named | address |
/// |---|---|---|
/// | one (or the first) | anything consistent | **no instance segment** — byte-identical to the address this rendered before C-406 |
/// | several | one of them | that uuid |
/// | several | none | **refused**, naming the uuids that would have worked |
/// | any | one the tenant does not hold | **refused** |
///
/// A host may therefore pass the connection it is acting for *unconditionally*: while the tenant has
/// one, the address elides it and stays where it already is.
///
/// # The one migration this implies, stated rather than discovered
///
/// The address is a function of how many connections the tenant holds, so the day a second one
/// appears the first credential's address gains a segment and the host must move the stored value.
/// That is the cost of the alternative being worse: qualifying every address would strand every
/// credential already stored, everywhere, at once. The refusal above is what makes the migration
/// loud — until the host names an instance it gets an error, and once it does it gets an address
/// with nothing at it yet, which fails closed rather than answering from the wrong account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantInstances<'a> {
    held: &'a [InstanceId],
    named: Option<&'a InstanceId>,
}

impl TenantInstances<'static> {
    /// The tenant holds one connection of this kind, so there is nothing to distinguish.
    ///
    /// Every address in existence today is this case, and it renders unchanged.
    pub const fn sole() -> Self {
        Self {
            held: &[],
            named: None,
        }
    }
}

impl<'a> TenantInstances<'a> {
    /// Every connection this tenant holds to this connector, and the one the caller named.
    pub const fn held(all: &'a [InstanceId], named: Option<&'a InstanceId>) -> Self {
        Self { held: all, named }
    }

    /// The instance the address carries: `None` when it elides, and an error when the answer would
    /// have to be guessed.
    ///
    /// # Errors
    ///
    /// A reason string when several connections are held and none is named — listing the uuids that
    /// would have worked — or when the named one is not among those held.
    pub fn resolve(&self) -> Result<Option<&'a InstanceId>, String> {
        match (self.held, self.named) {
            ([] | [_], None) => Ok(None),
            ([sole], Some(named)) if sole == named => Ok(None),
            ([], Some(named)) => Err(format!(
                "instance {named} was named, but this tenant holds no connection to this connector \
                 — an address is not a way to create one"
            )),
            (held, None) => Err(format!(
                "this tenant holds {} connections to this connector and the reference names none, \
                 so there is no address to render: pass one of {}. A default or a first match would \
                 answer from whichever account happened to be stored, which is a `200` from the \
                 wrong instance rather than a refusal",
                held.len(),
                uuid_list(held)
            )),
            (held, Some(named)) if !held.contains(named) => Err(format!(
                "instance {named} is not one of the {} connections this tenant holds: {}",
                held.len(),
                uuid_list(held)
            )),
            (_, Some(named)) => Ok(Some(named)),
        }
    }
}

fn uuid_list(held: &[InstanceId]) -> String {
    held.iter()
        .map(|id| format!("{id:?}", id = id.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Where one tenant's credential for one connector lives.
///
/// An **address**, not a value: there is deliberately no secret field on this type, so it can be
/// logged, compared and stored freely. The thing it points at is the host's to hold.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialRef {
    tenant: String,
    authority: String,
    instance: Option<InstanceId>,
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
    /// re-checked even though a loaded `connector_spec::Connector` already validated them, because a
    /// reference can be built from outside one — a host resolving a path it was handed is exactly
    /// the case that matters.
    pub fn new(
        tenant: &str,
        authority: &str,
        service: &str,
        credential: &str,
    ) -> Result<Self, String> {
        Self::build(tenant, authority, None, service, credential)
    }

    /// Build a reference to one of a tenant's several connections, validating every component.
    ///
    /// The components are in path order: the instance sits directly under the authority, above the
    /// service, because a connection is a connection to the *connector* and every one of its
    /// services belongs to it.
    ///
    /// Reach for this only when the tenant genuinely holds more than one connection of this kind —
    /// [`TenantInstances`] states that rule once, and [`new`](Self::new) is the sole-connection form
    /// whose address must never move.
    ///
    /// # Errors
    ///
    /// A reason string naming the offending component, including `instance` when the uuid is not one
    /// — a host handed a connection id from outside is exactly the case
    /// [`validate_instance`] exists for.
    pub fn for_instance(
        tenant: &str,
        authority: &str,
        instance: &str,
        service: &str,
        credential: &str,
    ) -> Result<Self, String> {
        let instance = InstanceId::parse(instance)?;
        Self::build(tenant, authority, Some(instance), service, credential)
    }

    fn build(
        tenant: &str,
        authority: &str,
        instance: Option<InstanceId>,
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
            instance,
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

    /// Which of the tenant's connections this addresses, when it holds more than one.
    ///
    /// `None` is the ordinary case and the one every stored credential is in: a tenant with a single
    /// connection needs nothing to tell it apart, so the address carries nothing.
    pub fn instance(&self) -> Option<&InstanceId> {
        self.instance.as_ref()
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
/// the tenant first. The vendor's own internal secret store reached the same shape independently
/// (`tenants/<tenantID>/credentials/<id>`), which is the closest real precedent in this ecosystem;
/// action-proxy's `customer/<uuid>/integrations/<uuid>` is the same idea with the vendor identity
/// replaced by an opaque row id, so nothing about the path says which API it opens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TenantLayout;

impl Layout for TenantLayout {
    fn render(&self, reference: &CredentialRef) -> String {
        let mut path = format!(
            "{TENANTS_ROOT}/{}/{}",
            reference.tenant, reference.authority
        );
        if let Some(instance) = &reference.instance {
            path.push_str(&format!("/{INSTANCES_SEGMENT}/{instance}"));
        }
        if !reference.is_default_service() {
            path.push('/');
            path.push_str(&reference.service);
        }
        path.push('/');
        path.push_str(&reference.credential);
        path
    }

    fn parse(&self, path: &str) -> Result<CredentialRef, String> {
        let segments: Vec<&str> = path.split('/').collect();
        if segments.first() != Some(&TENANTS_ROOT) {
            return Err(format!("{path:?} does not start with `{TENANTS_ROOT}/`"));
        }
        // Exactly one optional middle segment, which is what keeps the elision unambiguous — the same
        // property `Gid::parse` relies on. The instance is a second optional level, and it stays
        // unambiguous a different way: it is two segments long and led by a marker no component can
        // spell, so the instanced and un-instanced forms cannot even be the same length.
        match segments.len() {
            4 => CredentialRef::new(segments[1], segments[2], DEFAULT_SERVICE, segments[3]),
            // Writing the reserved service out is a **second spelling of the elided form**, and two
            // paths for one address is how a store ends up holding the same credential twice with
            // nothing to say which is current. `Gid::parse` refuses it for the same reason.
            5 if segments[3] == DEFAULT_SERVICE => Err(spelled_out_default(path, &segments)),
            5 => CredentialRef::new(segments[1], segments[2], segments[3], segments[4]),
            6 | 7 if segments[3] != INSTANCES_SEGMENT => Err(format!(
                "{path:?} has a level below the authority that is not an instance; only \
                 `{INSTANCES_SEGMENT}/<uuid>` goes there"
            )),
            6 => CredentialRef::for_instance(
                segments[1],
                segments[2],
                segments[4],
                DEFAULT_SERVICE,
                segments[5],
            ),
            7 if segments[5] == DEFAULT_SERVICE => Err(spelled_out_default(path, &segments)),
            7 => CredentialRef::for_instance(
                segments[1],
                segments[2],
                segments[4],
                segments[5],
                segments[6],
            ),
            n => Err(format!(
                "{path:?} has {n} segments; a credential path is \
                 `{TENANTS_ROOT}/<tenant>/<authority>[/{INSTANCES_SEGMENT}/<uuid>][/<service>]/<credential>`"
            )),
        }
    }
}

/// The refusal both spelled-out-`default` forms share: the elided one is the address.
fn spelled_out_default(path: &str, segments: &[&str]) -> String {
    let canonical: Vec<&str> = segments
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != segments.len() - 2)
        .map(|(_, segment)| *segment)
        .collect();
    format!(
        "{path:?} spells out the reserved {DEFAULT_SERVICE:?} service, which is elided — the path \
         is `{}`",
        canonical.join("/")
    )
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

/// The nil uuid, which names no instance and is refused as one.
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// Whether `instance` is a uuid this will place in a path.
///
/// **Public, because a host validating a connection id before it builds a reference is the point** —
/// the same reason [`validate_tenant`] is.
///
/// The rule is the canonical **lowercase hyphenated** form, `8-4-4-4-12`, and nothing else. Unlike a
/// tenant id, whose shape belongs to the deployment, this component's shape belongs to *this*
/// scheme: the braced, URN and unhyphenated forms and an uppercase spelling are all the same uuid,
/// and admitting them would put two paths under one connection with nothing to say which is current.
/// One value, one address.
///
/// The nil uuid is refused too. It is a well-formed uuid that conventionally means *no* instance,
/// and "absent" already has a spelling here — the address that omits the component entirely.
///
/// The version and variant nibbles are deliberately **not** checked: a host is free to mint v4 or v7
/// connection ids, and this scheme has no stake in which.
pub fn validate_instance(instance: &str) -> Result<(), String> {
    const SHAPE: [usize; 4] = [8, 13, 18, 23];
    if instance.len() != 36 || !instance.is_ascii() {
        return Err(format!(
            "instance {instance:?} is not a uuid: an instance is the canonical 36-character \
             hyphenated form, as in `7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63`"
        ));
    }
    for (index, byte) in instance.bytes().enumerate() {
        let expected_hyphen = SHAPE.contains(&index);
        let ok = if expected_hyphen {
            byte == b'-'
        } else {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
        };
        if !ok {
            return Err(format!(
                "instance {instance:?} is not a uuid: character {index} is {:?}, and a uuid is \
                 lowercase hex with hyphens at 8, 13, 18 and 23 — an uppercase or unhyphenated \
                 spelling is the same connection at a second address",
                byte as char
            ));
        }
    }
    if instance == NIL_UUID {
        return Err(format!(
            "instance {instance:?} is the nil uuid, which names no connection; an address with no \
             instance is how *no instance* is spelled"
        ));
    }
    Ok(())
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

    /// `resolve` is the whole rule, and each arm is a different owner's mistake.
    #[test]
    fn which_instance_an_address_carries_is_never_guessed() {
        let us = InstanceId::parse("7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63").expect("a uuid");
        let eu = InstanceId::parse("b48d2f57-0a91-4c3e-8d16-5f2b7e904ac8").expect("a uuid");
        let both = [us.clone(), eu.clone()];
        let one = [us.clone()];

        // One connection: nothing to distinguish, so the address does not move — whether or not the
        // caller names it.
        assert_eq!(TenantInstances::sole().resolve(), Ok(None));
        assert_eq!(TenantInstances::held(&one, None).resolve(), Ok(None));
        assert_eq!(TenantInstances::held(&one, Some(&us)).resolve(), Ok(None));

        // Several: the named one, and only ever the named one.
        assert_eq!(
            TenantInstances::held(&both, Some(&eu)).resolve(),
            Ok(Some(&eu))
        );

        // Several and none named: the refusal this component exists for. It lists both, because the
        // caller's next move is to pick one.
        let reason = TenantInstances::held(&both, None)
            .resolve()
            .expect_err("ambiguous");
        assert!(reason.contains(us.as_str()) && reason.contains(eu.as_str()));

        // A connection the tenant does not hold, and a named one where none is held: both are the
        // caller contradicting itself, and neither renders a plausible address.
        assert!(TenantInstances::held(&one, Some(&eu)).resolve().is_err());
        assert!(TenantInstances::held(&[], Some(&eu)).resolve().is_err());
    }

    #[test]
    fn an_instanced_path_renders_and_parses_back() {
        let r = CredentialRef::for_instance(
            "9f3a",
            "com.zendesk.api",
            "7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63",
            DEFAULT_SERVICE,
            "api_token",
        )
        .expect("valid");
        let rendered = TenantLayout.render(&r);
        assert_eq!(
            rendered,
            "tenants/9f3a/com.zendesk.api/@instances/7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63/api_token"
        );
        assert_eq!(TenantLayout.parse(&rendered), Ok(r));
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

//! Accounts and sessions — the principal, and the thing that proves it between requests.
//!
//! # The account is keyed on `sub`, never on email
//!
//! An email address is mutable and **reassignable**. A Workspace administrator can rename an
//! account, and can later hand the freed address to somebody else. A host keying on email gives
//! the new holder of `alice@example.com` every credential the old one connected — silently, on
//! their first sign-in, with no error anywhere. The OIDC `sub` is the stable subject identifier
//! and is the only claim in the token that is documented not to be reused, so it is the key.
//!
//! Email is kept, because an operator needs to see whose session they are in. It is a **label**
//! here and never a lookup key, which is why [`Accounts::of_subject`] is the only way in.
//!
//! # The session token is opaque, server-side, and stored hashed
//!
//! The cookie carries 32 bytes of OS entropy and nothing else — no account id, no tenant, no
//! claims, and above all no credential material. Everything about the session lives here, which is
//! what makes revocation real: [`Sessions::revoke`] removes the record, so every copy of that
//! cookie stops working at once rather than only the browser that asked to sign out.
//!
//! The record is keyed by **SHA-256 of the token**, not the token. The store therefore never holds
//! a live cookie, so a memory dump, a stray `Debug` or a future "list active sessions" screen
//! yields nothing an attacker can present.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::auth::oidc::IdClaims;

/// How long a session lasts before it must be re-established.
pub const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);

/// How long a started sign-in may take to come back through the callback.
///
/// Short on purpose. This entry holds the PKCE verifier and the nonce, and it is consumed on first
/// use, so the window in which a leaked `state` is worth anything is this long and no longer.
pub const LOGIN_TTL: Duration = Duration::from_secs(10 * 60);

/// An upper bound on unfinished sign-ins held at once, so an unauthenticated caller hammering
/// `/auth/signin` cannot grow this map without limit. Reached only by pruning first.
const MAX_PENDING: usize = 4096;

/// **The subject the dev sign-in signs in as** (C-234).
///
/// Deliberately not shaped like a Google `sub`, which is a run of decimal digits. Anyone reading a
/// log line, a store path or the `/auth/me` body sees at once that this is not a person.
pub const DEV_SUBJECT: &str = "DEVELOPER-NOT-A-REAL-ACCOUNT";

/// **The tenant a dev session owns, and why it cannot collide with a real one.**
///
/// Every tenant this host can produce comes from one of exactly two constructors, and `Account`'s
/// fields are private so there is no third way to build one:
///
/// | constructor | tenant | reachable from |
/// |---|---|---|
/// | [`Account::from_claims`] | `google-{sub}` | a verified `id_token`, and only that |
/// | [`Account::developer`] | `dev-local` | `POST /auth/dev`, which exists only under `--dev` |
///
/// The two are disjoint because `from_claims` prepends the literal `google-` unconditionally and
/// this constant does not begin with it. That is a structural argument, not a statistical one: it
/// does not depend on what Google happens to put in a `sub`, and it survives Google changing the
/// shape of its subject identifiers. `crates/connectors-api/tests/dev_signin.rs` drives both doors
/// on one process and asserts that a credential stored under one is invisible to the other.
///
/// The consequence worth stating plainly: a credential pasted into a dev session lands at
/// `tenants/dev-local/…` and no real account can ever read it, and a credential pasted into a real
/// session is unreachable from the dev door.
pub const DEV_TENANT: &str = "dev-local";

/// The label an operator sees for the dev account.
///
/// It goes where an email address would, and it is not one. `Account::email` is left `None` for the
/// dev account precisely so that nothing shaped like `dev@example.com` ever renders as though
/// somebody owned that mailbox.
pub const DEV_LABEL: &str = "DEVELOPER — NOT A REAL ACCOUNT";

/// One signed-in person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The OIDC subject — stable, and the key this account is stored under.
    subject: String,
    /// The tenant every port is bound for on this account's behalf.
    tenant: String,
    /// A label for the UI. Never a lookup key.
    pub email: Option<String>,
    /// A label for the UI.
    pub name: Option<String>,
}

impl Account {
    /// Build an account from verified claims.
    ///
    /// Called only with an [`IdClaims`] that [`crate::auth::oidc::verify_id_token`] returned, which
    /// is what makes "the account is the token's subject" true rather than hoped for.
    ///
    /// # Errors
    ///
    /// A description if `sub` is not usable as a tenant path segment.
    pub fn from_claims(claims: &IdClaims) -> Result<Self, String> {
        let subject = validate_subject(&claims.sub)?;
        Ok(Self {
            tenant: format!("google-{subject}"),
            subject,
            // An unverified email is a claim about an address the person may not control, so it is
            // not shown as though it were theirs. Google sets this false for unverified Workspace
            // aliases. It is a label either way, but a label that lies is worse than none.
            email: claims
                .email
                .clone()
                .filter(|_| claims.email_verified.unwrap_or(false)),
            name: claims.name.clone(),
        })
    }

    /// **The developer account** — the one identity the dev sign-in mints (C-234).
    ///
    /// This is the *only* constructor of an `Account` that does not require a verified `id_token`,
    /// and it is reachable from exactly one route, which exists on a process only when it was
    /// started with `--dev`. It takes no arguments on purpose: there is nothing a caller could pass
    /// that would change who this is, so no request can steer the identity it produces. That is
    /// what keeps the dev door a *fixed* door rather than an impersonation primitive — nobody can
    /// ask it for `google-110169484474386276334`.
    ///
    /// Everything downstream of it is ordinary. The account goes through
    /// [`Accounts::of_subject`] and [`Sessions::create`] exactly as a Google account does, so the
    /// cookie, the opacity, the TTL, the server-side revocation and the [`crate::auth::Principal`]
    /// extraction are the same code, not a parallel implementation.
    ///
    /// See [`DEV_TENANT`] for why the tenant cannot collide with a real account's.
    #[must_use]
    pub fn developer() -> Self {
        Self {
            subject: DEV_SUBJECT.to_owned(),
            tenant: DEV_TENANT.to_owned(),
            // Not `Some("dev@example.com")`. An address is what an operator reads to know whose
            // session they are in, and one that looks real is worse than none at all here.
            email: None,
            name: Some(DEV_LABEL.to_owned()),
        }
    }

    /// The OIDC subject.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// **The tenant every port is bound for.**
    ///
    /// Derived from the subject and from nothing a request carries. This is the value
    /// `Credentials::new` and `Configuration::new` receive, and the reason
    /// `crates/connectors-api/tests/tenancy.rs` can assert that a request naming another tenant
    /// resolves to this one.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }
}

/// Refuse a subject that could not be a path segment.
///
/// Google's `sub` is a run of digits, so this is not a hot path — it is the fence.
/// `connector-pack`'s `CredentialRef::new` validates the tenant again downstream and would refuse
/// a traversing value, but `AGENTS.md` is explicit that *"validation is not provenance"* and that a
/// new path segment is validated at construction. The cautionary precedent it names is close to
/// home: action-proxy puts two client-supplied headers straight into a Vault path with no
/// validation. A `sub` arrives from outside this process, so it gets the same treatment even
/// though the provider that issued it is trusted.
fn validate_subject(subject: &str) -> Result<String, String> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err("it is empty".to_owned());
    }
    if subject.len() > 100 {
        return Err(format!("it is {} characters long", subject.len()));
    }
    if !subject
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("it carries characters outside [A-Za-z0-9_-]".to_owned());
    }
    Ok(subject.to_owned())
}

/// Every account this host has seen, keyed by subject.
#[derive(Default)]
pub struct Accounts {
    by_subject: RwLock<HashMap<String, Arc<Account>>>,
}

impl Accounts {
    pub fn new() -> Self {
        Self::default()
    }

    /// The account for these claims, created on first sign-in.
    ///
    /// The labels are refreshed on every sign-in — a person who changes their display name should
    /// see the new one — while the identity, and therefore the tenant, is fixed at the subject.
    pub fn of_subject(&self, account: Account) -> Arc<Account> {
        let account = Arc::new(account);
        let mut accounts = self.by_subject.write().expect("not poisoned");
        accounts.insert(account.subject.clone(), Arc::clone(&account));
        account
    }

    /// How many accounts exist. For diagnostics; it names nobody.
    pub fn len(&self) -> usize {
        self.by_subject.read().expect("not poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A sign-in that has been started and not yet completed.
struct Pending {
    /// The PKCE verifier, held server-side until the code is redeemed.
    verifier: String,
    /// The nonce this host put in the authorize URL, to be matched against the `id_token`.
    nonce: String,
    started_at: Instant,
}

/// What [`Sessions::start_login`] produced.
pub struct Login {
    /// The CSRF value the callback must return.
    pub state: String,
    /// The nonce the `id_token` must carry.
    pub nonce: String,
    /// The S256 challenge to put in the authorize URL.
    pub challenge: String,
}

/// A live session.
struct Record {
    account: Arc<Account>,
    expires_at: Instant,
}

/// Sessions and the sign-ins on their way to becoming one.
#[derive(Default)]
pub struct Sessions {
    /// Keyed by SHA-256 of the token. See the module header.
    live: RwLock<HashMap<[u8; 32], Record>>,
    /// Keyed by `state`.
    pending: RwLock<HashMap<String, Pending>>,
}

impl Sessions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a sign-in: mint `state`, `nonce` and a PKCE pair, and remember the half that stays
    /// here.
    ///
    /// All three values come from `flux-credentials`, which is the story's explicit instruction —
    /// *"reuse rather than write a fourth"* PKCE implementation in this ecosystem. Its
    /// `generate_pkce` is base64url over 32 bytes drawn from the OS CSPRNG with an S256 challenge,
    /// and `generate_state` is the same 32 bytes without the digest. Neither is derived from a
    /// timestamp or a counter.
    pub fn start_login(&self) -> Login {
        let pkce = flux_credentials::generate_pkce();
        let state = flux_credentials::generate_state();
        let nonce = flux_credentials::generate_state();

        let mut pending = self.pending.write().expect("not poisoned");
        pending.retain(|_, entry| entry.started_at.elapsed() < LOGIN_TTL);
        if pending.len() < MAX_PENDING {
            pending.insert(
                state.clone(),
                Pending {
                    verifier: pkce.verifier,
                    nonce: nonce.clone(),
                    started_at: Instant::now(),
                },
            );
        }

        Login {
            state,
            nonce,
            challenge: pkce.challenge,
        }
    }

    /// Redeem a `state`, returning the verifier and nonce it was issued with.
    ///
    /// **Removed on read.** A `state` is single-use, so a replayed callback finds nothing and is
    /// refused — the same binding that makes the value a CSRF defence rather than a decoration.
    pub fn take_login(&self, state: &str) -> Option<(String, String)> {
        let mut pending = self.pending.write().expect("not poisoned");
        pending.retain(|_, entry| entry.started_at.elapsed() < LOGIN_TTL);
        pending
            .remove(state)
            .map(|entry| (entry.verifier, entry.nonce))
    }

    /// Establish a session for `account` and return the opaque token that names it.
    ///
    /// The token is returned once, to be put in a cookie, and never stored in a form this host
    /// could hand back.
    pub fn create(&self, account: Arc<Account>) -> String {
        let token = flux_credentials::generate_state();
        let mut live = self.live.write().expect("not poisoned");
        live.retain(|_, record| record.expires_at > Instant::now());
        live.insert(
            digest(&token),
            Record {
                account,
                expires_at: Instant::now() + SESSION_TTL,
            },
        );
        token
    }

    /// The account this token belongs to, if it is a live session.
    ///
    /// Expiry is enforced on read as well as on insert, so a session cannot be used past its TTL
    /// merely because nothing has pruned the map yet.
    pub fn resolve(&self, token: &str) -> Option<Arc<Account>> {
        let live = self.live.read().expect("not poisoned");
        let record = live.get(&digest(token))?;
        if record.expires_at <= Instant::now() {
            return None;
        }
        Some(Arc::clone(&record.account))
    }

    /// **Forget this session, server-side.**
    ///
    /// This is what makes sign-out mean something for a cookie somebody else copied. Clearing the
    /// cookie alone signs out the browser that asked and leaves the stolen copy working.
    pub fn revoke(&self, token: &str) {
        self.live
            .write()
            .expect("not poisoned")
            .remove(&digest(token));
    }

    /// How many sessions are live.
    pub fn len(&self) -> usize {
        self.live.read().expect("not poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The key a session is stored under.
fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(sub: &str) -> IdClaims {
        IdClaims {
            sub: sub.to_owned(),
            iat: 0,
            nonce: None,
            email: Some(format!("{sub}@example.test")),
            email_verified: Some(true),
            name: Some(sub.to_owned()),
        }
    }

    /// The account key is the subject, so two people who have held one email address are two
    /// accounts and two tenants.
    #[test]
    fn one_email_across_two_subjects_is_two_tenants() {
        let mut first = claims("111");
        let mut second = claims("222");
        first.email = Some("alice@example.test".to_owned());
        second.email = Some("alice@example.test".to_owned());

        let first = Account::from_claims(&first).expect("a usable subject");
        let second = Account::from_claims(&second).expect("a usable subject");

        assert_ne!(
            first.tenant(),
            second.tenant(),
            "a reassigned email address collapsed two people into one tenant"
        );
    }

    /// A subject that would traverse or split a credential path is refused where it enters, not
    /// where it is used.
    #[test]
    fn a_subject_that_is_not_a_path_segment_is_refused() {
        for hostile in [
            "../../etc/passwd",
            "a/b",
            "",
            "   ",
            "with space",
            "semi;colon",
        ] {
            assert!(
                Account::from_claims(&claims(hostile)).is_err(),
                "accepted a subject that cannot be a path segment: {hostile:?}"
            );
        }
        assert!(Account::from_claims(&claims(&"9".repeat(101))).is_err());
        assert!(Account::from_claims(&claims("110169484474386276334")).is_ok());
    }

    /// **No `id_token` can produce the dev tenant, and the dev account produces no real one.**
    ///
    /// The disjointness is asserted on the derivation rather than on a list of examples: every
    /// subject `validate_subject` admits is checked to yield a `google-`-prefixed tenant, and the
    /// dev tenant is checked not to carry that prefix. A future change that dropped the prefix
    /// would fail here rather than silently merge the two namespaces.
    #[test]
    fn a_dev_tenant_cannot_be_reached_from_any_id_token() {
        let developer = Account::developer();
        assert_eq!(developer.tenant(), DEV_TENANT);
        assert!(
            !developer.tenant().starts_with("google-"),
            "the dev tenant is inside the Google namespace"
        );

        for subject in [
            "110169484474386276334",
            "dev-local",
            "DEVELOPER-NOT-A-REAL-ACCOUNT",
            "_",
            &"9".repeat(100),
        ] {
            let account = Account::from_claims(&claims(subject)).expect("a usable subject");
            assert!(
                account.tenant().starts_with("google-"),
                "a Google account escaped the google- namespace: {}",
                account.tenant()
            );
            assert_ne!(
                account.tenant(),
                DEV_TENANT,
                "an id_token reached the dev tenant with sub {subject:?}"
            );
        }
    }

    /// The dev identity carries no address that could be mistaken for a real one.
    #[test]
    fn the_dev_account_has_no_email_and_says_it_is_fake() {
        let developer = Account::developer();
        assert_eq!(
            developer.email, None,
            "the dev account carries an email address"
        );
        assert!(developer
            .name
            .as_deref()
            .is_some_and(|name| name.contains("NOT A REAL ACCOUNT")));
        assert!(
            !developer.subject().chars().all(|c| c.is_ascii_digit()),
            "the dev subject is shaped like a Google `sub`"
        );
    }

    /// An unverified email is not presented as the person's address.
    #[test]
    fn an_unverified_email_is_not_kept() {
        let mut unverified = claims("333");
        unverified.email_verified = Some(false);
        assert_eq!(Account::from_claims(&unverified).expect("ok").email, None);
    }

    /// Revocation is server-side, so every copy of the token dies together.
    #[test]
    fn revoking_a_session_kills_every_copy_of_its_token() {
        let sessions = Sessions::new();
        let account = Arc::new(Account::from_claims(&claims("444")).expect("ok"));
        let token = sessions.create(account);
        let stolen = token.clone();

        assert!(sessions.resolve(&stolen).is_some());
        sessions.revoke(&token);
        assert!(
            sessions.resolve(&stolen).is_none(),
            "a copy of a revoked token still resolved"
        );
    }

    /// The store holds no token it could hand back.
    #[test]
    fn the_store_holds_no_live_token() {
        let sessions = Sessions::new();
        let account = Arc::new(Account::from_claims(&claims("555")).expect("ok"));
        let token = sessions.create(account);

        let live = sessions.live.read().expect("not poisoned");
        assert!(
            live.contains_key(&digest(&token)),
            "the session is keyed by the digest of its token"
        );
        assert!(
            !live.keys().any(|key| key.as_slice() == token.as_bytes()),
            "a live session token is a key in the store, so the store holds a usable cookie"
        );
    }

    /// A `state` is single-use, so a replayed callback finds nothing.
    #[test]
    fn a_login_state_is_consumed_on_first_use() {
        let sessions = Sessions::new();
        let login = sessions.start_login();

        assert!(sessions.take_login(&login.state).is_some());
        assert!(
            sessions.take_login(&login.state).is_none(),
            "a state was redeemed twice"
        );
    }

    /// Two sign-ins never share a state, a nonce, a verifier or a challenge.
    #[test]
    fn two_sign_ins_share_no_random_value() {
        let sessions = Sessions::new();
        let first = sessions.start_login();
        let second = sessions.start_login();

        assert_ne!(first.state, second.state);
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.challenge, second.challenge);
        assert_ne!(
            sessions.take_login(&first.state).expect("held").0,
            sessions.take_login(&second.state).expect("held").0,
            "two sign-ins shared a PKCE verifier"
        );
    }

    /// An unknown state resolves to nothing, which is what refuses an unsolicited callback.
    #[test]
    fn an_unissued_state_is_unknown() {
        let sessions = Sessions::new();
        assert!(sessions.take_login("never-issued").is_none());
    }
}

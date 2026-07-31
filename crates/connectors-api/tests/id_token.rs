//! **Every check on an `id_token`, asserted one at a time.**
//!
//! C-204's acceptance asks for this shape by name: *"a test asserts each check by feeding a token
//! that fails exactly one of them."* The word that carries the weight is **exactly**. A token that
//! is wrong in three ways proves only that something was refused, and a verifier that had quietly
//! stopped checking `aud` would still pass such a test on the strength of the other two. So every
//! case below starts from a token this host would accept, changes one thing, and names the error
//! it must produce.
//!
//! The signature is genuine in all but one case. These tokens are signed by a real RSA key this
//! test run generated, against a JWKS document it serves, so "the signature is fine and the `iss`
//! is wrong" is a state the tests actually construct rather than approximate.

mod support;

use connectors_api::auth::oidc::{verify_id_token, Expectations, Jwks, VerifyError};
use serde_json::{json, Value};
use support::{Idp, CLIENT_ID, KID};

/// The nonce the host is taken to have issued for this sign-in.
const NONCE: &str = "the-nonce-this-sign-in-asked-for";

/// A token that is correct in every respect.
fn good_claims(issuer: &str) -> Value {
    let now = support::unix_now();
    json!({
        "iss": issuer,
        "aud": CLIENT_ID,
        "sub": "110169484474386276334",
        "nonce": NONCE,
        "email": "operator@example.test",
        "email_verified": true,
        "name": "An Operator",
        "iat": now,
        "exp": now + 3600,
    })
}

/// What this host expects of a token, given the provider it is configured for.
fn expectations(issuers: &[String]) -> Expectations<'_> {
    Expectations {
        issuers,
        audience: CLIENT_ID,
        nonce: NONCE,
    }
}

/// Verify `claims`, signed the ordinary way, against `idp`'s published keys.
fn verify(idp: &Idp, claims: &Value) -> Result<connectors_api::auth::oidc::IdClaims, VerifyError> {
    verify_raw(idp, &idp.sign(claims))
}

fn verify_raw(idp: &Idp, token: &str) -> Result<connectors_api::auth::oidc::IdClaims, VerifyError> {
    let jwks = Jwks::from_json(&idp.jwks().to_string()).expect("a well-formed key set");
    let issuers = vec![idp.issuer.clone()];
    verify_id_token(token, &jwks, &expectations(&issuers))
}

/// **The control.** Without this the eight refusals below could all be one verifier that refuses
/// everything, which would pass every negative test and reject every real sign-in.
#[tokio::test]
async fn a_correct_token_is_accepted() {
    let idp = Idp::start().await;
    let claims = verify(&idp, &good_claims(&idp.issuer)).expect("a correct token is accepted");

    assert_eq!(claims.sub, "110169484474386276334");
    assert_eq!(claims.email.as_deref(), Some("operator@example.test"));
}

/// **Signature.** A token minted by someone who is not the provider.
///
/// The claims are perfect and the `kid` names the provider's published key; only the private key
/// that signed it is different. This is the check that stops anyone who can reach the callback
/// from writing their own identity.
#[tokio::test]
async fn a_token_signed_by_another_key_is_refused() {
    let idp = Idp::start().await;
    let attacker = Idp::foreign_key();
    let token = Idp::sign_with_foreign_key(&attacker, &good_claims(&idp.issuer));

    assert_eq!(
        verify_raw(&idp, &token).unwrap_err(),
        VerifyError::Signature
    );
}

/// **`iss`.** A genuinely signed token from a *different* OIDC provider.
///
/// Signed by the provider this host trusts, so only the issuer claim is wrong — which is what an
/// attacker running their own conformant provider would produce if this host were pointed at it.
#[tokio::test]
async fn a_token_from_another_issuer_is_refused() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims["iss"] = json!("https://accounts.example-attacker.test");

    assert_eq!(verify(&idp, &claims).unwrap_err(), VerifyError::Issuer);
}

/// **`aud`.** A token Google really did issue — to somebody else's application.
///
/// This is the classic OAuth confused deputy, and the one that is easiest to leave out because
/// everything about the token is otherwise real: right issuer, right signature, unexpired, a live
/// person's subject. Any other site's OAuth client can hand its tokens here, and without this
/// check they sign in as that person.
#[tokio::test]
async fn a_token_for_another_application_is_refused() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims["aud"] = json!("some-other-app.apps.googleusercontent.test");

    assert_eq!(verify(&idp, &claims).unwrap_err(), VerifyError::Audience);
}

/// **`exp`.** A token that was valid once.
#[tokio::test]
async fn an_expired_token_is_refused() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    let now = support::unix_now();
    claims["iat"] = json!(now - 7200);
    claims["exp"] = json!(now - 3600);

    assert_eq!(verify(&idp, &claims).unwrap_err(), VerifyError::Expired);
}

/// **`nbf`.** A token that is not valid yet.
///
/// `jsonwebtoken` defaults `validate_nbf` to false and Google does not currently send the claim, so
/// this test exists to hold the deliberate decision to turn it on: a standard temporal claim left
/// unenforced because the current issuer happens not to send it is a gap that opens silently the
/// day it does.
#[tokio::test]
async fn a_token_that_is_not_valid_yet_is_refused() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims["nbf"] = json!(support::unix_now() + 3600);

    assert_eq!(verify(&idp, &claims).unwrap_err(), VerifyError::NotYetValid);
}

/// **`iat`.** A token claiming to have been minted in the future.
///
/// `exp` cannot catch this: a token issued an hour ahead with a one-hour lifetime is unexpired for
/// two. Either a clock is badly wrong or the lifetime was stretched deliberately, and both are
/// worth refusing.
#[tokio::test]
async fn a_token_issued_in_the_future_is_refused() {
    let idp = Idp::start().await;
    let now = support::unix_now();
    let mut claims = good_claims(&idp.issuer);
    claims["iat"] = json!(now + 3600);
    claims["exp"] = json!(now + 7200);

    assert_eq!(
        verify(&idp, &claims).unwrap_err(),
        VerifyError::IssuedInTheFuture
    );
}

/// Clock skew within the leeway is tolerated, so the check above does not reject real traffic from
/// a machine a few seconds fast.
#[tokio::test]
async fn a_token_issued_a_few_seconds_ahead_is_accepted() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims["iat"] = json!(support::unix_now() + 5);

    assert!(
        verify(&idp, &claims).is_ok(),
        "ordinary clock skew was refused"
    );
}

/// **`nonce`.** A valid token, captured from another sign-in, replayed into this one.
#[tokio::test]
async fn a_token_carrying_another_sign_ins_nonce_is_refused() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims["nonce"] = json!("the-nonce-a-different-sign-in-asked-for");

    assert_eq!(verify(&idp, &claims).unwrap_err(), VerifyError::Nonce);
}

/// **`nonce`, absent.** The same check from the other side: a token with no nonce at all must not
/// pass by defaulting to the empty string and comparing equal to nothing in particular.
#[tokio::test]
async fn a_token_with_no_nonce_is_refused() {
    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims.as_object_mut().expect("an object").remove("nonce");

    assert_eq!(verify(&idp, &claims).unwrap_err(), VerifyError::Nonce);
}

/// **`alg`.** Key confusion — the real one, not an approximation of it.
///
/// `HS256` is a legal `alg`, so a verifier that reads the algorithm out of the token will try to
/// verify it as an HMAC — keyed on the only key material it has, the RSA public key, which the
/// provider publishes to the world. The token below is forged exactly that way: its tag is a
/// genuine `HMAC-SHA256` over the signing input, keyed on the JWKS modulus. Against a verifier
/// that honours the header this token **is valid**, minted by an attacker holding no secret.
///
/// An earlier version of this test signed RSA bytes under an `HS256` header, which is merely a
/// broken signature — it would have been refused by a verifier that was still exploitable, so it
/// did not test what its name claimed.
#[tokio::test]
async fn a_token_forged_by_key_confusion_is_refused() {
    let idp = Idp::start().await;
    let token = idp.forge_by_key_confusion(&good_claims(&idp.issuer));

    // Sanity: the forgery really is well-formed and really does name HS256, so the refusal below
    // is the `alg` pin doing its job and not a malformed-input accident.
    let header = token.split('.').next().expect("a header segment");
    let header: Value = serde_json::from_slice(
        &base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, header)
            .expect("base64url"),
    )
    .expect("json");
    assert_eq!(header["alg"], "HS256", "the forgery does not name HS256");
    assert_eq!(
        token.split('.').count(),
        3,
        "the forgery is not a compact JWS"
    );

    assert!(
        matches!(verify_raw(&idp, &token), Err(VerifyError::Malformed(_))),
        "a token forged by key confusion was accepted or refused for the wrong reason"
    );
}

/// **`alg: none`.** The same attack in its oldest form — a token that claims to need no signature.
#[tokio::test]
async fn a_token_claiming_no_signature_is_refused() {
    let idp = Idp::start().await;
    let token = idp.sign_with_header(
        &json!({ "alg": "none", "typ": "JWT", "kid": KID }),
        &good_claims(&idp.issuer),
    );

    assert!(
        matches!(verify_raw(&idp, &token), Err(VerifyError::Malformed(_))),
        "a token naming alg none was not refused outright"
    );
}

/// **`kid`.** A token naming a signing key the provider does not publish.
///
/// Distinguished from a bad signature on purpose: this is also what an ordinary Google key
/// rotation looks like from here, and the callback answers it by refetching the key set once
/// before giving up.
#[tokio::test]
async fn a_token_naming_an_unpublished_key_is_refused() {
    let idp = Idp::start().await;
    let token = idp.sign_with_header(
        &json!({ "alg": "RS256", "typ": "JWT", "kid": "a-key-that-was-never-published" }),
        &good_claims(&idp.issuer),
    );

    assert_eq!(
        verify_raw(&idp, &token).unwrap_err(),
        VerifyError::UnknownKey(Some("a-key-that-was-never-published".to_owned()))
    );
}

/// **`sub`.** A subject that would escape its own credential path.
///
/// The provider is trusted and Google's subjects are digits, so this can only happen if the trust
/// is misplaced — which is the case worth being fenced against, because the value becomes a
/// directory in `tenants/<tenant>/<authority>/<service>/<credential>`.
#[tokio::test]
async fn a_subject_that_would_escape_its_path_cannot_own_a_tenant() {
    use connectors_api::auth::session::Account;

    let idp = Idp::start().await;
    let mut claims = good_claims(&idp.issuer);
    claims["sub"] = json!("../../../etc/passwd");

    // The token itself is well-formed and correctly signed: the refusal is the account's, not the
    // verifier's, which is where a path segment belongs.
    let verified = verify(&idp, &claims).expect("the token is otherwise valid");
    assert!(
        Account::from_claims(&verified).is_err(),
        "a traversing subject was accepted as a tenant"
    );
}

/// **`sub` is the account key, not `email`.**
///
/// Two tokens, one address, two subjects — the sequence that happens when an administrator frees
/// an address and reassigns it. They must be two tenants.
#[tokio::test]
async fn two_subjects_sharing_an_email_are_two_tenants() {
    use connectors_api::auth::session::Account;

    let idp = Idp::start().await;

    let mut first = good_claims(&idp.issuer);
    first["sub"] = json!("111111111111111111111");
    first["email"] = json!("alice@example.test");

    let mut second = good_claims(&idp.issuer);
    second["sub"] = json!("222222222222222222222");
    second["email"] = json!("alice@example.test");

    let first = Account::from_claims(&verify(&idp, &first).expect("valid")).expect("a tenant");
    let second = Account::from_claims(&verify(&idp, &second).expect("valid")).expect("a tenant");

    assert_eq!(first.email, second.email, "the premise: one address");
    assert_ne!(
        first.tenant(),
        second.tenant(),
        "a reassigned email address let one person inherit another's credentials"
    );
}

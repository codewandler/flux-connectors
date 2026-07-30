//! The store composes with a [`Layout`] rather than hard-coding one.
//!
//! This is the decorator the credential-addressing epic exists for: the client is commodity, the
//! convention is the part worth owning, and a deployment that already has a secret layout keeps it
//! without inventing a second addressing scheme. The property worth asserting is narrow —
//! **a non-default layout changes the path, and nothing else.**

use connector_secrets::{CredentialRef, Layout, MemoryStore, Secret, SecretStore, TenantLayout};

/// Obviously not a credential. Nothing in this repository commits a value shaped like a real token.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET";

/// A deliberately different convention: a `flux/` root instead of `tenants/`, and the authority
/// ahead of the tenant.
///
/// It is not better than [`TenantLayout`] — it stands in for the layout a deployment already had
/// before this crate existed, which is the case the seam is for. It keeps the two rules that belong
/// to the *address* rather than to any one convention: it is lossless, so `parse(render(r)) == r`,
/// and `default` never reaches a path.
struct FlatLayout;

const FLAT_ROOT: &str = "flux";

impl Layout for FlatLayout {
    fn render(&self, reference: &CredentialRef) -> String {
        if reference.is_default_service() {
            format!(
                "{FLAT_ROOT}/{}/{}/{}",
                reference.authority(),
                reference.tenant(),
                reference.credential()
            )
        } else {
            format!(
                "{FLAT_ROOT}/{}/{}/{}/{}",
                reference.authority(),
                reference.tenant(),
                reference.service(),
                reference.credential()
            )
        }
    }

    fn parse(&self, path: &str) -> Result<CredentialRef, String> {
        match path.split('/').collect::<Vec<_>>()[..] {
            [FLAT_ROOT, authority, tenant, credential] => {
                CredentialRef::new(tenant, authority, "default", credential)
            }
            [FLAT_ROOT, _, _, "default", _] => Err(format!(
                "{path:?} spells out the reserved `default` service, which is elided"
            )),
            [FLAT_ROOT, authority, tenant, service, credential] => {
                CredentialRef::new(tenant, authority, service, credential)
            }
            _ => Err(format!("{path:?} is not one of ours")),
        }
    }
}

fn reference() -> CredentialRef {
    CredentialRef::new("9f3a4b2c", "com.zendesk.api", "support", "api_token").expect("valid")
}

#[tokio::test]
async fn a_non_default_layout_changes_the_path_and_nothing_else() {
    let reference = reference();

    let blessed = MemoryStore::with_layout(TenantLayout);
    let custom = MemoryStore::with_layout(FlatLayout);
    // The two are different types, so the shared half of the assertion runs through the trait —
    // which is also the way a host holds a store.
    let both: [&dyn SecretStore; 2] = [&blessed, &custom];

    for store in both {
        store
            .put(&reference, &Secret::new(SENTINEL))
            .await
            .expect("put");
    }

    // The path differs, and it differs in exactly the way the layout says.
    assert_eq!(
        blessed.paths(),
        vec!["tenants/9f3a4b2c/com.zendesk.api/support/api_token"]
    );
    assert_eq!(
        custom.paths(),
        vec!["flux/com.zendesk.api/9f3a4b2c/support/api_token"]
    );
    assert_ne!(blessed.paths(), custom.paths());

    // Nothing else differs. Same address in, same value out, same behaviour on an address that was
    // never written, and the same idempotent delete.
    let absent = CredentialRef::new("other-tenant", "com.zendesk.api", "support", "api_token")
        .expect("valid");
    for store in both {
        assert_eq!(
            store.get(&reference).await.expect("get").expose_secret(),
            SENTINEL
        );
        assert!(store.get(&absent).await.unwrap_err().is_not_found());

        store.delete(&reference).await.expect("delete");
        store.delete(&reference).await.expect("delete again");
        assert!(store.get(&reference).await.unwrap_err().is_not_found());
    }
    assert!(blessed.is_empty());
    assert!(custom.is_empty());
}

/// A custom layout does not get to widen what an address may contain. [`CredentialRef::new`] is
/// still the only door, and it still refuses a traversing tenant — which is what keeps a layout
/// from being a second place path safety has to be argued.
#[test]
fn a_custom_layout_cannot_widen_what_an_address_may_contain() {
    assert!(CredentialRef::new("../../etc", "com.zendesk.api", "support", "api_token").is_err());
    assert!(
        FlatLayout
            .parse("flux/com.zendesk.api/../../etc/api_token")
            .is_err(),
        "a layout parsing a hostile path must refuse rather than render it"
    );
}

/// `parse(render(r)) == r` is the [`Layout`] contract, and `default` never reaches a path. Both
/// belong to the address rather than to `TenantLayout`, so a custom layout owes them too.
#[test]
fn the_layout_contract_still_applies_to_a_custom_one() {
    let named = reference();
    let rendered = FlatLayout.render(&named);
    assert_eq!(rendered, "flux/com.zendesk.api/9f3a4b2c/support/api_token");
    assert_eq!(FlatLayout.parse(&rendered), Ok(named));

    let default_service =
        CredentialRef::new("9f3a4b2c", "com.slack.api", "default", "signing_secret")
            .expect("valid");
    let rendered = FlatLayout.render(&default_service);
    assert_eq!(rendered, "flux/com.slack.api/9f3a4b2c/signing_secret");
    assert!(
        !rendered.contains("default"),
        "`default` never reaches a path"
    );
    assert_eq!(FlatLayout.parse(&rendered), Ok(default_service));

    // Two spellings of one address is how a store holds the same credential twice with nothing to
    // say which is current.
    assert!(FlatLayout
        .parse("flux/com.slack.api/9f3a4b2c/default/signing_secret")
        .is_err());
    // And a path this layout did not write is refused rather than guessed at.
    assert!(FlatLayout
        .parse("tenants/9f3a4b2c/com.slack.api/signing_secret")
        .is_err());
}

//! **A credential outlives the process that was given it** (C-207), and does not gain a surface on
//! the way.
//!
//! `tests/host.rs` asserts the property this crate is most obliged to keep: no credential value
//! reaches anything it serves, including on error. That test builds one host, stores through it and
//! reads back from it — so everything it proves, it proves about a value that never left process
//! memory.
//!
//! Persistence changes what has to be proved, not how much. A value now goes to a file and comes
//! back from one, which is two new places for it to be quoted: the store's own error messages, and
//! whatever loaded it. So the sweep here is deliberately the *same* sweep, run against a host that
//! has just read the credential off disk rather than one that was handed it a moment ago. **A
//! guarantee that held only for the value you just stored is not the guarantee**, and re-running the
//! sweep is the only way to know which of the two this host has.
//!
//! # Why the host is rebuilt rather than the process restarted
//!
//! `App` holds every port, and dropping it drops the store with them; a second `App` over the same
//! path shares nothing with the first but the bytes on disk. That is the whole of what a restart
//! does to this host's credentials, and it is a thing a test can do — where re-executing the binary
//! is not, since the binary binds a fixed port. The real binary was driven by hand as well, and the
//! transcript is in `crates/connectors-api/README.md`.
//!
//! # Everything here goes through the HTTP surface and the environment
//!
//! No test below reaches for a Rust constructor that an operator does not have. The store is
//! selected the way an operator selects it — one environment variable — and the credential is stored
//! the way the page stores it, with `PUT /v1/credentials/…`. A test that bound a store directly
//! would prove the store works and leave the wiring between it and the route untested, which is
//! where a host's own defects live.

use crate::support;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::support::{client, sign_in, Idp};
use connectors_api::App;

/// The variable that selects the credential store.
///
/// Spelled out rather than imported for the same reason [`OPERATOR_TENANT`] is: this is the name an
/// operator types, so a rename should be a failing test here rather than a test that silently agrees
/// with whatever the code now calls it. `tests/credential_store.rs` pins it against the crate's own
/// constant, so the two cannot drift apart unnoticed.
const STORE_ENV: &str = "CONNECTORS_CREDENTIAL_STORE";

/// The data home `App::deployed` resolves its default store under.
const DATA_HOME_ENV: &str = "XDG_DATA_HOME";

/// An obviously-fake credential, long enough for flux's redactor to hold it — the same care
/// `tests/host.rs` takes with its own.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-connectors-api-persisted";

/// The subject these tests sign in as.
const OPERATOR: &str = "110169484474386276334";

/// The tenant that subject resolves to.
const OPERATOR_TENANT: &str = "google-110169484474386276334";

/// The address the credential below is stored at, in full.
///
/// Asserted at every step rather than assumed, because a store keyed loosely enough to collide would
/// still hand back *a* value and every other assertion here would pass. The tenant segment is
/// C-204's, and the absence of a service segment is `default` being elided; C-219's two-services
/// case is asserted against the store itself, in `connector-secrets`.
fn address() -> String {
    format!("tenants/{OPERATOR_TENANT}/com.anthropic.api/api_key")
}

// -------------------------------------------------------------------------------------------
// The failing-first test
// -------------------------------------------------------------------------------------------

/// **Store a credential, drop the host, build another one over the same location, and the
/// credential is still there — at the same address.**
///
/// Against `MemoryStore` the second host starts empty, which is the defect C-207 exists to close: an
/// operator with a durable account re-pastes every token after every restart, and that is the habit
/// that gets a token pasted somewhere it should not be.
///
/// The address is asserted alongside the flag on purpose. "A credential is stored" is not the
/// property worth having on its own — a store that collided across tenants, or across two services
/// of one vendor, would answer `true` for the wrong one and undo C-204 and C-219 without failing
/// anything.
#[tokio::test]
async fn a_credential_survives_the_host_being_rebuilt() {
    let scratch = Scratch::new("survives");
    let idp = Idp::start().await;

    // The first host: sign in, store a credential, confirm it took.
    let first = serve(&idp, &scratch).await;
    let cookie = sign_in(&first, OPERATOR).await;
    store_credential(&first, &cookie, SENTINEL).await;
    assert_eq!(
        stored_at(&first, &cookie).await,
        (true, address()),
        "the credential did not take on the host that was given it"
    );

    // Everything the first host held is gone. Only the file it wrote remains.
    first.stop().await;

    // The second host. A new session, because sessions are deliberately *not* persisted — C-207's
    // notes are explicit that conflating the two stores would make stolen cookies outlive the
    // process — so signing in again is part of what this asserts.
    let second = serve(&idp, &scratch).await;
    let cookie = sign_in(&second, OPERATOR).await;

    assert_eq!(
        stored_at(&second, &cookie).await,
        (true, address()),
        "the credential did not survive the host being rebuilt over the same store, or came back \
         at a different address"
    );
}

// -------------------------------------------------------------------------------------------
// The redaction guarantee, re-proved against a credential that came off disk
// -------------------------------------------------------------------------------------------

/// **A credential loaded from the persisted store reaches no surface either, including on error.**
///
/// The sweep `tests/host.rs` runs, run against a host whose only knowledge of the value is a file it
/// read at startup. The new failure persistence introduces is not a route that echoes what it just
/// stored — that one was already covered — but a *diagnostic*: a store naming the file it loaded
/// from, a refusal quoting the line it could not parse, a `Debug` printing the map. Each is the
/// natural shape of a helpful error, and each would put a token in a response body.
///
/// It begins by asserting the reload actually happened. Without that the sweep would pass on a host
/// that had loaded nothing, which is the shape of a test that goes green for the wrong reason and
/// then stays green through the regression it was written for.
#[tokio::test]
async fn a_credential_loaded_from_disk_reaches_no_surface() {
    let scratch = Scratch::new("no-surface");
    let idp = Idp::start().await;

    let first = serve(&idp, &scratch).await;
    let cookie = sign_in(&first, OPERATOR).await;
    store_credential(&first, &cookie, SENTINEL).await;
    first.stop().await;

    let base = serve(&idp, &scratch).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;
    let session_token = cookie.split_once('=').expect("name=value").1.to_owned();

    assert_eq!(
        stored_at(&base, &cookie).await,
        (true, address()),
        "this host loaded no credential, so the sweep below would assert nothing"
    );

    let secrets = [
        (SENTINEL, "the connector credential"),
        (support::CLIENT_SECRET, "the Google client secret"),
        (session_token.as_str(), "the session token"),
    ];

    for path in [
        "/v1/connectors",
        "/v1/connectors/anthropic",
        "/v1/operations/anthropic-models-list",
        "/auth/me",
        "/auth/status",
        "/",
    ] {
        let body = client
            .get(format!("{base}{path}"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"))
            .text()
            .await
            .expect("a body");
        for (secret, what) in secrets {
            assert!(
                !body.contains(secret),
                "`{path}` served {what} after a reload"
            );
        }
    }

    // The error paths, reached without a session, with a bad one, and with a state never issued.
    for (path, cookie_header) in [
        ("/v1/connectors", None),
        ("/v1/connectors", Some("connectors_session=not-a-session")),
        ("/auth/me", None),
        (
            "/auth/callback?code=x&state=never-issued",
            Some(cookie.as_str()),
        ),
    ] {
        let mut request = client.get(format!("{base}{path}"));
        if let Some(header) = cookie_header {
            request = request.header("cookie", header);
        }
        let body = request
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path}: {error}"))
            .text()
            .await
            .expect("a body");
        for (secret, what) in secrets {
            assert!(
                !body.contains(secret),
                "`{path}` served {what} on an error after a reload"
            );
        }
    }

    // Overwriting — the one path that both reads the old value and writes a new one — echoes
    // neither.
    let response = client
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &cookie)
        .json(&serde_json::json!({ "value": "SENTINEL-NOT-A-REAL-SECRET-replacement" }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(response.status(), 204);
    let body = response.text().await.expect("a body");
    assert!(
        !body.contains(SENTINEL),
        "an overwrite echoed the old value"
    );
    assert!(
        !body.contains("SENTINEL-NOT-A-REAL-SECRET-replacement"),
        "an overwrite echoed the new value"
    );

    // And the delete an operator revokes with actually reaches the file.
    let deleted = client
        .delete(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("the delete completes");
    assert_eq!(deleted.status(), 204);
    base.stop().await;

    let reopened = serve(&idp, &scratch).await;
    let cookie = sign_in(&reopened, OPERATOR).await;
    assert!(
        !stored_at(&reopened, &cookie).await.0,
        "a deleted credential came back after a restart, so revoking one does not stick"
    );
}

/// **A store that cannot be written to refuses, and says so without quoting the credential.**
///
/// The new error path persistence introduces. A full disk, a read-only mount or a directory an
/// operator moved is exactly where "could not store `<value>`" is the natural message to write, and
/// the refusal has to reach the operator with enough to act on and nothing else.
///
/// It also asserts the refusal is a refusal. A host that answered `204` and kept the value in memory
/// would have told the operator their credential was safe and then lost it on the next restart —
/// which is C-207's failure reached from the other side.
#[tokio::test]
async fn a_persistence_failure_refuses_without_quoting_the_value() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("write-failure");
    let idp = Idp::start().await;
    let base = serve(&idp, &scratch).await;
    let cookie = sign_in(&base, OPERATOR).await;

    let directory = scratch.store().parent().expect("a parent").to_owned();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))
        .expect("make the store directory read-only");

    let response = client()
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", &cookie)
        .json(&serde_json::json!({ "value": SENTINEL }))
        .send()
        .await
        .expect("the store call completes");
    let status = response.status();
    let body = response.text().await.expect("a body");

    // Restored before the assertions, so a failure still leaves a removable directory.
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .expect("restore the store directory");

    assert!(
        status.is_client_error() || status.is_server_error(),
        "a store that could not write answered {status}, so an operator believes a credential was \
         kept that will not be there after a restart: {body}"
    );
    assert!(
        !body.contains(SENTINEL),
        "the persistence failure quoted the credential: {body}"
    );
}

/// **The file a credential lands in is `0600`, inside a `0700` directory.**
///
/// Asserted on a store the *host* created through its own configuration, not on one the test built,
/// because the mode is only worth anything if it survives the path an operator actually takes.
#[tokio::test]
async fn the_store_the_host_creates_is_0600_in_a_0700_directory() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("modes");
    let idp = Idp::start().await;
    let base = serve(&idp, &scratch).await;
    let cookie = sign_in(&base, OPERATOR).await;
    store_credential(&base, &cookie, SENTINEL).await;

    let file = scratch.store();
    let mode = |path: &std::path::Path| {
        std::fs::metadata(path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
            .permissions()
            .mode()
            & 0o777
    };

    assert!(
        file.exists(),
        "the host stored a credential and wrote no file"
    );
    assert_eq!(
        mode(&file),
        0o600,
        "the credential file is readable by others"
    );
    assert_eq!(
        mode(file.parent().expect("a parent")),
        0o700,
        "the directory holding credentials is readable by others"
    );

    // The value is in there, and this is what "not encrypted" means — asserted so that nothing in
    // this repository can later claim a protection the store does not have. Hex, not plaintext, and
    // hex is framing rather than protection.
    let raw = std::fs::read_to_string(&file).expect("read the store");
    let hex: String = SENTINEL.bytes().map(|byte| format!("{byte:02x}")).collect();
    assert!(
        raw.contains(&hex),
        "the store did not hold the value where this test expected it, so the claim it makes \
         about what is at rest is no longer checked"
    );
}

// -------------------------------------------------------------------------------------------
// Fixtures
// -------------------------------------------------------------------------------------------

/// A directory of this test's own, removed when the guard drops.
///
/// Under the system temporary directory, which is outside the repository checkout — the same
/// property the host enforces for a real store, arrived at here so that a test never leaves a
/// credential file inside the tree.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "connectors-api-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    /// The scratch directory itself, used as `XDG_DATA_HOME`.
    fn data_home(&self) -> &std::path::Path {
        &self.0
    }

    /// Where `App::deployed` will put the store given that data home — the real default path, so
    /// these tests assert against the location an operator actually gets rather than one a test
    /// chose.
    fn store(&self) -> PathBuf {
        self.0.join("connectors-api").join("credentials")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // The write-failure test leaves a read-only directory behind if it panicked before
        // restoring it; make a best effort rather than failing a whole run on cleanup.
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(
            self.0.join("connectors-api"),
            std::fs::Permissions::from_mode(0o700),
        );
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Start a host over `scratch`'s store and return its base URL.
///
/// **Built with `App::deployed` and a scratch `XDG_DATA_HOME`**, which is the operator's own path
/// exactly: the variable is left unset, so the store lands at the default location the binary would
/// have chosen. That makes every test in this file an assertion about the shipped default rather
/// than about a location a test picked — the gap that let `App::deployed`'s unset-variable arm be
/// changed to `Memory` with the whole suite staying green.
///
/// `support::serve` cannot be reused: it builds the `App` itself with `App::new`, and the store is
/// what varies here. Everything else — the identity provider, the ephemeral loopback port, the real
/// router — is the same.
struct Hosted {
    base: String,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Hosted {
    async fn stop(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

impl std::ops::Deref for Hosted {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::fmt::Display for Hosted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.base.fmt(formatter)
    }
}

impl Drop for Hosted {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn serve(idp: &Idp, scratch: &Scratch) -> Hosted {
    use std::net::{Ipv4Addr, SocketAddr};

    let app = {
        // Held across "set the environment, build the `App`", as `support::env_lock` documents: the
        // environment is per process and `cargo test` runs these on parallel threads.
        let _guard = support::env_lock();
        std::env::set_var(
            connectors_api::auth::oidc::CLIENT_ID_ENV,
            support::CLIENT_ID,
        );
        std::env::set_var(
            connectors_api::auth::oidc::CLIENT_SECRET_ENV,
            support::CLIENT_SECRET,
        );
        std::env::set_var(
            connectors_api::auth::oidc::REDIRECT_URI_ENV,
            "http://127.0.0.1/auth/callback",
        );
        std::env::set_var("CONNECTORS_OIDC_ISSUER", &idp.issuer);
        std::env::set_var("CONNECTORS_OIDC_AUTHORIZE_URL", idp.authorize_url());
        std::env::set_var("CONNECTORS_OIDC_TOKEN_URL", idp.token_url());
        std::env::set_var("CONNECTORS_OIDC_JWKS_URL", idp.jwks_url());

        // Unset, deliberately: this exercises the default, and it also means an operator who has
        // exported it for their own host cannot change what these tests do.
        std::env::remove_var(STORE_ENV);
        let restore = std::env::var_os(DATA_HOME_ENV);
        std::env::set_var(DATA_HOME_ENV, scratch.data_home());

        let app = App::deployed(env!("CARGO_MANIFEST_DIR")).expect("a host over the scratch store");

        match restore {
            Some(previous) => std::env::set_var(DATA_HOME_ENV, previous),
            None => std::env::remove_var(DATA_HOME_ENV),
        }
        app
    };

    let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("an ephemeral loopback port");
    let address = listener.local_addr().expect("a bound address");
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, connectors_api::router(app)).await;
    });
    Hosted {
        base: format!("http://{address}"),
        task: Some(task),
    }
}

/// `PUT` one credential through the real HTTP surface, and assert the response carries nothing.
async fn store_credential(base: &str, cookie: &str, value: &str) {
    let response = client()
        .put(format!("{base}/v1/credentials/anthropic/anthropic.api_key"))
        .header("cookie", cookie)
        .json(&serde_json::json!({ "value": value }))
        .send()
        .await
        .expect("the store call completes");
    assert_eq!(response.status(), 204, "the credential was not accepted");
    assert!(
        !response.text().await.expect("a body").contains(value),
        "the store response echoed the credential back"
    );
}

/// `(whether a value is stored, the address it is stored at)`, read off the connector view.
async fn stored_at(base: &str, cookie: &str) -> (bool, String) {
    let view: serde_json::Value = client()
        .get(format!("{base}/v1/connectors/anthropic"))
        .header("cookie", cookie)
        .send()
        .await
        .expect("the view")
        .json()
        .await
        .expect("json");
    let api_key = view["credentials"]
        .as_array()
        .expect("credentials")
        .iter()
        .find(|credential| credential["name"] == "anthropic.api_key")
        .expect("the declared credential")
        .clone();
    (
        api_key["stored"].as_bool().expect("stored"),
        api_key["address"].as_str().expect("address").to_owned(),
    )
}

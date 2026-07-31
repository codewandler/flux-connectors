//! **Which store the host binds is a startup decision** (C-207), and a wrong one stops it.
//!
//! `tests/persistence.rs` asserts what a credential does once it is in a store. This file asserts
//! the decision *before* that: who may make it, that a value nobody implemented stops the process
//! rather than becoming memory, that a store which will not open does the same, that the file never
//! lands inside the tree the host runs under, and that what an operator is told at startup matches
//! the store that was actually bound.
//!
//! # Two properties that are easy to state and were not, at first, true
//!
//! **Exactly one constructor reads the environment.** The first version of C-207 had `App::new`
//! honour `CONNECTORS_CREDENTIAL_STORE` as well, on the reasoning that an operator who exported it
//! meant it. But `App::new` is what the test suite builds hosts with, and an environment variable is
//! ambient: with it exported — which this crate's own README instructs — every test host reached
//! into the operator's real credential file, and `tests/host.rs`'s
//! `an_operation_without_its_credential_refuses_by_address` answered `200` instead of `400` because
//! a value left by an earlier run resolved. `200` there means the host *dispatched*, so the suite
//! sent a live request to `api.anthropic.com`. The isolation is now structural — `App::new` has no
//! ambient input at all — and is pinned below.
//!
//! **There is no silent fallback to memory.** This was documented and unfalsifiable: making
//! `StoreChoice::open` answer a failed `FileStore::open` with a `MemoryStore` left the whole suite
//! green, because only `parse` failures were pinned. A credential silently living in memory when the
//! operator asked for a file is the failure this story exists to remove, so the *open* path is now
//! pinned too.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use connectors_api::secrets::{default_path, refuse_inside, StoreChoice, STORE_ENV};
use connectors_api::App;

/// The crate root, which is the `root` every test here builds a host under.
const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// An obviously-fake credential. Nothing here may commit a value shaped like a real token.
const SENTINEL: &str = "SENTINEL-NOT-A-REAL-SECRET-credential-store";

/// The name `tests/persistence.rs` spells out by hand, pinned against the one the crate exports.
///
/// That file writes the variable as a literal deliberately — it is the string an operator types, so
/// a rename should fail a test rather than silently follow the code. This is the other half of that
/// arrangement: without it the two could drift apart and nothing would say so.
#[test]
fn the_variable_name_is_the_one_the_other_tests_spell_out() {
    assert_eq!(STORE_ENV, "CONNECTORS_CREDENTIAL_STORE");
}

// ---------------------------------------------------------------------------------------------
// Who reads the environment
// ---------------------------------------------------------------------------------------------

/// **An ambient store variable does not reach a host built with `App::new`.**
///
/// The regression that made `cargo test` write an operator's real store and fire a real request at
/// a real vendor. Asserted three ways, because "it defaults to memory" and "it ignores the
/// environment" are different claims and only the second is the isolation: the host must report a
/// memory store, must create no file, and must still be holding nothing after a rebuild.
#[test]
fn an_ambient_store_variable_does_not_reach_a_host_built_with_new() {
    let scratch = Scratch::new("ambient");
    let _guard = env_lock();

    for ambient in [
        format!("file:{}", scratch.store().display()),
        "file".to_owned(),
    ] {
        std::env::set_var(STORE_ENV, &ambient);
        let app = App::new(ROOT).expect("a host still builds");
        let with_options = App::with_web_options(ROOT, flux_web::WebOptions::default())
            .expect("and so does one with an egress policy");
        std::env::remove_var(STORE_ENV);

        assert_eq!(
            app.storage(),
            &StoreChoice::Memory,
            "{STORE_ENV}={ambient} reached App::new, so an operator who exports it — as this \
             crate's README tells them to — cannot run the test suite without every test host \
             reaching into their real credential file"
        );
        assert_eq!(
            with_options.storage(),
            &StoreChoice::Memory,
            "{STORE_ENV}={ambient} reached App::with_web_options"
        );
        assert!(
            !scratch.store().exists(),
            "a host built with App::new created {} from an ambient variable",
            scratch.store().display()
        );
    }
}

/// **`App::deployed` is the one that does read it**, so the isolation above is not "nothing reads
/// the environment", which would be a different and wrong fix.
#[test]
fn the_deployed_constructor_is_the_one_that_reads_the_environment() {
    let scratch = Scratch::new("deployed-reads");
    let _guard = env_lock();

    std::env::set_var(STORE_ENV, format!("file:{}", scratch.store().display()));
    let app = App::deployed(ROOT).expect("a deployed host");
    std::env::remove_var(STORE_ENV);

    assert_eq!(app.storage(), &StoreChoice::File(scratch.store()));
    assert!(
        scratch.store().exists(),
        "App::deployed did not open the store it was pointed at"
    );
}

/// **The unset-variable default is a file under the data home** — the whole of C-207 for the binary.
///
/// Driven through `App::deployed` against a scratch `XDG_DATA_HOME`, and asserted on the file that
/// appears rather than on an equality between two other functions. The earlier version of this test
/// only reached `App::deployed` inside the `Err` branch of `default_path()`, which cannot run on a
/// machine with `HOME` set — so changing the unset arm to `Memory` left the suite green while the
/// story's acceptance cited that line as its evidence.
#[test]
fn the_deployed_default_is_a_file_under_the_data_home() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("deployed-default");
    std::fs::create_dir_all(scratch.path()).expect("a scratch data home");

    let (app, previous) = {
        let _guard = env_lock();
        std::env::remove_var(STORE_ENV);
        let previous = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", scratch.path());
        let app = App::deployed(ROOT).expect("a deployed host with no variable set");
        match &previous {
            Some(value) => std::env::set_var("XDG_DATA_HOME", value),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        (app, previous)
    };
    drop(previous);

    let expected = scratch.path().join("connectors-api").join("credentials");
    assert_eq!(
        app.storage(),
        &StoreChoice::File(expected.clone()),
        "an unset {STORE_ENV} did not resolve to a file under the data home, which is the one \
         behaviour that makes wiring a connector survive a restart without reading a variable name"
    );
    assert!(
        expected.exists(),
        "App::deployed reported a file store and created no file at {}",
        expected.display()
    );
    assert_eq!(
        std::fs::metadata(&expected)
            .expect("the store")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(expected.parent().expect("a parent"))
            .expect("the directory")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert!(
        app.storage_banner().contains("SURVIVE A RESTART"),
        "the default host must say its credentials outlive it"
    );
}

// ---------------------------------------------------------------------------------------------
// Never a silent fallback to memory
// ---------------------------------------------------------------------------------------------

/// **A store this host does not have stops it**, rather than becoming memory.
#[test]
fn a_store_it_does_not_have_stops_the_host() {
    for spec in [
        "vault",
        "File",
        "sqlite:///x",
        "yes",
        "file:",
        "file:rel/path",
    ] {
        let refusal = StoreChoice::parse(spec)
            .err()
            .unwrap_or_else(|| panic!("{spec:?} was accepted as a store selection"));
        let message = format!("{refusal:#}");
        assert!(
            message.contains(STORE_ENV),
            "{spec:?}: the refusal must name the variable an operator has to fix: {message}"
        );
    }
}

/// The same refusal, reached the way an operator reaches it: through the environment, at startup.
#[test]
fn a_bad_value_in_the_environment_refuses_to_build_a_host() {
    let _guard = env_lock();
    std::env::set_var(STORE_ENV, "vault");
    let refusal = App::deployed(ROOT).err();
    std::env::remove_var(STORE_ENV);

    let refusal = refusal.expect("an unknown store must stop the host");
    let message = format!("{refusal:#}");
    assert!(message.contains(STORE_ENV), "{message}");
    assert!(
        message.contains("memory"),
        "the refusal must say what would have worked: {message}"
    );
}

/// **A file store that will not open is an error, never a memory store.**
///
/// The path the documentation claimed and nothing checked. `StoreChoice::open`'s `File` arm could be
/// changed to answer a failed `FileStore::open` with `MemoryStore::new()` and the whole suite stayed
/// green — so a host could be told "keep these in a file", fail to open it, start anyway, serve
/// every route correctly and lose everything on the next restart. Every layer that could swallow it
/// is asserted separately, because a refusal in `StoreChoice::open` that `App` then ignored would be
/// the same defect one level up.
#[test]
fn a_store_that_will_not_open_refuses_and_does_not_become_memory() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("unopenable");
    std::fs::create_dir_all(scratch.path()).expect("a scratch directory");
    std::fs::set_permissions(scratch.path(), std::fs::Permissions::from_mode(0o500))
        .expect("make it read-only");
    // Inside a directory that cannot be written to, so creating the store's own directory fails.
    let unopenable = scratch.path().join("state").join("credentials");
    let choice = StoreChoice::File(unopenable.clone());

    let direct = choice.open().err();
    let through_app =
        App::with_credential_store(ROOT, flux_web::WebOptions::default(), choice.clone()).err();
    let through_env = {
        let _guard = env_lock();
        std::env::set_var(STORE_ENV, format!("file:{}", unopenable.display()));
        let refusal = App::deployed(ROOT).err();
        std::env::remove_var(STORE_ENV);
        refusal
    };

    std::fs::set_permissions(scratch.path(), std::fs::Permissions::from_mode(0o700))
        .expect("restore");

    let direct = direct.expect(
        "StoreChoice::open answered an unopenable file store with something rather than an error, \
         so a host told to keep credentials in a file would hold them in memory instead",
    );
    assert!(
        through_app.is_some(),
        "App::with_credential_store started against a store that would not open"
    );
    assert!(
        through_env.is_some(),
        "App::deployed started against a store that would not open"
    );

    let message = format!("{direct:#}");
    assert!(
        message.contains("does not fall back"),
        "the refusal must say that memory is not what happens instead: {message}"
    );
    assert!(!message.contains(SENTINEL));
}

// ---------------------------------------------------------------------------------------------
// Where the file may go
// ---------------------------------------------------------------------------------------------

/// **The persisted location is outside the repository checkout and outside the host's own root.**
/// Asserted, not assumed.
///
/// Two reasons, either sufficient. It is the directory `flux_system::Workspace` is rooted at; and it
/// is, in practice, a git checkout — `cargo run -p connectors-api` from the repository root is how
/// the README says to start this host, so a `credentials` file under it is one `git add -A` from a
/// commit. That has happened here before, with a stray capture (`3e86413`).
#[test]
fn a_store_inside_the_hosts_own_root_is_refused_before_it_is_created() {
    let root = Path::new(ROOT);
    for inside in [
        root.join("credentials"),
        root.join("src").join("credentials"),
        root.join("nested").join("deeper").join("credentials"),
    ] {
        let refusal = App::with_credential_store(
            ROOT,
            flux_web::WebOptions::default(),
            StoreChoice::File(inside.clone()),
        )
        .err()
        .unwrap_or_else(|| panic!("{} was accepted", inside.display()));

        assert!(
            !inside.exists(),
            "{} was created before being refused, which is too late",
            inside.display()
        );
        let message = format!("{refusal:#}");
        assert!(
            message.contains("inside"),
            "the refusal must say why: {message}"
        );
    }

    // And the check is a containment test, not a string prefix: a sibling whose name merely begins
    // with the root's is a different tree.
    assert!(refuse_inside(Path::new("/a/root"), Path::new("/a/rootless/x")).is_ok());
    assert!(refuse_inside(Path::new("/a/root"), Path::new("/a/root/x")).is_err());
}

/// The default location is absolute and outside the crate, whatever the machine's data home is.
#[test]
fn the_default_path_is_absolute_and_outside_the_crate() {
    let Ok(path) = default_path() else {
        // No data home. `App::deployed` must then refuse rather than invent a location, which is
        // what `a_bad_value_in_the_environment_refuses_to_build_a_host` covers for the other
        // unresolvable case.
        return;
    };
    assert!(path.is_absolute(), "{}", path.display());
    assert!(
        !path.starts_with(ROOT),
        "the default store {} is inside the crate the host is built from",
        path.display()
    );
    assert_eq!(
        StoreChoice::parse("file").expect("`file` parses"),
        StoreChoice::File(path),
        "`file` and the deployed default must be the same location, or an operator who writes the \
         variable out gets a different store from one who does not"
    );
}

// ---------------------------------------------------------------------------------------------
// What the operator is told
// ---------------------------------------------------------------------------------------------

/// **The startup banner describes the store that was bound, and overstates nothing.**
///
/// `main.rs` printed *"Credentials are held in memory only: stopping the process is the cleanup"*
/// for as long as that was the only store there was. It is now one of two answers, and an operator
/// reading the wrong one either believes a token died with the process when it did not, or pastes it
/// again every morning when they need not. So the text is assembled from the choice itself.
///
/// The file banner is held to three things a credential store owes an operator: that it survives,
/// that it is **not encrypted**, and how to destroy it. A store with no documented delete is one an
/// operator cannot revoke.
#[test]
fn the_banner_tells_the_truth_about_whichever_store_was_bound() {
    let path = PathBuf::from("/var/lib/connectors-api/credentials");
    let file = StoreChoice::File(path.clone()).banner();

    assert!(file.contains(&path.display().to_string()), "{file}");
    assert!(
        file.contains("SURVIVE A RESTART"),
        "the banner must say the credentials outlive the process: {file}"
    );
    assert!(
        file.contains("NOT ENCRYPTED"),
        "the banner must not imply a protection the store does not have: {file}"
    );
    assert!(
        file.contains("0600"),
        "the banner must say what does protect them: {file}"
    );
    assert!(
        file.contains(&format!(
            "rm -r {}",
            path.parent().expect("a parent").display()
        )),
        "a credential store with no documented delete is one an operator cannot revoke, and the \
         delete has to be the one that also removes a crashed write's leftovers: {file}"
    );
    assert!(
        file.contains(STORE_ENV),
        "the banner must say how to point it elsewhere: {file}"
    );

    let memory = StoreChoice::Memory.banner();
    assert!(memory.contains("memory only"), "{memory}");
    assert!(
        memory.contains("stopping the process is the cleanup"),
        "an in-memory host must still say what it always said: {memory}"
    );

    // Neither carries anything but a path. A `StoreChoice` holds no value, and this is the assertion
    // that keeps it that way if one is ever added to it.
    for banner in [file, memory] {
        assert!(!banner.to_lowercase().contains("sentinel"), "{banner}");
    }
}

/// The banner a host actually prints is the one its own store describes.
#[test]
fn the_host_prints_the_banner_of_the_store_it_bound() {
    let _guard = env_lock();
    std::env::set_var(STORE_ENV, "memory");
    let app = App::deployed(ROOT).expect("a host over a memory store");
    std::env::remove_var(STORE_ENV);

    assert_eq!(app.storage(), &StoreChoice::Memory);
    assert_eq!(app.storage_banner(), StoreChoice::Memory.banner());
    assert!(app.storage_banner().contains("memory only"));
}

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A directory of this test's own, removed when the guard drops.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "connectors-api-store-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn store(&self) -> PathBuf {
        self.0.join("state").join("credentials")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The environment is per process and `cargo test` runs these on parallel threads, so every test
/// that touches [`STORE_ENV`] or `XDG_DATA_HOME` holds this across the set-then-construct window.
///
/// A private copy of `tests/support`'s lock rather than a use of it: that module is compiled into
/// each test binary separately, so its `static` is already per binary, and importing it here would
/// pull in an identity provider this file has no use for.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

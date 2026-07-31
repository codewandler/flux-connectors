//! **Which store the host binds is a startup decision** (C-207), and a wrong one stops it.
//!
//! `tests/persistence.rs` asserts what a credential does once it is in a store. This file asserts
//! the decision *before* that: that an operator can express it, that a value nobody implemented
//! stops the process rather than becoming memory, that the file never lands inside the tree the host
//! runs under, and that what an operator is told at startup matches the store that was actually
//! bound.
//!
//! # Why "never a silent fallback to memory" gets its own file
//!
//! It is the failure C-207 exists to end, arrived at from the other direction. A host that answered
//! a store it could not open by holding credentials in memory instead would start successfully, look
//! exactly like a working one, serve every route correctly, and lose everything on the next restart
//! — and the operator would have no reason to suspect the store until they had already re-pasted a
//! token. Every path below is therefore asserted to *fail*, not to degrade.

use std::path::{Path, PathBuf};

use connectors_api::secrets::{default_path, refuse_inside, StoreChoice, STORE_ENV};
use connectors_api::App;

/// The crate root, which is the `root` every test here builds a host under.
const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// The name `tests/persistence.rs` spells out by hand, pinned against the one the crate exports.
///
/// That file writes the variable as a literal deliberately — it is the string an operator types, so
/// a rename should fail a test rather than silently follow the code. This is the other half of that
/// arrangement: without it the two could drift apart and nothing would say so.
#[test]
fn the_variable_name_is_the_one_the_other_tests_spell_out() {
    assert_eq!(STORE_ENV, "CONNECTORS_CREDENTIAL_STORE");
}

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
///
/// Asserted through `App::new` rather than `StoreChoice::parse` alone, because a parser that refuses
/// and a constructor that ignores the parser would both be true at once.
#[test]
fn a_bad_value_in_the_environment_refuses_to_build_a_host() {
    let _guard = env_lock();
    std::env::set_var(STORE_ENV, "vault");
    let refusal = App::new(ROOT).err();
    std::env::remove_var(STORE_ENV);

    let refusal = refusal.expect("an unknown store must stop the host");
    let message = format!("{refusal:#}");
    assert!(message.contains(STORE_ENV), "{message}");
    assert!(
        message.contains("memory"),
        "the refusal must say what would have worked: {message}"
    );
}

/// **The two constructors resolve an unset variable differently, and both do so deliberately.**
///
/// `App::deployed` is the binary's and persists by default — that is the whole of C-207.
/// `App::new` is a test's and an embedder's, and holds credentials in memory unless told where to
/// put them, because a constructor that persisted by default would write into a real operator's data
/// home the first time anybody ran `cargo test`.
///
/// Neither is a *fallback*: no path in either reaches memory after failing to open something else,
/// which is asserted above.
#[test]
fn the_deployed_default_persists_and_the_plain_one_does_not() {
    let _guard = env_lock();
    std::env::remove_var(STORE_ENV);

    let plain = App::new(ROOT).expect("a host with no store configured");
    assert_eq!(
        plain.storage(),
        &StoreChoice::Memory,
        "App::new persisted somewhere nobody named"
    );

    // `deployed` resolves to the default file location, which is under the operator's data home and
    // therefore outside this crate — so building one here would create a real file. The resolution
    // is asserted without building the host.
    match default_path() {
        Ok(path) => {
            assert!(path.is_absolute(), "{}", path.display());
            assert!(
                !path.starts_with(ROOT),
                "the default store {} is inside the crate the host is built from",
                path.display()
            );
            assert_eq!(
                StoreChoice::parse("file").expect("`file` parses"),
                StoreChoice::File(path),
                "`file` and the deployed default must be the same location, or an operator who \
                 writes the variable out gets a different store from one who does not"
            );
        }
        // No data home. `deployed` must then refuse rather than invent a location.
        Err(_) => {
            assert!(
                App::deployed(ROOT).is_err(),
                "with no data home, the deployed host must refuse rather than guess a path"
            );
        }
    }
}

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
        file.contains(&format!("rm {}", path.display())),
        "a credential store with no documented delete is one an operator cannot revoke: {file}"
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
    let app = App::new(ROOT).expect("a host over a memory store");
    std::env::remove_var(STORE_ENV);

    assert_eq!(app.storage_banner(), StoreChoice::Memory.banner());
    assert!(app.storage_banner().contains("memory only"));
}

/// The environment is per process and `cargo test` runs these on parallel threads, so every test
/// that sets [`STORE_ENV`] holds this across the set-then-construct window.
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

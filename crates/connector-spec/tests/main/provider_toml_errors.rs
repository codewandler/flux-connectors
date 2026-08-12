//! Golden-file snapshots of every rejection the provider loader makes.
//!
//! Error text is the authoring interface for `providers/*.toml`. Nobody attaches a debugger to a
//! config file; they read the message and edit the line it names. So the messages are pinned the
//! same way generated Flux will be — as committed artifacts a reviewer reads as a diff — rather
//! than asserted one substring at a time. A change that makes an error vaguer shows up in review
//! instead of quietly shipping.
//!
//! # Layout
//!
//! `tests/golden/<case>.toml` is the input; `tests/golden/<case>.error` is the exact rendering of
//! the error it must produce. Every `.toml` must have an `.error` and vice versa, so a fixture
//! cannot be added without its snapshot or left behind after its snapshot is deleted.
//!
//! # Regenerating
//!
//! ```text
//! UPDATE_GOLDEN=1 cargo test -p connector-spec --test main provider_toml_errors::
//! ```
//!
//! Then **read the diff**. Regenerating is how a snapshot is updated on purpose; it is also how a
//! regression is laundered into the repository, and only review tells the two apart.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Set this to rewrite every `.error` file from the loader's current output.
const UPDATE_ENV: &str = "UPDATE_GOLDEN";

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// The case names, from the `.toml` fixtures on disk. Sorted, so failures read in a stable order.
fn cases() -> BTreeSet<String> {
    fs::read_dir(golden_dir())
        .expect("the golden fixture directory must exist")
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .map(|path| {
            path.file_stem()
                .expect("a fixture has a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// Every fixture is rejected, and rejected with exactly the message its snapshot records.
#[test]
fn every_rejection_matches_its_golden_snapshot() {
    let update = std::env::var_os(UPDATE_ENV).is_some();
    let mut stale = Vec::new();

    for case in cases() {
        let toml_path = golden_dir().join(format!("{case}.toml"));
        let error_path = golden_dir().join(format!("{case}.error"));
        let source = fs::read_to_string(&toml_path).expect("fixture readable");

        // The display name is deliberately the path an author would actually type, because it is
        // the first thing the message says and it has to be a thing they can open.
        let name = format!("providers/{case}.toml");
        let error = connector_spec::provider::load(&name, &source)
            .err()
            .unwrap_or_else(|| {
                panic!(
                    "{} is a golden *rejection* fixture but the loader accepted it — either the \
                 fixture stopped being invalid or a check was lost",
                    toml_path.display()
                )
            });

        // Snapshots end with exactly one newline: `toml`'s multi-line parse errors already carry
        // one, ours do not, and a file that ends without one is a nuisance in every diff tool.
        let mut rendered = error.to_string();
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }

        if update {
            fs::write(&error_path, &rendered).expect("golden file writable");
            continue;
        }

        let expected = fs::read_to_string(&error_path).unwrap_or_else(|_| {
            panic!(
                "no snapshot at {}. Add one with `{UPDATE_ENV}=1 cargo test -p connector-spec \
                 --test provider_toml_errors`, then read the diff",
                error_path.display()
            )
        });

        if expected != rendered {
            stale.push(format!(
                "--- {case} — expected ---\n{expected}--- {case} — got ---\n{rendered}"
            ));
        }
    }

    assert!(
        stale.is_empty(),
        "{} golden snapshot(s) are out of date. Re-run with `{UPDATE_ENV}=1` once you have \
         confirmed the new text is an improvement:\n\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// A snapshot with no fixture is a snapshot nothing exercises. It would sit in the tree looking
/// like coverage.
#[test]
fn no_snapshot_is_orphaned() {
    let cases = cases();
    let orphans: Vec<String> = fs::read_dir(golden_dir())
        .expect("the golden fixture directory must exist")
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "error"))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            (!cases.contains(&stem)).then_some(stem)
        })
        .collect();

    assert!(
        orphans.is_empty(),
        "these `.error` snapshots have no `.toml` fixture: {orphans:?}"
    );
}

/// The four rejections C-3's Acceptance names by hand, plus the two C-2's review added and the ones
/// each later story's Acceptance names, each tied to the fixture that covers it.
///
/// The snapshot test above would still pass if a fixture were deleted, so this list is what makes
/// the required coverage explicit rather than incidental.
#[test]
fn every_required_rejection_has_a_fixture() {
    let required = [
        // Acceptance: "Validation rejects: unknown keys, …"
        "unknown-key",
        // "… an operation with no method or path, …"
        "operation-missing-method",
        "operation-missing-path",
        // "… an auth credential with no scheme, …"
        "credential-missing-scheme",
        // "… and a `basic` scheme missing `user_env`."
        "basic-missing-user-env",
        // C-2's review: a typo'd key must not fail towards sending credentials.
        "operation-auth-typo",
        "credential-env-typo",
        // C-2's review: no second spelling of "no auth".
        "empty-mechanism",
        // C-49: a service is a partition, so an operation outside every declared one is refused —
        // and the reserved `default` may not be redeclared.
        "undeclared-service",
        "reserved-default-service",
        // C-49 review: a service name reaches the emitted file path, so the address grammar is what
        // keeps a content field from choosing where a build writes.
        "service-name-escapes-the-repo",
        // C-120: a role is a contract, so each of its four refusals is pinned. An unknown name is
        // the failure mode the mechanism exists to prevent; an unsatisfied claim would make the
        // catalogue lie; `roles` is derived at provider level, not authored; a repeat is not a set.
        "unknown-service-role",
        "service-claims-an-unsatisfied-role",
        "provider-level-roles",
        "duplicate-service-role",
        // C-120 review: the two ways the mechanism was found to leak. A `default` entry beside a
        // named service hands a multi-service provider back the implicit service C-49 denies it, and
        // an event filling a role slot publishes a capability nothing can call.
        "default-service-beside-a-named-one",
        "role-slot-filled-by-an-event",
        // C-125: the composed `input_schema` is derived and never authored, and the one question it
        // could not answer is now answered by refusal — a body declared twice has no merge rule, so
        // it does not load.
        "authored-input-schema",
        "body-declared-twice",
        // C-405: the runtime axis is a closed set and an unrecognised word is refused rather than
        // defaulted. A typo that quietly became `http` is how a `process` connector ends up served
        // by a multi-tenant host, which is the one thing the declaration exists to prevent.
        "unknown-runtime",
    ];

    let cases = cases();
    for case in required {
        assert!(
            cases.contains(case),
            "the Acceptance requires a golden fixture for `{case}`, and it is missing"
        );
    }
}

//! A committed file under an artifact root that **no plan claims** — C-429.
//!
//! `build` and `diff` compare each *planned* artifact against what is on disk. The inverse question
//! — what is on disk that the build did not write — had no answer at all, so a rendering whose
//! operation was deselected survived a full `build` *and* a `diff` reporting every artifact up to
//! date. C-417 produced three of those in one story and C-426 five more; every one was deleted by
//! hand, which is not a mechanism.
//!
//! The fixtures here reproduce two of the real ones:
//!
//! * `crates/catalog/ops/babelforce/babelforce-token.flux` — an operation withheld at review.
//! * `connectors/babelforce.flux` — a provider that gained named services, so the unsuffixed module
//!   stopped being written.
//!
//! Each is asserted twice over: **the planned artifacts really are all current**, which is the trap,
//! and the CLI nonetheless refuses and names the file.
//!
//! Everything here drives [`connector_cli::run`] rather than reading the plan's orphan list, so the
//! file compiles against the code as it was before this story — which is what makes the failing-first
//! run at the merge base a measurement rather than a compile error.

use std::path::{Path, PathBuf};

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;

use crate::common::Fixture;

/// Run the CLI the way `main` does, returning whatever it printed.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|arg| arg.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// A provider definition carrying one operation per id in `operations`.
///
/// `common::definition` emits exactly one, and the whole question here is what happens when the
/// operation *set* shrinks, so the fixture has to be able to state more than one.
fn definition(provider: &str, operations: &[&str]) -> String {
    let mut toml = format!(
        "id = \"{provider}\"\n\
         vendor = \"{provider} Inc.\"\n\
         base_url = \"https://api.{provider}.example\"\n\
         description = \"A hand-authored fixture connector.\"\n"
    );
    for operation in operations {
        toml.push_str(&operation_block(operation, None));
    }
    toml
}

/// The same definition with every operation moved into one named service — the shape that stops
/// `connectors/<provider>.flux` from being written and starts
/// `connectors/<provider>-<service>.flux`.
fn definition_with_service(provider: &str, service: &str, operations: &[&str]) -> String {
    let mut toml = format!(
        "id = \"{provider}\"\n\
         vendor = \"{provider} Inc.\"\n\
         base_url = \"https://api.{provider}.example\"\n\
         description = \"A hand-authored fixture connector.\"\n\
         \n\
         [[services]]\n\
         name = \"{service}\"\n\
         description = \"The {service} API surface.\"\n"
    );
    for operation in operations {
        toml.push_str(&operation_block(operation, Some(service)));
    }
    toml
}

fn operation_block(operation: &str, service: Option<&str>) -> String {
    let service = match service {
        Some(service) => format!("service = \"{service}\"\n"),
        None => String::new(),
    };
    format!(
        "\n[[operations]]\n\
         id = \"{operation}\"\n\
         {service}\
         method = \"GET\"\n\
         direction = \"read\"\n\
         path = \"/v1/things/{{thing_id}}\"\n\
         description = \"Fetch one thing.\"\n\
         risk = \"low\"\n\
         idempotency = \"idempotent\"\n\
         \n\
         [[operations.params.path]]\n\
         name = \"thing_id\"\n\
         description = \"The thing to fetch.\"\n\
         required = true\n\
         schema = {{ type = \"integer\" }}\n"
    )
}

/// A fixture rooted where the CLI expects it, with its spec directory already in place.
fn fixture(label: &str, provider: &str, toml: &str) -> Fixture {
    let fixture = Fixture::new(label);
    fixture.write_spec(provider, "v1", "{\"openapi\":\"3.0.3\"}\n");
    fixture.write_provider(provider, toml);
    fixture
}

fn root_of(fixture: &Fixture) -> String {
    fixture
        .root()
        .to_str()
        .expect("the fixture root is UTF-8")
        .to_string()
}

/// Every artifact a full plan claims is already on disk with exactly these bytes.
///
/// This is the half that made the defect invisible, so it is asserted rather than assumed: the
/// refusal below has to be about the *unclaimed* file and nothing else.
fn assert_planned_artifacts_are_current(fixture: &Fixture) {
    let workspace = Workspace::new(fixture.root());
    let plan = pipeline::plan(&workspace, None).expect("the fixture connector compiles");
    let stale: Vec<String> = plan
        .changes()
        .map(|artifact| workspace.display_path(&artifact.path).display().to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "the fixture is supposed to be a fixed point before the orphan is judged; stale: {stale:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// The orphan is seen at all
// ---------------------------------------------------------------------------------------------

/// **The story's headline**, and C-417's third orphan reproduced exactly: a rendering left on disk
/// by an operation that was withheld at review.
///
/// Written into the tree directly rather than produced by a deselection, because *how* it got there
/// is not the property under test — a committed file no plan claims is an orphan however it arrived
/// — and stating it this way keeps the fixture a fixed point, so the refusal can only be about the
/// one unclaimed file.
#[test]
fn a_rendering_no_plan_claims_is_named_rather_than_left_looking_current() {
    let fixture = fixture(
        "orphan-rendering",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the build succeeds");

    let orphan = "crates/catalog/ops/babelforce/babelforce-token.flux";
    fixture.write(orphan, "op babelforce_token() {}\n");
    assert_planned_artifacts_are_current(&fixture);

    let error = run(&["diff", "--root", &root]).expect_err(
        "`diff` reported the tree up to date with an artifact on disk that no plan claims",
    );
    let message = format!("{error:#}");
    assert!(
        message.contains(orphan),
        "the refusal must name the file to remove, got:\n{message}"
    );
}

/// The same file, arrived at the way it actually arrives: the operation is withheld from the
/// provider definition and the next build simply stops writing its rendering.
#[test]
fn withholding_an_operation_leaves_its_rendering_behind_and_the_next_build_says_so() {
    let fixture = fixture(
        "orphan-withheld",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list", "babelforce-token"]),
    );
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the first build succeeds");

    let orphan = "crates/catalog/ops/babelforce/babelforce-token.flux";
    assert!(
        fixture.exists(orphan),
        "the first build must render both operations"
    );

    fixture.write_provider(
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    let error = run(&["build", "--root", &root]).expect_err(
        "`build` wrote a new module and left the withheld operation's rendering behind",
    );
    let message = format!("{error:#}");
    assert!(
        message.contains(orphan),
        "the refusal must name the file to remove, got:\n{message}"
    );
    assert!(
        fixture.exists(orphan),
        "`build` refuses; it must not delete a committed file on its own"
    );
}

/// C-417's first two orphans: a provider gains named services, so `connectors/<provider>.flux` and
/// its manifest stop being written and both keep sitting in `connectors/`.
#[test]
fn a_module_a_service_split_stopped_writing_is_named() {
    let fixture = fixture(
        "orphan-module",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the first build succeeds");
    assert!(fixture.exists("connectors/babelforce.flux"));

    fixture.write_provider(
        "babelforce",
        &definition_with_service("babelforce", "manager", &["babelforce-calls-list"]),
    );
    let error = run(&["build", "--root", &root])
        .expect_err("`build` left the unsuffixed module and manifest behind");
    let message = format!("{error:#}");
    for orphan in [
        "connectors/babelforce.flux",
        "connectors/babelforce.connector.toml",
    ] {
        assert!(
            message.contains(orphan),
            "the refusal must name {orphan}, got:\n{message}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// `build` refuses rather than removing
// ---------------------------------------------------------------------------------------------

/// A refusal that had already written half the run would be worse than no refusal: the tree would
/// carry a partial build *and* the orphan. Nothing is written, and nothing is deleted.
#[test]
fn a_refused_build_writes_nothing_and_deletes_nothing() {
    let fixture = fixture(
        "orphan-untouched",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the build succeeds");

    fixture.write(
        "crates/catalog/ops/babelforce/babelforce-token.flux",
        "op babelforce_token() {}\n",
    );
    // A stale module as well, so the run has something it *would* have written.
    fixture.write("connectors/babelforce.flux", "stale\n");
    let before = fixture.snapshot();

    run(&["build", "--root", &root]).expect_err("the build must refuse");
    assert_eq!(
        before,
        fixture.snapshot(),
        "a refused build changed the tree"
    );
}

// ---------------------------------------------------------------------------------------------
// A scoped run has no whole-catalogue view, so it must not judge
// ---------------------------------------------------------------------------------------------

/// `--provider` and `--service` compile a subset, so every artifact belonging to a provider the run
/// never looked at would read as unclaimed. The same hazard `connectors.lock` carries one layer
/// down (`lockfile.rs::a_scoped_build_leaves_the_lockfile_byte_identical`).
#[test]
fn a_scoped_run_reports_no_orphan_because_it_cannot_know() {
    let fixture = fixture(
        "orphan-scoped",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    fixture.write_spec("acme", "v1", "{\"openapi\":\"3.0.3\"}\n");
    fixture.write_provider("acme", &definition("acme", &["acme-thing-get"]));
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the full build succeeds");

    fixture.write(
        "crates/catalog/ops/babelforce/babelforce-token.flux",
        "op babelforce_token() {}\n",
    );

    run(&["build", "--provider", "acme", "--root", &root])
        .expect("a provider-scoped build must not judge artifacts it did not compile");
    run(&["diff", "--provider", "acme", "--root", &root])
        .expect("a provider-scoped diff must not judge artifacts it did not compile");
    run(&["build", "--service", "default", "--root", &root])
        .expect("a service-scoped build must not judge artifacts it did not compile");

    run(&["build", "--root", &root]).expect_err("the full build still sees it");
}

// ---------------------------------------------------------------------------------------------
// The roots are derived, and they are narrower than the repository
// ---------------------------------------------------------------------------------------------

/// The committed tree carries no orphan.
///
/// Doubles as the false-positive floor at full scale: `Cargo.lock`, `crates/catalog/src/lib.rs`,
/// `crates/catalog/ops/README.md`, `assets/readme-snippet.flux`, `assets/brand/*.svg` and
/// `web/public/CNAME` all sit beside generated files, and a root derived one notch too coarsely
/// reports every one of them.
///
/// Deliberately asserts only that the run does not *refuse*: a provider story's worktree has stale
/// whole-catalogue artifacts by design, and that is a different check's business.
#[test]
fn the_committed_tree_carries_no_orphaned_artifact() {
    let root = repo_root();
    let root = root.to_str().expect("the repository path is UTF-8");
    run(&["diff", "--root", root]).expect("the committed tree carries an orphaned artifact");
}

/// A file that is not shaped like anything the build writes into that directory is not an orphan.
///
/// Otherwise the check would report `crates/catalog/ops/README.md` — a hand-written file in a
/// directory whose *generated* contents are `.flux` — and a false positive in a gate is how a gate
/// stops being read.
#[test]
fn a_file_unlike_any_artifact_in_the_root_is_left_alone() {
    let fixture = fixture(
        "orphan-shape",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the build succeeds");

    fixture.write(
        "crates/catalog/ops/README.md",
        "How these renderings work.\n",
    );
    fixture.write("connectors/NOTES.md", "Scratch.\n");
    run(&["build", "--root", &root]).expect("a file of an unwritten shape is not an orphan");
}

/// The directories that merely *contain* a whole-catalogue artifact are not artifact roots.
///
/// `connectors.lock` sits in the repository root and `crates/catalog/src/generated.rs` sits beside
/// `lib.rs`; deriving a root from either one's parent directory would put `Cargo.lock` and every
/// hand-written module in the catalog crate up for deletion. Neither file can be orphaned in any
/// case — a full run always writes exactly one of each.
#[test]
fn a_directory_holding_one_whole_catalogue_file_is_not_a_root() {
    let fixture = fixture(
        "orphan-singletons",
        "babelforce",
        &definition("babelforce", &["babelforce-calls-list"]),
    );
    let root = root_of(&fixture);
    run(&["build", "--root", &root]).expect("the build succeeds");

    // Same directory and same extension as `connectors.lock` and `generated.rs` respectively.
    fixture.write("decoy.lock", "not an artifact\n");
    fixture.write("crates/catalog/src/lib.rs", "// hand-written\n");
    run(&["build", "--root", &root])
        .expect("a directory that holds one whole-catalogue file is not an artifact root");
}

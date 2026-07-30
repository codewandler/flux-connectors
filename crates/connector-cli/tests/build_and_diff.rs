//! `flux-connectors build` and `flux-connectors diff`, driven through the real command surface.
//!
//! Every test goes through [`connector_cli::run`] with arguments parsed by [`connector_cli::cli`],
//! so what is exercised here is exactly what the binary does.

mod common;

use common::Fixture;

/// Run the CLI the way `main` does, returning whatever it printed.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|a| a.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

#[test]
fn build_writes_both_artifacts_for_every_discovered_provider() {
    let fixture = Fixture::with_provider("build-writes", "zendesk");
    fixture.write_provider("freshdesk", "id = \"freshdesk\"\n");

    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("build succeeds");

    assert!(fixture.exists("connectors/zendesk.flux"));
    assert!(fixture.exists("connectors/zendesk.connector.toml"));
    assert!(fixture.exists("connectors/freshdesk.flux"));
    assert!(fixture.exists("connectors/freshdesk.connector.toml"));
}

/// Acceptance: "Building twice from unchanged inputs is a no-op producing byte-identical
/// artifacts."
#[test]
fn build_twice_is_a_byte_identical_no_op() {
    let fixture = Fixture::with_provider("build-twice", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("first build succeeds");
    let after_first = fixture.snapshot();

    let second = run(&["build", "--root", &root]).expect("second build succeeds");
    let after_second = fixture.snapshot();

    assert_eq!(
        after_first, after_second,
        "a rebuild from unchanged inputs changed the artifact tree"
    );
    assert!(
        second.contains("up to date"),
        "the second build should report itself a no-op, got:\n{second}"
    );
}

/// Acceptance: "`--provider <name>` restricts the build to one connector."
#[test]
fn provider_flag_restricts_the_build_to_one_connector() {
    let fixture = Fixture::with_provider("provider-flag", "zendesk");
    fixture.write_provider("freshdesk", "id = \"freshdesk\"\n");

    run(&[
        "build",
        "--root",
        fixture.root().to_str().unwrap(),
        "--provider",
        "zendesk",
    ])
    .expect("restricted build succeeds");

    assert!(fixture.exists("connectors/zendesk.flux"));
    assert!(
        !fixture.exists("connectors/freshdesk.flux"),
        "--provider zendesk still built freshdesk"
    );
}

#[test]
fn an_unknown_provider_is_an_error_that_names_what_exists() {
    let fixture = Fixture::with_provider("unknown-provider", "zendesk");

    let error = run(&[
        "build",
        "--root",
        fixture.root().to_str().unwrap(),
        "--provider",
        "nope",
    ])
    .expect_err("an unknown provider must not silently build nothing");

    let rendered = format!("{error:#}");
    assert!(rendered.contains("nope"), "error should name the request");
    assert!(
        rendered.contains("zendesk"),
        "error should name what exists"
    );
}

/// Acceptance: "`flux-connectors diff` shows what a rebuild would change without writing
/// anything."
#[test]
fn diff_reports_a_new_artifact_and_writes_nothing() {
    let fixture = Fixture::with_provider("diff-new", "zendesk");
    let before = fixture.snapshot();

    let output = run(&["diff", "--root", fixture.root().to_str().unwrap()]).expect("diff succeeds");

    assert_eq!(
        before,
        fixture.snapshot(),
        "diff modified the working tree; it must be read-only"
    );
    assert!(
        output.contains("connectors/zendesk.flux"),
        "diff should name the artifact it would create, got:\n{output}"
    );
}

#[test]
fn diff_shows_the_lines_a_stale_artifact_would_lose() {
    let fixture = Fixture::with_provider("diff-stale", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("build succeeds");
    let generated = fixture.read("connectors/zendesk.flux");
    fixture.write("connectors/zendesk.flux", "// hand-edited, stale\n");
    let before = fixture.snapshot();

    let output = run(&["diff", "--root", &root]).expect("diff succeeds");

    assert_eq!(
        before,
        fixture.snapshot(),
        "diff modified the working tree; it must be read-only"
    );
    assert!(
        output.contains("-// hand-edited, stale"),
        "diff should show the stale line being removed, got:\n{output}"
    );
    let first_generated_line = generated
        .lines()
        .next()
        .expect("generated module is non-empty");
    assert!(
        output.contains(&format!("+{first_generated_line}")),
        "diff should show the regenerated content being added, got:\n{output}"
    );
}

#[test]
fn diff_on_an_up_to_date_tree_reports_no_changes() {
    let fixture = Fixture::with_provider("diff-clean", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("build succeeds");
    let output = run(&["diff", "--root", &root]).expect("diff succeeds");

    assert!(
        output.contains("up to date"),
        "diff on a fresh build should report no changes, got:\n{output}"
    );
}

/// A changed input has to reach the artifact, or `diff` and `check` are both decorative.
#[test]
fn changing_an_input_changes_the_artifact() {
    let fixture = Fixture::with_provider("input-change", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("build succeeds");
    let first = fixture.read("connectors/zendesk.connector.toml");

    fixture.write_provider("zendesk", "id = \"zendesk\"\nvendor = \"Zendesk\"\n");
    run(&["build", "--root", &root]).expect("rebuild succeeds");
    let second = fixture.read("connectors/zendesk.connector.toml");

    assert_ne!(
        first, second,
        "editing providers/zendesk.toml left the artifact untouched"
    );
}

/// A run that cannot compile every provider must leave the tree exactly as it found it, rather
/// than half-updated.
#[test]
fn a_failing_run_writes_no_partial_artifacts() {
    let fixture = Fixture::with_provider("atomic-run", "zendesk");
    // An empty definition is the one thing the placeholder loader rejects today.
    fixture.write_provider("broken", "");
    let before = fixture.snapshot();

    run(&["build", "--root", fixture.root().to_str().unwrap()])
        .expect_err("a build with an unloadable provider must fail");

    assert_eq!(
        before,
        fixture.snapshot(),
        "a failed build left artifacts behind"
    );
}

#[test]
fn a_provider_needs_no_vendored_spec() {
    // "Two front-ends, one IR": a hand-authored connector has no spec file at all.
    let fixture = Fixture::new("no-spec");
    fixture.write_provider("ollama", "id = \"ollama\"\n");

    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("build succeeds");

    assert!(fixture.exists("connectors/ollama.flux"));
}

#[test]
fn commands_not_yet_implemented_say_which_story_lands_them() {
    let fixture = Fixture::with_provider("unimplemented", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();

    for (command, story) in [("check", "C-14"), ("fetch", "C-14"), ("install", "C-15")] {
        let error = run(&[command, "--root", &root])
            .err()
            .unwrap_or_else(|| panic!("`{command}` must not silently succeed"));
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains(story),
            "`{command}` should name the story that lands it, got: {rendered}"
        );
    }
}

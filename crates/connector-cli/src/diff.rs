//! Rendering a [`Plan`] as text a human reads before committing to it.
//!
//! Generated artifacts are reviewed as a diff in a PR, so `diff` is the preview of that review: it
//! answers "what would `build` do to this tree" without doing any of it.
//!
//! The edit script is intentionally simple — trim the common prefix and suffix, then show the
//! remaining old lines as removals and the remaining new lines as additions. That is a *valid* edit
//! script, just not always a minimal one, which is the right trade for generated files: they change
//! in whole blocks, and a minimal-diff algorithm would be a dependency this crate does not need.

use std::fmt::Write as _;

use crate::pipeline::{Change, Plan, PlannedArtifact};
use crate::workspace::Workspace;

/// Render every change a build would make.
pub fn render(workspace: &Workspace, plan: &Plan) -> String {
    if plan.is_up_to_date() {
        return format!(
            "{} up to date ({} checked)\n",
            describe_count(plan.artifacts.len(), "artifact"),
            describe_count(plan.providers.len(), "provider"),
        );
    }

    let mut out = String::new();
    for artifact in plan.changes() {
        render_artifact(workspace, artifact, &mut out);
    }

    let changed = plan.changes().count();
    let _ = writeln!(
        out,
        "{} would change ({} checked)",
        describe_count(changed, "artifact"),
        describe_count(plan.providers.len(), "provider"),
    );
    out
}

fn render_artifact(workspace: &Workspace, artifact: &PlannedArtifact, out: &mut String) {
    let path = workspace.display_path(&artifact.path).display();
    let old = artifact.current.as_deref().unwrap_or_default();

    match artifact.change {
        Change::Created => {
            let _ = writeln!(out, "--- /dev/null");
            let _ = writeln!(out, "+++ {path} (new file)");
        }
        Change::Modified => {
            let _ = writeln!(out, "--- {path}");
            let _ = writeln!(out, "+++ {path} (regenerated)");
        }
        Change::Unchanged => return,
    }

    for line in edit_script(old, &artifact.contents) {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
}

/// The `-`/`+`/` ` lines describing how to turn `old` into `new`.
fn edit_script(old: &str, new: &str) -> Vec<String> {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();

    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let remaining = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(remaining)
        .take_while(|(a, b)| a == b)
        .count();

    let mut script = Vec::new();
    for line in &old[..prefix] {
        script.push(format!(" {line}"));
    }
    for line in &old[prefix..old.len() - suffix] {
        script.push(format!("-{line}"));
    }
    for line in &new[prefix..new.len() - suffix] {
        script.push(format!("+{line}"));
    }
    for line in &old[old.len() - suffix..] {
        script.push(format!(" {line}"));
    }
    script
}

fn describe_count(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(old: &str, new: &str) -> Vec<String> {
        edit_script(old, new)
    }

    #[test]
    fn an_unchanged_file_produces_only_context() {
        assert_eq!(script("a\nb\n", "a\nb\n"), vec![" a", " b"]);
    }

    #[test]
    fn a_replaced_middle_keeps_its_context() {
        assert_eq!(
            script("a\nold\nz\n", "a\nnew\nz\n"),
            vec![" a", "-old", "+new", " z"]
        );
    }

    #[test]
    fn a_new_file_is_all_additions() {
        assert_eq!(script("", "a\nb\n"), vec!["+a", "+b"]);
    }

    #[test]
    fn an_appended_line_is_a_single_addition() {
        assert_eq!(script("a\n", "a\nb\n"), vec![" a", "+b"]);
    }

    #[test]
    fn a_removed_line_is_a_single_removal() {
        assert_eq!(script("a\nb\n", "a\n"), vec![" a", "-b"]);
    }

    #[test]
    fn a_wholly_different_file_shows_both_sides() {
        assert_eq!(script("x\n", "y\n"), vec!["-x", "+y"]);
    }
}

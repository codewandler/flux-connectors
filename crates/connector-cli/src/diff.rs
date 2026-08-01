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

use crate::pipeline::{Change, Orphan, Plan, PlannedArtifact};
use crate::workspace::Workspace;

/// Render every change a build would make.
///
/// An orphan ([`Plan::orphans`]) is counted here but described by [`orphan_refusal`], which is what
/// the caller fails with. Two channels, one list: this line keeps the summary from claiming a clean
/// tree, and the refusal on stderr carries the paths, so neither is complete-looking on its own and
/// neither repeats the other.
pub fn render(workspace: &Workspace, plan: &Plan) -> String {
    let mut out = String::new();

    let changed = plan.changes().count();
    if changed == 0 {
        let _ = writeln!(
            out,
            "{} up to date ({} checked)",
            describe_count(plan.artifacts.len(), "artifact"),
            describe_count(plan.providers.len(), "provider"),
        );
    } else {
        for artifact in plan.changes() {
            render_artifact(workspace, artifact, &mut out);
        }
        let _ = writeln!(
            out,
            "{} would change ({} checked)",
            describe_count(changed, "artifact"),
            describe_count(plan.providers.len(), "provider"),
        );
    }

    if !plan.orphans.is_empty() {
        let _ = writeln!(
            out,
            "{} under an artifact root claimed by no plan",
            describe_count(plan.orphans.len(), "committed file"),
        );
    }
    out
}

/// Why `build` and `diff` stop when a committed file under an artifact root is claimed by nothing —
/// C-429.
///
/// **The build refuses; it does not remove.** Deleting a committed file automatically is only as
/// safe as the root it was judged against, and a root is derived from what the emitter says it
/// writes — so an emitter bug becomes data loss instead of a failed build. `AGENTS.md`'s
/// *Non-negotiable engineering rules* already decide this shape of question: a loud compile-time
/// refusal is better than plausible but incorrect output. Removal is also the cheap half — one
/// `git rm`, reviewed in the same diff as the change that orphaned the file.
pub fn orphan_refusal(workspace: &Workspace, orphans: &[Orphan]) -> String {
    let paths: Vec<String> = orphans
        .iter()
        .map(|orphan| workspace.display_path(&orphan.path).display().to_string())
        .collect();
    let width = paths.iter().map(String::len).max().unwrap_or_default();

    let mut out = if orphans.len() == 1 {
        "1 committed file sits under an artifact root and no plan claims it:\n\n".to_string()
    } else {
        format!(
            "{} committed files sit under an artifact root and no plan claims them:\n\n",
            orphans.len()
        )
    };
    for (orphan, path) in orphans.iter().zip(&paths) {
        let _ = writeln!(
            out,
            "  {path:width$}   (artifact root: {})",
            workspace.display_path(&orphan.root).display(),
        );
    }
    out.push_str(
        "\nNothing writes these files any more, so `diff` cannot report them stale and `build` \
         will not\noverwrite them — they ship exactly as they are, forever. Remove each one \
         (`git rm <path>`)\nand run the build again.\n\n\
         `build` refuses rather than deleting: an artifact root is derived from what the emitter \
         says it\nwrites, so removing a file on a mis-derived root would turn an emitter bug into \
         data loss.",
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

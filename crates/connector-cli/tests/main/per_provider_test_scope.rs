//! **A provider's own test file asserts about that provider, never about the catalogue** (C-230).
//!
//! `AGENTS.md` says provider stories run in parallel because "a provider story writes
//! `providers/<id>.toml` plus only per-provider artifacts, so two implementors' write sets are
//! disjoint". A per-provider test that *enumerates* `providers/` breaks that guarantee **without
//! touching a shared file**, which is what makes this class expensive out of all proportion to its
//! size:
//!
//! - it is invisible from the implementor's own worktree, which holds only their provider;
//! - it is invisible to the other implementor, whose diff is entirely disjoint;
//! - it is **not** among the nine whole-catalogue staleness failures `AGENTS.md` tabulates, so it
//!   does not read as expected;
//! - and the coordinator cannot resolve it the way those eight are resolved. They are *regenerated*;
//!   this is a hand-written literal in a shipped test, and regenerating nothing fixes it.
//!
//! It surfaces for the first time at integration, attributed to whichever merge happened to be
//! second.
//!
//! **Two instances were measured on 2026-07-31.** C-216's Discord test walked every provider and
//! asserted the resulting prefix census equalled a four-element literal; Klaviyo, landing in the same
//! wave, declares a fifth. Each branch was green alone and red together. Review caught that one
//! before it merged. The second had been on `main` since C-165:
//! `trello_connector.rs::trello_is_the_only_query_placement_in_the_shipped_catalogue` compared a
//! catalogue-wide census against a two-element literal and was green *only* because no provider since
//! Trello had placed a credential in the query string. The next one that did would have turned
//! **Trello's** test red. That second instance is why this guard is mechanical rather than a note
//! asking reviewers to look: one instance was caught by review and one survived it.
//!
//! ## What this refuses
//!
//! A directory walk of `providers/` inside a **per-provider** test file — a file under
//! `crates/*/tests/` whose name begins with a shipped provider id (`trello_connector.rs`,
//! `babelforce_ivr.rs`). The owning id is derived from `providers/` rather than listed here, so this
//! guard has no inventory of its own to keep in step, and a new provider's test file falls under it
//! the moment the provider ships.
//!
//! ## Why the line is the *walk* and not the literal
//!
//! The defect is strictly narrower than what is refused here: it is a walk compared against a
//! hand-written literal, and a walk compared against a *monotone* claim — `resend_connector.rs`'s
//! `without_config.len() > 1` — could not be falsified by a provider landing. The broader line is
//! deliberate, for two reasons.
//!
//! Detecting "compared against a literal" textually means pairing a `read_dir` with an `assert_eq!`
//! several lines away and guessing whether the right-hand side is closed. A guard that is
//! approximately right about the exact hazard is worse than one that is exactly right about a
//! slightly wider one, because the first kind fails open and says nothing about it.
//!
//! The second reason is the one that actually settles it: **the author of a per-provider test cannot
//! review the population they are quantifying over.** They are working in a worktree holding their
//! provider, against a catalogue that will have grown by the time their branch merges. A monotone
//! claim written there is correct by luck rather than by construction, and the edit that turns it
//! into an equality is one word. So the rule is the simple one — if the claim is about this
//! provider, load it by name; if it is about the catalogue, it belongs in a file that owns that
//! claim.
//!
//! ## Where a catalogue-wide claim goes instead
//!
//! This is not "never measure the catalogue". Three homes already exist and are the pattern:
//!
//! - a **universally quantified property** over whatever ships, in a whole-catalogue test file —
//!   `crates/connector-flux/tests/query_placed_credentials.rs` (where Trello's census moved),
//!   `shipped_modules.rs`, `input_schema_agreement.rs`. Adding a provider that satisfies the
//!   property leaves it green, and one that violates it is exactly when the test should fail;
//! - a **claim about named connectors**, loaded by name, when the premise really is about specific
//!   ones — `discord_connector.rs`'s
//!   `the_non_bearer_prefixes_this_connector_joins_were_already_shipped`;
//! - a **ratcheted measurement** with a floor and a slack window, coordinator-owned and named in
//!   `AGENTS.md`'s fence — `crates/connector-spec/tests/response_schema_coverage.rs`.
//!
//! ## What this does not reach, by construction
//!
//! A list, so that nobody mistakes a green run for a proof of absence:
//!
//! - a walk spelled without `read_dir` — a `glob` crate, a `WalkDir`, a shell-out;
//! - a walk of `providers/` from a helper in a *non*-per-provider file that a per-provider file then
//!   calls (no such helper exists today; `crates/*/tests` have no shared provider-walking module);
//! - a catalogue claim assembled from `include_str!` of a generated index rather than from a walk;
//! - a per-provider claim in a file whose name does not begin with a provider id.
//!
//! Those are accepted gaps. This guard is aimed at the exact shape that regressed twice in one day,
//! and it lives in `connector-cli` for the reason C-54's
//! `no_test_hand_maintains_a_shipped_provider_list` gives for living here: this is the crate whose
//! tests already read the repository tree, and no other crate should grow that reach for one check.

use std::path::{Path, PathBuf};

use connector_cli::workspace::Workspace;

/// The marker for a directory walk. See the module's list of what this does not reach.
const WALK: &str = "read_dir";

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn workspace() -> Workspace {
    Workspace::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository root exists"),
    )
}

/// **A per-provider test file may not enumerate `providers/`** (C-230).
///
/// The failure names every offending file and points at the homes a catalogue-wide claim has, so a
/// provider implementor who trips this is told what to do rather than only what not to.
#[test]
fn no_per_provider_test_walks_the_provider_catalogue() {
    let root = workspace().root().to_path_buf();
    let ids = shipped(&root);

    let mut offenders: Vec<String> = Vec::new();
    for source in test_sources(&root) {
        let Some(owner) = owning_provider(&source, &ids) else {
            continue;
        };

        let text = std::fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));
        if !code_of(&text).contains(WALK) {
            continue;
        }

        offenders.push(format!(
            "{} — `{owner}`'s own test file walks providers/",
            relative(&source, &root)
        ));
    }
    offenders.sort();

    assert!(
        offenders.is_empty(),
        "a per-provider test enumerates the provider catalogue. Whatever it concludes is a claim \
         about every connector that ships, written in a worktree holding one of them — so an \
         unrelated provider landing can turn this provider's test red, and neither implementor can \
         see it coming.\n\n  {}\n\nIf the claim is about this connector, load what it names by \
         name. If it is genuinely about the catalogue, it belongs in a file that owns that claim: a \
         universally quantified property in a whole-catalogue test \
         (crates/connector-flux/tests/query_placed_credentials.rs), a claim about named predecessors \
         (discord_connector.rs), or a ratcheted measurement \
         (crates/connector-spec/tests/response_schema_coverage.rs). See AGENTS.md, \"A per-provider \
         test asserts about its provider\".",
        offenders.join("\n  ")
    );
}

/// The ids the repository ships — the set that decides which test files are per-provider ones.
///
/// Read from `providers/`, never listed: a guard against a hand-maintained inventory that kept one
/// of its own would be an odd thing to ship, and C-54's
/// `no_test_hand_maintains_a_shipped_provider_list` would refuse it besides.
fn shipped(root: &Path) -> Vec<String> {
    let dir = root.join("providers");
    let ids: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();

    assert!(
        !ids.is_empty(),
        "{} holds no provider definitions, so this guard would pass vacuously",
        dir.display()
    );
    ids
}

/// The provider whose file this is, if any — the **longest** id the file stem begins with.
///
/// Longest rather than first, because the ids are not prefix-free: `microsoft_graph_connector.rs`
/// belongs to `microsoft_graph`, and a shorter `microsoft` shipping later must not silently take
/// ownership of it and change what the failure message says.
///
/// The boundary is required. Matching a bare prefix would let a provider named `op` claim
/// `op_emitter.rs`, which is an emitter test and not a per-provider one.
fn owning_provider<'a>(source: &Path, ids: &'a [String]) -> Option<&'a str> {
    let stem = source.file_stem()?.to_string_lossy().into_owned();

    ids.iter()
        .filter(|id| stem == **id || stem.starts_with(&format!("{id}_")))
        .max_by_key(|id| id.len())
        .map(String::as_str)
}

/// `source` with whole-line comments removed, so the module docs above — and the ones this guard
/// asks authors to write — may name `read_dir` in prose without tripping it.
///
/// Only lines whose first non-whitespace is `//` are dropped. Trimming from a mid-line `//` instead
/// would cut a line at the `//` in a `https://` URL literal and could hide code after it.
fn code_of(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `*.rs` under `crates/*/tests`, recursively — the test tree this check governs.
fn test_sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let crates = std::fs::read_dir(root.join("crates")).expect("crates/ is readable");

    for entry in crates {
        let tests = entry
            .expect("readable directory entry")
            .path()
            .join("tests");
        if !tests.is_dir() {
            continue;
        }

        let mut pending = vec![tests];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
            {
                let path = entry.expect("readable directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    found.push(path);
                }
            }
        }
    }

    found
}

/// `path` spelled relative to the repository root, with forward slashes, for a stable message.
fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

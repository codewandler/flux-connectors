#!/usr/bin/env bash
#
# Cut a flux-connectors release: promote both changelogs, bump every version string, regenerate
# every artifact, run the full gate, then commit + tag. One command for what AGENTS.md § Release
# process recorded on 2026-08-01 as nine ordered hand-run steps (C-427).
#
#   scripts/cut-release.sh <version>            # explicit, e.g. 0.10.0
#   scripts/cut-release.sh patch                # bump the patch component
#   scripts/cut-release.sh minor                # bump minor, reset patch
#   scripts/cut-release.sh <ver> --notes FILE   # editorial tag body, instead of the promoted
#                                               #   WHATS-NEW.md section
#   scripts/cut-release.sh <ver> --no-gate      # commit without the gate, and DO NOT TAG
#
# Ported from ../flux's scripts/cut-release.sh, which solved the transactional half already. The
# differences below are this repository's, not restyling.
#
# ## What it does not do, and this is the whole safety boundary
#
# It does not push, and it never runs `cargo publish` in any form. **Pushing the `vX.Y.Z` tag IS the
# crates.io publication** (AGENTS.md § Publishing contract): a published version cannot be withdrawn
# or corrected, so that step is prepared here and deliberately not taken here. The script prints the
# two pushes as the next thing an agent decides to do.
#
# The corollary is `--no-gate`: it commits and then **stops before the tag**. A tag is the trigger,
# so a tag this script creates always sits behind a green gate. Without one you get a commit you can
# inspect, gate yourself, and tag by hand — never a publication trigger nobody checked.
#
# ## What stays a decision
#
# Two things, and the script is careful not to appear to make either:
#
#   - **The bump size.** Cargo pre-1.0 SemVer: for `0.y.z` the **minor** position is the breaking
#     signal. Scan `[Unreleased]` and the commits since the last tag — any breaking change is a
#     minor, additive and fixes only is a patch. Never a rolling patch counter. `major` is *refused*
#     while the major component is 0, because reaching 1.0.0 by typing a keyword is not a decision.
#   - **Whether WHATS-NEW.md says the right thing.** The script promotes what is written and warns
#     when the section is empty; it cannot know whether the words are right for a customer.
#
# ## The step that bites, and why this script exists rather than a checklist
#
# This repository is a compiler whose output records the compiler's own version: every generated
# `connectors/*.connector.toml` carries `generator = "flux-connectors <version>"` (from
# `crates/connector-cli/src/seam.rs`, which reads `CARGO_PKG_VERSION`), and since C-189
# `connectors.lock` hashes them. So bumping `[workspace.package].version` without running
# `connector-cli build` in the same commit leaves the tree inconsistent with itself and `diff` red —
# and it surfaces *after* the commit rather than during it. Cutting v0.9.0 by hand rewrote 184
# artifacts at that step. Step 5 regenerates and step 6 **refuses to continue** unless `diff` then
# reports everything up to date and no artifact still carries the old version.
#
# Step 6 reads `diff`'s *text*, not its exit status, and that is not belt-and-braces: `diff` is a
# preview and reports drift by printing it, exiting 0 either way (`crates/connector-cli/src/lib.rs`,
# `show_diff`). A `set -e` check on that command would pass through exactly the failure this script
# exists to prevent.
#
# ## Transactional (C-147's finding, ported)
#
# Everything below step 3 mutates tracked files, and the gate runs at the END — after those edits,
# because it has to test what is about to be tagged. When the gate then fails, a half-cut tree is
# left behind: re-running would promote `[Unreleased]` a SECOND time and mint a phantom version
# section. So every path this script may touch is snapshotted up front and restored on ANY non-zero
# exit before the commit. A failed cut is safe to re-run.
#
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

NO_GATE=0
NOTES_FILE=""
ARGS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --no-gate) NO_GATE=1 ;;
    --notes)
      shift
      [ "$#" -gt 0 ] || { echo "--notes needs a file" >&2; exit 2; }
      NOTES_FILE="$1"
      ;;
    --notes=*) NOTES_FILE="${1#--notes=}" ;;
    -h | --help)
      sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*) echo "unknown option: $1" >&2; exit 2 ;;
    *) ARGS+=("$1") ;;
  esac
  shift
done
[ "${#ARGS[@]}" -ge 1 ] || {
  echo "usage: scripts/cut-release.sh <version|patch|minor> [--notes FILE] [--no-gate]" >&2
  exit 2
}
[ -z "$NOTES_FILE" ] || [ -f "$NOTES_FILE" ] || { echo "no such notes file: $NOTES_FILE" >&2; exit 2; }

# ---------------------------------------------------------------------------------------------
# The paths a cut owns. Derived from AGENTS.md § Source and generated-file boundaries, which is the
# table that says which files a build writes — enumerate from *there* and from the tree, never from
# a list somebody typed once (§ Before you assert anything).
# ---------------------------------------------------------------------------------------------

# Written by `connector-cli build`. Required to be CLEAN before a cut: they are whole-catalogue
# artifacts, which AGENTS.md makes coordinator-owned precisely because a partial run cannot write
# one honestly. A cut regenerates them whole, so uncommitted work in them belongs to somebody else.
GENERATED_PATHS=(
  connectors
  connectors.lock
  # The canonical per-provider documents (C-536) and the pack compiled from them (C-537): both
  # carry the generator string, so a cut that regenerated them without committing them would leave
  # the release commit disagreeing with its own working tree.
  catalog
  crates/catalog-reader/catalog.pack
  crates/catalog/ops
  crates/catalog/src/generated.rs
  crates/catalog/src/generated
  web/public
)
# The README images are a glob rather than two names, so a third theme is picked up by the tree
# instead of by an edit here. `assets/readme-snippet.flux` is the hand-maintained *source* and is
# deliberately not in this list.
for image in assets/readme-snippet-*.svg; do
  [ -e "$image" ] && GENERATED_PATHS+=("$image")
done

# The packaged README install examples move on the same minor line as the workspace. Keep package
# names beside paths so the rewrite below is exact and the release-path allowlist cannot omit one.
PUBLIC_PACKAGES=(
  codewandler-connector-address
  codewandler-connector-catalog
  codewandler-connector-catalog-reader
  codewandler-connector-secrets
  codewandler-connector-pack
)
PUBLIC_README_PATHS=(
  crates/connector-address/README.md
  crates/catalog/README.md
  crates/catalog-reader/README.md
  crates/connector-secrets/README.md
  crates/connector-pack/README.md
)

# Written by this script. Also required to be clean: a cut is taken on top of committed work, so a
# dirty manifest or README means somebody's change is about to be labelled a release.
BUMPED_PATHS=(Cargo.toml Cargo.lock README.md "${PUBLIC_README_PATHS[@]}")

# The operator's own input to the cut, and the one thing that is EXPECTED to be dirty: AGENTS.md's
# step 1 is "polish both changelogs and check them against the diff". They are promoted and
# committed here.
CHANGELOG_PATHS=(CHANGELOG.md WHATS-NEW.md)

RELEASE_PATHS=("${BUMPED_PATHS[@]}" "${CHANGELOG_PATHS[@]}" "${GENERATED_PATHS[@]}")

# ---------------------------------------------------------------------------------------------
# 1) Read the current version, and decide the target.
# ---------------------------------------------------------------------------------------------

# Scoped to the `[workspace.package]` section rather than `grep -m1 '^version = '`: the
# `[workspace.dependencies]` requirements below it carry the same string, and a first-match grep is
# only correct while the section order happens to hold.
read_workspace_version() {
  awk '
    /^\[workspace\.package\]/ { section = 1; next }
    /^\[/                     { section = 0 }
    section && /^version[[:space:]]*=/ { print; exit }
  ' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/'
}

OLD=$(read_workspace_version)
[ -n "$OLD" ] || { echo "could not read [workspace.package].version from Cargo.toml" >&2; exit 1; }
IFS='.' read -r MA MI PA <<<"$OLD"

case "${ARGS[0]}" in
  patch) NEW="$MA.$MI.$((PA + 1))" ;;
  minor) NEW="$MA.$((MI + 1)).0" ;;
  major)
    if [ "$MA" -eq 0 ]; then
      echo "refusing 'major': pre-1.0 the MINOR position is the breaking signal (AGENTS.md)." >&2
      echo "Declaring 1.0.0 is a decision, not a keyword — spell it out if that is the intent:" >&2
      echo "    scripts/cut-release.sh 1.0.0" >&2
      exit 1
    fi
    NEW="$((MA + 1)).0.0"
    ;;
  *) NEW="${ARGS[0]}" ;;
esac
echo "$NEW" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "bad target version: $NEW" >&2; exit 1; }
[ "$NEW" != "$OLD" ] || { echo "target version equals current ($OLD)" >&2; exit 1; }
IFS='.' read -r NEW_MAJOR NEW_MINOR _ <<<"$NEW"
OLD_PUBLIC_REQUIREMENT="$MA.$MI"
NEW_PUBLIC_REQUIREMENT="$NEW_MAJOR.$NEW_MINOR"
! git rev-parse -q --verify "refs/tags/v$NEW" >/dev/null || {
  echo "tag v$NEW already exists — that version is cut (and possibly published)" >&2
  exit 1
}

DATE=$(date +%Y-%m-%d)

# ---------------------------------------------------------------------------------------------
# 2) Preflight, BEFORE anything is touched, so a refusal costs nothing.
#
# Agents work concurrently in this repository, so "another session's uncommitted work must never be
# swept into a release commit" is not a hypothetical. Two halves keep it out:
#
#   - here: refuse outright when a path this cut REGENERATES OR REWRITES is already dirty, because
#     a cut cannot tell that work apart from its own;
#   - at step 8: commit by explicit pathspec with `--only`, so nothing another session merely
#     *staged* rides along in the index.
# ---------------------------------------------------------------------------------------------
#
# `git status --porcelain` rather than `git diff HEAD`, and the difference is not cosmetic: it names
# the individual files (a directory pathspec through `git diff --quiet` can only say "connectors"),
# and it reports **untracked** ones. An untracked artifact sitting in `connectors/` is invisible to
# `git diff HEAD` and would be committed by the `git commit --only connectors` at step 8.
dirty=$(git status --porcelain -- "${BUMPED_PATHS[@]}" "${GENERATED_PATHS[@]}")
if [ -n "$dirty" ]; then
  echo "!! refusing to cut: these are uncommitted, and a cut rewrites them:" >&2
  printf '%s\n' "$dirty" | sed 's/^/!!   /' >&2
  echo "!!" >&2
  echo "!! A release is cut ON TOP of committed work. Commit or stash these first — if they are" >&2
  echo "!! another session's, they are not yours to fold into a release commit." >&2
  echo "!! (CHANGELOG.md and WHATS-NEW.md are exempt: polishing them is step 1 of the cut.)" >&2
  exit 1
fi

# The promotable section is the first release section in each file. Merely finding an
# `[Unreleased]` somewhere is insufficient: v0.11.0 and v0.12.0 were hand-inserted above it in
# WHATS-NEW.md, so the next promotion would have landed below two newer releases while still
# reporting success (C-453).
for changelog in "${CHANGELOG_PATHS[@]}"; do
  first_section=$(grep -m1 '^## ' "$changelog" || true)
  if [ "$first_section" != '## [Unreleased]' ]; then
    echo "!! refusing to cut: $changelog's first release section is not ## [Unreleased]:" >&2
    echo "!!   ${first_section:-<none>}" >&2
    echo "!! Put one promotable [Unreleased] section above every released version." >&2
    exit 1
  fi
done

echo "== cutting $OLD -> $NEW =="

# ---------------------------------------------------------------------------------------------
# 3) Arm the transaction. Everything after this line is undone by the EXIT trap unless the commit
#    lands. Directories are snapshotted whole and restored whole, so an artifact the cut CREATES is
#    removed by the restore rather than left behind as the one file `git status` still reports.
# ---------------------------------------------------------------------------------------------
SNAPSHOT="$(mktemp -d)"
for path in "${RELEASE_PATHS[@]}"; do
  [ -e "$path" ] || continue
  mkdir -p "$SNAPSHOT/$(dirname "$path")"
  cp -a "$path" "$SNAPSHOT/$path"
done

COMMITTED=0
restore_on_failure() {
  local status=$?
  if [ "$COMMITTED" -eq 1 ] || [ "$status" -eq 0 ]; then
    rm -rf "$SNAPSHOT"
    return
  fi
  echo >&2
  echo "!! cut FAILED (exit $status) — restoring the working tree to its pre-cut state" >&2
  for path in "${RELEASE_PATHS[@]}"; do
    # A relative, literal path from RELEASE_PATHS, under the repo root this script cd'd into.
    rm -rf -- "$path"
    [ -e "$SNAPSHOT/$path" ] || continue
    mkdir -p "$(dirname "$path")"
    cp -a "$SNAPSHOT/$path" "$path"
  done
  rm -rf "$SNAPSHOT"
  echo "!! restored. Fix the failure and re-run — no phantom version section was left behind." >&2
}
trap restore_on_failure EXIT
# A fatal signal has to reach the restore as well, and this is measured rather than defensive:
# piping this script's stdout into `head` while developing it killed it with SIGPIPE partway through
# step 5, and **an EXIT trap alone does not run for an untrapped fatal signal**. It left both
# changelogs promoted and the manifest bumped — precisely the half-cut tree the snapshot exists to
# prevent, reached through the one exit path the snapshot did not cover. Each handler `exit`s, which
# is what causes bash to run the EXIT trap.
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 141' PIPE
trap 'exit 143' TERM

# ---------------------------------------------------------------------------------------------
# 4) Promote `[Unreleased]` in BOTH changelogs. Two audiences, and every release touches both.
#    `## [X.Y.Z] — <date>` with an EM DASH, which is what every existing header in both files uses.
# ---------------------------------------------------------------------------------------------
promote() {
  local file=$1
  grep -q '^## \[Unreleased\]' "$file" || {
    echo "   !! no [Unreleased] header in $file — add the [$NEW] section by hand" >&2
    return 0
  }
  awk -v version="$NEW" -v date="$DATE" '
    !promoted && /^## \[Unreleased\]/ {
      print "## [Unreleased]"
      print ""
      print "## [" version "] — " date
      promoted = 1
      next
    }
    { print }
  ' "$file" >"$file.cut"
  mv "$file.cut" "$file"
  echo "   promoted $file: [Unreleased] -> [$NEW] — $DATE"
}

# An empty customer section is legal — an internal-only release has nothing to say to a user — but
# it is far more often the entry somebody forgot, so it is loud. AGENTS.md: "a *user-visible* change
# missing one is the defect this file exists to prevent."
if ! sed -n '/^## \[Unreleased\]/,/^## \[/p' WHATS-NEW.md | sed '1d;$d' | grep -q '[^[:space:]]'; then
  echo "   !! WHATS-NEW.md [Unreleased] is EMPTY — nothing customer-visible in $NEW?" >&2
  echo "   !! (legal for an internal-only release; otherwise write it before pushing the tag)" >&2
fi
promote CHANGELOG.md
promote WHATS-NEW.md

# ---------------------------------------------------------------------------------------------
# 5) Bump every version string, then regenerate. Each edit is VERIFIED from the tree afterwards
#    rather than trusted to have matched, because a `sed` that matches nothing exits 0.
# ---------------------------------------------------------------------------------------------

# 5a) `[workspace.package].version`, scoped to that section.
sed -i -E "/^\[workspace\.package\]/,/^\[/ \
  s/^version[[:space:]]*=[[:space:]]*\"$OLD\"/version = \"$NEW\"/" Cargo.toml
[ "$(read_workspace_version)" = "$NEW" ] || {
  echo "!! [workspace.package].version is still $(read_workspace_version), not $NEW" >&2
  exit 1
}
echo "   bumped [workspace.package].version $OLD -> $NEW"

# 5b) The internal path-dependency requirements in `[workspace.dependencies]`. Unlike flux — whose
#     pins deliberately sit at MINOR.0 across a patch line — every internal requirement here is the
#     exact workspace version, so all of them move on every cut. They are found by `path = "crates/`
#     rather than listed, so a crate added later is bumped without an edit to this script; the
#     assertion below is what makes that claim checkable instead of hopeful.
sed -i "\#path = \"crates/#{ s/version = \"$OLD\"/version = \"$NEW\"/g }" Cargo.toml
stale=$({ grep "path = \"crates/" Cargo.toml || true; } | grep -c "version = \"$OLD\"" || true)
[ "$stale" -eq 0 ] || {
  echo "!! $stale internal path dependency requirement(s) still ask for $OLD" >&2
  exit 1
}
pins=$({ grep "path = \"crates/" Cargo.toml || true; } | grep -c "version = \"$NEW\"" || true)
echo "   bumped $pins internal path-dependency requirement(s) in [workspace.dependencies]"

# 5c) README.md. Only `v$OLD` is rewritten: a bare `0.9.0` in prose could as easily be a flux-lang
#     or a dependency version, and this script does not get to guess.
readme=$(grep -c "v$OLD" README.md || true)
if [ "$readme" -gt 0 ]; then
  sed -i "s/v$OLD/v$NEW/g" README.md
  echo "   bumped $readme version mention(s) in README.md"
else
  echo "   !! README.md names no v$OLD — check whether it should name v$NEW" >&2
fi

# 5d) Packaged crate README requirements. Each exact old line must exist once before it is changed
# and each exact new line must exist once afterwards. Locating the literal line number before using
# `sed c` keeps regex metacharacters in TOML examples inert and prevents an unrelated version in
# prose from moving.
replace_exact_line() {
  local file="$1" old_line="$2" new_line="$3"
  local matches line_number
  matches=$(grep -Fxc -- "$old_line" "$file" || true)
  [ "$matches" -eq 1 ] || {
    echo "!! $file must contain exactly one release-owned line:" >&2
    echo "!!   $old_line" >&2
    echo "!! found: $matches" >&2
    exit 1
  }
  line_number=$(grep -Fnx -- "$old_line" "$file" | cut -d: -f1)
  sed -i "${line_number}c\\${new_line}" "$file"
  matches=$(grep -Fxc -- "$new_line" "$file" || true)
  [ "$matches" -eq 1 ] || {
    echo "!! $file did not receive the release-owned line:" >&2
    echo "!!   $new_line" >&2
    exit 1
  }
}

[ "${#PUBLIC_PACKAGES[@]}" -eq "${#PUBLIC_README_PATHS[@]}" ] || {
  echo "!! packaged README release-path inventory is inconsistent" >&2
  exit 1
}
for index in "${!PUBLIC_PACKAGES[@]}"; do
  package="${PUBLIC_PACKAGES[$index]}"
  readme="${PUBLIC_README_PATHS[$index]}"
  replace_exact_line \
    "$readme" \
    "$package = \"$OLD_PUBLIC_REQUIREMENT\"" \
    "$package = \"$NEW_PUBLIC_REQUIREMENT\""
done
replace_exact_line \
  crates/connector-secrets/README.md \
  "# codewandler-connector-secrets = { version = \"$OLD_PUBLIC_REQUIREMENT\", features = [\"vault\"] }" \
  "# codewandler-connector-secrets = { version = \"$NEW_PUBLIC_REQUIREMENT\", features = [\"vault\"] }"
echo "   bumped $(( ${#PUBLIC_PACKAGES[@]} + 1 )) packaged README dependency example(s) $OLD_PUBLIC_REQUIREMENT -> $NEW_PUBLIC_REQUIREMENT"

# 5e) Re-lock, so the workspace members' own entries in Cargo.lock carry $NEW. `--workspace` touches
#     only those: third-party pins are not re-resolved by a release cut.
cargo update --workspace >/dev/null 2>&1
echo "   re-locked the workspace"

# 5f) THE STEP THIS SCRIPT EXISTS FOR. Regenerate every artifact, so the 120-odd generated manifests
#     and the lockfile that hashes them carry $NEW in the same commit as the bump.
#     No `--png`: that needs `flux` on PATH and writes an unchecked asset.
echo "   regenerating every artifact (this rewrites every file carrying the generator string)"
build_output=$(cargo run -p connector-cli -- build) || {
  echo "!! regeneration failed" >&2
  exit 1
}
echo "   $(printf '%s\n' "$build_output" | tail -n1)"

# ---------------------------------------------------------------------------------------------
# 6) Refuse to have bumped without regenerating. Two independent checks, because they fail
#    differently: the first catches an artifact the build did not write, the second catches one it
#    wrote without the new version in it.
# ---------------------------------------------------------------------------------------------
diff_output=$(cargo run -p connector-cli -- diff)
# Text, not exit status — `diff` is a preview and exits 0 whether or not it found drift.
case "$diff_output" in
  *" up to date ("*) ;;
  *)
    echo "!! after regenerating, \`diff\` still reports drift:" >&2
    printf '%s\n' "$diff_output" | tail -n 20 >&2
    exit 1
    ;;
esac
echo "   $(printf '%s\n' "$diff_output" | tail -n1)"

# The generator string is the specific thing a forgotten regeneration leaves behind, so it is
# checked by name over the generated tree rather than inferred from `diff` agreeing with itself.
existing_generated=()
for path in "${GENERATED_PATHS[@]}"; do
  [ -e "$path" ] && existing_generated+=("$path")
done
[ "${#existing_generated[@]}" -gt 0 ] || { echo "!! no generated artifacts found at all" >&2; exit 1; }
if grep -rl -- "flux-connectors $OLD" "${existing_generated[@]}" >/dev/null 2>&1; then
  echo "!! generated artifacts still carry \`generator = \"flux-connectors $OLD\"\`:" >&2
  grep -rl -- "flux-connectors $OLD" "${existing_generated[@]}" | head -n 10 >&2
  exit 1
fi
stamped=$({ grep -rl -- "flux-connectors $NEW" "${existing_generated[@]}" || true; } | wc -l)
echo "   $stamped generated file(s) now carry the $NEW generator string"

# ---------------------------------------------------------------------------------------------
# 7) Every CI gate — AGENTS.md § Validation, in its order. `--no-fail-fast` is not optional: without
#    it `cargo test --workspace` stops at the first failing binary and reports a number that is
#    wrong. The two Node trees are explicit too: v0.12.0 proved that leaving them to post-push CI
#    lets the tag publish crates before the site reports red (C-453).
# ---------------------------------------------------------------------------------------------
if [ "$NO_GATE" -eq 0 ]; then
  echo "== gate =="
  # `set -e` would abort here anyway; naming the step matters because a bare non-zero exit scrolled
  # off screen reads as "the script died", not "the gate is red", and the restore message below is
  # easier to trust when it says which step caused it.
  gate() { "$@" || { echo "!! gate step failed: $*" >&2; exit 1; }; }
  gate cargo fmt --all
  gate cargo build --workspace
  gate cargo test --workspace --no-fail-fast
  gate cargo clippy --workspace --all-targets -- -D warnings
  gate cargo fmt --all --check
  gate npm --prefix web ci
  gate npm --prefix web run build
  gate npm --prefix web test
  gate npm --prefix crates/connectors-api/ui ci
  gate npm --prefix crates/connectors-api/ui test
  echo "   gate green"
else
  echo "== gate SKIPPED (--no-gate) — this cut will NOT be tagged =="
fi

# ---------------------------------------------------------------------------------------------
# 8) Commit ONLY the release paths.
#
# `--only` commits exactly these paths from the working tree and leaves any other staged work in
# the index alone. Without it `git commit` commits the INDEX, so a `git add` another session ran
# minutes ago rides along in the release commit — a hazard flux measured rehearsing its own 0.29.0.
# ---------------------------------------------------------------------------------------------
COMMIT_PATHS=()
for path in "${RELEASE_PATHS[@]}"; do
  [ -e "$path" ] && COMMIT_PATHS+=("$path")
done

git commit --only "${COMMIT_PATHS[@]}" -F - <<COMMIT_MSG || { echo "!! commit failed" >&2; exit 1; }
Release v$NEW

- Bump [workspace.package].version and $pins internal path-dependency
  requirement(s) $OLD -> $NEW, and re-lock the workspace.
- Move the four packaged README dependency examples from
  $OLD_PUBLIC_REQUIREMENT -> $NEW_PUBLIC_REQUIREMENT.
- Regenerate every artifact, so the generator string and connectors.lock
  carry $NEW in the same commit as the bump; $stamped generated file(s)
  name $NEW.
- Promote [Unreleased] to [$NEW] in CHANGELOG.md and WHATS-NEW.md.

Cut by scripts/cut-release.sh.
COMMIT_MSG

# Past this point the cut is in history: restoring the working tree would undo nothing useful and
# would clobber the committed state, so disarm the snapshot.
COMMITTED=1
echo "   committed: $(git log -1 --format='%h %s')"

if [ "$NO_GATE" -eq 1 ]; then
  echo
  echo "== NOT TAGGED: the gate did not run. ==" >&2
  echo "   The tag is the crates.io trigger, so this script only ever creates one behind a green" >&2
  echo "   gate. Run the gate yourself, then:  git tag -a v$NEW" >&2
  exit 0
fi

# ---------------------------------------------------------------------------------------------
# 9) Annotated, not lightweight: `git push --follow-tags` only pushes annotated tags, and a
#    lightweight one sits there looking cut while no workflow ever fires.
#
#    The body defaults to the WHATS-NEW.md section just promoted, because that is the voice the
#    existing tags are written in (`git show v0.8.0`) and it is already reviewed prose. `--notes`
#    overrides it: the tag message is editorial, and this script does not pretend to write it.
# ---------------------------------------------------------------------------------------------
tag_body() {
  if [ -n "$NOTES_FILE" ]; then
    cat "$NOTES_FILE"
  else
    awk -v header="## [$NEW] — $DATE" '
      $0 == header { inside = 1; next }
      inside && /^## \[/ { exit }
      inside { print }
    ' WHATS-NEW.md
  # Drop leading blank lines, so the heredoc below owns the one blank separating the headline from
  # the body — git treats the first line as the subject and the rest as the message.
  fi | sed '/./,$!d'
}

git tag -a --cleanup=verbatim "v$NEW" -F - <<TAG_MSG
flux-connectors $NEW

$(tag_body)
TAG_MSG
echo "   tagged v$NEW (annotated)"

cat <<NEXT

== cut v$NEW. Nothing has been pushed and nothing has been published. ==

Review, and amend while it is still local:
   git show v$NEW               # the release commit  (git commit --amend to reword)
   git tag -l -n99 v$NEW        # the tag body        (git tag -a -f v$NEW to reword)

Then, and only then — pushing the tag IS the crates.io publication, and it is not reversible:
   git push origin main
   git push origin "v$NEW"
   gh run watch                 # the publish is idempotent; a rate-limited run resumes
   gh release create v$NEW      # only once the publish is green
NEXT

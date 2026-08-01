#!/usr/bin/env bash
#
# Publish the flux-connectors crates.io closure, in strict dependency order.
#
# Modelled on ../flux's scripts/publish-crates-io.sh, with one deliberate difference: the order is
# **derived**, not listed. flux hand-lists 29 crates because its graph has ordering constraints a
# manifest does not state (an optional feature dependency, a protocol line versioned independently).
# This workspace has four crates in the closure and one non-obvious edge, so a topological sort over
# the manifests is exact — and a sort cannot go stale the way a list does.
#
# The non-obvious edge is `connector-secrets -> connector-address`: the addressing is re-exported,
# never redefined, so the crate that owns it is published behind `connector-secrets` without anyone
# asking for it. That edge used to point at `connector-spec` — the compiler — which is what C-407
# extracted the vocabulary to end. `crates/connector-cli/tests/publish_closure.rs` asserts both the
# derivation and the property that no crate in it is machinery.
#
# What *is* listed is ROOTS: which crates a consumer is meant to add. That is a policy choice
# (C-190's), not a dependency fact. Everything below it is computed.
#
#   - Idempotent: a crate@version already on crates.io is treated as done and skipped, so this is
#     safe to re-run after a partial or failed publish — it resumes at the first unpublished crate.
#     Without that, a multi-crate publish that trips crates.io's new-crate rate limit halfway is
#     unrecoverable, because the crates already up cannot be withdrawn.
#   - Needs a crates.io token: either CARGO_REGISTRY_TOKEN in the environment (CI) or a prior
#     `cargo login` (local). Publishing is CI-only by contract — see AGENTS.md — so the local path
#     exists for `--dry-run` and `--print-order`, not for releasing.
#   - Modern cargo blocks until each crate is in the index before returning, so the next crate
#     resolves it; the short sleep is belt-and-suspenders for index propagation.
#
# Usage:
#   scripts/publish-crates-io.sh                # publish (needs a token)
#   scripts/publish-crates-io.sh --dry-run      # package + verify only, uploads nothing
#   scripts/publish-crates-io.sh --print-order  # print the derived order, one crate per line
#
set -uo pipefail

MODE="publish"
case "${1:-}" in
  --dry-run) MODE="dry-run" ;;
  --print-order) MODE="print-order" ;;
  "") ;;
  *) echo "usage: $0 [--dry-run|--print-order]" >&2; exit 2 ;;
esac

cd "$(dirname "$0")/.."

# The consumable crates — what C-190 says a host adds. `connector-cli` and `connector-flux` are
# deliberately absent: the CLI is this repository's own build tool (and the name `connector-cli` is
# already taken on crates.io by an unrelated crate), and `connector-flux` is reachable only from it.
#
# Keep in sync with ROOTS in crates/connector-cli/tests/publish_closure.rs, which asserts both this
# list and the order derived from it.
ROOTS=(
  codewandler-connector-catalog
  codewandler-connector-secrets
  codewandler-connector-pack
)

# The closure of ROOTS over this workspace's own crates, topologically sorted so a crate always
# follows everything it depends on. Ties broken by name, so the order is deterministic.
#
# `--no-deps` means cargo reads the manifests without resolving the registry graph, so this needs no
# index and works `--offline` on a fresh checkout.
publish_order() {
  cargo metadata --format-version 1 --no-deps --offline --manifest-path Cargo.toml \
    | ROOTS="${ROOTS[*]}" python3 -c '
import json, os, sys

meta = json.load(sys.stdin)
members = {p["name"]: p for p in meta["packages"]}

# Edges to workspace crates only: a registry dependency is already published by definition.
#
# Match on `name`, never on `rename`. `members` is keyed by *package* name, while `rename` is the
# local alias a dependent chose — `connector-spec = { package = "codewandler-connector-spec" }`.
# Preferring the alias silently drops every aliased edge, which is not a hypothetical: this repo
# aliases all four published crates (and every flux crate) exactly that way, and an earlier form of
# this line read `d.get("rename") or d["name"]`. It looked correct for as long as nothing was
# aliased, then dropped `codewandler-connector-spec` out of the closure entirely and emitted an
# order that published a crate before its own dependency.
edges = {
    name: sorted({d["name"] for d in pkg["dependencies"]} & members.keys())
    for name, pkg in members.items()
}

roots = os.environ["ROOTS"].split()
for r in roots:
    if r not in members:
        sys.exit(f"ROOTS names `{r}`, which is not a member of this workspace")

closure, pending = set(roots), list(roots)
while pending:
    for dep in edges[pending.pop()]:
        if dep not in closure:
            closure.add(dep)
            pending.append(dep)

placed = []
while len(placed) < len(closure):
    ready = [
        c for c in sorted(closure)
        if c not in placed and all(d in placed for d in edges[c] if d in closure)
    ]
    if not ready:
        sys.exit(f"cycle in the workspace dependency graph among {sorted(closure)}")
    placed.append(ready[0])

print("\n".join(placed))
'
}

ORDER="$(publish_order)" || { echo "!! could not derive the publish order" >&2; exit 1; }
mapfile -t CRATES <<<"$ORDER"

if [ "$MODE" = "print-order" ]; then
  printf '%s\n' "${CRATES[@]}"
  exit 0
fi

# Versions cargo would publish, resolved once from the workspace.
VERSIONS="$(
  cargo metadata --format-version 1 --no-deps --offline --manifest-path Cargo.toml 2>/dev/null \
    | python3 -c "import json,sys
for pkg in json.load(sys.stdin)['packages']:
    print(pkg['name'], pkg['version'])"
)"

crate_version() {
  echo "$VERSIONS" | awk -v n="$1" '$1 == n { print $2; exit }'
}

# A dry run is ONE cargo invocation over the whole closure, not a loop, and that is not a shortcut:
# `cargo publish --dry-run -p codewandler-connector-secrets` on its own fails, because verifying it means
# building it against a `connector-spec` that is not on crates.io yet. Given every package at once,
# cargo verifies each against the others' freshly packaged copies — which is exactly the situation
# the real publish creates one crate at a time. Nothing is uploaded.
if [ "$MODE" = "dry-run" ]; then
  args=()
  for c in "${CRATES[@]}"; do args+=(-p "$c"); done
  echo "==> cargo publish --dry-run ${args[*]}"
  cargo publish --dry-run "${args[@]}" || exit 1
  echo "== all ${#CRATES[@]} crates package and verify (nothing uploaded) =="
  exit 0
fi

# Is crate@version already on crates.io? Answering from the index costs one HTTP GET; letting
# `cargo publish` discover it costs a full package + verify build per crate. Any doubt (network
# error, unparseable answer) falls through to the publish path, which is idempotent anyway — this is
# an optimization, never the correctness boundary.
already_published() {
  local name="$1" version="$2"
  [ -n "$version" ] || return 1
  curl -sS --max-time 20 -H "User-Agent: flux-connectors-release (codewandler/flux-connectors)" \
    "https://crates.io/api/v1/crates/$name/$version" 2>/dev/null \
    | grep -q "\"num\":\"$version\"" || return 1
  return 0
}

failed=""
for c in "${CRATES[@]}"; do
  version="$(crate_version "$c")"
  if already_published "$c" "$version"; then
    echo "==> $c@$version already on crates.io — skipping (no package)"
    continue
  fi
  # Retry the SAME crate on a crates.io new-crate rate limit (429) — a first release publishes four
  # crates that do not exist yet, which is exactly what trips it (burst, then ~1/10min). Parse the
  # "try again after <GMT>" hint and wait it out, so one run grinds through unattended.
  while true; do
    echo "==> cargo publish -p $c"
    if out=$(cargo publish -p "$c" 2>&1); then
      echo "    ok: $c"
      sleep 15
      break
    fi
    # Already-published is success for our purposes — it is what makes a re-run resumable.
    if echo "$out" | grep -qiE "already (exists|uploaded)|already been (uploaded|published)|crate version .* is already"; then
      echo "    already on crates.io — skipping $c"
      break
    fi
    if echo "$out" | grep -qiE "429 Too Many Requests|too many (new )?crates"; then
      retry_at=$(echo "$out" | grep -oiE "try again after [^.]*GMT" | sed -E "s/try again after //I" | head -1)
      now=$(date -u +%s)
      target=$(date -u -d "$retry_at" +%s 2>/dev/null || echo $((now + 600)))
      wait=$(( target - now + 20 ))
      [ "$wait" -lt 20 ] && wait=20
      echo "    rate-limited (429); waiting ${wait}s (until ${retry_at:-~10m}) then retrying $c..."
      sleep "$wait"
      continue
    fi
    echo "$out" | tail -25
    failed="$c"
    break 2
  done
done

if [ -n "$failed" ]; then
  echo "!! publish stopped at: $failed  (fix, then re-run — already-published crates are skipped)" >&2
  exit 1
fi
echo "== all ${#CRATES[@]} crates published/confirmed on crates.io =="

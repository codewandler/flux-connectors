#!/usr/bin/env bash
#
# Re-vendor the babelforce OpenAPI documents into `specs/babelforce/`, scrubbed and provenanced.
#
# The upstream documents live in an internal repository (`manager-sdk/specs/`, already post-`pull.sh`:
# `servers:` normalised to the public production host, generator-compatibility fixes applied). This
# repository is public, so what lands here is a **declared scrub** of what was pulled, and the scrub
# is this script rather than a hand edit — a hand edit is not reproducible, cannot be re-run against a
# fresh pull, and cannot be reviewed as a diff against a stated rule.
#
# The rule, in one sentence: a **credential literal** is the inline scalar value of a key named
# `accessId`, `accessToken` or `token` that is at least sixteen characters of hex digits and dashes,
# and every such literal is replaced — wherever it occurs, under any key — by the same literal with
# every hex digit zeroed. Two consequences are the point of spelling it that way:
#
#   - **Only values are scrubbed.** A schema property declaration (`accessId:` with the value on the
#     following lines) carries no inline scalar and is left alone, so `components.securitySchemes` and
#     the `accessId`/`accessToken` property declarations survive intact. Ingest must keep *seeing*
#     them — `providers/babelforce.toml` excludes the deprecated `X-Auth-Access-*` pair as an overlay
#     decision, and drift-check can only keep reporting on a thing the document still declares.
#   - **The literal is scrubbed everywhere, not only under the key that identified it.** In these
#     documents the `accessId` value is reused as a plain `id:` three lines above itself, so a
#     key-scoped substitution would have left the credential in the file under a different name.
#
# Zeroing rather than deleting keeps the shape and the length, so the example still says "a 32-character
# token goes here" while carrying no entropy. The replacement is single-quoted, because a zeroed token
# with no dashes is all digits and would otherwise resolve as a YAML number rather than a string.
#
# The script is **fail-closed** at four points: it aborts if it finds no credential literals at all
# (the discovery rule has gone stale and a silent no-op would vendor the secrets), if any literal
# survives in the output, if an internal marker appears in the output, or if a document loses its
# `securitySchemes` block.
#
# What is deliberately **not** copied: `sources.json` and `scripts/pull.sh`. They hold the GitLab host
# and the project ids, which is exactly the material that stays internal. See AGENTS.md, "Vendored
# specs: the pulled bytes, never the pull configuration".
#
# Usage:
#
#     scripts/vendor-babelforce-specs.sh <path-to-manager-sdk/specs>
#
# The source path is a required argument and has no default: the source is another repository that is
# not present on most machines, and a default would turn "you do not have it" into a confusing failure
# deep inside the script.

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
usage: scripts/vendor-babelforce-specs.sh <path-to-manager-sdk/specs>

Re-vendors the five babelforce OpenAPI documents into specs/babelforce/, scrubs the
credential-shaped example values, and rewrites specs/babelforce.provenance.toml.
USAGE
    exit 2
}

[ $# -eq 1 ] || usage
source_dir=$1
[ -d "$source_dir" ] || {
    echo "not a directory: $source_dir" >&2
    exit 1
}

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out_dir="$root/specs/babelforce"
provenance="$root/specs/babelforce.provenance.toml"

# The five documents, in the order the provenance file records them: the manager document first
# because it is the one a connector is built from, then the rest by descending operation count.
documents=(manager task-automation task-schedule user auth)

# A key whose inline scalar value is treated as a credential when it is hex-and-dash shaped.
credential_keys='accessId|accessToken|token'
# Sixteen characters is above every legitimate short identifier in these documents and below the
# shortest credential in them (a 32-character token).
credential_shape='[0-9a-fA-F][0-9a-fA-F-]{15,}'

# Internal markers, mirroring `manager-sdk/scripts/leak-markers.regex` — which is the authority and
# stays internal, so this is a copy rather than a reference.
#
# Two deliberate differences. The upstream regex names two AWS account ids; publishing them in a
# public repository to prove they are absent would be self-defeating, and they only ever occur inside
# an ECR image URI, which `\.dkr\.ecr` and `amazonaws\.com` already refuse. And the babelforce-specific
# host fragments are covered a second time, structurally, by the host allowlist in
# `crates/connector-spec/tests/vendored_specs.rs`: every URL in a vendored document must point at a
# host on a short allowlist, which refuses an internal host this list never anticipated.
markers='gitlab|nexus|\.dkr\.ecr|amazonaws\.com|latest\.dev|rc\.dev|preproduction|npm-internal|kubectl|sbf/'

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# ---------------------------------------------------------------------------------------------
# 1. Discover the credential literals, across all five documents at once.
# ---------------------------------------------------------------------------------------------
sources=()
for name in "${documents[@]}"; do
    file="$source_dir/$name.openapi.yaml"
    [ -f "$file" ] || {
        echo "missing upstream document: $file" >&2
        exit 1
    }
    sources+=("$file")
done

grep -hE "^[[:space:]]*(${credential_keys}):[[:space:]]*${credential_shape}[[:space:]]*$" "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e 's/[[:space:]]+$//' |
    sort -u >"$work/literals"

if [ ! -s "$work/literals" ]; then
    echo "no credential literals found under keys (${credential_keys}) — the discovery rule has" >&2
    echo "gone stale against the upstream documents. Refusing to vendor unscrubbed bytes." >&2
    exit 1
fi

# ---------------------------------------------------------------------------------------------
# 2. Build the substitution: every literal, under any key, zeroed and quoted.
# ---------------------------------------------------------------------------------------------
: >"$work/scrub.sed"
: >"$work/redactions"
while read -r literal; do
    zeroed=$(printf '%s' "$literal" | sed 's/[0-9a-fA-F]/0/g')
    # The format string is single-quoted deliberately: in double quotes bash collapses `\\1` to `\1`
    # before printf sees it, and printf then reads `\1` as an *octal escape* — which silently emits a
    # 0x01 byte in place of the backreference and rewrites `accessId: <literal>` to a line with no key
    # at all. The `no_control_character` guard below is what caught it.
    printf 's|^([[:space:]]*[A-Za-z_][A-Za-z0-9_]*):[[:space:]]*%s[[:space:]]*$|\\1: '\''%s'\''|\n' \
        "$literal" "$zeroed" >>"$work/scrub.sed"
    digest=$(printf '%s' "$literal" | sha256sum | cut -d' ' -f1)
    occurrences=$(grep -cF "$literal" "${sources[@]}" | awk -F: '{ total += $2 } END { print total }')
    printf '%s %s %s\n' "$digest" "$zeroed" "$occurrences" >>"$work/redactions"
done <"$work/literals"

# ---------------------------------------------------------------------------------------------
# 3. Scrub, verify, and write.
# ---------------------------------------------------------------------------------------------
mkdir -p "$out_dir"
: >"$work/provenance-entries"

for name in "${documents[@]}"; do
    upstream="$source_dir/$name.openapi.yaml"
    fetched_date=$(date -u -r "$upstream" +%F)
    fetched_at=$(date -u -r "$upstream" +%Y-%m-%dT%H:%M:%SZ)
    vendored="$out_dir/$name-$fetched_date.openapi.yaml"

    sed -E -f "$work/scrub.sed" "$upstream" >"$work/$name.yaml"

    # Fail-closed: no literal may survive, anywhere, under any key.
    while read -r literal; do
        if grep -qF "$literal" "$work/$name.yaml"; then
            echo "a credential literal survived the scrub in $name.openapi.yaml — refusing to write" >&2
            exit 1
        fi
    done <"$work/literals"

    if grep -qE "$markers" "$work/$name.yaml"; then
        echo "an internal marker appears in $name.openapi.yaml — refusing to write:" >&2
        grep -nE "$markers" "$work/$name.yaml" | head -5 >&2
        exit 1
    fi

    # The upstream documents carry no control byte — no tabs, nothing below 0x20 but the newlines. A
    # control character in the output therefore means the substitution itself is malformed rather than
    # the source, which is exactly how the `\1`-as-octal-escape defect above presented: valid-looking
    # YAML whose keys had been replaced by an invisible 0x01 byte. Byte-scoped rather than
    # `[[:print:]]`-scoped, because these documents are legitimately UTF-8 — the descriptions are full
    # of em-dashes, and a printability test flags every one of them.
    if LC_ALL=C grep -qP '[\x00-\x08\x0b-\x1f\x7f]' "$work/$name.yaml"; then
        echo "the scrub produced a control character in $name.openapi.yaml — the substitution is" >&2
        echo "malformed. Refusing to write:" >&2
        LC_ALL=C grep -nP '[\x00-\x08\x0b-\x1f\x7f]' "$work/$name.yaml" | head -3 | cat -v >&2
        exit 1
    fi

    if ! grep -q '^  securitySchemes:' "$work/$name.yaml"; then
        echo "$name.openapi.yaml lost its securitySchemes block — the scrub is removing" >&2
        echo "declarations, not just values. Refusing to write." >&2
        exit 1
    fi

    # A dated name means a fresh pull writes a *new* file; drop the previous date so re-vendoring
    # replaces rather than accumulates, and reads as one delete plus one add in review.
    rm -f "$out_dir/$name-"*.openapi.yaml
    cp "$work/$name.yaml" "$vendored"

    upstream_version=$(sed -nE 's/^[[:space:]]{2}version:[[:space:]]*(.+)$/\1/p' "$vendored" | head -1)
    sha=$(sha256sum "$vendored" | cut -d' ' -f1)
    upstream_sha=$(sha256sum "$upstream" | cut -d' ' -f1)

    cat >>"$work/provenance-entries" <<ENTRY

[[spec]]
name = "$name"
path = "specs/babelforce/$name-$fetched_date.openapi.yaml"
upstream_version = "$upstream_version"
fetched_at = "$fetched_at"
sha256 = "$sha"
upstream_sha256 = "$upstream_sha"
ENTRY
done

# ---------------------------------------------------------------------------------------------
# 4. Provenance.
# ---------------------------------------------------------------------------------------------
{
    cat <<'HEADER'
# Provenance for the vendored babelforce documents.
#
# Generated by `scripts/vendor-babelforce-specs.sh`. Do not edit by hand — re-run the script.
#
# `info.version` is the useless string `0.0.0-dev` on three of the five documents, so the identity of
# a vendored document is the pull date in its file name plus `sha256` of its bytes, not its declared
# version. Each `[[spec]]` entry carries exactly the fields of `connector_spec::SpecSource` — `path`,
# `upstream_version`, `fetched_at`, `sha256` — plus two of its own.
#
# **`source_url` is deliberately absent, not forgotten.** The only URL there is to record points at an
# internal GitLab host, and `SpecSource::source_url` is already `Option` precisely so a document whose
# origin cannot be published can still be provenanced. What it costs is that C-14 cannot re-fetch this
# document unattended; what `upstream_sha256` buys back is that it can still *detect* the drift, since
# the hash of the unscrubbed bytes is what upstream served (`LockEntry::upstream_spec_sha256`, C-25).
#
# `[[redaction]]` records the scrub itself, by digest. A credential literal cannot be written down
# here — that is the entire point — so what is recorded is its SHA-256, which is enough for
# `crates/connector-spec/tests/vendored_specs.rs` to refuse the literal's return under any key, in any
# document, forever, while publishing nothing.

version = 1
HEADER
    cat "$work/provenance-entries"
    while read -r digest zeroed occurrences; do
        cat <<ENTRY

[[redaction]]
sha256 = "$digest"
replaced_with = "$zeroed"
occurrences = $occurrences
ENTRY
    done <"$work/redactions"
} >"$provenance"

echo "vendored ${#documents[@]} documents into $out_dir"
echo "redacted $(wc -l <"$work/literals") credential literals; provenance in $provenance"

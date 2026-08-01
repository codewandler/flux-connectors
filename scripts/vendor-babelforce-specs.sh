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
# Three classes of thing come out, discovered from the source rather than hardcoded here — no secret,
# address or number is written into this repository, not even into the thing that removes them:
#
#   1. **Credentials.** The inline scalar value of a key named `accessId`, `accessToken` or `token`
#      that is at least sixteen characters of hex digits and dashes. Replaced by the same literal with
#      every hex digit zeroed.
#   2. **Email addresses.** Any address that is not on the short `publishable_addresses` allowlist —
#      so a new one in a future pull comes out by default. This class is **not** credentials, and that
#      is exactly why it needs stating: `will+test@babelforce.com` is a named individual's work
#      address and `trautomations@…​.iam.gserviceaccount.com` is an internal GCP service-account
#      identity. Neither is a secret; both are things a public repository must not carry, and
#      repository history makes either expensive to undo once pushed.
#   3. **Telephone numbers.** Any phone-keyed value that is not one of the constructed
#      `+49 30 0000 00xx` numbers the call examples are written against.
#
# What is deliberately **kept**: `Testers Inc.`, `Will Tester`, `firstName: Will`. Once the address
# and the number are gone these are fixture labels with nothing contactable behind them — `Tester`
# and `Testers Inc.` are self-evidently constructed, and a bare first name identifies nobody. Removing
# them would cost the examples their readability and buy no privacy. Recorded here so the decision is
# reviewable rather than merely observable.
#
# The credential rule, in one sentence, because two consequences are the point of spelling it that
# way: every such literal is replaced *wherever it occurs, under any key*.
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

# A key whose inline scalar value is treated as a telephone number.
phone_keys='phone|phoneNumber|msisdn|number|from|to'

# Addresses that may be published as they stand. Everything else that looks like an email address is
# scrubbed, so a new address in a future pull comes out by default rather than travelling on the
# strength of nobody having noticed it.
#
#   - `support@babelforce.com` is `info.contact.email`: the vendor's own published support contact,
#     a role address rather than a person, and real API metadata a caller wants.
#   - Anything at a domain RFC 2606 reserves for documentation is fictional by construction, which is
#     what an example address ought to be. Matched structurally rather than enumerated, so the
#     replacement below needs no entry of its own and neither does the next example address upstream
#     writes.
publishable_addresses='support@babelforce.com'
reserved_domains='example.com|example.net|example.org'
redacted_address='redacted@example.com'

# Numbers that may be published as they stand: the constructed `+49 30 0000 00xx` family the call
# examples are written against. They carry no subscriber and are what makes those examples readable.
synthetic_numbers='+493000000000 +493000000001 +493000000099'

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

# Escape a literal for use inside an ERE pattern. Not optional: `will+test@babelforce.com` contains a
# `+`, which is a repetition operator, so an unescaped literal would match `willtest@…`, `willtttest@…`
# and never the address itself — a substitution that silently does nothing.
ere_escape() {
    printf '%s' "$1" | sed -E 's/[][()|.*+?^$\\{}]/\\&/g'
}

# ---------------------------------------------------------------------------------------------
# 1. Discover what has to come out, across all five documents at once.
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

# `kind<TAB>literal<TAB>replacement`, one per line. Three discovery passes write into it and one
# substitution pass reads it, so adding a fourth class of thing-that-must-not-ship is a new pass here
# and nothing else.
: >"$work/substitutions"

# (a) Credentials. Zeroed rather than deleted: the shape and the length still say "a 32-character
#     token belongs here" while the entropy is gone.
grep -hE "^[[:space:]]*(${credential_keys}):[[:space:]]*${credential_shape}[[:space:]]*$" "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e 's/[[:space:]]+$//' |
    sort -u >"$work/credentials"

if [ ! -s "$work/credentials" ]; then
    echo "no credential literals found under keys (${credential_keys}) — the discovery rule has" >&2
    echo "gone stale against the upstream documents. Refusing to vendor unscrubbed bytes." >&2
    exit 1
fi

while read -r literal; do
    printf 'credential\t%s\t%s\n' "$literal" "$(printf '%s' "$literal" | sed 's/[0-9a-fA-F]/0/g')" \
        >>"$work/substitutions"
done <"$work/credentials"

# (b) Email addresses. Not credentials — which is exactly why they need saying out loud: a named
#     individual's work address and an internal GCP service-account identity are both things a public
#     repository must not carry, and neither is a secret. The allowlist is what makes this fail closed:
#     an address is scrubbed unless somebody has deliberately declared it publishable.
grep -hoE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' "${sources[@]}" |
    sort -u >"$work/addresses"

while read -r address; do
    case " $publishable_addresses " in
    *" $address "*) continue ;;
    esac
    if printf '%s' "$address" | grep -qE "@(${reserved_domains})\$"; then
        continue
    fi
    printf 'address\t%s\t%s\n' "$address" "$redacted_address" >>"$work/substitutions"
done <"$work/addresses"

# (c) Telephone numbers. The documents use one constructed family (`+49 30 0000 00xx`) for every call
#     example; those are worth keeping, because they are what makes the examples readable. Anything
#     else under a phone key is treated as a real subscriber number and zeroed like a credential.
grep -hE "^[[:space:]]*(${phone_keys}):[[:space:]]*'?\+?[0-9]{8,}'?[[:space:]]*$" "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e "s/'//g" -e 's/[[:space:]]+$//' |
    sort -u >"$work/numbers"

while read -r number; do
    case " $synthetic_numbers " in
    *" $number "*) continue ;;
    esac
    printf 'number\t%s\t%s\n' "$number" "$(printf '%s' "$number" | sed 's/[0-9]/0/g')" \
        >>"$work/substitutions"
done <"$work/numbers"

# ---------------------------------------------------------------------------------------------
# 2. Build the substitution: every literal, wherever it occurs, replaced in kind.
# ---------------------------------------------------------------------------------------------
: >"$work/scrub.sed"
: >"$work/redactions"
while IFS=$'\t' read -r kind literal replacement; do
    pattern=$(ere_escape "$literal")

    # The format strings are single-quoted deliberately: in double quotes bash collapses `\\1` to
    # `\1` before printf sees it, and printf then reads `\1` as an *octal escape* — which silently
    # emits a 0x01 byte in place of the backreference and rewrites `accessId: <literal>` to a line
    # with no key at all. The control-character guard below is what caught it.
    case "$kind" in
    address)
        # Global and unanchored: an address in a description is as public as one in an example, and
        # the literal is distinctive enough that a substring match cannot catch anything else.
        printf 's|%s|%s|g\n' "$pattern" "$replacement" >>"$work/scrub.sed"
        ;;
    *)
        # Anchored to a whole `key: value` line, so a substitution can never rewrite part of a longer
        # scalar. The optional quotes are for the phone numbers, which upstream already quotes.
        printf 's|^([[:space:]]*[A-Za-z_][A-Za-z0-9_]*):[[:space:]]*'\''?%s'\''?[[:space:]]*$|\\1: '\''%s'\''|\n' \
            "$pattern" "$replacement" >>"$work/scrub.sed"
        ;;
    esac

    digest=$(printf '%s' "$literal" | sha256sum | cut -d' ' -f1)
    occurrences=$(grep -cF "$literal" "${sources[@]}" | awk -F: '{ total += $2 } END { print total }')
    printf '%s\t%s\t%s\t%s\n' "$kind" "$digest" "$replacement" "$occurrences" >>"$work/redactions"
done <"$work/substitutions"

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

    # Fail-closed: no literal of any kind may survive, anywhere, under any key.
    while IFS=$'\t' read -r kind literal _; do
        if grep -qF "$literal" "$work/$name.yaml"; then
            echo "a scrubbed $kind literal survived in $name.openapi.yaml — refusing to write" >&2
            exit 1
        fi
    done <"$work/substitutions"

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
# `[[redaction]]` records the scrub itself, by digest, in three kinds — `credential`, `address` and
# `number`. A scrubbed literal cannot be written down here, which is the entire point, so what is
# recorded is its SHA-256. That is enough for `crates/connector-spec/tests/vendored_specs.rs` to refuse
# the literal's return under any key, in any document, forever, while publishing nothing.
#
# The `address` and `number` kinds are not credentials, and are removed anyway: a named individual's
# work address and an internal service-account identity are both things a public repository must not
# carry, and repository history makes either expensive to undo once pushed.

version = 1
HEADER
    cat "$work/provenance-entries"
    while IFS=$'\t' read -r kind digest replacement occurrences; do
        cat <<ENTRY

[[redaction]]
kind = "$kind"
sha256 = "$digest"
replaced_with = "$replacement"
occurrences = $occurrences
ENTRY
    done <"$work/redactions"
} >"$provenance"

echo "vendored ${#documents[@]} documents into $out_dir"
echo "redacted $(wc -l <"$work/substitutions") literals" \
    "($(grep -c '^credential' "$work/substitutions") credential," \
    "$(grep -c '^address' "$work/substitutions") address," \
    "$(grep -c '^number' "$work/substitutions") number); provenance in $provenance"

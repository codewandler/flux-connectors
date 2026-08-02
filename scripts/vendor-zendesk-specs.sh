#!/usr/bin/env bash
#
# Re-vendor Zendesk's three public OpenAPI documents, scrubbed and provenanced (C-459).
#
# With no source directory this script fetches the public sources. `--source-dir` accepts the exact
# upstream bytes instead, which makes a historical pull reproducible without trusting a moving URL;
# pair it with the original `--fetched-at` to reproduce the provenance file byte-for-byte.
#
# Example values are scrubbed and trailing horizontal whitespace is canonicalized. Security
# declarations, operation ids, paths, parameters, request and response schemas remain vendor data.
# The scrub is fail-closed: a newly shaped credential, contact address, or telephone number is
# discovered from the input and removed by default, and the output is refused if a discovered
# literal survives or a security scheme disappears.

set -euo pipefail

usage() {
    printf '%s\n' \
        'usage: scripts/vendor-zendesk-specs.sh [--source-dir DIR] [--fetched-at YYYY-MM-DDTHH:MM:SSZ]' \
        '' \
        'Without --source-dir, fetches the three public Zendesk sources. A source directory must' \
        'contain ticketing.openapi.yaml, help-center.openapi.yaml, and messaging.openapi.yaml.' >&2
    exit 2
}

source_dir=
fetched_at=
while [ $# -gt 0 ]; do
    case "$1" in
    --source-dir)
        [ $# -ge 2 ] || usage
        source_dir=$2
        shift 2
        ;;
    --fetched-at)
        [ $# -ge 2 ] || usage
        fetched_at=$2
        shift 2
        ;;
    *) usage ;;
    esac
done

if [ -n "$fetched_at" ] && ! printf '%s' "$fetched_at" |
    grep -qE '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$'; then
    printf 'invalid --fetched-at value: %s\n' "$fetched_at" >&2
    exit 2
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out_dir="$root/specs/zendesk"
provenance="$root/specs/zendesk.provenance.toml"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

names=(ticketing help-center messaging)
ticketing_url='https://developer.zendesk.com/zendesk/oas.yaml'
help_center_url='https://developer.zendesk.com/help_center/oas.yaml'
messaging_commit='a42f7055d829b67ef5c1d7c0f3e2c48cdddd026d'
messaging_url="https://raw.githubusercontent.com/zendesk/sunshine-conversations-api-spec/$messaging_commit/openapi.yaml"

source_url() {
    case "$1" in
    ticketing) printf '%s' "$ticketing_url" ;;
    help-center) printf '%s' "$help_center_url" ;;
    messaging) printf '%s' "$messaging_url" ;;
    *) return 1 ;;
    esac
}

if [ -z "$source_dir" ]; then
    source_dir="$work/source"
    mkdir -p "$source_dir"
    for name in "${names[@]}"; do
        curl -fsSL "$(source_url "$name")" -o "$source_dir/$name.openapi.yaml"
    done
    fetched_at=${fetched_at:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}
else
    [ -d "$source_dir" ] || {
        printf 'not a directory: %s\n' "$source_dir" >&2
        exit 1
    }
    [ -n "$fetched_at" ] || {
        printf '%s\n' '--source-dir requires --fetched-at so provenance is reproducible' >&2
        exit 2
    }
fi

sources=()
for name in "${names[@]}"; do
    source="$source_dir/$name.openapi.yaml"
    [ -f "$source" ] || {
        printf 'missing upstream document: %s\n' "$source" >&2
        exit 1
    }
    sources+=("$source")
done

fetched_date=${fetched_at%%T*}
reserved_domains='example.com|example.net|example.org'
redacted_address='redacted@example.com'
credential_keys='api_?key|token|access_?token|refresh_?token|secret|key_?secret|password|authorization'
credential_shape='[A-Za-z0-9._~+/=-]{12,}'
phone_keys='phone|phone_?number|phoneNumber|msisdn|from|to'
identifier_keys='key_?id|keyId|app_?id|appId|client_?id|clientId|integration_?id|integrationId'

ere_escape() {
    printf '%s' "$1" | sed -E 's/[][()|.*+?^$\\{}]/\\&/g'
}

: >"$work/substitutions"

# Addresses are global: an identity in prose is no less public than one below an `example` key.
grep -hoE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' "${sources[@]}" |
    sort -u >"$work/addresses"
while read -r address; do
    [ -n "$address" ] || continue
    domain=${address##*@}
    if printf '%s' "$domain" | grep -qiE "(^|\.)(${reserved_domains})$"; then
        continue
    fi
    printf 'address\t%s\t%s\n' "$address" "$redacted_address" >>"$work/substitutions"
done <"$work/addresses"

# Credential-shaped inline values under credential-named keys. A declaration has no inline scalar
# and is never selected, which is the declaration/value boundary this scrub must preserve.
grep -hiE "^[[:space:]]*(${credential_keys}):[[:space:]]*['\"]?${credential_shape}['\"]?[[:space:]]*$" \
    "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e "s/^['\"]//" -e "s/['\"]$//" |
    sort -u >"$work/credentials"
[ -s "$work/credentials" ] || {
    printf '%s\n' 'no credential-shaped examples found; refusing a scrub whose discovery rule went stale' >&2
    exit 1
}
while read -r credential; do
    [ -n "$credential" ] || continue
    printf 'credential\t%s\tredacted-credential\n' "$credential" >>"$work/substitutions"
done <"$work/credentials"

# Public examples also contain opaque client/application/integration ids. They are not secrets, but
# they identify systems in exactly the way a service-account address does, so they are a separate
# redaction kind rather than being mislabeled as credentials.
grep -hiE "^[[:space:]]*(${identifier_keys}):[[:space:]]*['\"]?[0-9a-f]{12,}['\"]?[[:space:]]*$" \
    "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e "s/^['\"]//" -e "s/['\"]$//" |
    sort -u >"$work/identifiers"
[ -s "$work/identifiers" ] || {
    printf '%s\n' 'no system identifiers found; refusing a scrub whose discovery rule went stale' >&2
    exit 1
}
while read -r identifier; do
    [ -n "$identifier" ] || continue
    printf 'identifier\t%s\tredacted-identifier\n' "$identifier" >>"$work/substitutions"
done <"$work/identifiers"

# Opaque hexadecimal examples are system or credential identifiers even when the surrounding
# schema names them only through a generic `example` key. This includes vendor-prefixed identifiers
# such as a Twilio Account SID because its `AC` prefix is itself hexadecimal. Treat the shape as
# private by default instead of maintaining an allowlist of vendor prefixes that secret scanning
# will inevitably outgrow.
grep -hiE "^[[:space:]]*example:[[:space:]]*['\"]?[0-9a-f]{24,}['\"]?[[:space:]]*$" \
    "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e "s/^['\"]//" -e "s/['\"]$//" |
    sort -u >"$work/opaque-examples"
[ -s "$work/opaque-examples" ] || {
    printf '%s\n' 'no opaque identifier examples found; refusing a scrub whose discovery rule went stale' >&2
    exit 1
}
while read -r opaque; do
    [ -n "$opaque" ] || continue
    printf 'opaque\t%s\tredacted-opaque\n' "$opaque" >>"$work/substitutions"
done <"$work/opaque-examples"

# Telephone values are selected by a phone-shaped key and a leading `+`; declarations and object
# fields named `from`/`to` do not match. The replacement stays visibly a telephone placeholder but
# carries too few digits to identify a subscriber.
grep -hiE "^[[:space:]]*(${phone_keys}):[[:space:]]*['\"]?\+[0-9 ()-]{7,}['\"]?[[:space:]]*$" \
    "${sources[@]}" |
    sed -E -e 's/^[^:]*:[[:space:]]*//' -e "s/^['\"]//" -e "s/['\"]$//" |
    sort -u >"$work/numbers"
[ -s "$work/numbers" ] || {
    printf '%s\n' 'no telephone examples found; refusing a scrub whose discovery rule went stale' >&2
    exit 1
}
while read -r number; do
    [ -n "$number" ] || continue
    printf 'number\t%s\t+000\n' "$number" >>"$work/substitutions"
done <"$work/numbers"

# IP addresses identify systems too. Replace every four-octet example (valid or not — one upstream
# example contains an out-of-range octet) with RFC 5737's documentation-only address.
grep -hoE '([0-9]{1,3}\.){3}[0-9]{1,3}' "${sources[@]}" |
    sort -u >"$work/ip-addresses"
[ -s "$work/ip-addresses" ] || {
    printf '%s\n' 'no IP address examples found; refusing a scrub whose discovery rule went stale' >&2
    exit 1
}
while read -r ip_address; do
    [ -n "$ip_address" ] || continue
    printf 'ip\t%s\t192.0.2.1\n' "$ip_address" >>"$work/substitutions"
done <"$work/ip-addresses"

: >"$work/scrub.sed"
: >"$work/redactions"
while IFS=$'\t' read -r kind literal replacement; do
    pattern=$(ere_escape "$literal")
    replacement_pattern=$(printf '%s' "$replacement" | sed 's/[&|\\]/\\&/g')
    printf 's|%s|%s|g\n' "$pattern" "$replacement_pattern" >>"$work/scrub.sed"
    digest=$(printf '%s' "$literal" | sha256sum | cut -d' ' -f1)
    occurrences=$(grep -hFo "$literal" "${sources[@]}" | wc -l)
    printf '%s\t%s\t%s\t%s\n' "$kind" "$digest" "$replacement" "$occurrences" \
        >>"$work/redactions"
done <"$work/substitutions"

mkdir -p "$out_dir"
: >"$work/provenance-entries"
for name in "${names[@]}"; do
    upstream="$source_dir/$name.openapi.yaml"
    output="$work/$name.openapi.yaml"
    sed -E -f "$work/scrub.sed" "$upstream" | sed -E 's/[[:blank:]]+$//' >"$output"

    while IFS=$'\t' read -r kind literal _; do
        if grep -qF "$literal" "$output"; then
            printf 'a scrubbed %s literal survived in %s\n' "$kind" "$name" >&2
            exit 1
        fi
    done <"$work/substitutions"

    if LC_ALL=C grep -qP '[\x00-\x08\x0b-\x1f\x7f]' "$output"; then
        printf 'the scrub produced a control character in %s\n' "$name" >&2
        exit 1
    fi
    grep -q '^[[:space:]]*securitySchemes:' "$output" || {
        printf '%s lost components.securitySchemes\n' "$name" >&2
        exit 1
    }
    case "$name" in
    ticketing | help-center)
        grep -q '^[[:space:]]*basicAuth:' "$output" || {
            printf '%s lost basicAuth\n' "$name" >&2
            exit 1
        }
        ;;
    messaging)
        for scheme in basicAuth bearerAuth; do
            grep -q "^[[:space:]]*$scheme:" "$output" || {
                printf 'messaging lost %s\n' "$scheme" >&2
                exit 1
            }
        done
        ;;
    esac

    rm -f "$out_dir/$name-"*.openapi.yaml
    vendored="$out_dir/$name-$fetched_date.openapi.yaml"
    cp "$output" "$vendored"

    upstream_version=$(sed -nE 's/^[[:space:]]{2}version:[[:space:]]*(.+)$/\1/p' "$upstream" |
        head -1 | tr -d '"' | tr -d "'")
    [ -n "$upstream_version" ] || {
        printf '%s has no info.version\n' "$name" >&2
        exit 1
    }
    sha=$(sha256sum "$vendored" | cut -d' ' -f1)
    upstream_sha=$(sha256sum "$upstream" | cut -d' ' -f1)
    {
        printf '\n[[spec]]\n'
        printf 'name = "%s"\n' "$name"
        printf 'path = "specs/zendesk/%s-%s.openapi.yaml"\n' "$name" "$fetched_date"
        printf 'source_url = "%s"\n' "$(source_url "$name")"
        printf 'upstream_version = "%s"\n' "$upstream_version"
        printf 'fetched_at = "%s"\n' "$fetched_at"
        printf 'sha256 = "%s"\n' "$sha"
        printf 'upstream_sha256 = "%s"\n' "$upstream_sha"
    } >>"$work/provenance-entries"
done

{
    printf '%s\n' \
        '# Provenance for Zendesk Ticketing, Help Center, and Messaging OpenAPI documents.' \
        '#' \
        '# Generated by `scripts/vendor-zendesk-specs.sh`; re-run the script instead of editing.' \
        '# `upstream_sha256` identifies the exact public bytes before the declared scrub, while' \
        '# `sha256` identifies the committed bytes. Redacted literals are represented only by digest.' \
        '' \
        'version = 1'
    sed -n '1,$p' "$work/provenance-entries"
    while IFS=$'\t' read -r kind digest replacement occurrences; do
        printf '\n[[redaction]]\n'
        printf 'kind = "%s"\n' "$kind"
        printf 'sha256 = "%s"\n' "$digest"
        printf 'replaced_with = "%s"\n' "$replacement"
        printf 'occurrences = %s\n' "$occurrences"
    done <"$work/redactions"
} >"$provenance"

printf 'vendored %s documents into %s\n' "${#names[@]}" "$out_dir"
printf 'redacted %s literals; provenance in %s\n' "$(wc -l <"$work/substitutions")" "$provenance"

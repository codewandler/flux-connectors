#!/usr/bin/env bash
#
# Re-vendor Stripe's pinned GA OpenAPI document for C-470.
#
# Stripe generates a semantically empty, optional form request body on GET collection operations.
# The current emitter correctly refuses form bodies, so this script removes that exact shape from
# the four frozen reads and nothing else. The full upstream hash remains in provenance, making the
# normalization reviewable without pretending the transformed bytes are the upstream document.

set -euo pipefail

usage() {
    printf '%s\n' \
        'usage: scripts/vendor-stripe-spec.sh [--source-file FILE] [--fetched-at YYYY-MM-DDTHH:MM:SSZ]' \
        '' \
        'Without --source-file, fetches the immutable first-party Stripe source. Supplying a file' \
        'requires --fetched-at so the output and provenance remain byte-reproducible.' >&2
    exit 2
}

source_file=
fetched_at=
while [ $# -gt 0 ]; do
    case "$1" in
    --source-file)
        [ $# -ge 2 ] || usage
        source_file=$2
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
commit='8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb'
source_url="https://raw.githubusercontent.com/stripe/openapi/$commit/latest/openapi.spec3.json"
expected_upstream_sha='6f3623aece40493eec2f5e3e631219f8c6bffa4f477e3807a4bf785ad377f237'
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

if [ -z "$source_file" ]; then
    source_file="$work/openapi.spec3.json"
    curl -fsSL "$source_url" -o "$source_file"
    fetched_at=${fetched_at:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}
else
    [ -f "$source_file" ] || {
        printf 'not a file: %s\n' "$source_file" >&2
        exit 1
    }
    [ -n "$fetched_at" ] || {
        printf '%s\n' '--source-file requires --fetched-at so provenance is reproducible' >&2
        exit 2
    }
fi

upstream_sha=$(sha256sum "$source_file" | cut -d' ' -f1)
[ "$upstream_sha" = "$expected_upstream_sha" ] || {
    printf 'Stripe source hash mismatch: expected %s, got %s\n' \
        "$expected_upstream_sha" "$upstream_sha" >&2
    exit 1
}

jq -e '
    .openapi == "3.0.0" and .info.version == "2026-07-29.dahlia" and
    (.paths | length) == 439 and
    ([.paths[] | to_entries[] |
      select(.key | IN("get", "put", "post", "delete", "options", "head", "patch", "trace"))]
      | length) == 621
' "$source_file" >/dev/null || {
    printf '%s\n' 'Stripe OpenAPI version or path/operation inventory moved' >&2
    exit 1
}

operation_inventory=$(python3 "$root/scripts/openapi_example_scrub.py" \
    --operation-inventory "$source_file")
[ "$operation_inventory" = '{"operations":621,"present":621,"unique":621}' ] || {
    printf 'Stripe operationId inventory contains a missing or duplicate id: %s\n' \
        "$operation_inventory" >&2
    exit 1
}

expected_body='{"content":{"application/x-www-form-urlencoded":{"encoding":{},"schema":{"additionalProperties":false,"properties":{},"type":"object"}}},"required":false}'
paths=(/v1/country_specs /v1/events /v1/exchange_rates /v1/billing/meters)
operation_ids=(GetCountrySpecs GetEvents GetExchangeRates GetBillingMeters)
for index in "${!paths[@]}"; do
    path=${paths[$index]}
    operation_id=${operation_ids[$index]}
    actual_id=$(jq -r --arg path "$path" '.paths[$path].get.operationId' "$source_file")
    [ "$actual_id" = "$operation_id" ] || {
        printf '%s GET changed operationId: expected %s, got %s\n' \
            "$path" "$operation_id" "$actual_id" >&2
        exit 1
    }
    body=$(jq -cS --arg path "$path" '.paths[$path].get.requestBody' "$source_file")
    [ "$body" = "$expected_body" ] || {
        printf '%s GET requestBody is no longer the exact empty optional form shape\n' "$path" >&2
        exit 1
    }
done

# Stripe currently publishes no `example` or `examples` keys. This is an explicit scrub boundary:
# if the pinned input is ever advanced and begins carrying values, refuse it until their public-data
# posture has been reviewed rather than copying them by default.
example_count=$(jq '[.. | objects | select(has("example") or has("examples"))] | length' "$source_file")
[ "$example_count" -eq 0 ] || {
    printf 'Stripe source now carries %s example-bearing objects; review before vendoring\n' \
        "$example_count" >&2
    exit 1
}

normalized="$work/openapi.json"
jq -c '
    del(
      .paths["/v1/country_specs"].get.requestBody,
      .paths["/v1/events"].get.requestBody,
      .paths["/v1/exchange_rates"].get.requestBody,
      .paths["/v1/billing/meters"].get.requestBody
    )
' "$source_file" >"$normalized"

for path in "${paths[@]}"; do
    jq -e --arg path "$path" '.paths[$path].get | has("requestBody") | not' "$normalized" \
        >/dev/null
done
jq -e '.paths["/v1/customers"].post | has("requestBody")' "$normalized" >/dev/null
jq -e '.components.securitySchemes.bearerAuth.type == "http"' "$normalized" >/dev/null

fetched_date=${fetched_at%%T*}
out_dir="$root/specs/stripe"
mkdir -p "$out_dir"
rm -f "$out_dir"/openapi-*.json
vendored="$out_dir/openapi-$fetched_date.json"
cp "$normalized" "$vendored"

vendored_sha=$(sha256sum "$vendored" | cut -d' ' -f1)
upstream_version=$(jq -r '.info.version' "$source_file")
provenance="$root/specs/stripe.provenance.toml"
{
    printf '%s\n' \
        '# Provenance for the pinned Stripe GA OpenAPI document.' \
        '#' \
        '# Generated by `scripts/vendor-stripe-spec.sh`; re-run the script instead of editing.' \
        '# The committed document differs only by removing the exact semantically empty, optional' \
        '# form requestBody on the four selected GETs. `upstream_sha256` identifies the complete' \
        '# first-party input; `sha256` identifies the normalized bytes builds ingest.' \
        '' \
        'version = 1' \
        '' \
        '[[spec]]' \
        'name = "ga"'
    printf 'path = "specs/stripe/%s"\n' "$(basename "$vendored")"
    printf 'source_url = "%s"\n' "$source_url"
    printf 'source_commit = "%s"\n' "$commit"
    printf 'upstream_version = "%s"\n' "$upstream_version"
    printf 'fetched_at = "%s"\n' "$fetched_at"
    printf 'sha256 = "%s"\n' "$vendored_sha"
    printf 'upstream_sha256 = "%s"\n' "$upstream_sha"
    printf '%s\n' \
        '' \
        '[[normalization]]' \
        'kind = "remove-empty-optional-get-form-body"' \
        'operations = ["GetCountrySpecs", "GetEvents", "GetExchangeRates", "GetBillingMeters"]' \
        'count = 4'
} >"$provenance"

printf 'vendored Stripe OpenAPI as %s\n' "$vendored"
printf 'normalized 4 empty optional GET form bodies; provenance in %s\n' "$provenance"

#!/usr/bin/env python3
"""Vendor Twilio's pinned API v2010 contract and exact C-473 extraction."""

from __future__ import annotations

import argparse
import copy
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import tempfile
import urllib.request


COMMIT = "97418cf0e4d6cf35b02333dd624090a8c62fa25d"
SOURCE_NAME = "twilio_api_v2010.json"
SOURCE_URL = (
    "https://raw.githubusercontent.com/twilio/twilio-oai/"
    f"{COMMIT}/spec/json/{SOURCE_NAME}"
)
LICENSE_URL = (
    "https://raw.githubusercontent.com/twilio/twilio-oai/"
    f"{COMMIT}/LICENSE"
)
UPSTREAM_SHA256 = "a6753266b8b05a201e8658734e332ee51d07a0913f2d419335d87bdb287643a2"
UPSTREAM_BYTES = 1_869_905
LICENSE_SHA256 = "282418d5a4ca0a6cd3476637c41bb229e592fc289156d22fbfa188cec1169ff3"
LICENSE_BYTES = 1_088
SOURCE_SERVER = "https://api.twilio.com"
VERSION_PREFIX = "/2010-04-01"
PUBLISHED_SERVER = f"{SOURCE_SERVER}{VERSION_PREFIX}"
HTTP_METHODS = {"get", "put", "post", "delete", "options", "head", "patch", "trace"}
SELECTIONS = (
    (
        "/2010-04-01/Accounts/{AccountSid}/Recordings.json",
        "get",
        "ListRecording",
    ),
    (
        "/2010-04-01/Accounts/{AccountSid}/Recordings/{Sid}.json",
        "get",
        "FetchRecording",
    ),
    (
        "/2010-04-01/Accounts/{AccountSid}/Usage/Records.json",
        "get",
        "ListUsageRecord",
    ),
    (
        "/2010-04-01/Accounts/{AccountSid}/Conferences.json",
        "get",
        "ListConference",
    ),
)


def fail(message: str) -> None:
    raise SystemExit(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def scrub_examples(value: object) -> int:
    """Remove every example value while preserving all declarations."""
    removed = 0
    if isinstance(value, dict):
        for key in ("example", "examples"):
            if key in value:
                del value[key]
                removed += 1
        for child in value.values():
            removed += scrub_examples(child)
    elif isinstance(value, list):
        for child in value:
            removed += scrub_examples(child)
    return removed


def local_refs(value: object) -> set[str]:
    found: set[str] = set()
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "$ref" and isinstance(child, str):
                if not child.startswith("#/components/"):
                    fail(f"selected Twilio surface has a non-local reference: {child}")
                found.add(child)
            else:
                found.update(local_refs(child))
    elif isinstance(value, list):
        for child in value:
            found.update(local_refs(child))
    return found


def decode_pointer(segment: str) -> str:
    return segment.replace("~1", "/").replace("~0", "~")


def resolve_component(document: dict, reference: str) -> tuple[str, str, object]:
    remainder = reference.removeprefix("#/components/")
    try:
        section, encoded_name = remainder.split("/", 1)
    except ValueError:
        fail(f"malformed component reference: {reference}")
    name = decode_pointer(encoded_name)
    try:
        value = document["components"][section][name]
    except KeyError:
        fail(f"component reference resolves to nothing: {reference}")
    return section, name, value


def extract(document: dict) -> tuple[dict, int]:
    paths: dict[str, object] = {}
    pending: set[str] = set()
    for source_path, method, operation_id in SELECTIONS:
        try:
            operation = document["paths"][source_path][method]
        except KeyError:
            fail(f"missing selected operation {method.upper()} {source_path}")
        if operation.get("operationId") != operation_id:
            fail(
                f"{method.upper()} {source_path} changed operationId: "
                f"expected {operation_id!r}, got {operation.get('operationId')!r}"
            )
        published_path = source_path.removeprefix(VERSION_PREFIX)
        if not published_path.startswith("/") or source_path == published_path:
            fail(f"version-prefix normalization did not consume {source_path}")
        if SOURCE_SERVER + source_path != PUBLISHED_SERVER + published_path:
            fail(f"version-prefix normalization moved {method.upper()} {source_path}")
        selected = copy.deepcopy(operation)
        selected["x-flux-source-path"] = source_path
        paths[published_path] = {method: selected}
        pending.update(local_refs(selected))

    components: dict[str, dict[str, object]] = {}
    visited: set[str] = set()
    while pending:
        reference = pending.pop()
        if reference in visited:
            continue
        visited.add(reference)
        section, name, value = resolve_component(document, reference)
        components.setdefault(section, {})[name] = copy.deepcopy(value)
        pending.update(local_refs(value) - visited)

    security_schemes = document.get("components", {}).get("securitySchemes", {})
    if security_schemes:
        components["securitySchemes"] = copy.deepcopy(security_schemes)

    extracted = {
        "openapi": document["openapi"],
        "info": copy.deepcopy(document["info"]),
        "servers": [{"url": PUBLISHED_SERVER}],
        "security": copy.deepcopy(document.get("security", [])),
        "paths": paths,
        "components": {
            section: dict(sorted(values.items()))
            for section, values in sorted(components.items())
        },
    }
    return extracted, len(visited)


def compact_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def provenance(
    fetched_at: str,
    scrubbed_path: str,
    scrubbed_sha: str,
    extraction_path: str,
    extraction_sha: str,
    license_path: str,
    scrubbed_keys: int,
    referenced_components: int,
) -> bytes:
    operation_ids = ", ".join(f'"{operation_id}"' for _, _, operation_id in SELECTIONS)
    rendered = f'''# Provenance for Twilio's pinned first-party API v2010 contract and C-473 extraction.
#
# Generated by `scripts/vendor-twilio-spec.py`; re-run the script instead of editing. The API
# document declares Apache-2.0, while the first-party repository declares MIT. Both notices are
# retained: the document declaration remains in both JSON files and the repository LICENSE is
# vendored beside them. `upstream_sha256` identifies the unmodified first-party API document.

version = 1
source_url = "{SOURCE_URL}"
source_commit = "{COMMIT}"
openapi_version = "3.0.1"
upstream_version = "1.0.0"
fetched_at = "{fetched_at}"
upstream_sha256 = "{UPSTREAM_SHA256}"
upstream_bytes = {UPSTREAM_BYTES}
document_license = "Apache-2.0"
repository_license = "MIT"
repository_license_url = "{LICENSE_URL}"
repository_license_path = "{license_path}"
repository_license_sha256 = "{LICENSE_SHA256}"
scrubbed_path = "{scrubbed_path}"
scrubbed_sha256 = "{scrubbed_sha}"
scrubbed_example_keys = {scrubbed_keys}
extraction_path = "{extraction_path}"
extraction_sha256 = "{extraction_sha}"
referenced_components = {referenced_components}
selected_operation_ids = [{operation_ids}]

[[normalization]]
kind = "strip-server-path-prefix"
source_server = "{SOURCE_SERVER}"
source_prefix = "{VERSION_PREFIX}"
published_server = "{PUBLISHED_SERVER}"
count = 4
'''
    return rendered.encode()


def replace_or_check(path: Path, expected: bytes, check: bool) -> None:
    if check:
        try:
            actual = path.read_bytes()
        except FileNotFoundError:
            fail(f"missing generated file: {path}")
        if actual != expected:
            fail(f"generated file is stale: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_bytes(expected)
    os.replace(temporary, path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source-dir",
        type=Path,
        help=f"directory containing {SOURCE_NAME} and LICENSE for offline replay",
    )
    parser.add_argument("--fetched-at")
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.source_dir and not args.fetched_at:
        fail("--source-dir requires --fetched-at so output is reproducible")
    if args.check and not args.fetched_at:
        fail("--check requires --fetched-at from the committed provenance")
    fetched_at = args.fetched_at or dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        dt.datetime.strptime(fetched_at, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        fail(f"invalid --fetched-at value: {fetched_at}")

    with tempfile.TemporaryDirectory() as directory:
        if args.source_dir:
            raw = (args.source_dir / SOURCE_NAME).read_bytes()
            license_bytes = (args.source_dir / "LICENSE").read_bytes()
        else:
            downloaded = Path(directory) / SOURCE_NAME
            downloaded_license = Path(directory) / "LICENSE"
            urllib.request.urlretrieve(SOURCE_URL, downloaded)
            urllib.request.urlretrieve(LICENSE_URL, downloaded_license)
            raw = downloaded.read_bytes()
            license_bytes = downloaded_license.read_bytes()

    if len(raw) != UPSTREAM_BYTES or sha256(raw) != UPSTREAM_SHA256:
        fail(
            "Twilio source identity changed: expected "
            f"{UPSTREAM_BYTES} bytes/{UPSTREAM_SHA256}, got {len(raw)} bytes/{sha256(raw)}"
        )
    if len(license_bytes) != LICENSE_BYTES or sha256(license_bytes) != LICENSE_SHA256:
        fail("Twilio repository LICENSE identity changed")
    document = json.loads(raw)
    if document.get("openapi") != "3.0.1" or document.get("info", {}).get("version") != "1.0.0":
        fail("Twilio source version changed")
    if document.get("info", {}).get("license", {}).get("name") != "Apache 2.0":
        fail("Twilio API document no longer declares the frozen Apache-2.0 notice")
    if document.get("servers") != [{"url": SOURCE_SERVER}]:
        fail(f"Twilio source server changed: {document.get('servers')!r}")
    paths = document.get("paths", {})
    operation_count = sum(
        1
        for item in paths.values()
        for method in item
        if method in HTTP_METHODS
    )
    if len(paths) != 121 or operation_count != 197:
        fail(
            f"Twilio source inventory is {len(paths)} paths/{operation_count} operations, "
            "expected 121/197"
        )

    scrubbed = copy.deepcopy(document)
    scrubbed_keys = scrub_examples(scrubbed)
    if scrubbed_keys != 967:
        fail(f"Twilio example-key inventory changed: expected 967, got {scrubbed_keys}")
    extraction, referenced_components = extract(scrubbed)
    scrubbed_bytes = compact_json(scrubbed)
    extraction_bytes = compact_json(extraction)

    root = Path(__file__).resolve().parent.parent
    fetched_date = fetched_at.split("T", 1)[0]
    scrubbed_rel = f"specs/twilio/upstream-{fetched_date}.openapi.json"
    extraction_rel = f"specs/twilio/selected-{fetched_date}.openapi.json"
    license_rel = "specs/twilio/LICENSE.twilio-oai.txt"
    generated = {
        root / scrubbed_rel: scrubbed_bytes,
        root / extraction_rel: extraction_bytes,
        root / license_rel: license_bytes,
        root / "specs/twilio.provenance.toml": provenance(
            fetched_at,
            scrubbed_rel,
            sha256(scrubbed_bytes),
            extraction_rel,
            sha256(extraction_bytes),
            license_rel,
            scrubbed_keys,
            referenced_components,
        ),
    }
    for path, expected in generated.items():
        replace_or_check(path, expected, args.check)

    action = "checked" if args.check else "vendored"
    print(
        f"{action} Twilio OpenAPI: 121 paths/197 operations, {scrubbed_keys} example keys "
        f"scrubbed, {referenced_components} referenced components retained"
    )


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Vendor OpenAI's pinned contract and its exact C-472 reference-closed extraction."""

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

from openapi_example_scrub import operation_id_inventory, scrub_examples


COMMIT = "117ce5680e4269f6656a4fd70d28f9755630d938"
SOURCE_URL = (
    "https://raw.githubusercontent.com/openai/openai-openapi/"
    f"{COMMIT}/openapi.json"
)
UPSTREAM_SHA256 = "ef36175ba6181b9d725a16b1eedcaa75a8a1268124896b10ccc5d9ddf0d543d3"
UPSTREAM_BYTES = 3_244_309
SOURCE_SERVER = "https://api.openai.com/v1"
PUBLISHED_SERVER = "https://api.openai.com"
HTTP_METHODS = {"get", "put", "post", "delete", "options", "head", "patch", "trace"}
SELECTIONS = (
    ("/responses/{response_id}", "get", "getResponse"),
    ("/responses/{response_id}/input_items", "get", "listInputItems"),
    ("/files", "get", "listFiles"),
    ("/batches", "get", "listBatches"),
)


def fail(message: str) -> None:
    raise SystemExit(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def local_refs(value: object) -> set[str]:
    found: set[str] = set()
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "$ref" and isinstance(child, str):
                if not child.startswith("#/components/"):
                    fail(f"selected OpenAI surface has a non-local reference: {child}")
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
        published_path = f"/v1{source_path}"
        if SOURCE_SERVER + source_path != PUBLISHED_SERVER + published_path:
            fail(f"path-prefix normalization moved {method.upper()} {source_path}")
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

    # Authentication is a declaration, not an example value, and the selected operations inherit
    # the root ApiKeyAuth requirement. Keep both first-party schemes and the root requirement.
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
    scrubbed_keys: int,
    referenced_components: int,
) -> bytes:
    operation_ids = ", ".join(f'"{operation_id}"' for _, _, operation_id in SELECTIONS)
    rendered = f'''# Provenance for OpenAI's pinned first-party OpenAPI contract and C-472 extraction.
#
# Generated by `scripts/vendor-openai-spec.py`; re-run the script instead of editing.
# The full vendored document removes every `example` and `examples` value. The extraction is
# reference-closed over four exact operationIds and materializes only the source server's `/v1`
# prefix. `upstream_sha256` still identifies the unmodified first-party input.

version = 1
source_url = "{SOURCE_URL}"
source_commit = "{COMMIT}"
openapi_version = "3.1.0"
upstream_version = "2.3.0"
fetched_at = "{fetched_at}"
upstream_sha256 = "{UPSTREAM_SHA256}"
upstream_bytes = {UPSTREAM_BYTES}
scrubbed_path = "{scrubbed_path}"
scrubbed_sha256 = "{scrubbed_sha}"
scrubbed_example_keys = {scrubbed_keys}
extraction_path = "{extraction_path}"
extraction_sha256 = "{extraction_sha}"
referenced_components = {referenced_components}
selected_operation_ids = [{operation_ids}]

[[normalization]]
kind = "materialize-server-path-prefix"
source_server = "{SOURCE_SERVER}"
published_server = "{PUBLISHED_SERVER}"
prefix = "/v1"
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
    parser.add_argument("--source-file", type=Path)
    parser.add_argument("--fetched-at")
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.source_file and not args.fetched_at:
        fail("--source-file requires --fetched-at so output is reproducible")
    if args.check and not args.fetched_at:
        fail("--check requires --fetched-at from the committed provenance")
    fetched_at = args.fetched_at or dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        dt.datetime.strptime(fetched_at, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        fail(f"invalid --fetched-at value: {fetched_at}")

    with tempfile.TemporaryDirectory() as directory:
        if args.source_file:
            raw = args.source_file.read_bytes()
        else:
            downloaded = Path(directory) / "openapi.json"
            urllib.request.urlretrieve(SOURCE_URL, downloaded)
            raw = downloaded.read_bytes()

    if len(raw) != UPSTREAM_BYTES:
        fail(f"OpenAI source byte count changed: expected {UPSTREAM_BYTES}, got {len(raw)}")
    actual_sha = sha256(raw)
    if actual_sha != UPSTREAM_SHA256:
        fail(f"OpenAI source hash mismatch: expected {UPSTREAM_SHA256}, got {actual_sha}")
    document = json.loads(raw)
    if document.get("openapi") != "3.1.0" or document.get("info", {}).get("version") != "2.3.0":
        fail("OpenAI source version changed")
    if document.get("info", {}).get("license", {}).get("identifier") != "MIT":
        fail("OpenAI source no longer declares the frozen MIT license")
    if document.get("servers") != [{"url": SOURCE_SERVER}]:
        fail(f"OpenAI source server changed: {document.get('servers')!r}")
    paths = document.get("paths", {})
    operation_count = sum(
        1
        for item in paths.values()
        for method in item
        if method in HTTP_METHODS
    )
    if len(paths) != 182 or operation_count != 288:
        fail(
            f"OpenAI source inventory is {len(paths)} paths/{operation_count} operations, "
            "expected 182/288"
        )
    id_inventory = operation_id_inventory(document)
    if id_inventory != {"operations": 288, "present": 288, "unique": 288}:
        fail(
            "OpenAI operationId inventory contains a missing or duplicate id: "
            f"{id_inventory!r}"
        )

    scrubbed = copy.deepcopy(document)
    scrubbed_keys = scrub_examples(scrubbed)
    if scrubbed_keys != 964:
        fail(f"OpenAI example-key inventory changed: expected 964, got {scrubbed_keys}")
    extraction, referenced_components = extract(scrubbed)
    scrubbed_bytes = compact_json(scrubbed)
    extraction_bytes = compact_json(extraction)

    root = Path(__file__).resolve().parent.parent
    fetched_date = fetched_at.split("T", 1)[0]
    scrubbed_rel = f"specs/openai/upstream-{fetched_date}.openapi.json"
    extraction_rel = f"specs/openai/selected-{fetched_date}.openapi.json"
    generated = {
        root / scrubbed_rel: scrubbed_bytes,
        root / extraction_rel: extraction_bytes,
        root / "specs/openai.provenance.toml": provenance(
            fetched_at,
            scrubbed_rel,
            sha256(scrubbed_bytes),
            extraction_rel,
            sha256(extraction_bytes),
            scrubbed_keys,
            referenced_components,
        ),
    }
    for path, expected in generated.items():
        replace_or_check(path, expected, args.check)

    action = "checked" if args.check else "vendored"
    print(
        f"{action} OpenAI OpenAPI: 182 paths/288 operations, {scrubbed_keys} example keys "
        f"scrubbed, {referenced_components} referenced components retained"
    )


if __name__ == "__main__":
    main()

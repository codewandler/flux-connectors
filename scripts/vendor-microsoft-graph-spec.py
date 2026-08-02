#!/usr/bin/env python3
"""Vendor deterministic, reference-closed Microsoft Graph OpenAPI extracts (C-471).

The 38 MB upstream document is evidence, not a build input.  This script verifies its immutable
first-party identity, copies four exact operation objects, materializes the upstream server's
``/v1.0`` prefix onto their path keys, and follows every local component reference to closure.
Build, diff, and check never invoke this script.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import urllib.request

import yaml


COMMIT = "60b50e2e5b23612aac74ecdf65d35d566c5a4031"
SOURCE_URL = (
    "https://raw.githubusercontent.com/microsoftgraph/msgraph-metadata/"
    f"{COMMIT}/openapi/v1.0/openapi.yaml"
)
UPSTREAM_SHA256 = "2749e51f363a471cdaa4835493c2c57198aa834262666da39c03a2e7f9f9d831"
UPSTREAM_BYTES = 38_050_122
UPSTREAM_VERSION = "v1.0"
SOURCE_SERVER = "https://graph.microsoft.com/v1.0"
PUBLISHED_BASE = "https://graph.microsoft.com"
METHODS = {"get", "put", "post", "delete", "options", "head", "patch", "trace"}

# One document per target service.  These exact selectors are the reviewed C-468 inventory; no
# prefix, tag, path family, or other sweep is accepted here.
SELECTIONS = {
    "mail": [
        ("/me/messages", "get", "me.ListMessages"),
    ],
    "calendar": [
        (
            "/me/outlook/masterCategories",
            "get",
            "me.outlook.ListMasterCategories",
        ),
        (
            "/me/outlook/supportedTimeZones()",
            "get",
            "me.outlook.supportedTimeZones-5c4f",
        ),
        (
            "/me/outlook/supportedLanguages()",
            "get",
            "me.outlook.supportedLanguages",
        ),
    ],
}


def fail(message: str) -> "None":
    raise SystemExit(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def yaml_key(line: bytes) -> str:
    token = line.decode().strip()
    if not token.endswith(":"):
        fail(f"cannot index Graph YAML key: {token!r}")
    parsed = yaml.safe_load(token[:-1])
    if not isinstance(parsed, str):
        fail(f"Graph YAML key is not a string: {token!r}")
    return parsed


def index_source(source: Path):
    """Index top-level path/component blocks without materializing the 38 MB YAML tree."""
    path_offsets: dict[str, tuple[int, int]] = {}
    component_offsets: dict[tuple[str, str], tuple[int, int]] = {}
    selected_ids = {operation_id: 0 for rows in SELECTIONS.values() for _, _, operation_id in rows}
    mode = None
    section = None
    current_path = None
    current_path_start = None
    current_component = None
    current_component_start = None
    operation_count = 0
    operation_ids: list[str] = []

    with source.open("rb") as handle:
        while True:
            offset = handle.tell()
            line = handle.readline()
            if not line:
                end = handle.tell()
                break
            stripped = line.lstrip(b" ")
            indent = len(line) - len(stripped)

            if indent == 0 and stripped.strip().endswith(b":"):
                if current_path is not None:
                    path_offsets[current_path] = (current_path_start, offset)
                    current_path = current_path_start = None
                if current_component is not None:
                    component_offsets[(section, current_component)] = (
                        current_component_start,
                        offset,
                    )
                    current_component = current_component_start = None
                root_key = yaml_key(line)
                mode = root_key if root_key in {"paths", "components"} else None
                section = None
                continue

            if mode == "paths" and indent == 2 and stripped.strip().endswith(b":"):
                if current_path is not None:
                    path_offsets[current_path] = (current_path_start, offset)
                current_path = yaml_key(line)
                current_path_start = offset
                continue
            if mode == "paths" and indent == 4:
                token = stripped.decode().split(":", 1)[0]
                if token in METHODS:
                    operation_count += 1
            if mode == "paths" and indent == 6 and stripped.startswith(b"operationId:"):
                operation_id = stripped.decode().split(":", 1)[1].strip()
                operation_ids.append(operation_id)
                if operation_id in selected_ids:
                    selected_ids[operation_id] += 1

            if mode == "components" and indent == 2 and stripped.strip().endswith(b":"):
                if current_component is not None:
                    component_offsets[(section, current_component)] = (
                        current_component_start,
                        offset,
                    )
                    current_component = current_component_start = None
                section = yaml_key(line)
                continue
            if mode == "components" and indent == 4 and stripped.strip().endswith(b":"):
                if current_component is not None:
                    component_offsets[(section, current_component)] = (
                        current_component_start,
                        offset,
                    )
                current_component = yaml_key(line)
                current_component_start = offset

    if current_path is not None:
        path_offsets[current_path] = (current_path_start, end)
    if current_component is not None:
        component_offsets[(section, current_component)] = (current_component_start, end)

    if len(path_offsets) != 10_790 or operation_count != 16_702:
        fail(
            f"Graph source inventory is {len(path_offsets)} paths/{operation_count} operations, "
            "expected 10790/16702"
        )
    if len(operation_ids) != operation_count or len(set(operation_ids)) != operation_count:
        fail(
            "Graph operationId inventory contains a missing or duplicate id: "
            f"operations={operation_count}, present={len(operation_ids)}, "
            f"unique={len(set(operation_ids))}"
        )
    duplicates = {name: count for name, count in selected_ids.items() if count != 1}
    if duplicates:
        fail(f"selected operationId inventory changed: {duplicates!r}")
    return path_offsets, component_offsets


def block(source: Path, offsets: tuple[int, int], indent: int):
    start, end = offsets
    with source.open("rb") as handle:
        handle.seek(start)
        raw = handle.read(end - start).decode()
    dedented = "".join(line[indent:] if line.strip() else line for line in raw.splitlines(True))
    return yaml.safe_load(dedented)


def refs(value) -> set[str]:
    found: set[str] = set()
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "$ref" and isinstance(child, str):
                found.add(child)
            else:
                found.update(refs(child))
    elif isinstance(value, list):
        for child in value:
            found.update(refs(child))
    return found


def decode_pointer(segment: str) -> str:
    return segment.replace("~1", "/").replace("~0", "~")


def component(source: Path, index, reference: str):
    prefix = "#/components/"
    if not reference.startswith(prefix):
        fail(f"selected Graph surface has a non-local component reference: {reference}")
    remainder = reference[len(prefix) :]
    try:
        section, encoded_name = remainder.split("/", 1)
    except ValueError:
        fail(f"malformed component reference: {reference}")
    name = decode_pointer(encoded_name)
    offsets = index.get((section, name))
    if offsets is None:
        fail(f"component reference resolves to nothing: {reference}")
    parsed = block(source, offsets, 4)
    if not isinstance(parsed, dict) or list(parsed) != [name]:
        fail(f"could not parse indexed component: {reference}")
    value = parsed[name]
    return section, name, value


def sensitive_example(value, path: tuple[str, ...] = ()) -> list[str]:
    """Return paths to credential/contact-shaped example values without logging their values."""
    findings: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            findings.extend(sensitive_example(child, (*path, key)))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(sensitive_example(child, (*path, str(index))))
    elif path and any(part in {"example", "examples", "value"} for part in path):
        rendered = str(value)
        credential = re.search(
            r"(?i)(bearer\s+|api[_ -]?key|access[_ -]?token|refresh[_ -]?token|password|secret)",
            rendered,
        )
        personal_email = re.search(
            r"[A-Z0-9._%+-]+@(?!example\.(?:com|net|org)\b)[A-Z0-9.-]+\.[A-Z]{2,}",
            rendered,
            re.IGNORECASE,
        )
        telephone = re.search(r"(?<!\w)\+[0-9 ()-]{7,}", rendered)
        if credential or personal_email or telephone:
            findings.append("/" + "/".join(path))
    return findings


def extract(
    source: Path,
    path_index,
    component_index,
    service: str,
    selections: list[tuple[str, str, str]],
):
    paths = {}
    pending: set[str] = set()
    for source_path, method, operation_id in selections:
        offsets = path_index.get(source_path)
        if offsets is None:
            fail(f"missing selected path {source_path}")
        parsed = block(source, offsets, 2)
        if not isinstance(parsed, dict) or list(parsed) != [source_path]:
            fail(f"could not parse indexed path {source_path}")
        source_item = parsed[source_path]
        operation = source_item.get(method)
        if not isinstance(operation, dict):
            fail(f"missing selected operation {method.upper()} {source_path}")
        if operation.get("operationId") != operation_id:
            fail(
                f"{method.upper()} {source_path} has operationId "
                f"{operation.get('operationId')!r}, expected {operation_id!r}"
            )

        description = source_item.get("description")
        published_path = f"/v1.0{source_path}"
        if SOURCE_SERVER + source_path != PUBLISHED_BASE + published_path:
            fail(f"path-prefix normalization moved {method.upper()} {source_path}")
        item = {"x-flux-source-path": source_path}
        if isinstance(description, str):
            item["description"] = description
        item[method] = operation
        paths[published_path] = item
        pending.update(refs(operation))

    components: dict[str, dict[str, object]] = {}
    visited: set[str] = set()
    while pending:
        reference = min(pending)
        pending.remove(reference)
        if reference in visited:
            continue
        visited.add(reference)
        section, name, value = component(source, component_index, reference)
        components.setdefault(section, {})[name] = value
        pending.update(refs(value) - visited)

    # Stable section/name ordering makes replay byte-for-byte deterministic independently of the
    # order references happen to occur inside an upstream object.
    stable_components = {
        section: {name: values[name] for name in sorted(values)}
        for section, values in sorted(components.items())
    }
    document = {
        "openapi": "3.0.4",
        "info": {
            "title": f"Microsoft Graph v1.0 — C-471 {service} reference-closed extraction",
            "version": UPSTREAM_VERSION,
            "license": {
                "name": "MIT",
                "url": f"https://github.com/microsoftgraph/msgraph-metadata/blob/{COMMIT}/LICENSE",
            },
        },
        "servers": [{"url": SOURCE_SERVER}],
        "x-flux-source": {
            "repository": "microsoftgraph/msgraph-metadata",
            "commit": COMMIT,
            "document": "openapi/v1.0/openapi.yaml",
            "server": SOURCE_SERVER,
            "upstream-sha256": UPSTREAM_SHA256,
        },
        "paths": paths,
        "components": stable_components,
    }
    findings = sensitive_example(document)
    if findings:
        fail(
            f"{service} extraction contains credential/contact-shaped examples at "
            + ", ".join(findings)
        )
    return document, visited


def render_provenance(fetched_at: str, outputs: dict[str, tuple[str, int]]) -> bytes:
    lines = [
        "# Provenance for the deterministic Microsoft Graph v1.0 extracts.",
        "#",
        "# Generated by `scripts/vendor-microsoft-graph-spec.py`; re-run it instead of editing.",
        "# `upstream_sha256` identifies the complete first-party 38 MB document. Each `sha256`",
        "# identifies the smaller reference-closed document that offline builds ingest.",
        "",
        "version = 1",
        f'source_commit = "{COMMIT}"',
        f'upstream_sha256 = "{UPSTREAM_SHA256}"',
        f"upstream_bytes = {UPSTREAM_BYTES}",
    ]
    date = fetched_at.split("T", 1)[0]
    for service in ("mail", "calendar"):
        digest, count = outputs[service]
        lines.extend(
            [
                "",
                "[[spec]]",
                f'name = "{service}"',
                f'path = "specs/microsoft_graph/{service}-{date}.openapi.json"',
                f'source_url = "{SOURCE_URL}"',
                f'upstream_version = "{UPSTREAM_VERSION}"',
                f'fetched_at = "{fetched_at}"',
                f'sha256 = "{digest}"',
                f'upstream_sha256 = "{UPSTREAM_SHA256}"',
                f"referenced_components = {count}",
            ]
        )
    return ("\n".join(lines) + "\n").encode()


def write_or_check(path: Path, content: bytes, check: bool) -> None:
    if check:
        if not path.is_file() or path.read_bytes() != content:
            fail(f"stale generated Microsoft Graph vendor file: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source-dir",
        type=Path,
        help="directory containing the exact upstream bytes as openapi.yaml",
    )
    parser.add_argument("--fetched-at", help="RFC 3339 UTC provenance timestamp")
    parser.add_argument("--check", action="store_true", help="refuse any stale output")
    args = parser.parse_args()

    if args.fetched_at and not re.fullmatch(
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z",
        args.fetched_at,
    ):
        fail(f"invalid --fetched-at value: {args.fetched_at}")
    if args.source_dir and not args.fetched_at:
        fail("--source-dir requires --fetched-at so provenance replay is deterministic")

    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory() as temporary:
        if args.source_dir:
            source = args.source_dir / "openapi.yaml"
            if not source.is_file():
                fail(f"missing upstream document: {source}")
        else:
            source = Path(temporary) / "openapi.yaml"
            try:
                with urllib.request.urlopen(SOURCE_URL) as response:
                    source.write_bytes(response.read())
            except OSError as error:
                fail(f"could not fetch {SOURCE_URL}: {error}")

        upstream_bytes = source.stat().st_size
        if upstream_bytes != UPSTREAM_BYTES:
            fail(f"upstream byte count is {upstream_bytes}, expected {UPSTREAM_BYTES}")
        digest = hashlib.sha256()
        with source.open("rb") as handle:
            for block in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(block)
        actual_upstream = digest.hexdigest()
        if actual_upstream != UPSTREAM_SHA256:
            fail(f"upstream sha256 is {actual_upstream}, expected {UPSTREAM_SHA256}")

        with source.open() as handle:
            header_lines = []
            for line in handle:
                if line == "paths:\n":
                    break
                header_lines.append(line)
        header = yaml.safe_load("".join(header_lines))
        if header.get("openapi") != "3.0.4":
            fail("Microsoft Graph source is no longer OpenAPI 3.0.4")
        if header.get("info", {}).get("version") != UPSTREAM_VERSION:
            fail(f"Microsoft Graph source no longer declares info.version {UPSTREAM_VERSION}")
        servers = header.get("servers")
        if servers != [{"url": SOURCE_SERVER}]:
            fail(f"Microsoft Graph source server inventory changed: {servers!r}")
        path_index, component_index = index_source(source)

        fetched_at = args.fetched_at or dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
        date = fetched_at.split("T", 1)[0]
        rendered: dict[str, bytes] = {}
        outputs: dict[str, tuple[str, int]] = {}
        union: set[str] = set()
        for service, selections in SELECTIONS.items():
            document, visited = extract(
                source, path_index, component_index, service, selections
            )
            content = (json.dumps(document, indent=2, ensure_ascii=False) + "\n").encode()
            rendered[service] = content
            outputs[service] = (sha256(content), len(visited))
            union.update(visited)
        if len(union) != 36:
            fail(f"reference closure retained {len(union)} components, expected 36")

        output_dir = root / "specs/microsoft_graph"
        expected_paths = {
            output_dir / f"{service}-{date}.openapi.json" for service in SELECTIONS
        }
        existing_paths = set(output_dir.glob("*.openapi.json")) if output_dir.is_dir() else set()
        unexpected = existing_paths - expected_paths
        if args.check and unexpected:
            fail(
                "unexpected stale Microsoft Graph extract(s): "
                + ", ".join(str(path) for path in sorted(unexpected))
            )
        if not args.check:
            for path in unexpected:
                path.unlink()
        for service, content in rendered.items():
            write_or_check(
                output_dir / f"{service}-{date}.openapi.json", content, args.check
            )
        write_or_check(
            root / "specs/microsoft_graph.provenance.toml",
            render_provenance(fetched_at, outputs),
            args.check,
        )

        print(
            f"Microsoft Graph {COMMIT}: {len(union)} reference-closed components; "
            + ", ".join(
                f"{service} {len(rendered[service])} bytes {outputs[service][0]}"
                for service in ("mail", "calendar")
            )
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())

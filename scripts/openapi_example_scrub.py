#!/usr/bin/env python3
"""Locate OpenAPI example-value keywords without erasing same-named declarations."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


# A key directly inside one of these maps is a declaration name, not an OpenAPI keyword. A schema
# property literally named `example`, for example, must survive even though the Schema Object that
# describes it may itself carry an `example` keyword.
DECLARATION_MAPS = {
    "$defs",
    "callbacks",
    "dependentSchemas",
    "definitions",
    "encoding",
    "headers",
    "links",
    "parameters",
    "pathItems",
    "patternProperties",
    "properties",
    "requestBodies",
    "responses",
    "schemas",
    "securitySchemes",
    "webhooks",
}
COMPONENT_DECLARATION_MAPS = {
    "callbacks",
    "examples",
    "headers",
    "links",
    "parameters",
    "pathItems",
    "requestBodies",
    "responses",
    "schemas",
    "securitySchemes",
}
HTTP_METHODS = {"get", "put", "post", "delete", "options", "head", "patch", "trace"}


def operation_id_inventory(document: dict) -> dict[str, int]:
    """Count HTTP operations, present operation ids, and distinct operation ids."""
    ids = [
        item[method].get("operationId")
        for item in document.get("paths", {}).values()
        for method in item
        if method in HTTP_METHODS and isinstance(item[method], dict)
    ]
    present = [operation_id for operation_id in ids if isinstance(operation_id, str) and operation_id]
    return {"operations": len(ids), "present": len(present), "unique": len(set(present))}


def example_keyword_paths(
    value: object,
    path: tuple[object, ...] = (),
    mode: str = "regular",
) -> list[list[object]]:
    """Return JSON paths to `example`/`examples` keywords, preserving declaration-map keys."""
    found: list[list[object]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            if mode == "example-object" and key in {"value", "externalValue"}:
                found.append([*path, key])
            elif key in {"example", "examples"} and mode == "regular":
                found.append([*path, key])
            else:
                if mode == "component-sections" and key == "examples":
                    child_mode = "example-declarations"
                elif mode == "component-sections" and key in COMPONENT_DECLARATION_MAPS:
                    child_mode = "declarations"
                elif mode == "component-sections":
                    # Extension values are arbitrary objects, not component declaration maps.
                    child_mode = "regular"
                elif mode == "example-declarations":
                    child_mode = "example-object"
                elif mode == "declarations":
                    child_mode = "regular"
                elif key == "components":
                    child_mode = "component-sections"
                elif key in DECLARATION_MAPS:
                    child_mode = "declarations"
                else:
                    child_mode = "regular"
                found.extend(example_keyword_paths(child, (*path, key), child_mode))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            found.extend(example_keyword_paths(child, (*path, index), "regular"))
    return found


def scrub_examples(value: object) -> int:
    """Remove the located keyword values in place and return their count."""
    paths = example_keyword_paths(value)
    for path in sorted(paths, key=len, reverse=True):
        current = value
        for step in path[:-1]:
            current = current[step]  # type: ignore[index]
        del current[path[-1]]  # type: ignore[index]
    return len(paths)


def self_test() -> None:
    fixture = {
        "components": {
            "schemas": {
                "example": {
                    "type": "object",
                    "properties": {
                        "examples": {"type": "string", "example": "remove me"},
                        "nested": {
                            "$defs": {
                                "example": {"type": "string", "example": "remove me"}
                            },
                            "patternProperties": {
                                "examples": {"type": "number", "examples": [1, 2]}
                            },
                        },
                    },
                    "example": {"examples": "remove me too"},
                }
            },
            "examples": {
                "example": {
                    "summary": "a declaration, not a keyword",
                    "value": "remove this example value",
                }
            },
            "x-policy": {"example": "remove extension value"},
        }
    }
    removed = scrub_examples(fixture)
    assert removed == 6, removed
    schema = fixture["components"]["schemas"]["example"]
    assert "example" not in schema
    assert schema["properties"]["examples"] == {"type": "string"}
    assert schema["properties"]["nested"]["$defs"]["example"] == {"type": "string"}
    assert schema["properties"]["nested"]["patternProperties"]["examples"] == {
        "type": "number"
    }
    assert fixture["components"]["examples"]["example"]["summary"]
    assert "value" not in fixture["components"]["examples"]["example"]
    assert fixture["components"]["x-policy"] == {}
    inventory_fixture = {
        "paths": {
            "/one": {"get": {"operationId": "same"}},
            "/two": {"post": {"operationId": "same"}},
            "/three": {"delete": {}},
        }
    }
    assert operation_id_inventory(inventory_fixture) == {
        "operations": 3,
        "present": 2,
        "unique": 1,
    }
    complete = {
        "paths": {
            "/one": {"get": {"operationId": "one"}},
            "/two": {"post": {"operationId": "two"}},
        }
    }
    expected = {"operations": 2, "present": 2, "unique": 2}
    assert operation_id_inventory(complete) == expected
    missing = json.loads(json.dumps(complete))
    del missing["paths"]["/two"]["post"]["operationId"]
    assert operation_id_inventory(missing) != expected
    duplicate = json.loads(json.dumps(complete))
    duplicate["paths"]["/two"]["post"]["operationId"] = "one"
    assert operation_id_inventory(duplicate) != expected
    drifted = json.loads(json.dumps(complete))
    drifted["paths"]["/three"] = {"delete": {"operationId": "three"}}
    assert operation_id_inventory(drifted) != expected


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", nargs="?", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--operation-inventory", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.source is None:
        parser.error("source is required unless --self-test is used")
    document = json.loads(args.source.read_bytes())
    result = operation_id_inventory(document) if args.operation_inventory else example_keyword_paths(document)
    json.dump(result, fp=__import__("sys").stdout, separators=(",", ":"))
    print()


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Normalize Asterisk 22.10.1's first-party ARI Swagger into deterministic OpenAPI 3.

The eleven Swagger 1.1/1.2 documents remain the source contract. This converter is deliberately
version- and inventory-closed: a later Asterisk description must be reviewed and pinned rather than
being interpreted according to guesses made for 22.10.1.
"""

from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import tempfile
from typing import Any, Callable


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = ROOT / "specs" / "asterisk"
DEFAULT_OUTPUT = DEFAULT_SOURCE / "ari-22.10.1.openapi.json"
SOURCE_REPOSITORY = "https://github.com/asterisk/asterisk"
SOURCE_TAG = "22.10.1"
SOURCE_TAG_OBJECT = "4f85d05889cf9fb9c9e2ae44cc3f4a825a74545a"
SOURCE_COMMIT = "f0e408a7b0d829c85bf15fa4b487870a50cb3000"
BASE_PATH = "http://localhost:8088/ari"
HTTP_METHODS = {"GET", "PUT", "POST", "DELETE"}
EXPECTED = {
    "applications.json": ("1.1", 4, 5, 1),
    "asterisk.json": ("1.1", 9, 16, 12),
    "bridges.json": ("1.1", 12, 18, 1),
    "channels.json": ("1.1", 25, 35, 5),
    "deviceStates.json": ("1.1", 2, 4, 1),
    "endpoints.json": ("1.1", 7, 7, 2),
    "events.json": ("1.2", 3, 3, 57),
    "mailboxes.json": ("1.1", 2, 4, 1),
    "playbacks.json": ("1.1", 2, 3, 1),
    "recordings.json": ("1.1", 8, 12, 2),
    "sounds.json": ("1.1", 2, 2, 2),
}
PRIMITIVES = {
    "string": {"type": "string"},
    "Date": {"type": "string", "format": "date-time"},
    "int": {"type": "integer", "format": "int32"},
    "long": {"type": "integer", "format": "int64"},
    "double": {"type": "number", "format": "double"},
    "boolean": {"type": "boolean"},
    "object": {"type": "object", "additionalProperties": True},
    "containers": {"type": "object", "additionalProperties": True},
}


class SpecError(ValueError):
    """The pinned legacy description has moved beyond the reviewed source shape."""


def fail(message: str) -> None:
    raise SpecError(message)


def require(value: Any, kind: type, where: str) -> Any:
    if not isinstance(value, kind):
        fail(f"{where}: expected {kind.__name__}")
    return value


def required_string(value: dict[str, Any], key: str, where: str) -> str:
    result = value.get(key)
    if not isinstance(result, str) or not result:
        fail(f"{where}.{key}: expected a non-empty string")
    return result


def optional_string(value: dict[str, Any], key: str, where: str) -> str | None:
    result = value.get(key)
    if result is not None and not isinstance(result, str):
        fail(f"{where}.{key}: expected a string")
    return result


def schema_for_type(type_name: str, models: set[str], where: str) -> dict[str, Any]:
    if type_name.startswith("List[") and type_name.endswith("]"):
        inner = type_name[5:-1]
        if not inner or "[" in inner or "]" in inner:
            fail(f"{where}: malformed list type {type_name!r}")
        return {"type": "array", "items": schema_for_type(inner, models, where)}
    if type_name in PRIMITIVES:
        return copy.deepcopy(PRIMITIVES[type_name])
    if type_name in models:
        return {"$ref": f"#/components/schemas/{type_name}"}
    fail(f"{where}: unknown type {type_name!r}")


def apply_allowable(
    schema: dict[str, Any], value: dict[str, Any], where: str
) -> None:
    allowable = value.get("allowableValues")
    if allowable is None:
        return
    allowable = require(allowable, dict, f"{where}.allowableValues")
    value_type = allowable.get("valueType")
    if value_type == "LIST":
        values = require(allowable.get("values"), list, f"{where}.allowableValues.values")
        if not values:
            fail(f"{where}.allowableValues.values: expected at least one value")
        schema["enum"] = copy.deepcopy(values)
    elif value_type == "RANGE":
        if not any(key in allowable for key in ("min", "max")):
            fail(f"{where}.allowableValues: empty range")
        for source, target in (("min", "minimum"), ("max", "maximum")):
            if source in allowable:
                bound = allowable[source]
                if not isinstance(bound, (int, float)) or isinstance(bound, bool):
                    fail(f"{where}.allowableValues.{source}: expected a number")
                schema[target] = bound
    else:
        fail(f"{where}.allowableValues.valueType: unsupported value {value_type!r}")


def load_documents(source: Path) -> tuple[dict[str, Any], list[tuple[str, dict[str, Any]]]]:
    api_dir = source / "api-docs"
    if not api_dir.is_dir():
        fail(f"Asterisk API document directory is absent: {api_dir}")
    actual = {path.name for path in api_dir.iterdir() if path.suffix == ".json"}
    expected = set(EXPECTED)
    if actual != expected:
        fail(
            "Asterisk API document inventory differs; "
            f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )

    resources_path = source / "resources.json"
    try:
        resources = require(json.loads(resources_path.read_text()), dict, str(resources_path))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot parse {resources_path}: {error}")
    if resources.get("apiVersion") != "10.0.0" or resources.get("swaggerVersion") != "1.1":
        fail("resources.json has an unknown source version")
    if resources.get("basePath") != BASE_PATH:
        fail(f"resources.json moved basePath from {BASE_PATH!r}")
    declared: set[str] = set()
    for index, raw_api in enumerate(require(resources.get("apis"), list, "resources.json.apis")):
        api = require(raw_api, dict, f"resources.json.apis[{index}]")
        path = required_string(api, "path", f"resources.json.apis[{index}]")
        prefix = "/api-docs/"
        suffix = ".{format}"
        if not path.startswith(prefix) or not path.endswith(suffix):
            fail(f"resources.json.apis[{index}].path: unsupported path {path!r}")
        declared.add(path[len(prefix) : -len(suffix)] + ".json")
    if declared != expected:
        fail(
            "resources.json document inventory differs; "
            f"missing={sorted(expected - declared)}, extra={sorted(declared - expected)}"
        )

    documents: list[tuple[str, dict[str, Any]]] = []
    for name in sorted(EXPECTED):
        path = api_dir / name
        try:
            document = require(json.loads(path.read_text()), dict, str(path))
        except (OSError, json.JSONDecodeError) as error:
            fail(f"cannot parse {path}: {error}")
        swagger, path_count, operation_count, model_count = EXPECTED[name]
        if document.get("swaggerVersion") != swagger or document.get("apiVersion") != "2.0.0":
            fail(f"{name} has an unknown source version")
        if document.get("basePath") != BASE_PATH:
            fail(f"{name} moved basePath from {BASE_PATH!r}")
        expected_resource_path = f"/api-docs/{name[:-5]}.{{format}}"
        if document.get("resourcePath") != expected_resource_path:
            fail(f"{name} moved resourcePath from {expected_resource_path!r}")
        apis = require(document.get("apis"), list, f"{name}.apis")
        models = require(document.get("models"), dict, f"{name}.models")
        operations = sum(
            len(require(api.get("operations"), list, f"{name}.apis[{index}].operations"))
            for index, api in enumerate(apis)
            if isinstance(api, dict)
        )
        if len(apis) != path_count or operations != operation_count or len(models) != model_count:
            fail(
                f"{name} inventory differs: expected {path_count} paths/{operation_count} "
                f"operations/{model_count} models, got {len(apis)}/{operations}/{len(models)}"
            )
        documents.append((name[:-5], document))
    return resources, documents


def normalize(source: Path) -> tuple[dict[str, Any], dict[str, int | list[str]]]:
    _, documents = load_documents(source)
    raw_models: dict[str, tuple[dict[str, Any], str]] = {}
    for resource, document in documents:
        for name, raw_model in document["models"].items():
            where = f"{resource}.models.{name}"
            model = require(raw_model, dict, where)
            if required_string(model, "id", where) != name:
                fail(f"{where}.id: expected {name!r}")
            if name in raw_models:
                fail(f"duplicate model {name!r}")
            raw_models[name] = (model, where)
    if len(raw_models) != 85:
        fail(f"model inventory differs: expected 85, got {len(raw_models)}")
    model_names = set(raw_models)

    schemas: dict[str, Any] = {}
    for name in sorted(raw_models):
        model, where = raw_models[name]
        properties: dict[str, Any] = {}
        required_properties: list[str] = []
        for property_name, raw_property in sorted(
            require(model.get("properties"), dict, f"{where}.properties").items()
        ):
            property_where = f"{where}.properties.{property_name}"
            prop = require(raw_property, dict, property_where)
            schema = schema_for_type(
                required_string(prop, "type", property_where), model_names, property_where
            )
            description = optional_string(prop, "description", property_where)
            if description is not None:
                schema["description"] = description
            apply_allowable(schema, prop, property_where)
            required = prop.get("required", False)
            if not isinstance(required, bool):
                fail(f"{property_where}.required: expected a boolean")
            if required:
                required_properties.append(property_name)
            properties[property_name] = schema
        schema = {
            "type": "object",
            "description": required_string(model, "description", where),
            "properties": properties,
        }
        if required_properties:
            schema["required"] = sorted(required_properties)
        subtypes = model.get("subTypes", [])
        if not isinstance(subtypes, list) or not all(isinstance(item, str) for item in subtypes):
            fail(f"{where}.subTypes: expected a string array")
        missing_subtypes = sorted(set(subtypes) - model_names)
        if missing_subtypes:
            fail(f"{where}.subTypes: unknown types {missing_subtypes}")
        if subtypes:
            schema["x-ari-subtypes"] = copy.deepcopy(subtypes)
        discriminator = optional_string(model, "discriminator", where)
        if discriminator is not None:
            if discriminator not in properties:
                fail(f"{where}.discriminator: unknown property {discriminator!r}")
            schema["discriminator"] = {"propertyName": discriminator}
            schema["x-ari-discriminator"] = discriminator
        schemas[name] = schema

    paths: dict[str, dict[str, Any]] = {}
    tags: list[dict[str, str]] = []
    operation_ids: set[str] = set()
    source_operations = 0
    deferred: list[str] = []
    source_paths: set[str] = set()
    for resource, document in documents:
        tags.append({"name": resource})
        for api_index, raw_api in enumerate(document["apis"]):
            api_where = f"{resource}.apis[{api_index}]"
            api = require(raw_api, dict, api_where)
            path = required_string(api, "path", api_where)
            if not path.startswith("/"):
                fail(f"{api_where}.path: expected an absolute API path")
            source_paths.add(path)
            path_description = optional_string(api, "description", api_where) or ""
            path_item = paths.setdefault(path, {})
            for operation_index, raw_operation in enumerate(api["operations"]):
                where = f"{api_where}.operations[{operation_index}]"
                operation = require(raw_operation, dict, where)
                source_operations += 1
                method = required_string(operation, "httpMethod", where)
                if method not in HTTP_METHODS:
                    fail(f"{where}.httpMethod: unsupported method {method!r}")
                nickname = required_string(operation, "nickname", where)
                operation_id = f"{resource}-{nickname}"
                if operation_id in operation_ids:
                    fail(f"duplicate operationId {operation_id!r}")
                operation_ids.add(operation_id)

                upgrade = operation.get("upgrade")
                if upgrade is not None and upgrade != "websocket":
                    fail(f"{where}.upgrade: unsupported value {upgrade!r}")
                websocket = upgrade == "websocket"
                if websocket:
                    if operation_id != "events-eventWebsocket":
                        fail(f"unaccounted WebSocket operation {operation_id!r}")
                    if operation.get("websocketProtocol") != "ari":
                        fail(f"{where}.websocketProtocol: expected 'ari'")
                    deferred.append(operation_id)
                    continue
                if "websocketProtocol" in operation:
                    fail(f"{where}.websocketProtocol exists without a WebSocket upgrade")

                response_class = required_string(operation, "responseClass", where)
                parameters: list[dict[str, Any]] = []
                request_body: dict[str, Any] | None = None
                parameter_names: set[tuple[str, str]] = set()
                raw_parameters = operation.get("parameters", [])
                raw_parameters = require(raw_parameters, list, f"{where}.parameters")
                for parameter_index, raw_parameter in enumerate(raw_parameters):
                    parameter_where = f"{where}.parameters[{parameter_index}]"
                    parameter = require(raw_parameter, dict, parameter_where)
                    name = required_string(parameter, "name", parameter_where)
                    placement = required_string(parameter, "paramType", parameter_where)
                    if placement not in {"path", "query", "body"}:
                        fail(f"{parameter_where}.paramType: unsupported placement {placement!r}")
                    identity = (placement, name)
                    if identity in parameter_names:
                        fail(f"{where}: duplicate {placement} parameter {name!r}")
                    parameter_names.add(identity)
                    required = parameter.get("required", False)
                    multiple = parameter.get("allowMultiple", False)
                    if not isinstance(required, bool) or not isinstance(multiple, bool):
                        fail(f"{parameter_where}: required/allowMultiple must be booleans")
                    data_type = required_string(parameter, "dataType", parameter_where)
                    translated = schema_for_type(data_type, model_names, parameter_where)
                    apply_allowable(translated, parameter, parameter_where)
                    if "defaultValue" in parameter:
                        translated["default"] = copy.deepcopy(parameter["defaultValue"])
                    if multiple:
                        translated = {"type": "array", "items": translated}
                    description = optional_string(parameter, "description", parameter_where) or ""

                    if placement == "body":
                        if request_body is not None:
                            fail(f"{where}: more than one body parameter")
                        if multiple:
                            fail(f"{parameter_where}: body parameters cannot set allowMultiple")
                        # ARI's legacy `containers` type is not an unnamed free-form body. In every
                        # pinned occurrence the parameter name is the top-level JSON key (usually
                        # `variables`, once `fields`), which the source descriptions state
                        # explicitly. Preserve that wrapper; otherwise OpenAPI ingest has to call
                        # the whole body `body`, colliding with endpoints whose query parameter is
                        # also named `body` and making their composed input schema ambiguous.
                        body_schema = translated
                        if data_type == "containers":
                            body_schema = {
                                "type": "object",
                                "properties": {name: translated},
                            }
                            if required:
                                body_schema["required"] = [name]
                        request_body = {
                            "description": description,
                            "required": required,
                            "content": {"application/json": {"schema": body_schema}},
                            "x-ari-name": name,
                            "x-ari-data-type": data_type,
                        }
                    else:
                        translated_parameter: dict[str, Any] = {
                            "name": name,
                            "in": placement,
                            # OpenAPI 3 requires every path parameter to say `required: true`.
                            # Seven endpoint parameters omit the legacy field; the exact source
                            # value remains visible both here and in x-ari-source-parameters.
                            "required": True if placement == "path" else required,
                            "description": description,
                            "schema": translated,
                            "x-ari-data-type": data_type,
                            "x-ari-allow-multiple": multiple,
                            "x-ari-source-required": parameter.get("required"),
                        }
                        if multiple:
                            translated_parameter.update({"style": "form", "explode": True})
                        parameters.append(translated_parameter)

                response: dict[str, Any] = {
                    "description": "Success",
                    "x-ari-response-class": response_class,
                }
                if response_class == "binary":
                    response["content"] = {
                        "application/octet-stream": {
                            "schema": {"type": "string", "format": "binary"}
                        }
                    }
                elif response_class != "void":
                    response["content"] = {
                        "application/json": {
                            "schema": schema_for_type(response_class, model_names, where)
                        }
                    }
                responses: dict[str, Any] = {"200": response}
                for error_index, raw_error in enumerate(operation.get("errorResponses", [])):
                    error_where = f"{where}.errorResponses[{error_index}]"
                    error = require(raw_error, dict, error_where)
                    source_code = error.get("code")
                    if isinstance(source_code, int) and not isinstance(source_code, bool):
                        code = source_code
                    elif isinstance(source_code, str) and source_code.isdigit():
                        code = int(source_code)
                    else:
                        code = 0
                    if not 400 <= code <= 599:
                        fail(f"{error_where}.code: expected an HTTP error status")
                    reason = required_string(error, "reason", error_where)
                    if str(code) in responses:
                        fail(f"{where}: duplicate response status {code}")
                    responses[str(code)] = {"description": reason}

                summary = required_string(operation, "summary", where)
                normalized_operation: dict[str, Any] = {
                    "operationId": operation_id,
                    "summary": summary,
                    "tags": [resource],
                    "parameters": parameters,
                    "responses": responses,
                    "security": [{"ariBasic": []}],
                    "x-ari-resource": resource,
                    "x-ari-nickname": nickname,
                    "x-ari-source-document": f"api-docs/{resource}.json",
                    "x-ari-path-description": path_description,
                    "x-ari-response-class": response_class,
                    "x-ari-source-parameters": copy.deepcopy(raw_parameters),
                    "x-ari-since": copy.deepcopy(operation.get("since", [])),
                }
                notes = optional_string(operation, "notes", where)
                if notes is not None:
                    normalized_operation["description"] = notes
                    normalized_operation["x-ari-notes"] = notes
                if request_body is not None:
                    normalized_operation["requestBody"] = request_body
                lower_method = method.lower()
                if lower_method in path_item:
                    fail(f"duplicate normalized route {method} {path}")
                path_item[lower_method] = normalized_operation

    if len(source_paths) != 76 or source_operations != 109:
        fail(
            f"whole-source inventory differs: expected 76 paths/109 operations, "
            f"got {len(source_paths)}/{source_operations}"
        )
    if deferred != ["events-eventWebsocket"]:
        fail(
            "unaccounted operation classification: expected only "
            f"events-eventWebsocket, got {deferred}"
        )
    rest_operations = source_operations - len(deferred)
    emitted = sum(len(path_item) for path_item in paths.values())
    if rest_operations != 108 or emitted != rest_operations:
        fail(
            f"normalization did not account for every operation: "
            f"source={source_operations}, deferred={len(deferred)}, emitted={emitted}"
        )

    document = {
        "openapi": "3.0.3",
        "info": {
            "title": "Asterisk REST Interface (ARI)",
            "version": SOURCE_TAG,
            "description": (
                "Deterministic OpenAPI 3 normalization of Asterisk's first-party ARI "
                "Swagger descriptions."
            ),
            "license": {"name": "GPL-2.0-only"},
        },
        "servers": [{"url": BASE_PATH}],
        "security": [{"ariBasic": []}],
        "tags": tags,
        "paths": paths,
        "components": {
            "securitySchemes": {
                "ariBasic": {"type": "http", "scheme": "basic"}
            },
            "schemas": schemas,
        },
        "x-ari-source": {
            "repository": SOURCE_REPOSITORY,
            "tag": SOURCE_TAG,
            "tagObject": SOURCE_TAG_OBJECT,
            "commit": SOURCE_COMMIT,
            "swaggerVersions": ["1.1", "1.2"],
            "documentCount": 11,
            "pathCount": 76,
            "operationCount": 109,
        },
        "x-ari-deferred-operation-ids": deferred,
    }
    census: dict[str, int | list[str]] = {
        "source_paths": len(source_paths),
        "source_operations": source_operations,
        "rest_operations": rest_operations,
        "deferred": deferred,
    }
    return document, census


def render(document: dict[str, Any]) -> bytes:
    return (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode()


def expect_error(source: Path, mutate: Callable[[Path], None], fragment: str) -> None:
    with tempfile.TemporaryDirectory(prefix="asterisk-normalizer-test-") as directory:
        candidate = Path(directory) / "source"
        shutil.copytree(source, candidate)
        mutate(candidate)
        try:
            normalize(candidate)
        except SpecError as error:
            if fragment not in str(error):
                fail(f"mutation failed for the wrong reason: expected {fragment!r}, got {error}")
        else:
            fail(f"mutation unexpectedly normalized; wanted error containing {fragment!r}")


def mutate_json(source: Path, name: str, mutate: Callable[[dict[str, Any]], None]) -> None:
    path = source / "api-docs" / name
    document = json.loads(path.read_text())
    mutate(document)
    path.write_text(json.dumps(document))


def self_test(source: Path) -> None:
    normalize(source)
    expect_error(
        source,
        lambda root: mutate_json(
            root, "applications.json", lambda document: document.__setitem__("swaggerVersion", "9.9")
        ),
        "unknown source version",
    )
    expect_error(
        source,
        lambda root: (root / "api-docs" / "unexpected.json").write_text("{}"),
        "document inventory differs",
    )

    def first_parameter(document: dict[str, Any]) -> dict[str, Any]:
        for api in document["apis"]:
            for operation in api["operations"]:
                parameters = operation.get("parameters", [])
                if parameters:
                    return parameters[0]
        raise AssertionError("mutation fixture has no parameter")

    def unknown_type(document: dict[str, Any]) -> None:
        first_parameter(document)["dataType"] = "mystery"

    expect_error(
        source,
        lambda root: mutate_json(root, "applications.json", unknown_type),
        "unknown type",
    )

    def unknown_placement(document: dict[str, Any]) -> None:
        first_parameter(document)["paramType"] = "header"

    expect_error(
        source,
        lambda root: mutate_json(root, "applications.json", unknown_placement),
        "unsupported placement",
    )

    def duplicate_identity(document: dict[str, Any]) -> None:
        first = document["apis"][0]["operations"][0]["nickname"]
        document["apis"][1]["operations"][0]["nickname"] = first

    expect_error(
        source,
        lambda root: mutate_json(root, "applications.json", duplicate_identity),
        "duplicate operationId",
    )

    def lose_websocket(document: dict[str, Any]) -> None:
        del document["apis"][0]["operations"][0]["upgrade"]
        del document["apis"][0]["operations"][0]["websocketProtocol"]

    expect_error(
        source,
        lambda root: mutate_json(root, "events.json", lose_websocket),
        "unaccounted operation classification",
    )

    def unknown_upgrade(document: dict[str, Any]) -> None:
        document["apis"][0]["operations"][0]["upgrade"] = "h2c"

    expect_error(
        source,
        lambda root: mutate_json(root, "events.json", unknown_upgrade),
        "unsupported value",
    )

    def extra_operation(document: dict[str, Any]) -> None:
        document["apis"][0]["operations"].append(copy.deepcopy(document["apis"][0]["operations"][0]))

    expect_error(
        source,
        lambda root: mutate_json(root, "applications.json", extra_operation),
        "inventory differs",
    )


def write_or_check(path: Path, expected: bytes, check: bool) -> None:
    if check:
        try:
            actual = path.read_bytes()
        except FileNotFoundError:
            fail(f"missing normalized document: {path}")
        if actual != expected:
            fail(f"normalized document is stale: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_bytes(expected)
    os.replace(temporary, path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    try:
        if args.self_test:
            self_test(args.source_dir)
            print("Asterisk ARI normalizer mutation tests passed")
            return
        document, census = normalize(args.source_dir)
        write_or_check(args.output, render(document), args.check)
        action = "checked" if args.check else "wrote"
        print(
            f"{action} {args.output}: {census['source_operations']} source, "
            f"{census['rest_operations']} REST, {len(census['deferred'])} deferred"
        )
    except (OSError, json.JSONDecodeError, SpecError) as error:
        raise SystemExit(f"error: {error}") from error


if __name__ == "__main__":
    main()

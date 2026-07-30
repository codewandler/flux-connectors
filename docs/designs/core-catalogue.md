# Core catalogue and dereferenceable specifications

## Decision

Flux owns the semantic source for built-in operations and language nodes. `flux catalog core
--format json` exports a deterministic, versioned bundle. flux-connectors vendors that bundle and
publishes it in the explorer; it never constructs a second built-in registry and never executes a
Flux runtime operation.

Core records are not generated connectors. They emit no `.flux`, do not enter `connector-catalog`,
and do not affect provider or connector-operation counts. They occupy three explicit kinds:
callable `operation`, non-callable language `node`, and `capability`, whose availability is either
`available` or `planned`.

## Identity and schemas

The canonical namespace is `https://flux.codewandler.org/v1/core`. Every identity ends in `.json`,
is the `$id` of the JSON document served at that URL, and declares the versioned
`https://flux.codewandler.org/v1/schema/core-entry.schema.json`. The catalogue index and the Flux AST
projection likewise have public `$id` values below `/v1/schema/`.

The paths encode a small stable taxonomy:

- `network/application/http/request.json`
- `network/application/dns.json`
- `network/transport/{tcp,udp}.json`
- `network/internet/icmp.json`
- `data/transform/<operation>.json`
- `language/node/<kind>.json`

Invocation spelling remains separate (`http.request`, `map`). Node records point to stable
`#node-<kind>` anchors in the published strict Flux AST schema. Planned capabilities carry no
`ToolSpec`, signature, or suggested runtime operation name.

## Publication

`specs/flux/core-v1.json` is the checked, offline compiler input. Build validation checks the schema
version, canonical host/prefix, `$id` to output-path mapping, uniqueness, variant invariants, and
schema references before the site emitter writes `/v1/core/index.json`, entry documents, and the
schema documents. The public `catalog.json` includes the same bundle additively for the explorer.

The VitePress site is served from `flux.codewandler.org` at base `/`; GitHub Pages' `CNAME` is the
repository half of that deployment. External DNS configuration remains an operator action.

## Foundational scope

The available operation set is `http.request` plus the 28 pure data transforms selected in flux
C-283. All AST node kinds are exported from the language schema source of truth. HTTP is an
available capability linked to its operation. DNS, TCP, UDP, and ICMP are planned capability
records backed by separate Flux stories. `noop` is deliberately absent: an empty block, `return
null`, and `return <value>` are language semantics rather than a host operation.


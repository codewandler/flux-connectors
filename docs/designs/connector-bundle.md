# Design: the connector bundle

**Status:** proposed · **Pillar:** Codegen · **Stories:** C-39 … C-41

## Why

A connector is more than a set of callable operations. It also has **schemas** (what goes in, what
comes back), **metadata** (vendor, version, addresses, which credentials it needs, which hosts it
reaches), **branding** (an icon, in the sizes a UI needs), and **documentation**. Today only the
first of those ships: `connectors/<name>.flux` plus a thin manifest.

So the build should emit a **bundle** per provider. The open question — and the whole substance of
this design — is *where each piece lives*, and specifically how much of it belongs **inside the
`.flux` file itself**.

## Approach

### What the `.flux` file is for, and what that costs

`connectors/<name>.flux` is not a document. It is **source that flux parses at session start**:
`DynamicComposites::load` (`../flux/crates/flux-flow/src/composites.rs:97`) reads every `.flux` in
`~/.flux/flows` and lifts its `op` declarations. Every byte in that file is parsed by every flux
session that has the connector installed.

That is the constraint that decides the rest.

### Metadata: yes, via synthetic operations — not via a decl

flux modules *can* carry arbitrary structured data. `datasource` and `channel` declarations collect
every unrecognised key into a `settings` record, and setting values nest (`../flux/crates/flux-lang/docs/syntax.md:130-150`).
So a metadata block is expressible.

**But it would be an abuse.** A `datasource` decl means something to flux — it declares a
datasource — and a connector's metadata is not one. `DynamicComposites::load` ignores non-`op`
declarations, so the metadata would be inert *there*, while other flux machinery could still try to
register it. Borrowing a load-bearing declaration kind as a data container is the sort of thing that
works until it doesn't.

**The right carrier is the user's own suggestion: a synthetic operation that returns a literal
record.** It is pure — no IO, no network — and it rides the mechanism that already exists:

```flux
op zendesk-describe -> Any
  description "Describe this connector: its operations, their schemas, and its metadata"
  risk "low"
  idempotency "idempotent"
  effects []
  expose false

  return { connector: "zendesk", ... }
```

This is strictly better than a metadata decl: it is introspectable **through the same interface as
everything else**, it needs no new flux concept, and `expose false` keeps it out of the model's tool
catalog so it costs no context until something asks for it.

Two synthetic ops, not one, because they are consulted at different times and have very different
sizes: `<provider>-describe` (small: vendor, addresses, credentials, host allowlist, operation list)
and `<provider>-schema` (large: full input and output JSON Schema per operation).

### Icons: alongside, never inside

The proposal was to put the icon in the `.flux` too. **Recommendation: don't.**

A base64 PNG is a few KB per size per format; several sizes across twenty providers is hundreds of
kilobytes of base64 that **every flux session parses at startup** to reach the ops. It also makes the
diff of a code artifact unreadable, and it conflates source with assets.

Icons ship as **files in the bundle directory**, referenced by relative path from the manifest and
the markdown. A consumer that wants them has the directory; a consumer that only runs operations
never pays for them.

### The bundle is a directory, not a file

```
connectors/<provider>/
  <provider>.flux              # executable ops + the two synthetic introspection ops
  <provider>.connector.toml    # manifest: addresses, credentials, hosts, icon paths
  <provider>.md                # documentation (C-31/C-32)
  icons/                       # icon.svg + rendered PNG sizes
```

"Bundle of things" is honoured by the *directory*, with each artifact in the form its consumers
actually want. The `.flux` stays the single installable unit (settled: the final build artifact is
one `.flux` per provider), and the rest sits beside it.

### Output schemas are the weak link

`Operation::response_schema` exists but nothing populates it richly — C-9 and C-17 both flagged this.
So `<provider>-schema` will return complete *input* schemas and mostly-empty *output* schemas until
response modelling improves. Ship it saying so rather than implying a fidelity that is not there.

## Alternatives considered

- **Everything inside one `.flux`, icons base64-encoded.** The literal reading of the request. One
  file to install, nothing to lose. Rejected on parse cost at every session start and on
  unreadable diffs — but it *is* expressible, so this is a judgement call, not an impossibility.
- **Metadata in a `datasource` decl's settings.** Works today with no new mechanism, and needs no
  synthetic op. Rejected because it borrows a declaration kind that means something else.
- **A sidecar JSON file per provider.** Simplest of all, and machine-readable without flux. Rejected
  as the *primary* carrier because it is not reachable from inside a flux session — which is exactly
  where "what can this connector do?" gets asked.
- **No introspection at all; read the manifest.** Fine for tooling, useless for an agent mid-session.

## Risks & open questions

- **Every synthetic op is still an op.** `expose false` keeps it out of the model's catalog, but it
  is registered, it occupies a name in the connector's namespace, and it must obey C-23's naming
  rules and C-37's addressing like any other.
- **`<provider>-schema` may be large.** Full JSON Schema for 25 operations embedded as a literal
  record is a big return value and a big chunk of parsed source. Measure before shipping it for a
  163-operation provider; it may need to be per-group rather than per-provider.
- **Icon generation needs a rasteriser.** Rendering PNG sizes from an SVG is a build dependency this
  workspace does not have, and `connector-spec` must stay dependency-light. Likely `connector-cli`'s
  job, or shipped SVG-only at first.
- **Where do icons come from?** Vendor logos are trademarked. Shipping them in a public repo is a
  licensing question, not a technical one, and it should be answered before any are committed.
- **Bundle layout is a breaking move.** `connectors/<provider>.flux` becomes
  `connectors/<provider>/<provider>.flux`; C-13's discovery, C-27's writer and C-33's checks all
  assume the flat shape.

## Acceptance / done

- Two synthetic operations per provider — `describe` and `schema` — pure, `expose false`, and passing
  the same parse-and-analyze gate as every other op.
- Metadata is reachable from inside a flux session without reading a file.
- Icons ship as files referenced from the manifest, not embedded in source.
- The bundle directory is produced deterministically and checked for drift like every other artifact.

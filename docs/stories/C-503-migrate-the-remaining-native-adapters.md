---
id: C-503
title: "Migrate AWS, Homer, Hugging Face and web search adapters"
pillar: Connector
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [providers, runtime]
note: "close the residual inventory: SigV4/CLI-backed AWS, Homer HTTP/JWT, Hugging Face and provider-selecting web search"
---

# Migrate the remaining native adapters

## Goal

Close the official native-adapter inventory with connector replacements for AWS, Homer, Hugging Face
and web search, choosing generated HTTP or a connector-owned runtime artifact from measured protocol
needs rather than from the legacy crate shape.

## Acceptance

- [ ] AWS has an explicit decision for SigV4 versus a guarded CLI/runtime adapter; neither credential
      material nor an ambient AWS profile is silently inherited.
- [ ] Homer and Hugging Face use generated provider facts wherever their public APIs describe the
      surface, with any residual code isolated in connector runtime artifacts.
- [ ] Web search declares provider selection and result normalization without making Flux know a
      vendor or returning to a built-in vendor switch.
- [ ] Each of the four passes frozen plugin parity and local/hosted conformance where supported.
- [ ] Replacement addresses and intentional incompatibilities are published before Flux retirement.

## Progress

- (not started)

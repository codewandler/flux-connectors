---
id: C-504
title: "Project every connector runtime through the zero-IO pack"
pillar: Bridge
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [connector-pack, runtime]
note: "connector-pack currently resolves an HTTP Tool; make it produce a declared runtime plan that Flux and Exchange dispatch without either host rebuilding vendor behavior"
---

# Project every connector runtime through the zero-IO pack

## Goal

Generalize `connector-pack` from an HTTP-only resolved Tool to a zero-IO runtime plan shared by local
Flux and Exchange, while preserving the single compiled connector path and the pack's transport ban.

## Acceptance

- [ ] A closed plan vocabulary covers HTTP, socket, process, container, plugin and remote, including
      one-shot, stream and lease lifecycles from C-497.
- [ ] The pack resolves tenant-bound credential/configuration references into the host context without
      opening a transport or exposing a credential value in the returned public result.
- [ ] Both hosts consume the same plan; neither constructs a vendor request/argv/handshake alongside
      the connector implementation.
- [ ] Exhaustive compile-time matches make a new runtime kind a deliberate change in every consumer.
- [ ] Failing-first tests prove a caller cannot alter runtime, artifact, authority or credential
      through operation parameters.

## Progress

- (not started)

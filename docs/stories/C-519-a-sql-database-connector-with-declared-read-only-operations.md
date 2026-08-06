---
id: C-519
title: "A SQL database connector with declared read-only operations"
pillar: "Core"
status: backlog
epic: database-datasources
design: database-datasources
note: "Decisions 0006 and 0008: PostgreSQL first; declared connection form with a file-shaped secret and the private destination class; read-only query and schema introspection under the rich-runtime plan"
---

# A SQL database connector with declared read-only operations

## Goal

The catalogue's fifty-plus connectors are all HTTP vendor APIs; no connector can read a database.
This story declares the first SQL connector — PostgreSQL first — as ordinary connector contract:
a declared connection form (host, port, database, user, TLS mode) whose credential is a
file-shaped secret and whose destination uses the private/local destination class from
Decision 0008, plus declared read-only operations (parameterized query with an enforced row/byte
bound, schema introspection) executing under the rich-runtime plan. No write-capable operation is
declared in this story.

## Acceptance

- [ ] The connection declaration renders through the generated form machinery with every
      non-secret field typed and the password/DSN as a file-shaped secret; nothing is collectable
      from argv or environment.
- [ ] Declared operations cover bounded parameterized reads and schema introspection; the
      declaration marks the surface read-only, and a write statement is refused by contract tests
      rather than by convention.
- [ ] The destination declaration uses the Decision 0008 private destination class so an Exchange
      may pin the admitted database address; no operation can name a destination.
- [ ] Conformance fixtures freeze the surface against a hermetic PostgreSQL double, following the
      established legacy-versus-Exchange fixture format.
- [ ] Depends on the Decision 0008 vocabulary and rich-runtime contract stories; not dispatchable
      before them.

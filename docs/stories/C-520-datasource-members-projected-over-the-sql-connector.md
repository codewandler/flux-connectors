---
id: C-520
title: "Datasource members projected over the SQL connector"
pillar: "Core"
status: backlog
epic: database-datasources
design: database-datasources
note: "Decision 0006 rule 6: datasource members as projections over the declared operations; schema, list and get fixtures for the Exchange read seam"
---

# Datasource members projected over the SQL connector

## Goal

Decision 0006 rule 6 makes a datasource member a projection over a connector's declared
operations. This story declares the SQL connector's datasource members — tables/views as
enumerable record surfaces with declared schemas, list and get backed by the C-519 read
operations — so an Exchange tenant binding (`exchange/X-131`…`X-133`) can serve them through the
governed read seam and a Flux program can bind them by connection label.

## Acceptance

- [ ] Datasource members declare their record schema, list pagination (Exchange-minted opaque
      cursors) and get-by-key as projections over declared operations; credentials appear only as
      the backing operation's declared auth, never a value.
- [ ] The member declaration validates against the published datasource-member vocabulary that
      `exchange/X-131` consumes; an invalid projection is a contract-test failure in this
      repository, not an Exchange runtime surprise.
- [ ] Fixtures prove schema, list and get against the hermetic PostgreSQL double, reusable by the
      Exchange read-seam suite.
- [ ] Depends on C-519 and on the datasource-member vocabulary line; not dispatchable before them.

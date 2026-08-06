# Design — Database datasources

## Why

"A datasource pointing at a database" is the single most requested read surface a deployment can
declare, and today it cannot exist: every catalogue connector is an HTTP vendor API, so
Decision 0006's family test — a named, declared, read-only record surface — has no SQL member to
project. The hosted single-org journey (Decision 0019) ends at exactly this gap: the deployment
declares the connection and the tenant binding, and then there is nothing behind them.

## Approach

Ordinary connector contract, nothing bespoke: a PostgreSQL connector declares its connection form
with a file-shaped secret and the Decision 0008 private destination class, declares bounded
read-only query and introspection operations under the rich-runtime plan, and then declares
datasource members as projections over those operations per Decision 0006 rule 6. Exchange's
governed read seam (`exchange/X-131`…`X-133`) and Flux's embedded-client binding consume the
result unchanged. Write-capable SQL, other engines and streaming reads are explicit follow-ups,
not scope creep here.

## Stories

- C-519 — a SQL database connector with declared read-only operations
- C-520 — datasource members projected over the SQL connector

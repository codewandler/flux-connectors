---
id: C-154
title: "Add YouTube as a fourth Google service"
pillar: Spec
status: ready
priority: 4
design: docs/designs/provider-services.md
epic: provider-fleet-2
areas: [providers]
note: "google already publishes gmail, calendar and drive on three different hosts — the multi-service case C-49 was built for. YouTube is a fourth, and it is what makes C-153's `social` tag have a second value to filter on"
---

# Add YouTube as a fourth Google service

## Goal

Publish a `youtube` service on the `google` connector, so the YouTube Data API is reachable and
[C-153](C-153-service-tags.md)'s tag vocabulary has a genuinely different kind of surface to sort.

## Why here rather than as its own connector

`google` is already the repo's multi-service showcase and the reason C-49 exists: `gmail`, `calendar`
and `drive` share an authority and **not** a host — `gmail.googleapis.com` versus
`www.googleapis.com` — which is exactly what per-service `base_url` was added for. YouTube is the same
shape (`youtube.googleapis.com`), so it belongs beside them rather than as a nineteenth-plus connector.

It is also the case that makes tagging worth anything: gmail is office, youtube is social. Two tags
with one service each is a taxonomy; two tags across four services is a filter.

## Acceptance

- [ ] A `youtube` service on `google`, with its own `base_url` and `api_version`, and **operations
      selected rather than mechanically enumerated** — `vision.md`: "Mechanically emitting all 400
      endpoints of a large spec produces an unusable tool catalog."
- [ ] Declared `risk` and `idempotency` chosen deliberately. A read of public video metadata is not the
      same as anything that touches a channel a user owns; if a write is included, its risk must say so.
- [ ] **Quota is the thing to model or exclude honestly.** The YouTube Data API bills a per-operation
      *quota cost* (a search costs far more than a video read) against a daily allowance. There is no
      IR field for that today. Either record it in each operation's `description`, or say in Progress
      that quota is unmodelled — do not let a caller discover it by exhausting the day's budget.
- [ ] Generated Flux parses, analyzes and is a **fixed point of flux's own formatter** — the standing
      per-provider gate.
- [ ] No credential value anywhere, and **no realistic-looking `example` on a secret field** — a
      token-shaped placeholder has tripped GitHub push protection and blocked a release here before.
- [ ] The build stays a fixed point and the gate is green.

## Notes

- **Auth is the first thing to settle, and it may block.** Gmail, Calendar and Drive are OAuth2
  user-scoped; YouTube Data has both an **API-key** path (public reads) and an OAuth2 path (anything
  touching a user's channel). The API-key path is simple and probably where to start. But
  [C-88](C-88-prove-oauth2.md) records that `OAuth2Spec` is a landed type **no shipped provider uses**,
  so if this service needs the OAuth2 path it inherits that gap — say so rather than half-declaring it.
- **`const_headers` is available now** (C-55), so a required constant header emits as a literal rather
  than a caller-overridable argument. Check whether YouTube needs one before assuming it does not.
- Whole-catalogue artifacts are coordinator-owned since C-104. Use `build --provider google` as your
  gate. Note this is a *change to an existing provider*, so it leaves **three** tests red, not the
  eight a new provider leaves — `AGENTS.md` tabulates both.
- Coordinate with [C-153](C-153-service-tags.md): it tags this service `social`. Either can land first;
  if tags land first, this story adds the tag with the service.
- `google.toml` is a large file with real prose about the three-host split. Read it before adding a
  fourth surface — the reasoning there is what keeps `http_hosts` honest.

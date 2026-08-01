---
id: C-438
title: "35 of 54 provider files already carry a vendor documentation URL — in a comment"
pillar: Spec
status: backlog
priority: 3
design: docs/designs/connector-presentation.md
epic: connector-presentation
areas: [providers]
note: "the epic's cheapest win and its best evidence: an author already went and found these links. They sit in prose where no artifact can reach them, which makes this a declaration problem rather than a research one"
---

# 35 of 54 provider files already carry a vendor documentation URL — in a comment

## Goal
Turn the documentation links already sitting in provider comments into declared resources, so the
work an author already did reaches a listing.

## The measurement

`grep -lE 'https?://(developer|docs|api)\.' providers/*.toml` matches **35 of 54** files. A sample of
what is in there: `api.bitbucket.org/2.0`, `api.calendly.com/scheduled_events/`,
`api.clickup.com/api/v2`, `api.cloudflare.com/client/v4`, `api.contentful.com/spaces/`,
`api.machines.dev/v1`.

Someone found each of those, judged it worth writing down, and wrote it where nothing can publish it.

## Acceptance
- [ ] Every provider file's comment URLs are **reviewed** and the ones that are genuinely a resource
      become `[[resources]]` entries (C-436's declaration).
- [ ] **Lifted by review, never by script.** A comment is prose written for a human: a URL in one may
      be the vendor's documentation, a citation supporting a decision, an example of a *bad* endpoint,
      or a caveat about a deprecated path. The comments are the **input**; the judgement is the work.
      A regex sweep would declare citations as resources.
- [ ] A URL that is a **citation rather than a resource stays a comment**, and the story says roughly
      how many fell that way — that number is the evidence that the review was real.
- [ ] The `kind` set C-436 declares is **checked against what these 35 actually point at**, and any
      kind the corpus needs but the set lacks is reported rather than forced into the nearest match.
- [ ] Providers with no such comment are left alone and are not worse for it.
- [ ] Artifacts regenerate; the count moves and is stated.

## Progress
- (not started)

## Notes
- **Blocked on [C-436](C-436-connector-resources.md)** — there is nothing to lift these into yet.
- This is the story that tells C-436 whether its closed `kind` set is right, so the two are best read
  together even though they land in order.
- Watch for a URL in a comment that is deliberately *not* a resource — `providers/babelforce.toml`
  and `providers/twilio.toml` both cite vendor documentation to justify a decision, and citations are
  the thing this story must not promote.

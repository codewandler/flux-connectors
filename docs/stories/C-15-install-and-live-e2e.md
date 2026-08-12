---
id: C-15
title: Install into flux and prove milestone 1 end to end
pillar: Build
status: done
design: docs/designs/connectors-v1.md
epic: connectors-v1
areas: [connector-cli, flux-bridge]
note: "CLOSED 2026-08-12 as superseded by flux-roadmap Decision 0022 (adopted by C-535), never implemented. Flux never grows a connector module loader, so there is nothing for an installer to install into ~/.flux/flows; `install` still exits with an error pointing here, which now reads as honest history. The live proof this story wanted happened on the host path instead (connectors-api, C-200)"
---

# Install into flux and prove milestone 1 end to end

## Goal
Close the loop: install generated connectors into a real flux and demonstrate that Zendesk and
Anthropic ops register, appear as LLM tools, and call the live APIs successfully.

## Acceptance
- [ ] `flux-connectors install` writes `<provider>.flux` to `~/.flux/flows/` and
      `<provider>.connector.toml` to `~/.flux/connectors/`, with `--dry-run` and an explicit summary
      of the capabilities each manifest declares.
- [ ] Install never silently widens capabilities — installing a connector is a trust decision equal
      to installing a plugin, and the output must say what is being granted.
- [ ] A live `flux` session lists `zendesk.ticket.show` and `anthropic.messages.create` among its
      ops.
- [ ] One live API call succeeds for each provider, authenticated through the `$auth` seam with no
      pre-composed credential anywhere.
- [ ] The Zendesk connector covers the operation set of `../flux/plugins/zendesk/src/main.rs`,
      substantiating the plugin-replacement claim.

## Progress
- **2026-08-12 — closed as superseded by Decision 0022
  (`../flux-roadmap/decisions/0022-connectors-compile-to-a-catalog-artifact.md`), adopted by
  [C-535](C-535-adopt-decision-0022.md). Nothing above was implemented, and the acceptance boxes
  stay unticked deliberately** — that is the honest history (the C-496 pattern). This story's
  premise was flux loading installed `.flux` modules from `~/.flux/flows` through the `$auth` seam;
  Decision 0022 rule 5 states Flux never grows a connector module loader, C-10 is closed with this
  story, and the compiled form of a connector becomes a catalog artifact
  ([C-534](C-534-catalog-artifact-epic.md)). What it wanted proven arrived on the host path
  instead: operations register as flux Tools through `connector-pack`, and the first live,
  authenticated vendor call is recorded in `crates/connectors-api/README.md` §"The live leg,
  performed and labelled". `flux-connectors install` keeps its explicit error pointing here rather
  than gaining best-effort behaviour; whether it acquires a catalog-artifact meaning is C-534's
  program to decide.

## Notes
- **Superseded without implementation** by Decision 0022 via [C-535](C-535-adopt-decision-0022.md);
  see Progress. The notes below are kept as written — they describe the module-loading world this
  story was filed for, and the `done` status records the close, not delivery.
- **Blocked on `C-16`**: the live run needs the `$auth` seam shipped in a `../flux` release. Every
  other story in the epic is independent of it, so this is the only place the cross-repo dependency
  bites.
- Anthropic uses a raw `x-api-key` header, which works with flux's existing `$secret` marker — it can
  therefore go green *before* the seam lands. Zendesk (Basic) cannot.
- Record the flux version this was verified against.

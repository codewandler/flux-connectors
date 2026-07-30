---
id: C-15
title: Install into flux and prove milestone 1 end to end
pillar: Build
status: backlog
design: docs/designs/connectors-v1.md
epic: connectors-v1
areas: [connector-cli, flux-bridge]
note: milestone 1 · needs the $auth seam released in ../flux
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
- (not started)

## Notes
- **Blocked on `C-16`**: the live run needs the `$auth` seam shipped in a `../flux` release. Every
  other story in the epic is independent of it, so this is the only place the cross-repo dependency
  bites.
- Anthropic uses a raw `x-api-key` header, which works with flux's existing `$secret` marker — it can
  therefore go green *before* the seam lands. Zendesk (Basic) cannot.
- Record the flux version this was verified against.

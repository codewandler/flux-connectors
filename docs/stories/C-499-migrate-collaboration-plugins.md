---
id: C-499
title: "Migrate the Confluence, GitLab, Jira and Slack plugins"
pillar: Connector
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [providers, runtime, channels]
note: "four catalogue providers already exist, but operation parity, Slack Socket Mode and a measured cutover from the native crates do not"
---

# Migrate the collaboration plugins

## Goal

Make the Confluence, GitLab, Jira and Slack connectors complete Exchange-executed replacements for
their native Flux plugins, including Slack's long-lived Socket Mode, then retain the proof Flux needs
to delete the four integration crates.

## Acceptance

- [ ] Freeze and compare every current plugin operation, schema, effect, refusal and event against the
      corresponding connector; every difference is implemented or recorded as an intentional
      breaking removal.
- [ ] Ordinary request/response operations use generated connector definitions rather than copied
      Rust request construction.
- [ ] Slack Socket Mode uses the declared socket/runtime binding and the generic connector channel;
      no Slack-specific channel arm remains necessary (coordinates with C-489…C-492 and Flux D-220).
- [ ] C-505's frozen legacy-plugin-versus-Exchange fixtures produce equivalent observable results,
      declared errors and lifecycle behavior for the supported surface.
- [ ] Published migration notes identify replacement operation addresses before Flux removes the
      native crates.

## Progress

- (not started)

## Notes

- This is migration wave 1. It begins only after C-505 establishes the shared inventory and
  conformance ratchet; later waves consume the evidence added here.

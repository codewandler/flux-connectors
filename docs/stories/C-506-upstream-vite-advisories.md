---
id: C-506
title: "Upgrade the public-site toolchain past the upstream Vite advisories"
area: Surfaces
status: blocked
areas: [web, release, supply-chain]
note: "BLOCKED upstream — VitePress 1.6.4 resolves Vite <=6.4.2 and esbuild <=0.24.2; npm reports two moderate and one high development-server advisory with no fix available"
---

# Upgrade the public-site toolchain past the upstream Vite advisories

## Goal

Remove the known Vite and esbuild development-server advisories from the public documentation
toolchain as soon as VitePress exposes a non-vulnerable dependency line.

## Acceptance

- [ ] A released VitePress version resolves Vite and esbuild versions outside all three reported
      advisory ranges without an override that splits the supported toolchain.
- [ ] `npm audit` reports no high or moderate vulnerability in `web/`.
- [ ] The public-site build and rendered-content tests pass after the upgrade.

## Progress

- 2026-08-03 — `npm audit --json` reports VitePress 1.6.4 transitively resolving Vite `<=6.4.2`
  and esbuild `<=0.24.2`: two moderate advisories and one high Windows alternate-path
  `server.fs.deny` bypass. npm reports `fixAvailable: false` for all three dependency records.

## Notes

- The affected paths are development-server behavior. The release publishes the statically built
  documentation from Linux and does not expose that server, so this recorded upstream block does not
  stop v0.18.0. It must not be suppressed or treated as resolved.

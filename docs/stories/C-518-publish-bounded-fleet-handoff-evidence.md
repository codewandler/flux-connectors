---
id: C-518
title: "Connector story workers publish bounded Fleet handoff evidence"
pillar: Bridge
status: ready
priority: 1
areas: [agents, scripts, tests]
note: "Fleet dogfood — one deterministic targeted-check receipt without terminal scraping or whole-gate duplication"
---

# Connector story workers publish bounded Fleet handoff evidence

## Goal

Give an isolated native Fleet story worker one repository-owned way to run targeted checks and
return a small deterministic receipt that the Fleet handoff can verify before the final wave gate.

## Acceptance

- [ ] Failing first, a self-test demonstrates that today's free-form command transcript can grow
      without bound and does not identify the exact commit, story, check set or terminal result.
- [ ] A repository script accepts an explicit story id and a closed targeted-check profile, runs
      only that profile, and emits one versioned JSON receipt with commit, dirty-state verdict,
      commands, exit outcomes, durations and a bounded diagnostic summary.
- [ ] The script never infers or changes Board status, creates a branch/worktree, runs the full
      repository gate, pushes, publishes, releases or communicates through tmux. Unknown profiles
      and a dirty or mismatched checkout refuse before checks run.
- [ ] Receipt size has a hard tested ceiling. stdout/stderr are summarized atomically with byte
      counts and digests; no byte slicing can produce invalid JSON or retain an unbounded vendor
      fixture, generated catalogue or compiler trace.
- [ ] A hermetic self-test covers pass, fail, timeout, oversized output, unknown profile and dirty
      checkout behavior without network or provider credentials.
- [ ] Public contributor guidance names the receipt as targeted story evidence only; one full
      repository gate still runs at the integrated wave boundary.

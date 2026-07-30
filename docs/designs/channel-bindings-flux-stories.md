# Handoff ledger: the flux stories filed for the generic `connector` channel kind

> **This file is a handoff record, not a tracked backlog.** Nothing in it is a story on *this* repo's
> board, and `/track:board` must never pick these up. Unlike the earlier handoffs
> ([auth-seam-flux-stories.md](auth-seam-flux-stories.md),
> [inbound-events-flux-stories.md](inbound-events-flux-stories.md)), these stories are **already
> filed** — this is a ledger of what was written and where, not a set of blocks awaiting a paste.
>
> Source design: [connector-channel-seam.md](connector-channel-seam.md) · Parent story:
> [C-84](../stories/C-84-flux-connector-channel-seam.md) · Epic:
> [C-82](../stories/C-82-channel-bindings-epic.md)

## What was filed

Six files, created **uncommitted** in `/home/timo/projects/flux/docs/stories/` on **2026-07-30**, epic
slug `connector-channels`, pillar `Agent`, status `backlog`. flux's board
(`docs/stories/README.md`) was **not** touched; whoever owns that tree regenerates it.

| id | title | depends on |
|---|---|---|
| `D-215` | The generic `connector` channel kind — one arm instead of one adapter per vendor (**epic**) | — |
| `D-216` | `build_channels` gains one `connector` arm — a manifest binding, with every rule a load error | `C-291`, this repo's C-83 |
| `D-217` | A channel can call an operation — `Deliverer::call_operation` through the full safety envelope | — |
| `D-218` | The binding's reply is the connector Tool pack's operation — delete the hand-built `chat.postMessage` | `D-216`, `D-217`, this repo's C-115/C-117 |
| `D-219` | Who may trigger this agent stays operator config — allow-lists keyed on the binding's payload symbols | `D-216` |
| `D-220` | Socket Mode becomes a transport under the binding driver — the last 40 lines of the Slack adapter | `D-218` |

```
D-216 ──┬── D-218 ── D-220
        └── D-219
D-217 ──┘
```

`D-216` and `D-217` are independent of each other and can run in parallel. `D-218` needs both.

### Why the `D-` series, and why `Agent`

Channels are flux's `D-` line: `D-04` (event-trigger channels), `D-09` (agentic channel target),
`D-203`…`D-213` (meeting rooms, which add a `room` channel kind), `D-214` (the connector Tool pack
repoint). All carry `pillar: Agent`. Filing these as `C-` would have separated them from every story
they extend.

## What was *not* filed, and why

**The inbound transport primitive.** While this design was being written, flux's board gained
`C-291` … `C-295` (epic `verified-webhook-channel`) — the stories
[C-64](../stories/C-64-flux-webhook-seam.md) owns, filed from
[verified-webhook-seam.md](verified-webhook-seam.md):

| flux id | what it gives |
|---|---|
| `C-291` | raw request bytes captured before parsing, plus a declared `verify` block |
| `C-292` | one parameterized HMAC — constant-time, replay-bounded, vendor test vectors |
| `C-293` | the endpoint challenge/handshake, answered without waking an agent |
| `C-294` | discriminator → `"<channel>.<event>"` trigger label, exact matching kept |
| `C-295` | the delivery envelope — an id and a `verified` flag a payload cannot forge |

`D-215`'s epic **depends on all five and duplicates none of them.** The division is worth stating
because it is the thing most likely to be re-litigated: `C-291`…`C-295` let an **operator hand-write**
those parameters in a program; `D-216` supplies the same parameters **from a published manifest**. One
verifier, one router, one envelope — two declaration sources, and the manifest source is the one that
cannot be weakened by hand.

Had those five not been filed first, this handoff would have carried them, and the earlier plan said
so. They were, so it does not.

## Verifying this ledger

Story ids move. Re-check before treating any number here as current:

```bash
ls ../flux/docs/stories | grep -oP '^D-\d+' | sort -t- -k2 -n | tail -1   # was D-220 when filed
ls ../flux/docs/stories | grep -oP '^C-\d+' | sort -t- -k2 -n | tail -1   # was C-295 when filed
grep -l 'epic: connector-channels' ../flux/docs/stories/*.md              # the six above
```

Every `path:line` cited inside those six stories was read in `/home/timo/projects/flux` at workspace
version **0.40.0**, commit **`2abd0a13`**, on **2026-07-30**. Symbol names are stable and line numbers
are not — re-grep by symbol.

## What this repository owes in return

Filed as findings in [connector-channel-seam.md](connector-channel-seam.md) §"What this repository
owes"; none of them belongs to C-84, and all four bound how far the flux epic can get:

1. `EventDecl::when` cannot express **absence**, so Slack's `bot_id`/`subtype` loop guard is not
   reproducible from a binding — `D-218` ships `app_mention` only because of it.
2. `providers/slack.toml` declares that guard in `schema` rather than `when`.
3. `ChannelBinding` has no `challenge`, so `C-293`'s hook has no manifest parameters — which is the
   whole reason `D-220` (Socket Mode) is a child of the epic rather than an optional extra.
4. The loader should refuse a **body-sourced** verification timestamp, as `C-291` does: honouring one
   would require parsing before verifying.

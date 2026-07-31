<!--
  WHATS-NEW.md — the CUSTOMER changelog. Audience: people who USE flux-connectors, not people
  who build it. Voice rules:
    - Plain language, feature-first. Say what the user can now do or what behaves
      differently — never how it is implemented.
    - NO story IDs, NO crate names, NO internal jargon (engineering detail lives in
      CHANGELOG.md).
    - Per release, use only the sections that apply: "### New", "### Improved",
      "### Fixed", "### Action needed" (breaking or attention-worthy changes).
  Started at 0.8.0; earlier releases are described in CHANGELOG.md only.
-->

# What's new in flux-connectors

## [Unreleased]

## [0.8.0] — 2026-08-01

### New

- **You can now run the app without registering anything with Google.** Start it with `--dev` and a
  single button signs you in as an obviously-fake developer account, so you can browse the
  connectors, paste a credential and make a real call in about a minute. Without that flag the
  developer door does not exist at all — it is not hidden or disabled, it is simply not there — so
  it cannot be reached by accident on a machine you did not mean to open up.

- **Credentials survive a restart.** Until now everything you pasted lived in memory and disappeared
  when the process stopped, so wiring up a connector was work you had to redo every time. They are
  now kept in a file that only your user account can read, in a directory only your user account can
  enter, and the app refuses to start if either has been loosened rather than quietly tightening it
  behind your back. **These credentials are not encrypted** — anything running as you, or any backup
  of your home directory, can read them. The app says so on startup rather than leaving you to
  assume otherwise.

- **The connector list tells you what still needs your attention.** Each connector now says whether
  it is ready, partly ready, or needs nothing from you at all — that last one used to look identical
  to "you have not set this up yet", which sent people hunting for a token that does not exist. And
  it counts per operation, so supplying the one key that most operations use marks those operations
  usable instead of waiting for a second key that only the admin endpoints need.

- **Eight more services**: Bitbucket, Mailchimp, Klaviyo, Supabase, Resend, Discord, Confluence and
  New Relic. That is 53 services and 299 operations in total.

### Fixed

- **Requests now identify this software to the vendor.** Every outgoing call previously went out
  anonymously, and at least one vendor refuses those with an error that says *authorization* — so a
  perfectly good key looked like a bad one, and the natural reaction was to rotate a key that was
  never the problem.

- **A connector setting you paste can no longer send a request somewhere you did not intend.** Some
  connectors ask you for part of the web address — your workspace name, your account's region. A
  value with the wrong punctuation in it could push the request to a different host entirely while
  still looking correct. Those values are now checked at the moment they are used, not just when
  they are first entered.

- **Signing in is bound to the browser that started it.** A sign-in begun in one browser can no
  longer be completed in another, which previously made it possible to trick someone into
  finishing a sign-in that landed them in an account that was not theirs — and every credential they
  then pasted would have gone with it.

- **Two connectors described their own behaviour incorrectly.** Operations that are genuinely safe
  to repeat now say so, and say *why* — so anything deciding whether to retry a failed call has an
  accurate answer instead of a cautious guess.

### Action needed

- **If you build against these packages, this release contains breaking changes.** Provider
  definitions that put a placeholder value on a secret field are now rejected when they load —
  previously they were accepted, and a realistic-looking placeholder is the exact thing that gets
  mistaken for a real credential. If you write your own provider definitions, remove any example
  value from a field marked secret. Rust code that constructs the operation type directly will also
  need updating; nothing else in the published interface changed.

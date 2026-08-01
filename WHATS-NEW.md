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

## [0.9.1] — 2026-08-01

### Action needed

- **Four operations that handed you a credential have been withdrawn, across Zoom and Postmark.**
  Creating or reading a Zoom meeting returned a start link that embeds the host's own token — anyone
  holding it starts the meeting as the host. Reading a Postmark server returned that server's live API
  tokens in plain text. Both are gone.

  They will come back when the platform can hand you a *reference* to a secret instead of the secret
  itself. Postmark's account-level surface goes with them, along with the account token it asked you
  to supply — there is nothing left that needs it, and a connector should not ask for a credential it
  cannot use.

- **Four babelforce operations have been withdrawn, and one class of them will keep being withdrawn.**
  The three OAuth endpoints and the account-details call are gone. The OAuth ones describe *how to log
  in* — that is something the platform does for you, not an operation you call — and the account call
  returned live API credentials in its reply.

  If you were calling any of them, there is no replacement and that is deliberate: an operation whose
  answer contains a password, a token or a key is one we will not ship until the platform can hand you
  a reference to the secret instead of the secret itself.

## [0.9.0] — 2026-08-01

### New

- **The whole babelforce API is now available — 391 operations, up from nine.** Everything the
  babelforce SDK reaches, this connector now reaches: agents, calls, sessions, queues, campaigns,
  routing, task automation, scheduling and the rest. You call any of them by name.

  Nine of them are offered to an AI assistant as tools, exactly as before. The other 382 are
  available to your own code but are deliberately not put in front of a model — handing an assistant
  nearly four hundred tools makes it worse at choosing between them, and a good many of those
  operations delete things.

  One operation is deliberately missing: the endpoint that mints an access token. Its reply *is* a
  credential, and this platform cannot yet guarantee such a reply never lands somewhere it should not
  — so it is withheld until it can, rather than shipped with a warning attached.

- **A connector can now be built from a vendor's own API description instead of written by hand.**
  Point it at the published specification, say which operations you want, and the rest — parameters,
  types, response shapes, descriptions — comes from the vendor. babelforce is the first connector
  built this way: 391 operations described in 751 lines, where writing them out by hand would have
  taken several thousand and would have been out of date the day the vendor changed something.

### Improved

- **Operations can be available without being offered to an AI assistant.** Previously every
  operation a connector published was also a tool a model could pick up. Those are now separate: a
  connector can carry its full API surface for your code to call, while the assistant sees only the
  handful you meant it to see.

- **You can leave out parameters a vendor documents but nobody wants.** Some endpoints publish dozens
  of near-identical filters — one babelforce reporting call offers thirty-eight, eighteen of them
  duplicates under different names. You can now name the ones to drop, so the operation stays usable
  instead of arriving with thirty-eight arguments.

- **Every build now records what produced it.** A lockfile captures the exact inputs behind each
  generated file, so if a vendor changes their API or a file drifts out of step, it is detected
  rather than quietly absorbed.

- **The connector browser no longer claims a connector lacks something when the information simply
  was not published.** A catalogue that carries less detail used to show missing pieces as red
  warnings on every card — a statement about the connector rather than about the catalogue. Those are
  now told apart.

### Action needed

- **If you use these packages directly, one name has changed.** The credential-address types moved
  into a smaller package of their own so that installing them no longer pulls in the whole compiler.
  If you depended on that vocabulary through the secrets package, nothing changes. If you named the
  compiler package to get it, depend on the address package instead.


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

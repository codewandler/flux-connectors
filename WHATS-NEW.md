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

## [0.23.0] — 2026-08-12

### New

- **Requests are now built from the catalogue's own documents.** The library that assembles a
  connector call no longer reconstructs it by re-reading generated source code: it reads the same
  validated per-connector document the catalogue publishes, and an automated comparison proves —
  for every one of the 835 operations — that what goes on the wire is byte-for-byte identical to
  what was sent before. A new lightweight library exposes that request plan as plain data, so a
  host can inspect exactly what would be sent, with secrets always shown redacted.
- **The catalogue file is now downloadable from every release page, with its checksum beside
  it.** You no longer need any of our tooling to obtain the full connector catalogue: fetch the
  file and its checksum from the release, verify the checksum, and read it — from any language.
  Each release's file is attached automatically and is refused, loudly, if it does not match what
  that release's own records say it should be.

## [0.22.0] — 2026-08-12

### New

- **Every connector is now also published as one machine-readable document.** For each provider
  there is a single validated JSON file describing everything the connector can do — every
  operation with its exact request shape, the credentials and settings an operator must supply,
  the events it can receive, and its safety annotations — readable from any language without
  installing anything of ours. Nothing you use today changes; the documents are additional, and
  the tooling that consumes them directly is on its way.
- **The whole catalogue now travels as one compact, integrity-checked file.** Everything those
  per-connector documents describe is also compiled into a single file, together with a small
  reader library that serves it. A program can carry the entire catalogue with it, prove the copy
  is intact before trusting a single entry, and look up any connector or operation instantly —
  including a catalogue newer than the program itself, which is refused loudly if it is damaged
  or from an incompatible future version rather than half-read. Catalogue updates stop being tied
  to code updates; nothing you use today changes.

## [0.21.0] — 2026-08-12

### New

- **GitLab can sign a person in, instead of asking them to paste a token.** A GitLab connection can
  now be authorized by the person using it, so calls are made with their own permissions rather than
  a shared credential's. The older arrangement still works unchanged: an organisation that prefers to
  provide one access token for everyone can keep doing exactly that. A connection chooses one of the
  two; both are offered.
- **A self-managed GitLab is asked for its address once.** Point a connection at your own GitLab and
  both its API and its sign-in pages follow that single approved address together. There is no second
  place to fill in, and no way for the two to end up pointing somewhere different.
- **GitHub and GitLab can now be browsed rather than guessed at.** You can list the organisations a
  connection can reach, the repositories inside them, and — on GitLab — the groups and projects
  available to you, complete with the address you would clone from. Until now every call needed the
  owner, repository or project identifier supplied up front, with nothing to look them up with.
- **GitHub connections have a Test connection check.** It reports which account the configured token
  is acting as, so a settings page can confirm a connection works before anything depends on it.
- **A GitLab connection asks for the callback address your deployment serves.** Registering a
  GitLab application gives you an application id, a secret and a redirect address; all three are now
  collected together during setup. Previously only a callback on the same machine could be described,
  so a hosted installation had nowhere to record the address it actually serves and only found out
  when a sign-in attempt was rejected.
- **A connection can say whose permissions a credential carries.** Credentials that act as the
  integration itself are now distinguishable from credentials that act on behalf of a specific
  person. Where that has not been reviewed yet, it says so plainly rather than guessing — so nothing
  is quietly treated as more or less privileged than it is.

### Action needed

- **If you build against the credential or configuration data directly, two additions need
  handling.** Credentials now describe whose permissions they carry, configuration fields can name
  additional services they apply to, and credentials can describe a sign-in flow. Code that lists
  every possible credential type, or constructs these entries by hand, needs updating to account for
  them. Reading the data is unaffected.

## [0.20.0] — 2026-08-04

### Action needed

- **Stop every process using the old local credential store before upgrading.** The new release
  prevents two current processes from writing one store, but older processes do not participate in
  that protection. Quiesce every 0.19.1 process before the first 0.20 open: an already-open legacy
  process can rewrite its cached v1 image and erase v2 recovery state. Once a transaction migrates
  the file, a newly started 0.19.1 process safely refuses its newer format.

### New

- **Hosts can recover credential changes coordinated with their own metadata after a crash.** One
  complete credential update can be prepared invisibly, committed atomically or abandoned without
  resurrection. Explicit generation retirement keeps recovery history bounded, and the durable
  local store rejects a second writer for its lifetime.

### Fixed

- **Dependency examples and documentation links now point to the published packages.** Copying an
  installation snippet or following API documentation no longer lands on an unrelated or missing
  package name.

## [0.19.1] — 2026-08-04

### Fixed

- **Durable local credentials now work on Windows as well as Linux and macOS.** New state is
  restricted to the current operating-system user, while existing files or directories with unsafe
  ownership, permissions or links are refused without changing them. A store placed directly under
  a shared directory now tells the operator to choose a private child or per-user state location;
  it no longer suggests changing the shared directory itself.

## [0.19.0] — 2026-08-04

### Action needed

- **Public catalogue JSON now uses schema 3.** The `auth.oauth2` value is the complete OAuth
  declaration instead of a boolean, so consumers must read its scopes, grants and endpoint fields
  from the new object shape. Rust code that constructs a configuration-field value directly must
  also provide its closed approval policy.

### New

- **Connection forms can now be generated from the complete connector declaration.** Settings,
  help, validation, defaults, credentials and the safe Test-connection operation are available to
  browser and command-line onboarding without maintaining a second vendor-specific field list.

- **GitLab connections can use an operator-approved self-managed HTTPS origin.** GitLab.com still
  works without configuration. A custom installation remains inactive until an operator approves
  the exact origin, while the connector retains control of the `/api/v4` path and refuses unsafe or
  replaced proposals without exposing their values.

### Improved

- **The integration roadmap now has one execution boundary.** Official integrations will run through
  Exchange, while Flux contains the built-in client that reaches them. Connector definitions and
  runtime artifacts stay here, vendor credentials stay in Exchange, and migration evidence is
  required before each existing adapter is removed. This corrects the plan; it does not change which
  integrations run today.

## [0.18.0] — 2026-08-03

### Improved

- **Hosts can keep several connections to the same integration separate.** They can enumerate a
  tenant's connection addresses, select one stable instance for both credentials and configuration,
  and move a connection's secrets as one atomic change. A secret backend that cannot guarantee the
  operation refuses it instead of leaving a partial migration.

- **Operation consequences are more precise.** Connectors can now declare semantic effects such as
  moving money independently of the host resource they touch. Stripe capture and refund declare a
  money effect, and capture is now correctly treated as destructive for grant and confirmation
  policy.

- **The roadmap now has one home for every integration.** Rich systems such as Docker, Kubernetes,
  SQL, and Prometheus will move to connectors that can run locally or through Exchange. This release
  changes the plan, not the integrations available today.

### Fixed

- **Query values are encoded without changing their meaning.** Explicit `false` and `0` values are
  preserved and scalar values cannot reshape a request URL. Operations whose vendors require an
  undeclared collection encoding stay out of the catalogue until that wire format can be expressed
  safely.

## [0.17.0] — 2026-08-03

### Action needed

- **Upgrade a host's Flux engine to 0.54 when adopting this release.** The connector pack and its
  host exchange runtime objects directly, and the new generated WebSocket plans require the guarded
  channel executor introduced on that engine line. Keeping a host on Flux 0.52 resolves two
  incompatible engines instead of linking them.

### New

- **Asterisk ARI can now deliver live events over its WebSocket.** Configure the same HTTPS
  endpoint and Basic credentials used by the REST connector, choose an ARI application, and receive
  any of Asterisk's 45 documented event types with its complete original payload. The existing 108
  REST operations remain available beside it.

- **Generated connectors can describe ordinary WebSocket event channels end to end.** A host can
  prepare the exact connection URL, authentication, headers and subprotocols from tenant settings,
  then route vendor event names and delivery identifiers without reading provider source files.
  Connection plans stay offline until the host's guarded WebSocket runtime executes them, and
  diagnostics do not print credential-bearing values.

## [0.16.0] — 2026-08-02

### Action needed

- **Upgrade a host's Flux engine to 0.52 when adopting this release.** The connector pack and its
  host exchange Flux runtime objects directly, so they must use the same engine generation. A host
  that stays on Flux 0.49 will not link with the updated pack. Connector operations and generated
  modules are unchanged by this compatibility move.

## [0.15.0] — 2026-08-02

### New

- **Asterisk ARI is now available as a complete REST connector.** It covers all 108 ordinary HTTP
  operations documented by Asterisk 22.10.1, including channels, bridges, endpoints, recordings,
  playback, device state, mailboxes and administration. Connection setup uses an HTTPS endpoint plus
  Asterisk Basic credentials. The event WebSocket is intentionally not included; inbound eventing
  will follow once channels have a stable design.

### Improved

- **Zendesk's 35 operations now all follow Zendesk's own API descriptions.** Support and Messaging
  no longer rely on handwritten request copies, and recursive response data remains visible without
  weakening request validation. Three redundant ticket-update variants are now the single update
  operation Zendesk documents; this greenfield catalogue does not retain aliases for the removed
  copies.

## [0.14.0] — 2026-08-02

### Action needed

- **Rust consumers that construct connector provenance directly must add one field.**
  `Provenance` now carries an `operation_specs` map so catalogue users can see which API document
  produced each selected operation. Add an empty map for hand-built values, or construct with the
  default and override the fields you need. The public catalogue change itself is additive: every
  operation now has `spec_source`, which is `null` for an inline operation.

- **Rust consumers using the connector specification types have three more compatibility changes.**
  `Service` values need `legacy: false` (or a default update); `Pin` is no longer copyable and its
  variable is now a borrowed-or-owned string; and `LoadedProvider` must be obtained through the
  loader rather than constructed or exhaustively destructured by fields.

- **Rust consumers that exhaustively match connector-pack errors must add the unsafe-path case.**
  Calls now stop before authentication or network access when a caller-controlled path string could
  escape its URL segment. Add the new error arm or a wildcard.

### New

- **Zendesk expands from 7 operations to 37 across Support, Help Center, and Messaging.** Support can
  read audit history, recent/view tickets, users, organizations, groups, fields, forms, statuses,
  incremental exports, and custom-object definitions. Help Center can browse and publish knowledge
  base articles. Messaging can manage conversations, participants, messages, and users with its own
  app id and app-scoped key. Existing Support calls and addresses do not move.

  Lists deliberately omit string filters and cursors that cannot yet be encoded safely. Messaging
  message creation is text-only in this first slice. Zendesk webhook administration and inbound
  events do not ship: ordinary responses may expose a signing secret, and the complete setup flow
  cannot yet store that generated secret without exposing it.

- **GitHub, Stripe, Microsoft Graph, OpenAI, and Twilio each gain four reads sourced from pinned
  first-party API descriptions.** The additions cover common issue/workflow/commit, billing/event,
  mail/calendar-metadata, stored-response/file/batch, and recording/usage/conference workflows. The
  public catalogue now identifies the exact source operation and document for every spec-selected
  call; inline operations say so with a null source.

  These are bounded additions, not bulk imports. Unsafe string filters/cursors and unsupported body
  shapes stay absent. GitHub does not yet send its dated version header, Stripe follows the account's
  pinned API version, Twilio send/call writes still wait for form encoding, and Stripe's exchange-rate
  endpoint remains vendor-deprecated and restricted.

### Improved

- **Zendesk Support is visible as “Primary” wherever services are filtered or listed.** Its existing
  machine address remains unchanged, while Support can now be selected alongside Help Center and
  Messaging. Connectors with only one unnamed surface stay uncluttered.

- **Caller-controlled path identifiers can no longer change the request route.** Values containing
  path/query/fragment delimiters, percent escapes, whitespace/control characters, or `.`/`..` are
  refused before credentials or network access. Safe text and numeric ids behave as before.

- **Twilio's four new reads reuse the configured Account SID.** The same non-secret value supplies
  the Basic username and the account path, so connection setup does not ask for it twice.

## [0.13.0] — 2026-08-02

### Action needed

- **Upgrade a host's Flux engine to 0.49 when adopting this release.** The connector pack and the
  host exchange Flux runtime objects directly, so they must use the same engine generation. Keeping
  a host on 0.47 while upgrading its connectors will not link. The connector catalogue itself is
  unchanged by this move.

## [0.12.0] — 2026-08-02

### New

- **A connector can now address an envelope-shaped API.** Some vendors want a request body whose
  fields sit inside nested arrays — SendGrid's send is the clearest example. That shape could not be
  expressed before, and a connector for such a vendor would have produced a request the vendor
  rejects. It works now, for a body whose shape the connector file spells out.

  One limit worth knowing: this covers a fixed shape, not a variable-length list. A send to one
  recipient is declared as one; a send to two is declared as two. Passing "however many recipients
  the caller has" still is not possible, and a connector that needs it will say so rather than build
  the wrong request.

### Action needed

- **If you build against the `connector-flux` crate and match exhaustively on its `Error` type, your
  build will break.** Three variants were added for the new refusals. Add the arms, or a `_` arm.

## [0.11.0] — 2026-08-02

### New

- **Anthropic's Admin API now covers who is in your organization.** Six more read operations: list
  organization members and read one by id, list the members of a workspace and read one by id, read a
  single workspace, and list outstanding invites. These join the existing organization, workspace and
  API-key reads.

  Two things to know. Every one of these calls returns the whole first page and no more — this
  connector cannot yet ask for the next one, and each call says so in its own description, so a large
  organization will see a partial list rather than an error. And these responses carry real people's
  names and email addresses; every such field is marked so, and the marking is enforced by a test
  rather than by review.

- **Every connector now says what kind of thing it is.** All 54 carry a domain label — telephony,
  payments, support, observability and twenty-odd more — so a catalogue can be filtered by domain
  instead of scrolled. Providers whose surfaces differ are labelled per surface: Google's mail is
  email, its Drive is storage. Nothing you can call has changed; this is descriptive.

### Action needed

- **If you build against the `codewandler-connector-spec` crate and construct a `Service` value
  directly, your build will break.** The struct gained a `tags` field; add `tags: Vec::new()` or use
  `..Default::default()`. Reading and writing connector files is unaffected.

## [0.10.1] — 2026-08-01

### Improved

- **A connector can no longer make two contradictory claims about one response.** Saying "this reply
  contains a credential, so do not offer this call" and "this reply contains a credential, so hand
  back a reference instead" at the same time is now refused, with a message saying which one applies.

## [0.10.0] — 2026-08-01

### New

- **Twilio's webhooks can now be checked for authenticity.** Twilio signs its callbacks differently from
  most services — over the address it called plus the form fields, rather than the raw message — and
  that shape could not be described here, so Twilio's events shipped with no verification at all. It is
  described now, and checked against Twilio's own published example signature.

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

//! Discord (C-216) is the epic's probe for **a credential prefix that is not `Bearer `** — and,
//! underneath it, for a vendor whose object ids do not fit in a JSON number.
//!
//! [C-184](../../../docs/stories/C-184-authscheme-prefix.md) taught `AuthScheme::Header` to carry a
//! `prefix` so a vendor's scheme word is connector data on the placement rather than something baked
//! into the credential an operator pastes (`crates/connector-spec/src/auth.rs`, `AuthScheme::Header`).
//! Discord authenticates a bot with `Authorization: Bot <token>`, and `Bearer ` on the *same header*
//! is not a typo — it is Discord's spelling for a different credential with different capabilities.
//! Getting it wrong therefore does not fail loudly: it returns a `401` that reads exactly like a bad
//! token.
//!
//! Four findings, each pinned below:
//!
//! 1. **The prefix is `"Bot "`, trailing space included, on `Authorization`.** Asserted as an exact
//!    string rather than "some prefix is set", because the whole hazard is a plausible neighbouring
//!    value.
//! 2. **The premise this story was filed on, corrected against the three connectors it is about.**
//!    The story states that every shipped connector using a prefix spells `Bearer `. It is wrong:
//!    Okta's `SSWS `, Statuspage's `OAuth ` and PagerDuty's `Token token=` were all shipping before
//!    Discord, and no connector spells `Bearer ` as a `Header` prefix at all — the `Bearer` *preset*
//!    is a separate variant and stays one (`AuthScheme::Bearer`). What Discord actually adds is the
//!    first prefix whose neighbouring value is **also valid vendor syntax for a different
//!    credential**, which is the sharper case.
//! 3. **One credential kind, chosen deliberately.** Discord publishes two authentication mechanisms
//!    for this API — a bot token and an OAuth2 bearer token — and they are different credentials
//!    with different capabilities, not interchangeable alternatives of one mechanism. This connector
//!    declares the bot token only, so `default_auth` holds exactly one alternative naming exactly one
//!    credential, and no `oauth2` grant is declared.
//! 4. **Every snowflake is a string.** A Discord id is a 64-bit integer; ids past 2^53 lose precision
//!    in any consumer that parses JSON numbers as doubles, which is most of them. So every id this
//!    connector accepts or describes is declared `type = "string"`, and this test walks the declared
//!    schemas to prove no id-shaped field is typed as a number anywhere.
//!
//! The fifth property has no assertion of its own because it has no IR field to assert on: Discord's
//! rate limits are **per-route, dynamic, and returned in response headers**. `quirks.rate_limit`
//! takes a fixed `requests`/`per_seconds` pair and cannot express that, so this connector declares
//! none and states the rule in prose a model reads instead — which *is* asserted, below, because
//! prose that nothing checks is how a caller ends up discovering a limit by being rate-limited.
//!
//! # A scoping rule for this file, learned the hard way
//!
//! **Nothing here walks `providers/`.** A per-provider contract test may load a provider it *names* —
//! finding 2 loads three predecessors deliberately — but it must never enumerate the directory and
//! assert on what it finds. An earlier version of finding 2 did, and Klaviyo landing a fifth prefix
//! in the same wave turned this file red from a worktree it could not see: a failure that is not one
//! of the eight whole-catalogue staleness checks `AGENTS.md` tabulates, that no regeneration at
//! integration resolves, and that breaks the disjoint-write-set property letting provider stories run
//! in parallel. Catalogue-wide claims belong in a catalogue-wide test that the coordinator owns;
//! model-wide claims about the prefix axis belong in
//! `crates/connector-spec/tests/auth_prefix.rs`, which is fixture-based and therefore cannot be
//! falsified by a new connector at all. That is the pattern to copy.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Binding, Connector, HttpMethod, Idempotency, Risk};

/// The provider under test.
const PROVIDER: &str = "discord";

/// The one credential this connector declares.
const CREDENTIAL: &str = "discord.bot_token";
/// A variable *name*; no credential value appears in this repository.
const CREDENTIAL_ENV: &str = "DISCORD_BOT_TOKEN";

const AUTH_HEADER: &str = "Authorization";

/// **The probe.** The trailing space is part of the literal — the host concatenates prefix and
/// credential with nothing in between.
const PREFIX: &str = "Bot ";

/// The neighbouring value that must never appear here. It is not a typo of `Bot `; it is Discord's
/// own spelling for a *user* OAuth2 token, so a connector sending it authenticates as the wrong kind
/// of principal and is told only that the token is invalid.
const WRONG_PREFIX: &str = "Bearer ";

const BASE_URL: &str = "https://discord.com/api/v10";
const AUTHORITY: &str = "com.discord.api";
const API_VERSION: &str = "v10";

/// The verification read — argument-free, so a settings page can run it unattended.
const VERIFY: &str = "discord-current-user";

/// The six curated operations, in the order `providers/discord.toml` declares them.
const OPERATIONS: &[&str] = &[
    "discord-current-user",
    "discord-guild-list",
    "discord-guild-get",
    "discord-guild-channels",
    "discord-channel-messages",
    "discord-message-create",
];

/// The pattern every snowflake-valued parameter declares: decimal digits, carried as a string.
const SNOWFLAKE_PATTERN: &str = "^[0-9]+$";

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader — the same route `shipped_modules.rs` takes, so
/// this file cannot pass against a fixture that drifted from what ships.
fn load() -> Connector {
    load_provider(PROVIDER)
}

fn load_provider(id: &str) -> Connector {
    let path = providers_dir().join(format!("{id}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-216 ships the Discord connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{id}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{id}.toml does not load: {error}"))
        .connector
}

/// **Finding 1: the credential travels as `Authorization: Bot <token>`, and the prefix is asserted
/// exactly.**
///
/// `Bearer` is wrong here in the most expensive way available: it is a *valid* Discord scheme word,
/// selecting an OAuth2 user token, so sending it with a bot token yields `401 Unauthorized` with
/// Discord's generic invalid-token body. Nothing distinguishes that from a revoked token, an expired
/// token, or a typo, so an operator debugging it looks at the credential — the one thing that was
/// correct. The prefix is the part under this repository's control, so it is the part this file
/// pins, character for character.
#[test]
fn the_bot_token_travels_with_the_bot_prefix_and_never_bearer() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "Discord");
    assert_eq!(connector.base_url, BASE_URL);
    assert_eq!(connector.authority.as_deref(), Some(AUTHORITY));
    assert_eq!(connector.api_version.as_deref(), Some(API_VERSION));

    let credential = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("discord declares `{CREDENTIAL}`"));

    assert_eq!(
        credential.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string(),
            prefix: PREFIX.to_string(),
        },
        "the bot token is `Authorization: Bot <token>` — the prefix is the vendor's scheme word, \
         supplied by the connector, and the credential is appended by the host"
    );

    // Spelled out separately from the equality above, so the failure message names the hazard rather
    // than printing two structs and leaving a reader to spot the difference.
    let AuthScheme::Header { name, prefix } = &credential.scheme else {
        panic!("the bot token is a header placement");
    };
    assert_eq!(name, AUTH_HEADER);
    assert_eq!(
        prefix, PREFIX,
        "the prefix is `Bot ` exactly, trailing space included: the host concatenates prefix and \
         credential with nothing between them, so `Bot` alone would send `Authorization: Bot<token>`"
    );
    assert_ne!(
        prefix.as_str(),
        WRONG_PREFIX,
        "`Bearer ` is not a near-miss of `Bot ` — it is Discord's own scheme word for an OAuth2 \
         user token, so this connector would authenticate as the wrong kind of principal and be \
         told only that the token is invalid"
    );
    assert!(
        !prefix.to_ascii_lowercase().contains("bearer"),
        "the prefix must not spell Discord's other scheme word in any casing"
    );
    assert_ne!(
        credential.scheme,
        AuthScheme::Bearer,
        "the `Bearer` preset is the wrong credential here, not merely the wrong spelling"
    );

    assert_eq!(credential.env, [CREDENTIAL_ENV]);
}

/// **The prefix never reaches the module, asserted so that it can actually fail.**
///
/// `AGENTS.md`'s authentication contract says generated Flux names a credential and nothing more:
/// the host applies the placement, so `Bot ` must not appear in an emitted module. The obvious way to
/// check that is to emit each operation and search the text for the scheme word — and this file did
/// exactly that, twice, wrongly.
///
/// The first version searched the whole module and **failed**, because `discord-current-user`'s
/// `description` deliberately names the `Bot ` scheme word for the model reading it. The second
/// version dropped `description` lines to fix that, and passed — but review found it **inert**:
/// `crates/connector-flux/src` never references `AuthScheme` at all, so no emitted module can contain
/// an auth prefix under *any* declaration. A text search over the output could not have failed for a
/// prefix regression before the change or after it. It was reassurance wearing an assertion's clothes.
///
/// So the property is asserted at its cause instead: emitting an operation against a connector whose
/// credentials have been **removed entirely** produces byte-identical Flux. That is the strongest
/// form of "the prefix does not reach the module" available here — not that the scheme word happens
/// to be absent from the text, but that the emitter's output does not depend on the auth declaration
/// at all. Teaching the emitter to read `AuthScheme` — the change that would put a prefix in a module
/// — turns this red on the first operation, which a text search never would.
///
/// The declaration-side pin, which is the one that catches a wrong scheme word actually shipping, is
/// [`the_bot_token_travels_with_the_bot_prefix_and_never_bearer`].
#[test]
fn the_emitter_never_reads_the_credential_declaration() {
    let connector = load();

    let mut blind = connector.clone();
    blind.auth.clear();
    blind.default_auth.clear();

    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("discord declares `{id}`"));
        let with_auth = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{id} does not emit: {error}"));

        let blind_operation = blind
            .operation(id)
            .expect("the stripped connector keeps its operations");
        let without_auth = emit_operation(&blind, blind_operation)
            .unwrap_or_else(|error| panic!("{id} does not emit without credentials: {error}"));

        assert_eq!(
            with_auth, without_auth,
            "{id}'s emitted Flux changed when the credential declaration was removed, so the \
             emitter now reads auth and a scheme word can reach a module"
        );

        // With the above established, these are statements about the emitted text that mean
        // something: the module carries no credential material of any kind.
        assert!(
            !with_auth.contains(CREDENTIAL_ENV) && !with_auth.contains(CREDENTIAL),
            "{id} emits credential material into the module:\n{with_auth}"
        );
    }
}

/// **Finding 2: the correction to the claim that filed this story — measured, and scoped to the
/// connectors the claim is actually about.**
///
/// `docs/stories/C-216-provider-discord.md` says "Every shipped connector that uses it spells
/// `Bearer `". That is false in both directions, and this is where the evidence lives: Okta, PagerDuty
/// and Statuspage were all shipping non-`Bearer ` prefixes before Discord existed, and **no**
/// connector spells `Bearer ` as a `Header` prefix at all — `AuthScheme::Bearer` is a preset variant
/// of its own, which `connector-spec`'s `auth_prefix.rs::the_preset_schemes_carry_no_prefix_of_their
/// _own` pins at the model, where it belongs.
///
/// So Discord is not the first non-`Bearer ` prefix and this file does not claim it is. What it *is*
/// — and what makes it worth a probe — is the first prefix whose **neighbouring value is also valid
/// vendor syntax, for a different credential**. `SSWS <token>` sent as `Bearer <token>` is rejected
/// by a vendor that has no bearer scheme at all; `Bot <token>` sent as `Bearer <token>` is a
/// well-formed request for a principal the caller does not hold.
///
/// **On the shape of this assertion, because the first version of it was wrong in a way worth
/// recording.** It walked `providers/*.toml` and asserted the resulting census equalled a four-element
/// literal. That is a *whole-catalogue* assertion living inside a *per-provider* test, and it made
/// this file falsifiable by work it has nothing to do with: Klaviyo landed `Klaviyo-API-Key ` in the
/// same wave and turned this test red, from another worktree, with a red that is not among the eight
/// staleness checks `AGENTS.md` tabulates and that no regeneration at integration could resolve. It
/// broke the property that lets provider stories run in parallel at all — disjoint write sets, and
/// no per-provider test that a stranger's provider can falsify.
///
/// The fix is not to append Klaviyo; that reproduces the defect one wave later. It is to assert what
/// the claim is actually about. The premise named specific connectors, so the correction names them
/// too: each predecessor is loaded **by name** and its own prefix checked. A fifth, tenth or fiftieth
/// prefix landing in the catalogue cannot falsify this, because the catalogue's *membership* was never
/// the evidence — what Okta, PagerDuty and Statuspage declare is. If one of those three ever changes
/// its scheme word this test fires, which is exactly when the evidence would have stopped being true.
///
/// **Why the three are a `let` in the body and not a `const`.** `connector-cli`'s C-54 guard
/// `shipped_providers_build.rs::no_test_hand_maintains_a_shipped_provider_list` refuses a test
/// `const` naming two or more shipped providers, because five such constants once drifted out of step
/// and silently switched off a connector's whole gate. That guard's own documentation carves out the
/// case this is — "a per-provider claim inside a test body … is an assertion about each provider
/// rather than a copy of the provider set, and stays" — and the distinction is exactly right here:
/// this is not the shipped set, and it must *not* grow when a provider is added. Keeping it in the
/// body says so to both the guard and the next reader. Do not lift it back out to a `const`.
#[test]
fn the_non_bearer_prefixes_this_connector_joins_were_already_shipped() {
    // (provider, credential, the exact scheme word it shipped before Discord existed)
    let predecessors = [
        ("okta", "okta.api_token", "SSWS "),
        ("pagerduty", "pagerduty.api_token", "Token token="),
        ("statuspage", "statuspage.api_key", "OAuth "),
    ];

    for (id, credential, expected) in predecessors {
        let connector = load_provider(id);
        let method = connector
            .auth_method(credential)
            .unwrap_or_else(|| panic!("{id} declares `{credential}`"));

        let AuthScheme::Header { name, prefix } = &method.scheme else {
            panic!("{id}'s `{credential}` is a header placement carrying a scheme word");
        };
        assert_eq!(name, AUTH_HEADER);
        assert_eq!(
            prefix, expected,
            "{id} ships `{expected}`, which is the evidence that this story's premise — that every \
             prefix spells `Bearer ` — was never true"
        );
        assert_ne!(
            prefix, WRONG_PREFIX,
            "{id} is one of the connectors that disproves the premise; it cannot spell `Bearer `"
        );
    }

    // And Discord joins them rather than starting them. Asserted here too, so this test states the
    // whole of the correction and not merely its first half.
    let discord = load();
    let AuthScheme::Header { prefix, .. } = &discord
        .auth_method(CREDENTIAL)
        .expect("discord declares its bot token")
        .scheme
    else {
        panic!("discord's bot token is a header placement");
    };
    assert_eq!(prefix, PREFIX);
    assert!(
        predecessors
            .iter()
            .all(|(_, _, shipped)| *shipped != PREFIX),
        "`Bot ` is a scheme word none of the predecessors already used"
    );
}

/// **Finding 3: one credential kind, and the other one is refused rather than offered.**
///
/// Discord documents two ways to authenticate this API. A **bot token** belongs to an application's
/// bot user, is granted by the guilds that installed it, and is what a background integration holds.
/// An **OAuth2 bearer token** belongs to a Discord *person*, is scoped by what that person consented
/// to, and expires. They are different credentials with different capabilities — most of the routes
/// below are unreachable with a bare OAuth2 token, and `/users/@me/guilds` returns a *different set*
/// under each — so declaring them as two alternatives of one mechanism would tell a host that either
/// satisfies any operation here. Neither does.
///
/// This connector is the bot-token one. The choice is recorded in the provider file's header; this
/// test is what stops it being widened by adding a second `[[auth]]` entry.
#[test]
fn exactly_one_credential_is_declared_and_it_is_the_bot_token() {
    let connector = load();

    assert_eq!(
        connector.auth.len(),
        1,
        "one credential. A bot token and an OAuth2 bearer token are different credentials with \
         different capabilities, not two spellings of one mechanism"
    );
    assert_eq!(connector.auth[0].name, CREDENTIAL);
    assert!(
        connector.auth[0].oauth2.is_none(),
        "no OAuth2 grant: nothing here mints, refreshes or exchanges a token"
    );

    assert_eq!(
        connector.default_auth.len(),
        1,
        "one alternative. A second would say some other credential also authenticates these routes"
    );
    let mechanism: Vec<&str> = connector.default_auth[0]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(mechanism, [CREDENTIAL]);

    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("discord declares `{id}`"));
        let effective: Vec<Vec<&str>> = connector
            .effective_auth(operation)
            .iter()
            .map(|requirement| requirement.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            effective,
            vec![vec![CREDENTIAL]],
            "every operation authenticates with the bot token; none overrides the default"
        );
    }

    // The configuration half: one renderable secret field, bound to that credential, carrying no
    // example. A placeholder shaped like a real bot token has tripped GitHub's push protection in
    // this repository's history, and a bot token is the one Discord credential that is a bearer of
    // full application authority.
    let field = connector
        .config
        .iter()
        .find(|field| field.name == "bot_token")
        .expect("discord declares the `bot_token` config field");
    assert_eq!(
        field.binding(),
        Some(Binding::Credential { name: CREDENTIAL })
    );
    assert!(
        field.secret,
        "`bot_token` binds a credential, so it is secret — the agreement is a loader rule"
    );
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "`bot_token` is renderable"
    );
    assert!(
        field.example.is_none(),
        "a secret field carries no example — a token-shaped placeholder has tripped push \
         protection here before"
    );
    assert_eq!(
        connector.config.len(),
        1,
        "Discord's base URL is a fixed literal with no `{{variable}}`, so the token is the whole of \
         what a human supplies"
    );
}

/// **Finding 4: every snowflake is a string, everywhere it is declared.**
///
/// A Discord id is a 64-bit integer built from a timestamp, a worker/process id and a sequence
/// counter. Ids minted after roughly 2015 already exceed 2^53, so a consumer parsing them as JSON
/// numbers — which is what `JSON.parse` and most JSON libraries do by default — silently rounds
/// them. The rounded value is still a plausible id, addresses a different object or none, and no
/// error is raised anywhere. Discord's own API therefore serializes every snowflake as a string, and
/// this connector declares them the same way.
///
/// Both halves are asserted, because either alone is satisfiable by a connector that declares
/// nothing: **no** id-shaped field anywhere in the declared schemas is numeric, and a named list of
/// ids that must be present really is present and really is a string.
#[test]
fn every_snowflake_is_declared_as_a_string() {
    let connector = load();

    // Every path parameter this connector takes is a snowflake, and each is a digits-only string.
    let mut path_params: Vec<String> = Vec::new();
    for operation in &connector.operations {
        for param in &operation.params.path {
            path_params.push(format!("{}:{}", operation.id, param.name));
            assert_eq!(
                param.schema.get("type").and_then(|t| t.as_str()),
                Some("string"),
                "{}'s `{}` is a snowflake and must be a string — a 64-bit id typed as a number is \
                 rounded by any consumer that parses JSON numbers as doubles",
                operation.id,
                param.name
            );
            assert_eq!(
                param.schema.get("pattern").and_then(|p| p.as_str()),
                Some(SNOWFLAKE_PATTERN),
                "{}'s `{}` declares the snowflake pattern, so a caller passing a name or a URL is \
                 refused before the request is built",
                operation.id,
                param.name
            );
        }
    }
    assert_eq!(
        path_params,
        [
            "discord-guild-get:guild_id",
            "discord-guild-channels:guild_id",
            "discord-channel-messages:channel_id",
            "discord-message-create:channel_id",
        ],
        "the addressable snowflakes this connector takes, in declaration order"
    );

    // The negative half, over every declared response shape: nothing id-shaped is numeric.
    let mut numeric_ids: Vec<String> = Vec::new();
    for operation in &connector.operations {
        if let Some(schema) = &operation.response_schema {
            collect_numeric_ids(&operation.id, "", schema, &mut numeric_ids);
        }
        for param in operation
            .params
            .path
            .iter()
            .chain(&operation.params.query)
            .chain(&operation.params.body)
            .chain(&operation.params.header)
        {
            collect_numeric_ids(&operation.id, &param.name, &param.schema, &mut numeric_ids);
        }
    }
    assert!(
        numeric_ids.is_empty(),
        "these id-shaped fields are declared as numbers, and a Discord id does not fit in one: \
         {numeric_ids:?}"
    );

    // The positive half. Without it, "no numeric ids" would be satisfied by declaring no ids at all.
    for (operation_id, property) in [
        ("discord-current-user", "id"),
        ("discord-guild-list", "id"),
        ("discord-guild-get", "id"),
        ("discord-guild-get", "owner_id"),
        ("discord-guild-channels", "id"),
        ("discord-guild-channels", "guild_id"),
        ("discord-channel-messages", "id"),
        ("discord-channel-messages", "channel_id"),
        ("discord-message-create", "id"),
        ("discord-message-create", "channel_id"),
    ] {
        let operation = connector
            .operation(operation_id)
            .unwrap_or_else(|| panic!("discord declares `{operation_id}`"));
        let schema = operation
            .response_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{operation_id} declares a response shape"));
        let declared = find_property_type(schema, property)
            .unwrap_or_else(|| panic!("{operation_id}'s response shape declares `{property}`"));
        assert_eq!(
            declared, "string",
            "{operation_id}'s `{property}` is a snowflake and is declared as {declared:?}"
        );
    }
}

/// Walks a schema and records every property whose name is id-shaped and whose declared type is
/// numeric. The name test is deliberately broad — `id`, `*_id`, `*_ids` — because the failure this
/// guards is an author typing `type = "integer"` next to a field they were thinking of as a number.
fn collect_numeric_ids(
    operation: &str,
    path: &str,
    schema: &serde_json::Value,
    found: &mut Vec<String>,
) {
    let id_shaped = |name: &str| name == "id" || name.ends_with("_id") || name.ends_with("_ids");
    let numeric = matches!(
        schema.get("type").and_then(|t| t.as_str()),
        Some("integer") | Some("number")
    );
    if numeric && path.rsplit('.').next().is_some_and(id_shaped) {
        found.push(format!("{operation}:{path}"));
    }

    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, child) in properties {
            let child_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            collect_numeric_ids(operation, &child_path, child, found);
        }
    }
    if let Some(items) = schema.get("items") {
        collect_numeric_ids(operation, path, items, found);
    }
}

/// The declared `type` of the first property named `wanted`, found anywhere in the schema tree.
fn find_property_type<'a>(schema: &'a serde_json::Value, wanted: &str) -> Option<&'a str> {
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        if let Some(found) = properties.get(wanted) {
            return found.get("type").and_then(|t| t.as_str());
        }
        for child in properties.values() {
            if let Some(found) = find_property_type(child, wanted) {
                return Some(found);
            }
        }
    }
    schema
        .get("items")
        .and_then(|items| find_property_type(items, wanted))
}

/// **The rate limits, which have no field to live in.**
///
/// Discord rate-limits **per route**, with a separate bucket per major path parameter, and it does
/// not publish the numbers: a caller learns its remaining budget from `X-RateLimit-Remaining` and
/// its wait from `Retry-After` on a `429`. `quirks.rate_limit` takes an exact `requests` /
/// `per_seconds` pair (`crates/connector-spec/src/ir.rs`, `RateLimit`), which cannot express a bound
/// that is discovered rather than declared — and the one fixed number Discord *does* publish, a
/// global 50 requests per second, is a budget shared across every route, so writing it onto each
/// operation would state a per-route allowance six times over that no route actually has. That is
/// `providers/hubspot.toml`'s reasoning about tier-dependent limits, met at a different axis.
///
/// So the connector declares no `rate_limit` and says the rule in the prose a model reads. Prose
/// nothing checks is how a caller discovers a limit by being rate-limited, so it is checked here.
#[test]
fn the_rate_limit_rule_is_stated_where_a_model_reads_it() {
    let connector = load();

    for operation in &connector.operations {
        assert!(
            operation.quirks.rate_limit.is_none(),
            "{} declares a fixed rate limit. Discord's are per-route and discovered from response \
             headers; a fixed pair here would be a bound nobody verified",
            operation.id
        );
    }

    for needle in ["Bot <token>", "snowflake", "429", "Retry-After"] {
        assert!(
            connector.description.contains(needle),
            "the connector description states {needle:?}, because no field carries it: {:?}",
            connector.description
        );
    }

    // The write is the operation a caller is most likely to run in a loop, so it repeats the rule
    // where the caller is rather than relying on the connector-level summary.
    let write = connector
        .operation("discord-message-create")
        .expect("the curated set includes the message write");
    assert!(
        write.description.contains("Retry-After"),
        "the message write states the retry rule in its own description: {:?}",
        write.description
    );
}

/// **`verify` is a read that runs unattended.**
///
/// A "Test connection" button is pressed whenever someone opens a settings page, so it must be a
/// read — the loader checks the declared risk — *and* it must need no argument, which the loader does
/// not check and a connector can still get wrong. `GET /users/@me` is the request Discord's own
/// documentation uses to confirm a bot token: it takes nothing but the credential, and it is the
/// narrowest call that distinguishes "the token is wrong" from "the prefix is wrong", because a bot
/// token sent as `Bearer` fails here too.
#[test]
fn verify_is_an_argument_free_read() {
    let connector = load();

    assert_eq!(connector.verify.as_deref(), Some(VERIFY));
    let operation = connector
        .operation(VERIFY)
        .expect("verify names an operation");

    assert_eq!(operation.method, HttpMethod::Get);
    assert_eq!(operation.risk, Risk::Low);
    assert_eq!(operation.idempotency, Idempotency::Idempotent);
    assert!(
        operation.params.path.is_empty()
            && operation.params.query.is_empty()
            && operation.params.body.is_empty()
            && operation.params.header.is_empty()
            && operation.params.body_schema.is_none(),
        "a connection test that needs an argument cannot run unattended"
    );

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "the curated set, in declaration order"
    );
}

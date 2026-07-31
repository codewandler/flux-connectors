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
//! 2. **The catalogue's prefix census, measured rather than asserted from memory.** The story that
//!    filed this work states that every shipped connector using a prefix spells `Bearer `. That is
//!    **not what the catalogue says**, and this test is where the correction lives: `SSWS `,
//!    `OAuth ` and `Token token=` were already shipped, and no connector spells `Bearer ` as a
//!    `Header` prefix at all — the `Bearer` *preset* is a separate variant and stays one
//!    (`AuthScheme::Bearer`). What Discord actually adds is the first prefix whose neighbouring
//!    value is **also valid vendor syntax for a different credential**, which is the sharper case.
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

/// Every provider id in the repository, sorted.
fn shipped_providers() -> Vec<String> {
    let mut providers: Vec<String> = std::fs::read_dir(providers_dir())
        .expect("providers/ is readable")
        .filter_map(|entry| {
            let path = entry.expect("a readable directory entry").path();
            (path.extension()? == "toml").then(|| {
                path.file_stem()
                    .expect("a .toml file has a stem")
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .collect();
    providers.sort();
    providers
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

    // The prefix is connector data and must stay out of the module: generated Flux names a
    // credential and nothing more (`AGENTS.md`, the authentication contract).
    //
    // The scan is over the emitted *code*, with the `description` line removed. A description is
    // prose a model reads and `discord-current-user`'s deliberately names the scheme word — telling
    // a caller which header arrangement a 401 would be blaming. The hazard this assertion guards is
    // the module *assembling* the header, which would appear as a `headers:` argument or an
    // interpolated string, never as documentation.
    for id in OPERATIONS {
        let operation = connector
            .operation(id)
            .unwrap_or_else(|| panic!("discord declares `{id}`"));
        let flux = emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("{id} does not emit: {error}"));
        let code = flux_without_descriptions(&flux);
        assert!(
            !code.contains(PREFIX.trim()) && !code.contains("Bearer"),
            "{id} emits a scheme word into the module; the prefix belongs to the placement, and \
             the host applies it:\n{flux}"
        );
        assert!(
            !flux.contains(CREDENTIAL_ENV),
            "{id} emits the credential's environment variable into the module:\n{flux}"
        );
    }
}

/// The emitted module with its `description` lines dropped, leaving the declaration and the
/// statements — the part that becomes a request.
fn flux_without_descriptions(flux: &str) -> String {
    flux.lines()
        .filter(|line| !line.trim_start().starts_with("description \""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// **Finding 2: the catalogue's prefix census — and the correction to the claim that filed this
/// story.**
///
/// `docs/stories/C-216-provider-discord.md` says "Every shipped connector that uses it spells
/// `Bearer `". Measured over `providers/*.toml`, that is false in both directions: three connectors
/// already ship non-`Bearer ` prefixes (Okta's `SSWS `, Statuspage's `OAuth `, PagerDuty's
/// `Token token=`), and **no** connector spells `Bearer ` as a `Header` prefix, because `Bearer` is a
/// preset variant of its own and stays one — `AuthScheme::Header`'s own documentation records why
/// collapsing it would move fifteen providers' committed artifacts to say what they already say.
///
/// So Discord is not the first non-`Bearer ` prefix, and this file does not claim it is. What it *is*
/// — and what makes it worth a probe — is the first prefix whose **neighbouring value is also valid
/// vendor syntax, for a different credential**. `SSWS <token>` sent as `Bearer <token>` is rejected
/// by a vendor that has no bearer scheme at all; `Bot <token>` sent as `Bearer <token>` is a
/// well-formed request for a principal the caller does not hold.
///
/// The census is asserted as a whole list rather than as a lookup, so a fourth prefix landing in the
/// catalogue makes this test say so instead of passing quietly.
#[test]
fn the_catalogue_prefix_census_is_exactly_these_four() {
    let mut census: Vec<String> = Vec::new();

    for id in shipped_providers() {
        for method in &load_provider(&id).auth {
            match &method.scheme {
                AuthScheme::Header { name, prefix } if !prefix.is_empty() => {
                    census.push(format!("{id}:{}:{name}:{prefix}", method.name));
                }
                // A `Bearer`/`Basic` preset carries its prefix in the variant, not in a string, and
                // the loader has no way to spell a second one on top. Nothing to census.
                _ => {}
            }
        }
    }

    assert_eq!(
        census,
        [
            "discord:discord.bot_token:Authorization:Bot ",
            "okta:okta.api_token:Authorization:SSWS ",
            "pagerduty:pagerduty.api_token:Authorization:Token token=",
            "statuspage:statuspage.api_key:Authorization:OAuth ",
        ],
        "the shipped `Header` prefixes, provider-sorted. If this list changed, re-read finding 2: \
         the story's premise was that `Bearer ` was the only value, and it never was"
    );

    // The other half of the correction, stated as an assertion: `Bearer ` is not spellable as a
    // header prefix in the shipped catalogue, because the preset is a distinct variant.
    assert!(
        !census.iter().any(|entry| entry.ends_with(WRONG_PREFIX)),
        "no connector spells `Bearer ` as a header prefix — it is `AuthScheme::Bearer`, a variant"
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

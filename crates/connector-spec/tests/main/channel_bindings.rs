//! Channel bindings: the third member kind, and the composition that lets a connector describe a
//! flux ingress surface instead of flux hand-writing one per vendor.
//!
//! Every assertion goes through `provider::load`, because the file is the surface an author writes
//! and every rule here is a rule about a file. The tests are arranged around the one property that
//! matters: **a binding either holds completely or is refused.** A binding that half-holds — an
//! unresolvable reply, a required parameter nobody bound, a poll with no cursor — builds, ships,
//! passes every artifact check, and then fails on an operator's first real delivery. That is the
//! plausible-but-wrong output `AGENTS.md` requires the pipeline to refuse rather than emit, and it is
//! why almost every test below asserts on a refusal.

use connector_spec::{
    provider, Binding, Connector, TimestampFormat, Transport, VerificationScheme,
};

use crate::shipped_provider;

/// A connector with one reply-shaped operation and one event, ready for a binding to be bolted on.
/// The `{binding}` placeholder is where each test writes the binding under test.
fn fixture(binding: &str) -> String {
    format!(
        r#"
id = "acme"
vendor = "Acme"
authority = "com.acme.api"
api_version = "v1"
base_url = "https://api.acme.example"

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]

[[auth]]
name = "acme.webhook_secret"
scheme = "signing"
env = ["ACME_WEBHOOK_SECRET"]

[[operations]]
id = "acme-reply"
method = "POST"
direction = "write"
path = "/reply"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "room"
required = true
schema = {{ type = "string" }}

[[operations.params.body]]
name = "text"
required = true
schema = {{ type = "string" }}

[[operations.params.body]]
name = "parent"
required = false
schema = {{ type = "string" }}

[[events]]
name = "thing.created"

{binding}
"#
    )
}

fn load(source: &str) -> Connector {
    provider::load("providers/fixture.toml", source)
        .unwrap_or_else(|error| panic!("the fixture must load:\n{error}"))
        .connector
}

fn refuse(source: &str) -> String {
    let error = provider::load("providers/fixture.toml", source)
        .err()
        .unwrap_or_else(|| panic!("this definition must not load"));
    format!("{error}")
}

/// The binding every "one thing is wrong" test below starts from, so that a refusal is attributable
/// to the single field the test changed.
const GOOD: &str = r#"
[[channels]]
name = "hook"
transport = "socket"
events = ["thing.created"]

[channels.payload]
room = "event.room"
body = "event.body"

[channels.reply]
operation = "acme-reply"
result = "text"

[channels.reply.bind]
room = "room"
parent = "body"
"#;

const GENERIC_SOCKET: &str = r#"
[[config]]
name = "app"
label = "ARI application"
help = "The Stasis application whose events this connection receives"
example = "support"
binds = "channel.hook.query.app"

[[config]]
name = "subscribe_all"
label = "Subscribe to all events"
help = "Whether ARI should send events for every application"
example = "false"
required = false
default = "false"
binds = "channel.hook.query.subscribeAll"

[[channels]]
name = "hook"
transport = "socket"
events = ["thing.created"]
payload_root = true

[channels.connect]
path = "/events"
auth = [{ credentials = ["acme.token"] }]
subprotocols = ["ari"]

[channels.connect.query]
app = "{app}"
subscribeAll = "{subscribe_all}"

[channels.connect.headers]
X-Acme-Mode = "events"
"#;

#[test]
fn a_generic_socket_round_trips_every_connect_event_payload_and_config_fact() {
    let source = fixture(GENERIC_SOCKET).replace(
        "name = \"thing.created\"",
        "name = \"thing.created\"\nwire_value = \"ThingCreated\"",
    );
    let connector = load(&source);
    let event = connector.event("thing.created").expect("event loads");
    let channel = connector.channel("hook").expect("channel loads");
    let connect = channel.connect.as_ref().expect("socket connect loads");

    assert_eq!(event.wire_value.as_deref(), Some("ThingCreated"));
    assert!(channel.payload_root);
    assert_eq!(connect.path, "/events");
    assert_eq!(connect.query["app"], "{app}");
    assert_eq!(connect.query["subscribeAll"], "{subscribe_all}");
    assert_eq!(connect.headers["X-Acme-Mode"], "events");
    assert_eq!(connect.subprotocols, ["ari"]);
    assert_eq!(connect.auth.len(), 1);

    let subscribe_all = connector
        .config
        .iter()
        .find(|field| field.name == "subscribe_all")
        .expect("optional socket setting loads");
    assert_eq!(subscribe_all.default.as_deref(), Some("false"));
    assert!(matches!(
        subscribe_all.binding(),
        Some(Binding::ChannelQuery {
            channel: "hook",
            parameter: "subscribeAll"
        })
    ));
}

#[test]
fn socket_connect_declarations_fail_closed_at_load() {
    for (source, expected) in [
        (
            fixture(GENERIC_SOCKET)
                .replace("path = \"/events\"", "path = \"wss://evil.example/events\""),
            "relative WebSocket path",
        ),
        (
            fixture(GENERIC_SOCKET).replace("transport = \"socket\"", "transport = \"poll\""),
            "only the `socket` transport",
        ),
        (
            fixture(GENERIC_SOCKET).replace(
                "payload_root = true",
                "payload_root = true\n\n[channels.payload]\nbody = \"event\"",
            ),
            "payload_root",
        ),
        (
            fixture(GENERIC_SOCKET).replace("X-Acme-Mode = \"events\"", "Host = \"evil.example\""),
            "handshake-owned header",
        ),
        (
            fixture(GENERIC_SOCKET).replace(
                "subprotocols = [\"ari\"]",
                "subprotocols = [\"ari events\"]",
            ),
            "subprotocol",
        ),
        (
            fixture(GENERIC_SOCKET).replace(
                "binds = \"channel.hook.query.app\"",
                "binds = \"channel.missing.query.app\"",
            ),
            "no channel binding",
        ),
    ] {
        let error = refuse(&source);
        assert!(error.contains(expected), "expected {expected:?}:\n{error}");
    }
}

#[test]
fn a_complete_binding_loads_and_composes_an_event_with_a_reply() {
    let connector = load(&fixture(GOOD));
    let channel = connector.channel("hook").expect("the binding loads");

    assert_eq!(channel.transport, Transport::Socket);
    assert_eq!(channel.events, ["thing.created"]);

    // The composition, stated: the inbound half names a declared event, and the outbound half names
    // a declared operation. Neither is a new primitive.
    assert!(connector.event(&channel.events[0]).is_some());
    let reply = channel.reply.as_ref().expect("the binding replies");
    assert!(connector.operation(&reply.operation).is_some());
    assert_eq!(reply.result.as_deref(), Some("text"));
}

// ---------------------------------------------------------------------------------------------
// The reply must resolve, and it must be completely settled at build time
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reply_naming_an_operation_nobody_declares_is_refused() {
    let error = refuse(&fixture(&GOOD.replace("acme-reply", "acme-nonexistent")));
    assert!(
        error.contains("no `[[operations]]` block declares"),
        "the error must name the dangling reference:\n{error}"
    );
}

#[test]
fn a_reply_leaving_a_required_parameter_unbound_is_refused() {
    // `room` is required and its binding is removed; `text` is still covered by `result`.
    let error = refuse(&fixture(&GOOD.replace("room = \"room\"\n", "")));
    assert!(
        error.contains("required parameter \"room\" unbound"),
        "the error must name the parameter that would fail on the first delivery:\n{error}"
    );
}

#[test]
fn a_reply_with_no_result_leaves_the_journey_output_parameter_unbound() {
    // Dropping `result` un-covers `text`, which no payload path can reach — this is the case that
    // motivated `result` existing at all.
    let error = refuse(&fixture(&GOOD.replace("result = \"text\"\n", "")));
    assert!(
        error.contains("required parameter \"text\" unbound"),
        "a reply whose journey-output parameter is unnamed must be refused:\n{error}"
    );
    assert!(
        error.contains("name it as `result`"),
        "the error must point at the fix:\n{error}"
    );
}

#[test]
fn a_reply_binding_a_parameter_the_operation_does_not_declare_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace("parent = \"body\"", "nonesuch = \"body\""),
    ));
    assert!(
        error.contains("binds reply parameter \"nonesuch\""),
        "the error must name the parameter that does not exist:\n{error}"
    );
}

#[test]
fn a_reply_binding_from_a_symbol_the_payload_never_produces_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace("parent = \"body\"", "parent = \"absent\""),
    ));
    assert!(
        error.contains("which its `payload` map does not declare"),
        "a reply is filled from the payload, so an unproduced symbol must be refused:\n{error}"
    );
}

#[test]
fn a_parameter_cannot_be_both_bound_and_the_journey_result() {
    let error = refuse(&fixture(
        &GOOD.replace("parent = \"body\"", "text = \"body\""),
    ));
    assert!(
        error.contains("One parameter carries one value"),
        "one parameter carrying two values must be refused:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// Events are members of the binding's own service
// ---------------------------------------------------------------------------------------------

#[test]
fn a_binding_carrying_an_undeclared_event_is_refused() {
    let error = refuse(&fixture(&GOOD.replace("thing.created", "thing.imagined")));
    assert!(
        error.contains("no `[[events]]` block declares"),
        "the error must name the undeclared event:\n{error}"
    );
}

#[test]
fn a_push_binding_carrying_no_events_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace("events = [\"thing.created\"]", "events = []"),
    ));
    assert!(
        error.contains("lists no `events`"),
        "a binding that routes nothing must be refused:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// Verification: the tri-state, and the HMAC matrix's own consistency
// ---------------------------------------------------------------------------------------------

/// The rule this repository would most regret getting wrong: silence on an open endpoint is how an
/// unverified event gets presented to a flow as a trusted one.
#[test]
fn a_webhook_binding_that_states_no_verification_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace(r#"transport = "socket""#, r#"transport = "webhook""#),
    ));
    assert!(
        error.contains("states no `verification`"),
        "an unverified open endpoint must be refused, not defaulted:\n{error}"
    );
}

/// The escape hatch is *saying so*, not staying quiet — and it is a different thing from silence.
#[test]
fn a_webhook_binding_may_declare_itself_unverifiable_deliberately() {
    let source = fixture(
        &GOOD
            .replace(
                r#"transport = "socket""#,
                "transport = \"webhook\"\nverification = \"none\"",
            )
            // A webhook must also say how it is registered — the setup rule below.
            .replace(
                "[channels.payload]",
                "[channels.setup]\nsteps = [\"Paste the Request URL into the Acme dashboard\"]\n\n\
                 [channels.payload]",
            ),
    );
    let connector = load(&source);
    assert_eq!(
        connector.channel("hook").expect("loads").verification,
        Some(VerificationScheme::None),
        "`none` is a stated position the manifest can publish loudly, not an absence"
    );
}

#[test]
fn verification_on_a_transport_that_cannot_use_it_is_refused() {
    let source = fixture(&GOOD.replace("events = [", "verification = \"none\"\nevents = ["));
    let error = refuse(&source);
    assert!(
        error.contains("which only the `webhook` transport uses"),
        "a socket binding is authenticated by the connection that opened it:\n{error}"
    );
}

/// Slack's own published parameters, which is the point: the matrix is filled from vendor
/// documentation rather than from a shape this repository invented.
const HMAC: &str = r#"
[[channels]]
name = "hook"
transport = "webhook"
events = ["thing.created"]

[channels.verification.hmac]
algorithm = "sha256"
encoding = "hex"
header = "X-Acme-Signature"
prefix = "v0="
signed = "v0:{timestamp}:{body}"
timestamp = { source = "header", name = "X-Acme-Timestamp" }
secret = "acme.webhook_secret"
tolerance = "5m"

[channels.setup]
steps = ["Paste the Request URL into the Acme dashboard"]
"#;

#[test]
fn a_timestamped_hmac_scheme_loads_with_its_window_and_its_selector() {
    let connector = load(&fixture(HMAC));
    let Some(VerificationScheme::Hmac(hmac)) =
        &connector.channel("hook").expect("loads").verification
    else {
        panic!("the binding verifies with an HMAC scheme");
    };
    assert_eq!(hmac.signed, "v0:{timestamp}:{body}");
    assert_eq!(hmac.tolerance.as_deref(), Some("5m"));
    assert!(hmac.timestamp.is_some());
}

/// A signature that stays valid forever is strictly worse than no timestamp, because it reads as
/// though replay had been handled.
#[test]
fn a_timestamped_scheme_without_a_tolerance_is_refused() {
    let error = refuse(&fixture(&HMAC.replace("tolerance = \"5m\"\n", "")));
    assert!(
        error.contains("replays forever"),
        "an unbounded replay window must be refused:\n{error}"
    );
}

#[test]
fn a_timestamped_scheme_without_a_timestamp_selector_is_refused() {
    let error = refuse(&fixture(&HMAC.replace(
        "timestamp = { source = \"header\", name = \"X-Acme-Timestamp\" }\n",
        "",
    )));
    assert!(
        error.contains("fall back to its own clock"),
        "the template says the value is signed; it cannot say where it is read from:\n{error}"
    );
}

/// Twilio's scheme, written as an author would write it — **the story's failing-first test.**
///
/// `X-Twilio-Signature` is base64(HMAC-SHA1(AuthToken, url + sorted form fields)), and before C-188
/// every one of those axes but the digest and the encoding was unwritable: `signed` admitted `{body}`
/// and `{timestamp}` and nothing else, so `providers/twilio.toml` shipped its events with **no
/// `[[channels]]` binding at all** and a test asserted that absence. This is the assertion that the
/// file an author writes now loads.
///
/// It is a declarability test and deliberately stops there. That the declaration *reproduces
/// Twilio's own published signature* is a different claim and lives in
/// `verification_conformance.rs`, which checks it against the vendor's worked example — because a
/// scheme that loads and verifies nothing is the outcome this story exists to avoid, not achieve.
#[test]
fn twilios_url_and_sorted_form_scheme_is_declarable() {
    let connector = load(&fixture(
        r#"
[[channels]]
name = "hook"
transport = "webhook"
events = ["thing.created"]

[channels.verification.hmac]
algorithm = "sha1"
encoding = "base64"
header = "X-Twilio-Signature"
signed = "{url}{sorted_form}"
secret = "acme.webhook_secret"

[channels.setup]
steps = ["Paste the Request URL into the Acme dashboard"]
"#,
    ));
    let Some(VerificationScheme::Hmac(hmac)) =
        &connector.channel("hook").expect("loads").verification
    else {
        panic!("the binding verifies with an HMAC scheme");
    };
    assert_eq!(hmac.signed, "{url}{sorted_form}");
    assert_eq!(
        hmac.timestamp, None,
        "Twilio signs no timestamp, so it declares no selector and no window — the vendor's own \
         scheme, not an omission"
    );
    assert_eq!(hmac.tolerance, None);
}

/// `{sorted_form}` covers the payload, so it satisfies the rule `{body}` used to be the only way to
/// satisfy — and `{url}` does not, because it is a per-endpoint constant.
///
/// The pairing is the point. Widening the placeholder set is exactly the change that could have
/// turned "the signed string must cover the body" into a rule that no longer means what it said,
/// and `verification_conformance.rs` demonstrates the forgery `signed = "{url}"` would ship.
#[test]
fn a_signed_template_covering_only_the_url_is_refused() {
    let error = refuse(&fixture(
        r#"
[[channels]]
name = "hook"
transport = "webhook"
events = ["thing.created"]

[channels.verification.hmac]
algorithm = "sha1"
encoding = "base64"
header = "X-Twilio-Signature"
signed = "{url}"
secret = "acme.webhook_secret"

[channels.setup]
steps = ["Paste the Request URL into the Acme dashboard"]
"#,
    ));
    assert!(
        error.contains("never interpolates {body}") && error.contains("{sorted_form}"),
        "the request URL is the same for every delivery to one endpoint, so a signature over it \
         alone verifies every forged payload forever:\n{error}"
    );
}

#[test]
fn a_signed_template_the_host_cannot_fill_is_refused() {
    let error = refuse(&fixture(
        &HMAC.replace("v0:{timestamp}:{body}", "v0:{timestamp}:{nonce}"),
    ));
    assert!(
        error.contains("the host can fill only"),
        "an unfillable template would fail open or fail confusingly:\n{error}"
    );
}

/// **The refusal that keeps a signature meaning something.**
///
/// Every other rule in this section is about a declaration that would fail; this one is about a
/// declaration that would *succeed* at authenticating the wrong thing. `signed = "v0:{timestamp}:"`
/// is well formed, names only fillable placeholders, carries its selector and carries its window — and
/// signs a string the payload never enters, so one captured signature verifies every forged body for
/// the length of the window. `verification_conformance.rs` demonstrates the forgery itself before
/// demanding this refusal; here it is the loader rule.
#[test]
fn a_signed_template_that_never_interpolates_the_body_is_refused() {
    let error = refuse(&fixture(
        &HMAC.replace("v0:{timestamp}:{body}", "v0:{timestamp}:"),
    ));
    assert!(
        error.contains("never interpolates {body}"),
        "a signed string the body never enters authenticates every forged payload:\n{error}"
    );
}

/// A window is parsed, not merely required — see `inbound::parse_tolerance`.
#[test]
fn a_tolerance_that_is_not_a_duration_is_refused() {
    let error = refuse(&fixture(&HMAC.replace("\"5m\"", "\"banana\"")));
    assert!(
        error.contains("not a window a host can apply"),
        "a window nobody can read leaves the real window to whatever a host decides:\n{error}"
    );
}

/// A count too large to scale is refused through the loader, in **both** build profiles.
///
/// `parse_tolerance` scaled with `*`, so this declaration panicked inside `provider::load` in a debug
/// build and, in a release build, wrapped `i64::MAX * 60` to `-60` — a negative window that satisfies
/// every remaining check and therefore *loaded*. A declared window no host could apply is the defect
/// item 2 of this story exists to close, so the overflow class belongs in the same gate.
#[test]
fn a_tolerance_too_large_to_scale_is_refused_by_the_loader() {
    let error = refuse(&fixture(
        &HMAC.replace("\"5m\"", "\"9223372036854775807m\""),
    ));
    assert!(
        error.contains("too large to be a window"),
        "an overflowing count must come back as a refusal, never as a wrapped window:\n{error}"
    );
}

/// Finding the timestamp would mean parsing the bytes whose trustworthiness it helps decide.
#[test]
fn a_verification_timestamp_read_from_the_body_is_refused() {
    let error = refuse(&fixture(&HMAC.replace(
        "{ source = \"header\", name = \"X-Acme-Timestamp\" }",
        "{ source = \"body\", name = \"event.created_at\" }",
    )));
    assert!(
        error.contains("inverts the order verification depends on"),
        "verification runs before parsing, so its inputs cannot come from the parse:\n{error}"
    );
}

/// The format axis is declared beside the selector: *where* the timestamp is read from, and *how* it
/// is spelled. Absent means unix seconds, which is what Slack, Stripe and GitHub send.
#[test]
fn a_timestamp_format_loads_beside_its_selector() {
    let connector = load(&fixture(&HMAC.replace(
        "secret = \"acme.webhook_secret\"",
        "timestamp_format = \"rfc3339\"\nsecret = \"acme.webhook_secret\"",
    )));
    let Some(VerificationScheme::Hmac(hmac)) =
        &connector.channel("hook").expect("loads").verification
    else {
        panic!("the binding verifies with an HMAC scheme");
    };
    assert_eq!(hmac.timestamp_format, Some(TimestampFormat::Rfc3339));

    let default = load(&fixture(HMAC));
    let Some(VerificationScheme::Hmac(hmac)) =
        &default.channel("hook").expect("loads").verification
    else {
        panic!("the binding verifies with an HMAC scheme");
    };
    assert_eq!(
        hmac.timestamp_format, None,
        "an omitted format is unix seconds, and stays absent from the encoding"
    );
}

#[test]
fn a_webhook_secret_that_no_credential_declares_is_refused() {
    let error = refuse(&fixture(
        &HMAC.replace("acme.webhook_secret", "acme.unknown_secret"),
    ));
    assert!(
        error.contains("which no `[[auth]]` block declares"),
        "an inbound secret is a credential like any other:\n{error}"
    );
}

/// The two directions must not share one value: a bearer spent verifying is a bearer that has been
/// handed to whoever can reach the endpoint.
#[test]
fn a_webhook_secret_declared_as_an_outbound_credential_is_refused() {
    let error = refuse(&fixture(&HMAC.replace("acme.webhook_secret", "acme.token")));
    assert!(
        error.contains(r#"scheme = "signing""#),
        "a verification secret must be declared for that purpose:\n{error}"
    );
}

#[test]
fn an_operation_cannot_authenticate_with_a_signing_credential() {
    let source = fixture(GOOD).replace(
        "[[operations]]\nid = \"acme-reply\"",
        "[[operations]]\nid = \"acme-reply\"\nauth = [{ credentials = [\"acme.webhook_secret\"] }]",
    );
    let error = refuse(&source);
    assert!(
        error.contains("never placed in an outgoing one"),
        "the complement of the rule above — a signing secret has no outbound placement:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// Polling: the cursor carries the correctness, because the schedule cannot
// ---------------------------------------------------------------------------------------------

/// flux's cron is best-effort — a restart drops ticks and replays none of them — so a poll that
/// cannot resume from a recorded position loses events with nothing to detect it. The cursor is
/// mandatory for that reason and not as a stylistic preference.
#[test]
fn a_poll_binding_without_a_cursor_is_refused() {
    let source = fixture(
        r#"
[[channels]]
name = "sweep"
transport = "poll"
interval = "5m"
"#,
    );
    let error = refuse(&source);
    assert!(
        error.contains("a restart drops ticks"),
        "the refusal must explain why the cursor is what makes a poll correct:\n{error}"
    );
}

#[test]
fn a_poll_binding_with_a_cursor_loads_and_may_omit_its_events() {
    let connector = load(&fixture(
        r#"
[[channels]]
name = "sweep"
transport = "poll"
cursor = "acme-reply"
interval = "5m"
"#,
    ));
    let channel = connector.channel("sweep").expect("loads");
    assert_eq!(channel.transport, Transport::Poll);
    assert_eq!(channel.cursor.as_deref(), Some("acme-reply"));
    assert!(
        channel.events.is_empty(),
        "a poll carries its cursor, not an event list"
    );
}

#[test]
fn a_cursor_on_a_transport_that_is_not_polled_is_refused() {
    let error = refuse(&fixture(&GOOD.replace(
        "events = [\"thing.created\"]",
        "cursor = \"acme-reply\"\nevents = [\"thing.created\"]",
    )));
    assert!(
        error.contains("which only the `poll` transport uses"),
        "a socket binding is woken by the vendor, not by a schedule:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// One namespace per service, and one spelling rule
// ---------------------------------------------------------------------------------------------

/// Neither list has a duplicate; only their union does. No single-kind pass can see this.
#[test]
fn an_event_and_an_operation_may_not_share_a_name() {
    // Rename the event *and* the reference to it, so the collision is the only problem in the file.
    let source = fixture(GOOD).replace("thing.created", "acme-reply");
    let error = refuse(&source);
    assert!(
        error.contains("names both an operation and an event"),
        "the three member kinds share one namespace:\n{error}"
    );
}

#[test]
fn a_channel_and_an_operation_may_not_share_a_name() {
    let error = refuse(&fixture(
        &GOOD.replace(r#"name = "hook""#, r#"name = "acme-reply""#),
    ));
    assert!(
        error.contains("names both an operation and a channel binding"),
        "the three member kinds share one namespace:\n{error}"
    );
}

/// A within-kind duplicate is reported once, by the pass that owns that kind — not twice, once here
/// and once by the namespace check.
#[test]
fn a_duplicate_operation_id_is_reported_once_and_not_also_as_a_namespace_collision() {
    let source = fixture(GOOD).replace(
        "[[events]]\nname = \"thing.created\"",
        "[[operations]]\nid = \"acme-reply\"\nmethod = \"GET\"\npath = \"/other\"\nrisk = \"medium\"\n\
         direction = \"read\"\nidempotency = \"non_idempotent\"\n\n[[events]]\nname = \"thing.created\"",
    );
    let error = refuse(&source);
    assert_eq!(
        error.matches("acme-reply").count(),
        1,
        "one problem must produce one line, or an author fixes one thing and sees two:\n{error}"
    );
}

/// An event keeps its vendor name. Slack's really is `app_mention`, and respelling it would be this
/// repository renaming someone else's API.
#[test]
fn an_event_name_may_carry_the_vendors_own_dots_and_underscores() {
    let source = fixture(GOOD).replace("thing.created", "app_mention.v2");
    let connector = load(&source);
    assert!(connector.event("app_mention.v2").is_some());
}

#[test]
fn an_event_name_that_could_not_travel_in_an_address_is_refused() {
    let source = fixture(GOOD).replace("thing.created", "Thing/Created");
    let error = refuse(&source);
    assert!(
        error.contains("invalid `name`"),
        "a member name is an address fragment:\n{error}"
    );
}

/// `$a-b` reads as a subtraction, so a hyphen in a payload key would bind something else entirely.
#[test]
fn a_payload_key_that_is_not_a_flux_symbol_is_refused() {
    let error = refuse(&fixture(
        &GOOD.replace("room = \"event.room\"", "my-room = \"event.room\""),
    ));
    assert!(
        error.contains("not snake case"),
        "a payload key is bound as a Flux symbol:\n{error}"
    );
}

#[test]
fn a_payload_source_path_with_an_empty_segment_is_refused() {
    let error = refuse(&fixture(&GOOD.replace("event.room", "event..room")));
    assert!(
        error.contains("empty segment"),
        "payload paths reuse the `wire` dotted grammar:\n{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// The shipped provider, and the address the composition rests on
// ---------------------------------------------------------------------------------------------

/// Slack is the proving vendor: flux's `adapters/slack.rs` ends by hand-building a
/// `chat.postMessage` from the message it just received, and this is that adapter's two halves
/// declared as data.
#[test]
fn the_shipped_slack_bindings_describe_both_of_slacks_real_transports() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../providers/slack.toml")
            .canonicalize()
            .expect("providers/slack.toml exists"),
    )
    .expect("providers/slack.toml reads");
    let connector = shipped_provider::load_definition("slack", &source)
        .expect("the shipped slack provider loads")
        .connector;

    let socket = connector
        .channel("socket")
        .expect("slack declares a socket binding");
    let events_api = connector
        .channel("events-api")
        .expect("slack declares an events-api binding");

    assert_eq!(socket.transport, Transport::Socket);
    assert_eq!(events_api.transport, Transport::Webhook);

    // Same events, same payload map, same reply — two transports. That is what makes "inbound is an
    // abstraction over transports" a claim this repository demonstrates rather than asserts.
    assert_eq!(socket.events, events_api.events);
    assert_eq!(socket.payload, events_api.payload);
    assert_eq!(socket.reply, events_api.reply);

    // Only the webhook carries a signature, and only it needs one.
    assert!(socket.verification.is_none());
    assert!(matches!(
        events_api.verification,
        Some(VerificationScheme::Hmac(_))
    ));

    // The reply is an operation the pipeline already emits — nothing new is generated for it.
    let reply = socket.reply.as_ref().expect("the binding replies");
    assert_eq!(reply.operation, "slack-chat-post-message");
    assert!(connector.operation(&reply.operation).is_some());
    assert_eq!(reply.result.as_deref(), Some("text"));
}

/// The three member kinds share one address form, so the `#` fragment needs no kind discriminator.
#[test]
fn every_member_kind_addresses_and_round_trips_through_one_oip_form() {
    let connector = load(&fixture(GOOD));

    for name in ["acme-reply", "thing.created", "hook"] {
        let oip = connector
            .oip_of_member("default", name)
            .unwrap_or_else(|| panic!("{name} addresses"));
        let rendered = oip.to_string();
        assert_eq!(
            rendered,
            format!("com.acme.api:v1#{name}"),
            "the reserved `default` service is elided from every published address"
        );
        assert_eq!(
            connector_spec::Oip::parse(&rendered).expect("an oip round-trips"),
            oip
        );
    }
}

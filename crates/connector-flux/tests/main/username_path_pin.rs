//! C-475: a Basic username may also pin an exact request path position.

use connector_flux::emit_operation;
use connector_spec::provider;

const DEFINITION: &str = r#"
id = "twilio"
vendor = "Twilio"
authority = "com.twilio.api"
api_version = "2010-04-01"
base_url = "https://api.twilio.com/2010-04-01"
description = "fixture"
default_auth = [{ credentials = ["twilio.basic_auth"] }]

[[auth]]
name = "twilio.basic_auth"
scheme = "basic"
env = ["TWILIO_AUTH_TOKEN"]
user_env = ["TWILIO_ACCOUNT_SID"]
description = "fixture"

[[operations]]
id = "twilio-recording-get"
method = "GET"
direction = "read"
path = "/Accounts/{AccountSid}/Recordings/{Sid}.json"
description = "fixture"
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "Sid"
required = true
description = "recording sid"
schema = { type = "string" }

[[config]]
name = "account_sid"
label = "Account SID"
help = "The same Account SID authenticates and scopes every request"
example = "AC00000000000000000000000000000000"
format = "token"
binds = "username.twilio.basic_auth"
also_binds = ["path.AccountSid"]

[[config]]
name = "auth_token"
label = "Auth Token"
help = "Twilio Auth Token"
format = "token"
secret = true
binds = "credential.twilio.basic_auth"
"#;

#[test]
fn a_username_head_emits_one_qualified_non_caller_path_pin() {
    let loaded = provider::load("twilio", DEFINITION).expect("the username-backed pin loads");
    let operation = loaded
        .connector
        .operation("twilio-recording-get")
        .expect("the fixture operation exists");
    let flux = emit_operation(&loaded.connector, operation).expect("the operation emits");

    assert!(
        flux.contains("AccountSid = \"{username.twilio.basic_auth}\""),
        "the emitted placeholder must retain its configuration kind:\n{flux}"
    );
    assert!(
        flux.starts_with("op twilio-recording-get(Sid: String)"),
        "AccountSid is operator-pinned, not caller supplied:\n{flux}"
    );
}

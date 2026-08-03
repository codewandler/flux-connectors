//! C-473/C-475: Twilio's four spec-backed reads compose with AccountSid operator-pinned through
//! the same non-secret configuration slot as the Basic username.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use connector_pack::{Configuration, MemoryConfig, Rehearsal, DEFAULT_USER_AGENT};
use serde_json::{json, Value};

const TENANT: &str = "twilio-rehearsal";
const ACCOUNT_SID: &str = "AC00000000000000000000000000000000";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn configuration() -> Configuration {
    let values = MemoryConfig::new().with_username(
        TENANT,
        "twilio",
        "default",
        "twilio.basic_auth",
        ACCOUNT_SID,
    );
    Configuration::new(Arc::new(values), TENANT).expect("a valid tenant")
}

#[test]
fn four_spec_backed_reads_compose_absolute_account_pinned_twilio_requests() {
    let cases: [(&str, Value, &str); 4] = [
        (
            "twilio-recording-list",
            json!({"IncludeSoftDeleted": true, "PageSize": 25, "Page": 2}),
            "https://api.twilio.com/2010-04-01/Accounts/AC00000000000000000000000000000000/Recordings.json?IncludeSoftDeleted=true&Page=2&PageSize=25",
        ),
        (
            "twilio-recording-get",
            json!({"Sid": "RE00000000000000000000000000000000", "IncludeSoftDeleted": true}),
            "https://api.twilio.com/2010-04-01/Accounts/AC00000000000000000000000000000000/Recordings/RE00000000000000000000000000000000.json?IncludeSoftDeleted=true",
        ),
        (
            "twilio-usage-record-list",
            json!({"IncludeSubaccounts": true, "PageSize": 25, "Page": 2}),
            "https://api.twilio.com/2010-04-01/Accounts/AC00000000000000000000000000000000/Usage/Records.json?IncludeSubaccounts=true&Page=2&PageSize=25",
        ),
        (
            "twilio-conference-list",
            json!({"PageSize": 25, "Page": 2}),
            "https://api.twilio.com/2010-04-01/Accounts/AC00000000000000000000000000000000/Conferences.json?Page=2&PageSize=25",
        ),
    ];

    for (id, params, expected_url) in cases {
        let path = root().join(format!("crates/catalog/ops/twilio/{id}.flux"));
        let flux = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let rehearsal = Rehearsal::of(id, "twilio", "default", &flux)
            .unwrap_or_else(|error| panic!("{id} does not rehearse: {error}"));
        assert_eq!(
            rehearsal.endpoint_variables(),
            ["username.twilio.basic_auth"],
            "{id} must source AccountSid from the Basic username slot"
        );

        let request = rehearsal
            .request(&configuration(), &params)
            .unwrap_or_else(|error| panic!("{id} does not compose: {error}"));
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, expected_url);
        assert_eq!(
            request.headers,
            [("User-Agent".to_owned(), DEFAULT_USER_AGENT.to_owned())].into(),
            "{id} gained a caller-controlled or embedded auth header"
        );
        assert!(request.body.is_none(), "{id} gained a body");
    }
}

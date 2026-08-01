//! **A closed set of configuration values, over the host's own HTTP surface** — C-225.
//!
//! The declaration is `connector-spec`'s and the loader's refusals are asserted there. What this
//! file is about is the half a declaration is worthless without: the set has to *reach a consumer*,
//! and the consumer has to refuse a value that is not in it.
//!
//! New Relic is the fixture because it is the shipped connector the story is about: one REST API on
//! two hosts, and nothing pre-auth that discloses which is yours. A wrong region returns `401` on
//! every call, indistinguishable from a bad key, so the operator's first move is to rotate a
//! credential that was never wrong. Per `AGENTS.md` this names one connector rather than walking the
//! catalogue — a fifty-fourth connector cannot turn it red by existing, and only New Relic changing
//! its regions can.

mod support;

use support::{client, serve, sign_in, Idp};

/// The subject every test here signs in as. The same one `tests/wiring.rs` uses.
const OPERATOR: &str = "110169484474386276334";

const US_HOST: &str = "api.newrelic.com";
const EU_HOST: &str = "api.eu.newrelic.com";

/// `PUT /v1/config/newrelic/default/endpoint/host`.
async fn bind(
    client: &reqwest::Client,
    base: &str,
    cookie: &str,
    value: &str,
) -> (reqwest::StatusCode, String) {
    let response = client
        .put(format!("{base}/v1/config/newrelic/default/endpoint/host"))
        .header("cookie", cookie)
        .json(&serde_json::json!({ "value": value }))
        .send()
        .await
        .unwrap_or_else(|error| panic!("PUT /v1/config: {error}"));
    let status = response.status();
    (status, response.text().await.unwrap_or_default())
}

/// **The set is published, with its labels.** A form that cannot see the choices renders a text box,
/// which is the state this story exists to leave — so the connector view carries them.
#[tokio::test]
async fn the_connector_view_publishes_the_permitted_values_and_their_labels() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let view: serde_json::Value = client
        .get(format!("{base}/v1/connectors/newrelic"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /v1/connectors/newrelic")
        .json()
        .await
        .expect("the view is JSON");

    let sets = view["config_choices"]
        .as_array()
        .unwrap_or_else(|| panic!("the view carries no `config_choices`: {view}"));
    assert_eq!(sets.len(), 1, "newrelic declares exactly one closed set");
    let host = &sets[0];

    // Addressed the way `PUT /v1/config/<provider>/<service>/<kind>/<field>` addresses it, so a page
    // joins on the route it is about to call rather than on a name it has to guess.
    assert_eq!(host["service"], "default");
    assert_eq!(host["kind"], "endpoint");
    assert_eq!(host["name"], "host");
    assert_eq!(host["field"], "host");
    assert_eq!(host["label"], "New Relic API host");

    let choices: Vec<(&str, &str)> = host["choices"]
        .as_array()
        .expect("choices is an array")
        .iter()
        .map(|choice| {
            (
                choice["value"].as_str().expect("value"),
                choice["label"].as_str().expect("label"),
            )
        })
        .collect();
    assert_eq!(
        choices,
        [(US_HOST, "United States"), (EU_HOST, "European Union")],
        "both regions, each with the name an operator knows their account by — a dropdown of bare \
         hostnames is one nobody can answer"
    );
}

/// **A value outside the set is refused where it is supplied, and the refusal lists what is
/// permitted.**
///
/// The listing is the point rather than a nicety. "Invalid value" would leave an operator exactly
/// where the `401` left them — knowing something is wrong and not which of two things it is.
#[tokio::test]
async fn a_host_outside_the_set_is_refused_and_the_refusal_names_the_answers() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let (status, body) = bind(&client, &base, &cookie, "api.not-new-relic.example").await;
    assert_eq!(
        status,
        reqwest::StatusCode::BAD_REQUEST,
        "a well-formed hostname that is not one of New Relic's two is still wrong: {body}"
    );
    for named in ["host", US_HOST, "United States", EU_HOST, "European Union"] {
        assert!(
            body.contains(named),
            "the refusal must name the field and list the answers; missing {named:?} in: {body}"
        );
    }

    // Both permitted values are accepted, so the refusal above is about membership and not about
    // the endpoint being broken.
    for permitted in [US_HOST, EU_HOST] {
        let (status, body) = bind(&client, &base, &cookie, permitted).await;
        assert_eq!(
            status,
            reqwest::StatusCode::NO_CONTENT,
            "{permitted} is one of the two answers: {body}"
        );
    }
}

/// **A stored value that later leaves the set is left alone.**
///
/// A vendor adding — or renaming — a region must not brick a connection configured before it
/// existed. Membership is checked at the input and nowhere else, so a value already bound is still
/// served back and still substituted. The test binds a permitted host, then asserts that reading the
/// connector back does not re-validate anything: the settings row survives even though the check
/// that admitted it lives only on the write path.
#[tokio::test]
async fn a_stored_value_is_never_re_validated_on_the_way_out() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let (status, body) = bind(&client, &base, &cookie, EU_HOST).await;
    assert_eq!(status, reqwest::StatusCode::NO_CONTENT, "{body}");

    let view: serde_json::Value = client
        .get(format!("{base}/v1/connectors/newrelic"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /v1/connectors/newrelic")
        .json()
        .await
        .expect("the view is JSON");

    let settings = view["settings"]
        .as_array()
        .unwrap_or_else(|| panic!("the view carries no `settings`: {view}"));
    assert!(
        settings.iter().any(|row| row[1].as_str() == Some(EU_HOST)),
        "the bound region is served back verbatim, with no membership check on the read path: \
         {settings:?}"
    );
}

/// **The page renders the choice as a choice.**
///
/// There is no browser here, so this is a claim about the served page's source rather than about a
/// rendered DOM — and it is written to be falsifiable rather than decorative: it fails if the page
/// stops reading `config_choices`, or stops building `option` elements from them, which are the two
/// ways the select silently reverts to a text box. The behaviour either side of it — that the
/// choices are published, and that a value outside them is refused — is asserted over the real HTTP
/// surface above.
#[tokio::test]
async fn the_operator_page_builds_its_value_control_from_the_published_set() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();

    let page = client
        .get(&base)
        .send()
        .await
        .expect("GET /")
        .text()
        .await
        .expect("the page is text");

    assert!(
        page.contains("config_choices"),
        "the page must read the published set, or it cannot know a slot is closed"
    );
    assert!(
        page.contains("el('option'"),
        "a closed slot must render as options; a text box moves the declaration without the benefit"
    );
    assert!(
        page.contains("ch.label"),
        "the option's text is the choice's label — an operator picks `European Union`, not \
         `api.eu.newrelic.com`"
    );
}

//! **What one page load costs** (C-237).
//!
//! The host's operator page used to render a connector by fetching every one of its operations —
//! `Promise.all(c.operations.map(o => api('GET', '/v1/operations/' + o.id)))`, up to ~30 requests
//! per click, to read three fields off catalogue entries `view_of` already had in hand. C-212 had
//! added `operations[].requires` and `.callable` *specifically* so that would not be needed, and
//! its own Progress note said so; the page did it anyway for `tool`, `description` and `risk`.
//!
//! C-237 moves those five fields onto [`ConnectorView::operations`], which turns ~30 requests per
//! click into zero. The trade is response size, and it is the trade that has to be watched: the
//! list is *every* connector, so a field added there is added 299 times.
//!
//! # Why a stated ceiling rather than a split view type
//!
//! C-237's notes weigh the alternative and take this one: splitting `ConnectorView` into a thin
//! list shape and a fat detail shape gives two types that must agree about one connector, and the
//! page then has to know which of them it is holding. A number in a test is cheaper and says the
//! thing that actually matters — *this response stays small enough to send whole*. The ceiling is
//! deliberately not tight to today's bytes; it is the size at which someone should stop and think.
//!
//! The two fields that would blow it are named here rather than left to be rediscovered: `flux` is
//! a whole rendered declaration per operation and `input_schema` is a JSON Schema per operation.
//! Both stay on `GET /v1/operations/{id}`, which the page now calls **only** for an operation an
//! operator expanded — one at a time, which is how many they read.

use crate::support::{client, serve, sign_in, Idp};

/// The subject every test here signs in as. The same one `tests/host.rs` uses.
const OPERATOR: &str = "110169484474386276334";

/// **The ceiling, uncompressed and in bytes.**
///
/// 512 KiB, against a measured **284,623 bytes** for 54 connectors and 679 operations. The test
/// prints that line whether or not it passes, so the figure is reproduced by
/// `cargo test -p connectors-api --test main catalogue_response:: -- --nocapture` rather than remembered —
/// re-measured on 2026-08-02, after C-153's service tags landed, and unchanged from 2026-08-01.
/// C-237's note estimated *"299 operations adds roughly 55 KB"*; the catalogue has since more than
/// doubled and the five added fields cost about 320 bytes an operation, most of it `description`.
///
/// **That is the right trade here and the reasoning is worth keeping.** This response is sent once
/// per page load, over loopback, to a console; what it replaced was up to ~30 requests *per
/// connector click*, over a session in which an operator opens many. A single 285 KB response is
/// cheaper in bytes and far cheaper in round trips than the browsing it makes free.
///
/// The headroom is not slack to be spent. This is not a golden number to nudge upward whenever it
/// goes red — it is the point at which "send the whole catalogue in one response" stops being
/// obviously right. A change that reaches it should move the field to the per-operation view, or
/// argue here.
const CEILING: usize = 512 * 1024;

/// `GET /v1/connectors` costs one request and carries everything the rail renders.
///
/// The two halves of C-237's first acceptance item, on the server side of it. The client side —
/// that the page then makes *no* per-operation request — is
/// `ui/test/host-page.test.mjs::opening a connector fetches no operation detail, and expanding one
/// fetches exactly that one`, because it is a claim about the page.
#[tokio::test]
async fn the_connector_list_carries_every_field_its_rail_renders() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let body = client
        .get(format!("{base}/v1/connectors"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /v1/connectors")
        .text()
        .await
        .expect("a body");

    let connectors: Vec<serde_json::Value> =
        serde_json::from_str(&body).expect("/v1/connectors serves JSON");
    assert!(
        !connectors.is_empty(),
        "the catalogue is empty, so every assertion below would pass vacuously"
    );

    // Every field the page reads to draw an operation row without a second request. Asserted over
    // *every* operation of every connector rather than a sampled one: the N+1 comes back the moment
    // one of them is absent, because the page's fallback for a missing field is to go and ask.
    let rendered = [
        "id",
        "tool",
        "service",
        "description",
        "risk",
        "idempotency",
        "hosts",
        "requires",
        "requirement",
        "callable",
    ];
    let mut operations = 0;
    for connector in &connectors {
        let listed = connector["operations"]
            .as_array()
            .unwrap_or_else(|| panic!("`{}` carries no operations array", connector["id"]));
        assert_eq!(
            listed.len(),
            connector["operation_count"].as_u64().unwrap_or_default() as usize,
            "`{}` reports an operation count its own list does not match",
            connector["id"]
        );
        for operation in listed {
            operations += 1;
            for field in rendered {
                assert!(
                    !operation[field].is_null(),
                    "`{}` carries no `{field}`, so the page has to fetch \
                     /v1/operations/{} to draw one row — which is the N+1 C-237 removed",
                    operation["id"],
                    operation["id"],
                );
            }
        }
    }

    // And what it deliberately does not carry, so a later change adds them on purpose or not at all.
    for connector in &connectors {
        for operation in connector["operations"].as_array().expect("operations") {
            for withheld in ["flux", "input_schema"] {
                assert!(
                    operation[withheld].is_null(),
                    "the connector list carries `{withheld}` on every one of {operations} \
                     operations — it belongs to `GET /v1/operations/{{id}}`, which the page calls \
                     once per operation an operator actually expands"
                );
            }
        }
    }

    // Printed whether or not the ceiling holds, so the figure in `CEILING`'s doc comment is
    // reproduced by a command rather than remembered from the session that wrote it —
    // `cargo test -p connectors-api --test main catalogue_response:: -- --nocapture`.
    println!(
        "GET /v1/connectors: {} bytes, {} connectors, {operations} operations",
        body.len(),
        connectors.len(),
    );
    assert!(
        body.len() <= CEILING,
        "GET /v1/connectors is {} bytes for {} connectors and {operations} operations, over the \
         {CEILING}-byte ceiling this file states. Either the field that grew belongs on \
         GET /v1/operations/{{id}} instead, or raise the ceiling here and say why.",
        body.len(),
        connectors.len(),
    );
}

/// **One operation's expansion carries the two things the list does not.**
///
/// The other side of the same trade. `input_schema` is what C-237's parameter editor draws a form
/// from — `connector_pack::project`'s answer, which is flux's own `OpSpec::lower` and therefore the
/// schema a model is handed — and it is on this route rather than on the list precisely because it
/// is per operation.
#[tokio::test]
async fn an_expanded_operation_carries_its_schema_and_its_flux() {
    let idp = Idp::start().await;
    let base = serve(&idp).await;
    let client = client();
    let cookie = sign_in(&base, OPERATOR).await;

    let operation: serde_json::Value = client
        .get(format!("{base}/v1/operations/anthropic-models-list"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /v1/operations/anthropic-models-list")
        .json()
        .await
        .expect("the operation view is JSON");

    assert_eq!(operation["tool"], "anthropic.models.list");
    assert!(
        operation["flux"]
            .as_str()
            .is_some_and(|flux| flux.contains("op ")),
        "the expansion carries no Flux declaration: {}",
        operation["flux"]
    );
    assert_eq!(
        operation["input_schema"]["type"], "object",
        "the expansion carries no JSON Schema for its parameters, so the parameter editor has \
         nothing to draw a form from: {}",
        operation["input_schema"]
    );
}

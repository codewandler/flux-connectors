//! `providers/clickup.toml` exists, emits analyzable Flux, and holds the two properties C-178
//! ships it for:
//!
//! **1. ClickUp's token travels as `Authorization: <token>`, raw — no scheme word at all.**
//! [`AuthScheme`] is a closed, five-member enum with no prefix field
//! (`crates/connector-spec/src/auth.rs:70-102`, measured by C-161's Okta probe). Okta needed a
//! prefix (`SSWS <token>`) and could not honestly get one, so C-161 recorded a refusal instead of
//! shipping a provider. ClickUp needs the *opposite* — no prefix at all — and that is exactly what
//! `AuthScheme::Header { name: "Authorization" }` already produces on the wire: the header's whole
//! value is the resolved secret and nothing else. Where Okta's story was "the trap", ClickUp's is
//! "the trap is exactly the right shape" — so this connector needs no change to `connector-spec`.
//!
//! **2. ClickUp's containment tree is five levels deep (team -> space -> folder -> list -> task),
//! and this connector does not ship every rung as its own operation.** Two rungs are deliberately
//! missing as *operations*, not as an oversight:
//!
//! - **No `clickup-team-space-list`.** A space is addressed the same way `providers/gitlab.toml`
//!   addresses a project — a caller reads the id out of ClickUp's own UI URL
//!   (`app.clickup.com/{team_id}/v/o/s/{space_id}`) and supplies it directly, the same "the id is
//!   already in the address bar" reasoning gitlab's numeric `project_id` rests on.
//! - **No `clickup-folder-list-list`.** ClickUp's own `GET /space/{space_id}/folder` response
//!   already nests each folder's lists inline (`folders[].lists`), so a caller who lists a space's
//!   folders already has every list id in the same response. A separate rung would re-fetch data
//!   the first call already returned — the "five navigation endpoints teach nothing" hazard the
//!   story names, made concrete.
//!
//! What ships instead is six operations: the parameterless verify read (`clickup-team-list`, which
//! doubles as the only reachable "top of the tree" read), `clickup-space-folder-list` (the one
//! navigation rung that earns its place, because it is where list ids come from), and the four
//! task operations the story starts from. [`OPERATIONS`] pins the exact set so a future change that
//! quietly adds a navigation rung shows up as a diff here.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, AuthScheme, Connector, HttpMethod, Idempotency};

const PROVIDER: &str = "clickup";

/// The credential this connector declares, the header it travels in, and the variable it resolves
/// from.
const CREDENTIAL: &str = "clickup.token";
const AUTH_HEADER: &str = "Authorization";
const TOKEN_ENV: &str = "CLICKUP_TOKEN";

const BASE_URL: &str = "https://api.clickup.com/api/v2";

/// The curated set, in the order `providers/clickup.toml` declares it. See the module docs for why
/// this stops short of every navigation rung ClickUp's tree actually has.
const OPERATIONS: &[&str] = &[
    "clickup-team-list",
    "clickup-space-folder-list",
    "clickup-list-task-list",
    "clickup-task-get",
    "clickup-task-create",
    "clickup-task-update",
];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// The shipped definition, through the real loader.
fn load() -> Connector {
    let path = providers_dir().join(format!("{PROVIDER}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-178 ships the ClickUp connector",
            path.display()
        )
    });
    provider::load(&format!("providers/{PROVIDER}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{PROVIDER}.toml does not load: {error}"))
        .connector
}

/// **The finding this connector exists to demonstrate.** ClickUp's token is the entire value of
/// `Authorization` — no `Bearer `, no vendor scheme word — and that is expressible today as
/// `AuthScheme::Header { name: "Authorization" }`, the same variant C-161's Okta probe proved *legal
/// to load* but wrong for a scheme that needs a prefix. ClickUp needs no prefix, so the same variant
/// that was a trap for Okta is the correct answer here.
#[test]
fn the_clickup_connector_authenticates_with_a_bare_authorization_header() {
    let connector = load();

    assert_eq!(connector.id, PROVIDER);
    assert_eq!(connector.vendor, "ClickUp");
    assert_eq!(connector.base_url, BASE_URL);

    assert_eq!(
        connector.auth.len(),
        1,
        "clickup authenticates with one credential; a second would need a reason"
    );
    let method = connector
        .auth_method(CREDENTIAL)
        .unwrap_or_else(|| panic!("clickup declares `{CREDENTIAL}`"));
    assert_eq!(
        method.scheme,
        AuthScheme::Header {
            name: AUTH_HEADER.to_string()
        },
        "ClickUp's personal API token is the entire value of `{AUTH_HEADER}` — no `Bearer `, no \
         scheme word — the same shape C-161 measured as legal but wrong for Okta's `SSWS ` prefix; \
         ClickUp is the case where that shape is exactly right"
    );
    assert_eq!(method.env, [TOKEN_ENV]);
    assert!(
        method.user_env.is_empty() && method.user_suffix.is_none(),
        "`user_env`/`user_suffix` are Basic's; a bare header credential has no user half"
    );

    for operation in &connector.operations {
        assert!(
            operation.params.header.is_empty(),
            "operation `{}` declares caller-supplied headers; `{AUTH_HEADER}` is injected by the \
             host and must never travel through the parameter surface",
            operation.id
        );
    }
}

/// **The curated-hierarchy finding.** The exact operation set names which rungs of ClickUp's
/// five-level tree (team -> space -> folder -> list -> task) this connector exposes, and — just as
/// important — which it deliberately does not: no bare "list a team's spaces" and no "list a
/// folder's lists" operation, for the reasons in the module docs.
#[test]
fn the_curated_set_stops_short_of_every_navigation_rung() {
    let connector = load();

    let declared: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        declared, OPERATIONS,
        "the operation set drifted from the curated six this connector ships — if a navigation rung \
         was added, the module docs must justify it the way clickup-space-folder-list is justified"
    );

    assert!(
        !declared.iter().any(|id| id.contains("team-space")),
        "a team's spaces are not a declared operation — a space id comes from ClickUp's own UI URL, \
         the same way gitlab's numeric project_id does"
    );
    assert!(
        !declared.iter().any(|id| id.contains("folder-list-list")
            || id.contains("folder") && id.contains("list") && id.contains("get")),
        "a folder's lists are not a separate operation — GET /space/{{space_id}}/folder already \
         nests each folder's lists inline, so clickup-space-folder-list already returns them"
    );
}

/// Every declared write is `non_idempotent` — ClickUp documents no idempotency key on task create
/// or update, and this repository's convention (first written for `providers/asana.toml`) is that
/// a write is never marked idempotent on the strength of an accident of implementation; only a
/// vendor-documented guarantee earns the label.
#[test]
fn every_write_is_conservatively_non_idempotent() {
    let connector = load();

    for operation in &connector.operations {
        if operation.method != HttpMethod::Get {
            assert_eq!(
                operation.idempotency,
                Idempotency::NonIdempotent,
                "operation `{}` is a {:?} marked idempotent; ClickUp documents no idempotency key \
                 for it",
                operation.id,
                operation.method
            );
        }
    }
}

/// Every operation emits analyzable Flux through the real emitter — the same route
/// `shipped_modules.rs` exercises, so a hand-authored TOML that loads but does not emit is still
/// caught here.
#[test]
fn every_operation_emits() {
    let connector = load();
    assert!(!connector.operations.is_empty());
    for operation in &connector.operations {
        emit_operation(&connector, operation)
            .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
    }
}

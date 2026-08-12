//! **One integration-test binary for this crate** (C-533).
//!
//! Every `.rs` file directly under `tests/` is its own crate, which Cargo links into its own
//! executable carrying the entire dependency graph — measured at 179 files, 792 executables and
//! 30 GB in `target/debug/deps`, and the disk exhaustion that failed the v0.21.0 cut. The files
//! under `tests/main/` are therefore modules of this single test target. Each one is the same
//! documented argument it always was; only the linkage changed. Run one of them with
//! `cargo test -p <package> --test main <module>::`.
//!
//! The `#[path]` attribute on every declaration is load-bearing: this file is a crate root, and a
//! crate root resolves a bare `mod x;` in its **own** directory (`tests/`) — the same rule that
//! makes `tests/common/mod.rs` reachable from every root here — never in `tests/main/`.

// Shared test scaffolding, declared once for the whole binary and reached as
// `crate::<module>` from the test modules.
mod support;

#[path = "main/catalogue_response.rs"]
mod catalogue_response;
#[path = "main/config_choices.rs"]
mod config_choices;
#[path = "main/connector_staging_compatibility.rs"]
mod connector_staging_compatibility;
#[path = "main/credential_store.rs"]
mod credential_store;
#[path = "main/dev_signin.rs"]
mod dev_signin;
#[path = "main/dry_run.rs"]
mod dry_run;
#[path = "main/gitlab_origin_live.rs"]
mod gitlab_origin_live;
#[path = "main/host.rs"]
mod host;
#[path = "main/id_token.rs"]
mod id_token;
#[path = "main/live_egress.rs"]
mod live_egress;
#[path = "main/persistence.rs"]
mod persistence;
#[path = "main/tenancy.rs"]
mod tenancy;
#[path = "main/wiring.rs"]
mod wiring;
#[path = "main/wiring_vocabulary.rs"]
mod wiring_vocabulary;

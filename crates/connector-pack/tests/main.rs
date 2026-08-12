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
#[path = "../../connector-address/tests/fixtures/origin_corpus.rs"]
mod origin_corpus;

#[path = "main/channel_plan.rs"]
mod channel_plan;
#[path = "main/configuration_value_guard.rs"]
mod configuration_value_guard;
#[path = "main/credentials.rs"]
mod credentials;
#[path = "main/differential.rs"]
mod differential;
#[path = "main/document_differential.rs"]
mod document_differential;
#[path = "main/dry_run.rs"]
mod dry_run;
#[path = "main/endpoint_configuration.rs"]
mod endpoint_configuration;
#[path = "main/exposure.rs"]
mod exposure;
#[path = "main/github_rehearsal.rs"]
mod github_rehearsal;
#[path = "main/gitlab_origin.rs"]
mod gitlab_origin;
#[path = "main/metadata_coherence.rs"]
mod metadata_coherence;
#[path = "main/microsoft_graph_rehearsal.rs"]
mod microsoft_graph_rehearsal;
#[path = "main/network_gate.rs"]
mod network_gate;
#[path = "main/openai_rehearsal.rs"]
mod openai_rehearsal;
#[path = "main/origin_grammar_parity.rs"]
mod origin_grammar_parity;
#[path = "main/path_parameter_guard.rs"]
mod path_parameter_guard;
#[path = "main/projection.rs"]
mod projection;
#[path = "main/rehearsal.rs"]
mod rehearsal;
#[path = "main/request.rs"]
mod request;
#[path = "main/service_scoped_configuration.rs"]
mod service_scoped_configuration;
#[path = "main/twilio_rehearsal.rs"]
mod twilio_rehearsal;
#[path = "main/zendesk_help_center_rehearsal.rs"]
mod zendesk_help_center_rehearsal;
#[path = "main/zendesk_messaging_rehearsal.rs"]
mod zendesk_messaging_rehearsal;
#[path = "main/zendesk_rehearsal.rs"]
mod zendesk_rehearsal;

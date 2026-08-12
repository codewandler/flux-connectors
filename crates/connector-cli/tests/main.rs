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
mod common;
#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

#[path = "main/build_and_diff.rs"]
mod build_and_diff;
#[path = "main/catalog_artifacts.rs"]
mod catalog_artifacts;
#[path = "main/catalog_document.rs"]
mod catalog_document;
#[path = "main/catalog_index.rs"]
mod catalog_index;
#[path = "main/catalog_pack.rs"]
mod catalog_pack;
#[path = "main/core_catalog.rs"]
mod core_catalog;
#[path = "main/credential_requirement.rs"]
mod credential_requirement;
#[path = "main/cut_release.rs"]
mod cut_release;
#[path = "main/dependency_fence.rs"]
mod dependency_fence;
#[path = "main/engine_free_core.rs"]
mod engine_free_core;
#[path = "main/exposure_artifacts.rs"]
mod exposure_artifacts;
#[path = "main/fixture_hygiene.rs"]
mod fixture_hygiene;
#[path = "main/flux_engine_line.rs"]
mod flux_engine_line;
#[path = "main/gitlab_default_status.rs"]
mod gitlab_default_status;
#[path = "main/inbound_artifacts.rs"]
mod inbound_artifacts;
#[path = "main/lockfile.rs"]
mod lockfile;
#[path = "main/many_spec_documents.rs"]
mod many_spec_documents;
#[path = "main/msrv_fence.rs"]
mod msrv_fence;
#[path = "main/native_plugin_migration.rs"]
mod native_plugin_migration;
#[path = "main/no_network.rs"]
mod no_network;
#[path = "main/orphaned_artifacts.rs"]
mod orphaned_artifacts;
#[path = "main/pack_links_no_http_client.rs"]
mod pack_links_no_http_client;
#[path = "main/per_provider_test_scope.rs"]
mod per_provider_test_scope;
#[path = "main/publish_closure.rs"]
mod publish_closure;
#[path = "main/readme_snippet.rs"]
mod readme_snippet;
#[path = "main/repeatability_condition_artifact.rs"]
mod repeatability_condition_artifact;
#[path = "main/runtime_axis.rs"]
mod runtime_axis;
#[path = "main/scaffold.rs"]
mod scaffold;
#[path = "main/service_units.rs"]
mod service_units;
#[path = "main/shipped_providers_build.rs"]
mod shipped_providers_build;
#[path = "main/site_catalog.rs"]
mod site_catalog;
#[path = "main/wiring.rs"]
mod wiring;

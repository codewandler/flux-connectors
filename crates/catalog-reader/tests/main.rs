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

#[path = "main/pack.rs"]
mod pack;

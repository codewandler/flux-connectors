//! The connector IR and its two front-ends.
//!
//! A connector is described once — either as a pointer at a vendor OpenAPI document plus patches, or
//! as a complete hand-authored definition — and normalized into one [`Connector`] value that
//! `connector-flux` compiles into a Flux-Lang module.
//!
//! **This crate performs no network IO.** Ingest takes bytes, so every stage here is a pure,
//! unit-testable function; fetching lives in `connector-cli` alone.

/// Placeholder error type for the crate. Replaced with real variants as the IR lands (C-2).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A provider definition could not be parsed or validated.
    #[error("invalid provider definition: {0}")]
    Invalid(String),
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;

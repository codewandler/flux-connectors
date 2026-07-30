//! Emits Flux-Lang modules from the connector IR.
//!
//! Codegen builds real [`flux_lang`] AST nodes and formats them with flux-lang's own formatter, so
//! unparseable or non-canonical output is structurally impossible. **Never emit Flux through string
//! templates** — that convention is stated in AGENTS.md, and this crate is where it is enforced.

/// Placeholder error type for the crate. Replaced with real variants as the emitter lands (C-8).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A connector could not be lowered into a Flux-Lang module.
    #[error("cannot emit Flux module: {0}")]
    Emit(String),
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;

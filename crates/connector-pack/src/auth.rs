//! The two axes of authentication — **acquisition** and **placement** — now owned by
//! [`connector_resolve::auth`] (C-538).
//!
//! The module moved rather than being copied, and it moved because of what the *plan* is: a
//! `RequestPlan` carries the placed credential and the set of strings a redactor must hold, so auth
//! that ran anywhere else would make the plan a description of a request rather than the request.
//! That is the second request-composition path this family already rejected.
//!
//! Nothing here changed with the move. `docs/designs/connector-tool-pack.md` and the target module's
//! own documentation carry the reasoning in full: why a **prefixed** header registers the bare
//! credential and not the prefixed form, why a **query** placement registers a second string
//! (percent-encoding does not preserve `+`, `/` and `=`, which is a base64 credential's whole
//! alphabet), and why [`placed_form`] is the single exhaustive answer to "does this placement
//! transform the value" rather than a rule each caller re-derives.
//!
//! This shim exists so that the crate's own call sites and doc links keep one spelling for the seam.

pub(crate) use connector_resolve::auth::{acquire, place, placed_form, query_encode, Assembled};

//! The operator's page.
//!
//! Served from this binary rather than from `web/`, and that separation is not incidental. `web/` is
//! a public GitHub Pages site, and [`C-147`]'s acceptance forbids it collecting a credential or
//! implying a live call — *"a reader must not come away believing the site called the vendor"*. This
//! surface is the exact opposite on both counts: it does call the vendor, it must say "sent", and
//! collecting a credential is the point. Its safety comes from being loopback-only and unpublished,
//! which is a property the public site structurally cannot have, and vice versa.
//!
//! [`C-147`]: https://github.com/codewandler/flux-connectors/blob/main/docs/stories/C-147-explorer-runs-an-operation.md

use axum::response::Html;

/// The single page.
pub async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

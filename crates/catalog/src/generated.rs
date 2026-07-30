//! The generated catalog data, one module per provider.
//!
//! **This file is hand-written; the modules it names are not.** `flux-connectors build` writes
//! `generated/<provider>.rs` and the `ops/<provider>/*.flux` renderings those modules embed. The
//! list below is the one thing a human adds, and it is deliberate: `build --provider zendesk`
//! compiles a single provider, so an index generated from that run would have to drop the other
//! two to stay a function of its inputs. Keeping the index by hand makes each provider's generated
//! module independent, which is what makes a scoped build sound.
//!
//! Forgetting the line is not silent: `tests/embedded_operations.rs` compares this list against
//! `providers/` and fails when they disagree.

pub(crate) mod babelforce;
pub(crate) mod freshdesk;
pub(crate) mod github;
pub(crate) mod intercom;
pub(crate) mod jira;
pub(crate) mod openai;
pub(crate) mod shopify;
pub(crate) mod slack;
pub(crate) mod zendesk;

use crate::Provider;

/// Every provider, ordered by id — the order [`crate::providers`] publishes and the order the
/// catalog's flat listing walks.
pub(crate) static PROVIDERS: &[&Provider] = &[
    &babelforce::PROVIDER,
    &freshdesk::PROVIDER,
    &github::PROVIDER,
    &intercom::PROVIDER,
    &jira::PROVIDER,
    &openai::PROVIDER,
    &shopify::PROVIDER,
    &slack::PROVIDER,
    &zendesk::PROVIDER,
];

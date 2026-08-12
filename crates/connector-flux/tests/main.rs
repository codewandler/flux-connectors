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
#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

#[path = "main/airtable_connector.rs"]
mod airtable_connector;
#[path = "main/algolia_connector.rs"]
mod algolia_connector;
#[path = "main/anthropic_connector.rs"]
mod anthropic_connector;
#[path = "main/asana_connector.rs"]
mod asana_connector;
#[path = "main/babelforce_ivr.rs"]
mod babelforce_ivr;
#[path = "main/bitbucket_connector.rs"]
mod bitbucket_connector;
#[path = "main/body_arrays.rs"]
mod body_arrays;
#[path = "main/body_encoding.rs"]
mod body_encoding;
#[path = "main/box_connector.rs"]
mod box_connector;
#[path = "main/calendly_connector.rs"]
mod calendly_connector;
#[path = "main/clickup_connector.rs"]
mod clickup_connector;
#[path = "main/cloudflare_connector.rs"]
mod cloudflare_connector;
#[path = "main/confluence_connector.rs"]
mod confluence_connector;
#[path = "main/constant_headers.rs"]
mod constant_headers;
#[path = "main/contentful_connector.rs"]
mod contentful_connector;
#[path = "main/datadog_connector.rs"]
mod datadog_connector;
#[path = "main/discord_connector.rs"]
mod discord_connector;
#[path = "main/docusign_connector.rs"]
mod docusign_connector;
#[path = "main/dropbox_connector.rs"]
mod dropbox_connector;
#[path = "main/exposure.rs"]
mod exposure;
#[path = "main/figma_connector.rs"]
mod figma_connector;
#[path = "main/flux_lang_smoke.rs"]
mod flux_lang_smoke;
#[path = "main/fly_connector.rs"]
mod fly_connector;
#[path = "main/front_connector.rs"]
mod front_connector;
#[path = "main/github_connector.rs"]
mod github_connector;
#[path = "main/gitlab_connector.rs"]
mod gitlab_connector;
#[path = "main/google_connector.rs"]
mod google_connector;
#[path = "main/graph_emitter.rs"]
mod graph_emitter;
#[path = "main/hubspot_connector.rs"]
mod hubspot_connector;
#[path = "main/input_schema_agreement.rs"]
mod input_schema_agreement;
#[path = "main/intercom_connector.rs"]
mod intercom_connector;
#[path = "main/jira_connector.rs"]
mod jira_connector;
#[path = "main/klaviyo_connector.rs"]
mod klaviyo_connector;
#[path = "main/launchdarkly_connector.rs"]
mod launchdarkly_connector;
#[path = "main/linear_connector.rs"]
mod linear_connector;
#[path = "main/mailchimp_connector.rs"]
mod mailchimp_connector;
#[path = "main/microsoft_graph_connector.rs"]
mod microsoft_graph_connector;
#[path = "main/miro_connector.rs"]
mod miro_connector;
#[path = "main/newrelic_connector.rs"]
mod newrelic_connector;
#[path = "main/notion_connector.rs"]
mod notion_connector;
#[path = "main/okta_connector.rs"]
mod okta_connector;
#[path = "main/op_emitter.rs"]
mod op_emitter;
#[path = "main/openai_connector.rs"]
mod openai_connector;
#[path = "main/openrouter_connector.rs"]
mod openrouter_connector;
#[path = "main/pagerduty_connector.rs"]
mod pagerduty_connector;
#[path = "main/pinned_config.rs"]
mod pinned_config;
#[path = "main/postmark_connector.rs"]
mod postmark_connector;
#[path = "main/query_placed_credentials.rs"]
mod query_placed_credentials;
#[path = "main/readme_snippet_svg.rs"]
mod readme_snippet_svg;
#[path = "main/repeatability_condition.rs"]
mod repeatability_condition;
#[path = "main/resend_connector.rs"]
mod resend_connector;
#[path = "main/salesforce_connector.rs"]
mod salesforce_connector;
#[path = "main/sendgrid_connector.rs"]
mod sendgrid_connector;
#[path = "main/sentry_connector.rs"]
mod sentry_connector;
#[path = "main/shipped_modules.rs"]
mod shipped_modules;
#[path = "main/shopify_connector.rs"]
mod shopify_connector;
#[path = "main/slack_connector.rs"]
mod slack_connector;
#[path = "main/statuspage_connector.rs"]
mod statuspage_connector;
#[path = "main/stripe_connector.rs"]
mod stripe_connector;
#[path = "main/supabase_connector.rs"]
mod supabase_connector;
#[path = "main/trello_connector.rs"]
mod trello_connector;
#[path = "main/twilio_connector.rs"]
mod twilio_connector;
#[path = "main/typeform_connector.rs"]
mod typeform_connector;
#[path = "main/username_path_pin.rs"]
mod username_path_pin;
#[path = "main/vercel_connector.rs"]
mod vercel_connector;
#[path = "main/webflow_connector.rs"]
mod webflow_connector;
#[path = "main/zoom_connector.rs"]
mod zoom_connector;

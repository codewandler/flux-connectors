//! What a **human** must supply before a connector can run — the surface a settings page renders.
//!
//! Everything else in this crate models *how a credential reaches the wire*. This module models *how
//! it gets there in the first place*: which fields to show, what to call them, what they mean, and
//! where each value goes once it is collected. It is the missing half of an integration — a connector
//! that describes eleven operations and cannot tell a product to ask for a subdomain is not
//! installable by anyone who has not read its source.
//!
//! # This repository declares; the host resolves; a UI renders
//!
//! Nothing here holds a value, a URL, or a callback address. A [`ConfigField`] says *"ask for this,
//! call it that, and put it there"*. flux already owns resolution — `ConfigSpec`, `EndpointSpec` and
//! its host-side `{placeholder}` templating — so [`ConfigField::binds`] names the destination rather
//! than re-implementing it.
//!
//! # Configuration has two levels, and conflating them is a real defect
//!
//! ```text
//! operator level     set once per vendor, by whoever runs the product
//!                    the OAuth app registration: client_id, client_secret
//!
//! connection level   set once per tenant, by each end user
//!                    the subdomain, the pasted token, the grant result
//! ```
//!
//! Ask an end user for a client secret and you have leaked the product's own credential to every
//! customer; hard-code a subdomain and the connector serves exactly one of them. So every field has a
//! [`Level`] — and it is **derived from [`binds`](ConfigField::binds), never authored**, because it is
//! a consequence of where the value goes and an author who could state it could state it wrongly.
//!
//! # Why `format` is a closed enum rather than a regex
//!
//! A renderer given `^[a-z0-9][a-z0-9-]*$` can reject a value and cannot explain why. A renderer given
//! [`Format::Subdomain`] knows the rule, the error message and the example. The enum is also what lets
//! this crate check a field against itself: [`Format::validate`] is applied to
//! [`example`](ConfigField::example) at load, so a provider claiming `format = "subdomain"` with
//! `example = "https://acme.zendesk.com"` is refused rather than shipped into a form as a misleading
//! placeholder.
//!
//! A free-form `pattern` escape hatch is deliberately **not** here yet. None of the shipped providers
//! needs one, and adding it means a regex dependency in a crate that has six — it lands when a real
//! provider needs something the enum cannot say.

use serde::{Deserialize, Serialize};

use crate::ir::{default_service, is_default_service};

/// Who supplies a configuration value, and how often.
///
/// **Derived from [`ConfigField::binds`], never deserialized.** See the module docs for why the
/// distinction is load-bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Set once per vendor by whoever runs the product — the OAuth app registration.
    Operator,
    /// Set once per tenant by each end user — the subdomain, the token, the grant.
    Connection,
}

/// The shape of a configuration value, and therefore how a form validates it.
///
/// Closed on purpose: a renderer switches on this to pick an input type, a validator and an error
/// message. A variant that means "anything" ([`Text`](Self::Text)) is the escape hatch, and using it
/// is a statement that the value genuinely has no shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// No particular shape. A free-text input.
    #[default]
    Text,
    /// One DNS label — `acme` in `acme.zendesk.com`. The most common tenant value there is.
    Subdomain,
    /// A full host — `acme.freshdesk.com`. Freshdesk asks for the whole thing where Zendesk asks for
    /// the label.
    Hostname,
    /// An absolute `https://` URL.
    Url,
    /// An email address. Zendesk and Jira both put one in the Basic username position.
    Email,
    /// An opaque credential string. No shape beyond "no whitespace" — a vendor's token format is the
    /// vendor's business, and a repository that guessed at it would reject valid tokens the day the
    /// vendor changed them.
    Token,
}

impl Format {
    /// The token this format serializes as, for error text.
    pub fn word(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Subdomain => "subdomain",
            Self::Hostname => "hostname",
            Self::Url => "url",
            Self::Email => "email",
            Self::Token => "token",
        }
    }

    /// Whether `value` has this format, with a reason when it does not.
    ///
    /// Applied by the loader to [`ConfigField::example`] only — this crate never sees a real
    /// configuration value, and it is a UI that applies this to user input. Checking the example is
    /// what gives the declaration teeth: a placeholder that would fail the field's own validation is
    /// worse than no placeholder, because a user copies it.
    pub fn validate(self, value: &str) -> Result<(), String> {
        if value.is_empty() {
            return Err("a value must not be empty".to_owned());
        }
        match self {
            Self::Text => Ok(()),
            Self::Token => match value.chars().find(|c| c.is_whitespace()) {
                Some(bad) => Err(format!("{value:?} contains whitespace ({bad:?})")),
                None => Ok(()),
            },
            Self::Subdomain => validate_label(value),
            Self::Hostname => {
                if !value.contains('.') {
                    return Err(format!(
                        "{value:?} is a single label, not a hostname — use `subdomain` for the label \
                         alone, or give the full host as in `acme.freshdesk.com`"
                    ));
                }
                for label in value.split('.') {
                    validate_label(label)?;
                }
                Ok(())
            }
            Self::Url => {
                if !value.starts_with("https://") {
                    return Err(format!(
                        "{value:?} is not an absolute `https://` URL. Plain http would carry a \
                         credential over the network in the clear"
                    ));
                }
                if value["https://".len()..].is_empty() {
                    return Err(format!("{value:?} has no host"));
                }
                Ok(())
            }
            Self::Email => {
                let mut halves = value.split('@');
                let (Some(local), Some(domain), None) =
                    (halves.next(), halves.next(), halves.next())
                else {
                    return Err(format!("{value:?} is not an email address"));
                };
                if local.is_empty() || domain.is_empty() || !domain.contains('.') {
                    return Err(format!("{value:?} is not an email address"));
                }
                Ok(())
            }
        }
    }
}

/// One DNS label: lowercase alphanumerics and inner hyphens.
fn validate_label(label: &str) -> Result<(), String> {
    if label.is_empty() {
        return Err("a label must not be empty".to_owned());
    }
    if !label
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!(
            "{label:?} is not a DNS label; labels are lowercase ASCII letters, digits and `-`"
        ));
    }
    if label.starts_with('-') || label.ends_with('-') {
        return Err(format!("{label:?} starts or ends with `-`"));
    }
    Ok(())
}

/// Where a collected value goes. The parsed form of [`ConfigField::binds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding<'a> {
    /// A `{variable}` in the service's base URL — `endpoint.subdomain`.
    Endpoint {
        /// The template variable name, without braces.
        variable: &'a str,
    },
    /// The secret half of a declared credential — `credential.zendesk.api_token`.
    Credential {
        /// The [`AuthMethod::name`](crate::AuthMethod::name).
        name: &'a str,
    },
    /// The username half of a `basic` credential — `username.zendesk.api_token`.
    ///
    /// A separate prefix rather than a `.user` suffix on the credential form, because credential
    /// names contain dots (`zendesk.api_token`) and a suffix would be ambiguous with a credential
    /// genuinely named `…​.user`.
    Username {
        /// The [`AuthMethod::name`](crate::AuthMethod::name).
        name: &'a str,
    },
    /// The OAuth app's client id — `oauth.client_id`. Operator level.
    OAuthClientId,
    /// The OAuth app's client secret — `oauth.client_secret`. Operator level, and secret.
    OAuthClientSecret,
}

impl Binding<'_> {
    /// Who supplies this value. See [`Level`].
    pub fn level(self) -> Level {
        match self {
            Self::OAuthClientId | Self::OAuthClientSecret => Level::Operator,
            Self::Endpoint { .. } | Self::Credential { .. } | Self::Username { .. } => {
                Level::Connection
            }
        }
    }

    /// Whether the value behind this binding is a secret.
    ///
    /// **This is the fact [`ConfigField::secret`] must agree with**, and the agreement is a loader
    /// rule rather than a convention. flux partitions secret from non-secret *by type* — an
    /// `AuthMethod` versus a `ConfigSpec` — and enforces it host-side, refusing to hand a
    /// secret-classified env key back through the non-secret `config` capability. A field that
    /// declared itself non-secret while binding a credential would put a second, disagreeing source
    /// of truth in front of that enforcement.
    pub fn is_secret(self) -> bool {
        match self {
            Self::Credential { .. } | Self::OAuthClientSecret => true,
            // A Basic username is config, not a gated secret — `AuthMethod::user_env` documents the
            // same split, and it is why zendesk's agent email may appear in a log where its token
            // may not.
            Self::Endpoint { .. } | Self::Username { .. } | Self::OAuthClientId => false,
        }
    }
}

/// Parse a [`ConfigField::binds`] string.
///
/// A validated string rather than an externally-tagged table, matching
/// [`Param::wire`](crate::Param::wire) and [`HmacSpec::signed`](crate::HmacSpec::signed): the forms
/// are few, closed, and read better inline than as a nested table.
pub fn parse_binding(binds: &str) -> Result<Binding<'_>, String> {
    if let Some(variable) = binds.strip_prefix("endpoint.") {
        if variable.is_empty() {
            return Err("`endpoint.` names no variable".to_owned());
        }
        return Ok(Binding::Endpoint { variable });
    }
    if let Some(name) = binds.strip_prefix("credential.") {
        if name.is_empty() {
            return Err("`credential.` names no credential".to_owned());
        }
        return Ok(Binding::Credential { name });
    }
    if let Some(name) = binds.strip_prefix("username.") {
        if name.is_empty() {
            return Err("`username.` names no credential".to_owned());
        }
        return Ok(Binding::Username { name });
    }
    match binds {
        "oauth.client_id" => Ok(Binding::OAuthClientId),
        "oauth.client_secret" => Ok(Binding::OAuthClientSecret),
        _ => Err(format!(
            "{binds:?} is not a binding. A configuration value goes to exactly one of: \
             `endpoint.<variable>`, `credential.<name>`, `username.<name>`, `oauth.client_id`, \
             `oauth.client_secret`"
        )),
    }
}

/// One thing a human must supply, and everything a form needs to ask for it.
///
/// The presentation fields are separate from [`description`](crate::Operation::description) on
/// purpose. Every `description` in this crate is **already spoken for** — it is the text a model
/// receives as a tool contract — so overloading it with UI copy would make one string serve two
/// audiences that want different things, which is the state `providers/slack.toml`'s credential
/// description is in today (a label, a placeholder and a scope list in one sentence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigField {
    /// The field name, e.g. `subdomain`. Unique across every member kind of its service, and the key
    /// a host stores the collected value under.
    pub name: String,
    /// The [`Service`](crate::Service) this field configures — exactly one, always concrete.
    #[serde(
        default = "default_service",
        skip_serializing_if = "is_default_service"
    )]
    pub service: String,
    /// The form label, e.g. `Zendesk subdomain`. Sentence case, no trailing colon.
    ///
    /// Mandatory and non-empty. Defaulting it to [`name`](Self::name) would quietly ship
    /// `zendesk.api_token` into a form as user-facing copy, which is exactly the plausible-but-wrong
    /// output this pipeline refuses elsewhere.
    pub label: String,
    /// One line telling a user what the value *is* and, where it is not obvious, how to find it —
    /// "The part of your Zendesk URL before `.zendesk.com`".
    ///
    /// Mandatory, for the same reason as `label`. A field a user cannot answer is a field that stops
    /// the installation.
    pub help: String,
    /// A realistic example, rendered as a placeholder. Never a default — nothing is pre-filled with
    /// it, and it is validated against [`format`](Self::format) so it cannot mislead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// The value's shape, which decides the input type and the validation a form applies.
    #[serde(default, skip_serializing_if = "is_text")]
    pub format: Format,
    /// Whether the connector cannot function without it.
    ///
    /// Defaults to `true`: a connector declaring a field it does not need is the rarer case, and the
    /// safe default is to ask.
    #[serde(default = "default_required", skip_serializing_if = "is_true")]
    pub required: bool,
    /// Whether the value is a secret — masked on input, never logged, never echoed back.
    ///
    /// **Must agree with [`binds`](Self::binds)**; see [`Binding::is_secret`] for why that is a rule
    /// and not a convention.
    #[serde(default, skip_serializing_if = "is_false")]
    pub secret: bool,
    /// Where a user goes to obtain the value — the vendor's own page, never ours.
    ///
    /// The single highest-value field here for anyone actually installing a connector, and the one
    /// thing no artifact in this repository carries today: vendor documentation links exist only in
    /// TOML comments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    /// Where the collected value goes. See [`parse_binding`].
    pub binds: String,
}

impl ConfigField {
    /// The parsed [`binds`](Self::binds). `None` when it is malformed — which the loader refuses, so
    /// on a loaded connector this is always `Some`.
    pub fn binding(&self) -> Option<Binding<'_>> {
        parse_binding(&self.binds).ok()
    }

    /// Who supplies this value, derived from its binding. See [`Level`].
    pub fn level(&self) -> Option<Level> {
        Some(self.binding()?.level())
    }
}

/// The `{variable}` names a base URL template carries, in order, deduplicated.
///
/// Replaces `connector-cli`'s `first_template_variable`, whose name was an accurate description of
/// its bug: `https://{region}.{tenant}.example` reported one variable and left the other invisible to
/// every consumer of the catalogue's status.
pub fn template_variables(base_url: &str) -> Vec<&str> {
    let mut found: Vec<&str> = Vec::new();
    let mut rest = base_url;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        let variable = &after[..close];
        if !variable.is_empty() && !found.contains(&variable) {
            found.push(variable);
        }
        rest = &after[close + 1..];
    }
    found
}

fn default_required() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_text(format: &Format) -> bool {
    *format == Format::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_parse_and_carry_their_level_and_secrecy() {
        assert_eq!(
            parse_binding("endpoint.subdomain"),
            Ok(Binding::Endpoint {
                variable: "subdomain"
            })
        );
        // A credential name contains dots, which is why the username half is a prefix and not a
        // `.user` suffix.
        assert_eq!(
            parse_binding("credential.zendesk.api_token"),
            Ok(Binding::Credential {
                name: "zendesk.api_token"
            })
        );
        assert_eq!(
            parse_binding("username.zendesk.api_token"),
            Ok(Binding::Username {
                name: "zendesk.api_token"
            })
        );

        assert_eq!(
            parse_binding("oauth.client_id").map(Binding::level),
            Ok(Level::Operator)
        );
        assert_eq!(
            parse_binding("endpoint.subdomain").map(Binding::level),
            Ok(Level::Connection)
        );
        assert_eq!(
            parse_binding("credential.x").map(Binding::is_secret),
            Ok(true)
        );
        assert_eq!(
            parse_binding("username.x").map(Binding::is_secret),
            Ok(false)
        );
        assert_eq!(
            parse_binding("oauth.client_secret").map(Binding::is_secret),
            Ok(true)
        );

        assert!(parse_binding("subdomain").is_err());
        assert!(parse_binding("endpoint.").is_err());
        assert!(parse_binding("oauth.client_token").is_err());
    }

    #[test]
    fn formats_validate_the_values_they_claim() {
        assert!(Format::Subdomain.validate("acme").is_ok());
        assert!(Format::Subdomain.validate("acme-corp").is_ok());
        assert!(Format::Subdomain.validate("acme.zendesk.com").is_err());
        assert!(Format::Subdomain.validate("-acme").is_err());
        assert!(Format::Subdomain.validate("Acme").is_err());

        assert!(Format::Hostname.validate("acme.freshdesk.com").is_ok());
        assert!(Format::Hostname.validate("acme").is_err());

        assert!(Format::Url.validate("https://acme.example").is_ok());
        assert!(Format::Url.validate("http://acme.example").is_err());

        assert!(Format::Email.validate("a@b.com").is_ok());
        assert!(Format::Email.validate("a@b").is_err());
        assert!(Format::Email.validate("a@b@c.com").is_err());

        assert!(Format::Token.validate("xoxb-123").is_ok());
        assert!(Format::Token.validate("xoxb 123").is_err());

        assert!(Format::Text.validate("anything at all").is_ok());
        assert!(Format::Text.validate("").is_err());
    }

    #[test]
    fn every_template_variable_is_reported_not_only_the_first() {
        assert_eq!(
            template_variables("https://{subdomain}.zendesk.com"),
            ["subdomain"]
        );
        assert_eq!(
            template_variables("https://api.github.com"),
            Vec::<&str>::new()
        );
        // The case that motivated replacing `first_template_variable`.
        assert_eq!(
            template_variables("https://{region}.{tenant}.example"),
            ["region", "tenant"]
        );
        assert_eq!(template_variables("https://{a}/x/{a}"), ["a"]);
        assert_eq!(
            template_variables("https://{unterminated"),
            Vec::<&str>::new()
        );
    }
}

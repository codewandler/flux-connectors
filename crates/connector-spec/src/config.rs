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
//!
//! # A field can also name the values it permits, and that is a different question (C-225)
//!
//! [`Format`] says what *shape* a value has. [`ConfigField::choices`] says which *values* are legal.
//! Two vendors measured the difference in one wave: New Relic serves one REST API from
//! `api.newrelic.com` and `api.eu.newrelic.com` and nothing pre-auth discloses which, and Intercom
//! adds `api.au.intercom.io` to the same shape. `format = "hostname"` accepts every syntactically
//! valid host on the internet, so a wrong region loaded, rendered as a text box, and failed as
//! `401 Unauthorized` on every call — **indistinguishable from a bad key**, so the operator's first
//! move is to rotate a credential that was never wrong.
//!
//! **`choices` narrows a formatted field; it does not replace `format`.** The two are kept separate
//! deliberately, and three consequences follow from it rather than from taste:
//!
//! 1. Every permitted value is validated against the field's own `format` at load, so a set cannot
//!    smuggle in a value the field's own rule would reject.
//! 2. A renderer still knows the input type — which is what it falls back to when it cannot draw a
//!    select, and what it validates a typed value with.
//! 3. `example` keeps answering to both: it satisfies the format *and* is one of the choices, which
//!    is the same defect class the format/example rule already refuses.
//!
//! **Deliberately not a constraint language.** A closed list of values, each with a label, is the
//! whole of it. Ranges, patterns and conditionals are each their own argument, and the same call
//! `Format`'s missing `pattern` already makes applies: they land when a real provider needs them.
//!
//! # What a host does with a stored value that later leaves the set
//!
//! A vendor adding a region must not brick an existing connection, so membership is checked **where
//! a value is supplied** and nowhere else. [`ConfigField::permits`] is called by whatever accepts a
//! value from a human — this repository's own connect API does it in `PUT /v1/config/…` — and it is
//! *not* consulted when a stored value is read back and substituted into a request. A connection
//! configured with a host the connector no longer lists keeps working; the next edit of that field
//! is where the operator is asked to pick again. The alternative, refusing at read time, converts a
//! catalogue update into an outage on connections that were never wrong.
//!
//! # An operator can pin a tenant scope, not only a host (C-187)
//!
//! [`Binding::Endpoint`] reaches a `{placeholder}` in a service's `base_url` and nothing else. That
//! covered every vendor whose tenancy is a *hostname* — `{subdomain}.zendesk.com` — and none whose
//! tenancy sits further down the request. Two shipped connectors measured the gap in one wave:
//! Cloudflare's `zone_id` is a **path** segment on every scoped operation (C-169), Vercel's `teamId`
//! is a **query** parameter (C-170), and both had to ship as per-call arguments a model chooses on
//! every invocation. A third, Algolia (C-164), could not ship at all, because its application id has
//! to reach a **header** and the only declaration that reached one was a credential.
//!
//! [`Binding::Request`] is the answer: three positions ([`Position`]), one placeholder per pinned
//! value, and the value supplied once when the connection is made. Three properties make it a pin
//! rather than a default:
//!
//! 1. **It is not also an argument.** The loader refuses a service whose operations declare a
//!    parameter the pin already claims, and `connector-flux` refuses the same shape independently.
//!    A pin a caller can override is advisory, which is the opposite of the point.
//! 2. **It is mandatory.** A host substitutes a pinned placeholder into a string literal and refuses
//!    a request when it has no value (`connector-pack`'s `Error::MissingConfig`), so `required =
//!    false` on a pinned field describes a connector that cannot compose a URL. It is refused. This
//!    is also the whole reason a *query* pin is safe: Vercel's `teamId` is dangerous precisely
//!    because omitting it silently redirects the call to a personal account, and an optional pin
//!    would reproduce that with extra steps.
//! 3. **It is never a secret.** See [`Binding::Request`].
//!
//! # One question can reach more than one destination (C-229)
//!
//! [`Binding::Request`] gave a field a request position. It still gave it exactly *one* destination,
//! and Algolia (C-164) is the vendor that shape cannot express: its application id composes the
//! hostname (`{app_id}-dsn.algolia.net`) **and** travels as `X-Algolia-Application-Id` on every
//! call. Of C-187's three motivating vendors it is the only one whose tenant scope sits in two
//! positions — Cloudflare's `zone_id` and Vercel's `teamId` sit in one each, which is why those
//! ship.
//!
//! Neither of the two shapes that existed works, and C-164 measured both rather than arguing them:
//!
//! - **Two fields, two names** (`endpoint.app_id` + `header.X-Algolia-Application-Id`) loads, and is
//!   the problem: two host-side slots for one answer, no honest `help` for the second field — the
//!   only truthful text is *"type the same value again"* — and a typo in one of them produces a DNS
//!   error or a vendor `403` that neither declaration explains.
//! - **Two fields, one name** is refused by the loader's shared-slot pass, *by name*: **two
//!   questions that share an answer are one question**. That rule is right (C-197: two fields keyed
//!   to one slot silently discard one answer) and it stays. What was missing is the other half — a
//!   way to write **the one question**.
//!
//! [`ConfigField::also_binds`] is that half. `binds` keeps naming exactly one destination and
//! `also_binds` names the rest, so the field stays one `name`, one `label`, one `help`, one row in a
//! form, and one value a host stores.
//!
//! ## Why `also_binds` and not `binds` becoming a list
//!
//! A list of peers has no head, and this declaration needs one. Three consequences follow from that
//! rather than from taste:
//!
//! 1. **The placeholder question gets a structural answer.** [`Binding::Request`]'s `name` is
//!    deliberately both the `{placeholder}` and the wire spelling, so a field whose destinations
//!    spell the value differently — `app_id` in the host, `X-Algolia-Application-Id` on the wire —
//!    forces a choice about which one the emitted module carries. With a head there is one rule and
//!    no conditional: **the emitted module carries `binds`' own target, everywhere**
//!    ([`ConfigField::slot`]), and a further destination contributes only the spelling the vendor
//!    sees. With a bare list the answer would be "element zero", which is a convention about
//!    ordering rather than a property of the declaration.
//! 2. **One slot, provably.** A host keys a value by `(tenant, provider, service, kind, name)` and
//!    `connector-pack` resolves every `{placeholder}` in an emitted module by that one name, so
//!    "one slot" *is* "one placeholder". Deriving the slot from `binds` alone keeps
//!    [`ConfigField::binding`], [`ConfigField::level`] and the stored address exactly what they were
//!    for every field that existed before this landed.
//! 3. **It reads as what it is.** `also_binds` is subordinate at the declaration site; `binds =
//!    ["endpoint.app_id", "header.X-…"]` reads as a set of equals, which is the shape the shared-slot
//!    rule refuses to key.
//!
//! ## What a further destination may be, and why the set is narrow
//!
//! Only a request position (`path.`, `query.`, `header.`). Every other destination resolves through
//! a **different port**: a credential and an OAuth secret through the secret side, a `username.`
//! through its own `(kind, name)` address. Sharing a placeholder with any of them is not something
//! this model can arrange, so a field naming one of them names it alone — and the `secret`/`binds`
//! agreement would refuse the credential half anyway, since a value that reaches a URL must never be
//! a secret.
//!
//! An `endpoint.` destination is the **head or nothing**, for the same reason it is the head in
//! Algolia: its spelling is fixed by a `base_url` the author already wrote, so it is the destination
//! with the least freedom, and making it the slot is what keeps one rule instead of two.
//!
//! ## Every destination validates, not just the first
//!
//! A value substituted into two positions must satisfy **both** of their rules, and it is encoded
//! the same way in each — the alternative, encoding per destination, would send the vendor two
//! different strings for one answer. So [`Position::validate_value`] is applied to
//! [`example`](ConfigField::example) and to **every** [`choice`](ConfigField::choices) once per
//! destination the field reaches, and the host half is [`validate_host_value`], which is the strict
//! one: `acme.example@evil.example` is a legal header value and a legal path segment, and it moves
//! the authority. `connector-pack` reaches the same conclusion from the other end — a variable it
//! sees in two positions collapses to a slot held to every rule at once.
//!
//! # `ConfigField` stays connector-scoped, and that is recorded rather than assumed
//!
//! C-177 asked whether [`ConfigField::name`] should be a per-*service* namespace like operations,
//! events and channels are — Contentful ships `delivery_space_id`/`management_space_id` where two
//! services would each say `space_id`. The answer is **no, not through this story**, for a reason
//! the runtime port makes concrete: a value is addressed by `(tenant, provider, service, kind,
//! name)` where `name` is the **binding target** and not the field name (C-197), so the service is
//! *already* what keeps Contentful's two spaces apart. What a per-service field namespace would buy
//! is shorter form-field names, and what it would cost is renaming every shipped field a host has
//! already stored a value under. A field that legitimately spans services has no answer under it at
//! all, which is the second reason to leave it: `service` is exactly one, always concrete, so
//! "spans services" is spelled as two fields today and would still be two fields after.

use std::borrow::Cow;

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

/// Extra authorization required before a supplied configuration value becomes active.
///
/// This is independent of [`Level`]: a self-managed origin is still one value per connection, but
/// activating a value other than the connector's reviewed default requires deployment/operator
/// policy. The declaration carries policy only; it never carries an approved installation value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    /// The ordinary configuration flow may activate the value.
    #[default]
    None,
    /// A deployment operator must approve and pin a non-default value for this connection.
    Operator,
}

impl Approval {
    /// The stable token published to manifests and catalogues.
    pub fn word(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Operator => "operator",
        }
    }
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
    /// An absolute HTTPS origin: scheme, authority and optional explicit port, with no userinfo,
    /// path, query or fragment. The connector appends every API path itself.
    ///
    /// The grammar is [`connector_address::HttpsOrigin`]'s, and a **declaration** must carry the
    /// canonical spelling of it — see [`Format::validate`].
    Origin,
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
            Self::Origin => "origin",
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
            // **A declaration, not a connection value** — so the canonical spelling is required
            // rather than merely accepted (C-523). This is applied to an `example`, a `default` and
            // every `choice`, each of which is copied verbatim into the connector manifest, the
            // embedded catalogue and the public catalogue; a second safe spelling of one origin
            // would ship as a second origin, and the runtime, which normalizes a supplied value
            // before comparing it against the reviewed default, would disagree with the artifact
            // about which destination that is. A tenant's own value goes through
            // `HttpsOrigin::parse` in `connector-pack` and may be spelled any equivalent safe way.
            Self::Origin => connector_address::HttpsOrigin::parse_canonical(value)
                .map(|_| ())
                .map_err(|refusal| refusal.to_string()),
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

/// Where on a request an operator-pinned value lands. The detail of [`Binding::Request`].
///
/// **A closed set of three, and it is closed on the same principle
/// [`BodyEncoding`](crate::BodyEncoding) is**: an unknown spelling must be a load error rather than
/// a key the loader accepts and ignores. The three are exactly the request positions a *tenant
/// scope* sits in on the vendors this repository has met — a path segment (Cloudflare's `zone_id`),
/// a query parameter (Vercel's `teamId`), a header (Algolia's `X-Algolia-Application-Id`).
///
/// # Why the body is not a fourth
///
/// A body field the connector fixes is already expressible — a JSON Schema `const` on a
/// [`Param`](crate::Param), which `connector-flux` sends without declaring — and no vendor met so
/// far scopes a tenant through a request body. Adding a variant with no vendor behind it is how a
/// closed set stops meaning anything; it lands when a real provider needs it, which is the call
/// [`Format`]'s missing `pattern` already makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// A `{variable}` in an operation's path template — `/zones/{zone_id}/dns_records`.
    Path,
    /// A query parameter, sent on every operation of the field's service — `?teamId=…`.
    Query,
    /// A request header, sent on every operation of the field's service —
    /// `X-Algolia-Application-Id`.
    Header,
}

impl Position {
    /// The prefix a [`ConfigField::binds`] string spells this position with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Header => "header",
        }
    }

    /// **Whether `value` can be substituted into this position without reshaping the request.**
    ///
    /// This crate never sees a real configuration value, exactly as [`Format::validate`] never
    /// does: it is applied by the loader to [`ConfigField::example`], and by a host to the value it
    /// is about to substitute. The reason it exists at all is that a pinned value lands *inside* a
    /// URL a host composes, so a value carrying the wrong character does not fail — it produces a
    /// **different, valid request**, which is the failure mode this whole surface is arranged
    /// against:
    ///
    /// - a path pin holding `a/../b` or `../../accounts` **escapes its segment** and addresses a
    ///   resource the operator never named, on the same host, with the same credential;
    /// - a query pin holding `x&admin=1` appends a parameter, and nothing here percent-encodes —
    ///   flux registers no URL-encoding op, which is the standing gap `op.rs` records for query
    ///   values generally;
    /// - a header pin holding a CR or LF appends a header of the value's choosing to every request.
    ///
    /// A brace is refused in every position for a fourth reason: substitution fills
    /// `{placeholder}`s in emitted string literals, so a value that spells one would either be
    /// filled in a second time or survive into the URL as text.
    pub fn validate_value(self, value: &str) -> Result<(), String> {
        if value.is_empty() {
            return Err("a pinned value must not be empty".to_owned());
        }
        if let Some(bad) = value.chars().find(|c| *c == '{' || *c == '}') {
            return Err(format!(
                "{value:?} contains {bad:?}. A pinned value is substituted into a `{{placeholder}}` \
                 in the emitted module, so a value spelling one of its own would be filled in twice \
                 or reach the vendor verbatim"
            ));
        }
        match self {
            Self::Path => {
                if value == "." || value == ".." {
                    return Err(format!(
                        "{value:?} is a relative path segment, so the request would address the \
                         segment above or beside the one it was pinned to"
                    ));
                }
                if let Some(bad) = value
                    .chars()
                    .find(|c| "/?#%\\".contains(*c) || c.is_whitespace() || c.is_control())
                {
                    return Err(format!(
                        "{value:?} contains {bad:?}, which does not stay inside one path segment — \
                         a `/` (or a `%` that could encode one) reshapes the URL, and a `?` or `#` \
                         ends the path entirely"
                    ));
                }
                Ok(())
            }
            Self::Query => {
                if let Some(bad) = value
                    .chars()
                    .find(|c| "&=?#+%".contains(*c) || c.is_whitespace() || c.is_control())
                {
                    return Err(format!(
                        "{value:?} contains {bad:?}, and nothing percent-encodes a query value on \
                         the way out — an `&` or `=` would add a parameter of its own to every \
                         request"
                    ));
                }
                Ok(())
            }
            Self::Header => {
                if let Some(bad) = value
                    .chars()
                    .find(|c| !c.is_ascii() || c.is_ascii_control())
                {
                    return Err(format!(
                        "{value:?} contains {bad:?}, which is not an HTTP field value (RFC 9110 \
                         §5.5). A newline in particular would append a header of its own to every \
                         request"
                    ));
                }
                if value.trim() != value {
                    return Err(format!(
                        "{value:?} starts or ends with whitespace, which an HTTP field value may \
                         not (RFC 9110 §5.5)"
                    ));
                }
                Ok(())
            }
        }
    }
}

/// **Whether `value` can be substituted into a `base_url` without moving the authority it declared.**
///
/// The host counterpart of [`Position::validate_value`], and the strict one of the family: it is an
/// **allow-list** of the characters that cannot delimit an authority, because a blocklist stops the
/// `@` somebody thought of and not the `:` that becomes a port, the `%` that decodes to a delimiter
/// or the `/` that ends the authority early. `acme.zendesk.com@evil.example` is a legal HTTP field
/// value and a legal path segment; substituted into `https://{subdomain}.zendesk.com` it sends the
/// request, and the operator's own credential, to a host nobody named.
///
/// Applied by the loader to [`ConfigField::example`] and to every [`ConfigField::choices`] entry of
/// a field that binds `endpoint.<variable>` — this crate never sees a real configuration value, and
/// `connector-pack` applies the same rule to the value a tenant actually supplies. It exists here
/// because C-229 lets one value reach a hostname *and* a request position, and a field checked only
/// against its request positions would be checked against the **weaker** of the two.
pub fn validate_host_value(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("a value substituted into a host must not be empty".to_owned());
    }
    if let Some(bad) = value
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == '_'))
    {
        return Err(format!(
            "{value:?} contains {bad:?}, which is not a host character — a value substituted into a \
             `base_url` may not introduce one, because `@`, `:`, `/` and `%` all move or truncate \
             the authority the connector declared"
        ));
    }
    if value.split('.').any(str::is_empty) {
        return Err(format!(
            "{value:?} has an empty label, so it cannot be part of a hostname"
        ));
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
    /// **A tenant-scoping value an operator pins at install time**, landing on one position of
    /// every request the field's service makes — `path.zone_id`, `query.teamId`,
    /// `header.X-Algolia-Application-Id`.
    ///
    /// # It is configuration, not a credential — and the distinction is enforced, not stated
    ///
    /// A pinned value is **operator-supplied configuration**: it is readable back, shown in a
    /// settings page, and may appear in a log. [`is_secret`](Self::is_secret) is `false` for it
    /// unconditionally, so the loader's `secret`/`binds` agreement refuses a field that claims
    /// otherwise, and nothing here registers it with a host's redactor. That is the point rather
    /// than an omission: the one route that already reached a header was an `[[auth]]`-declared
    /// credential, and taking it for a public identifier would have bought the header at the cost
    /// of a false `secret = true` (C-164 measured exactly that on Algolia's application id).
    ///
    /// The converse is guarded too, because a URL is the one place a secret must never be pinned:
    /// a field binding a credential cannot also bind a request position — `binds` names exactly one
    /// destination — and [`Position::validate_value`] refuses the characters that would let a
    /// pinned value reshape the request it is substituted into.
    ///
    /// # `name` is the placeholder *and* the wire spelling
    ///
    /// One string, deliberately: it is the `{variable}` in an operation's path for
    /// [`Path`](Position::Path), and both the wire name and the placeholder for
    /// [`Query`](Position::Query) and [`Header`](Position::Header). A second, private spelling
    /// would be a second thing to keep in step, and the endpoint form already demonstrates that a
    /// binding target and a field `name` are allowed to differ where they need to.
    Request {
        /// Where on the request the value lands.
        position: Position,
        /// The `{placeholder}` the emitted module carries, and the name the vendor sees.
        name: &'a str,
    },
    /// One query parameter on one declared socket channel —
    /// `channel.ari-events.query.subscribeAll`.
    ChannelQuery {
        /// The [`ChannelBinding`](crate::ChannelBinding) name.
        channel: &'a str,
        /// The query parameter name on that channel's connect declaration.
        parameter: &'a str,
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
    /// The redirect URI registered with the OAuth app — `oauth.redirect_uri`. Operator level, and
    /// **not** secret (C-531).
    ///
    /// It is the third half of one registration. A client id, a client secret and a redirect URI are
    /// issued together when somebody registers an application with the vendor, so they are supplied
    /// together, by the same person, at the same level. Leaving this one out was the reason a hosted
    /// deployment had nowhere to put its callback: [`OAuth2Spec::redirect`](crate::OAuth2Spec)
    /// models a **loopback port and path** — the shape RFC 8252 §7.3 describes for a native app on
    /// the operator's own machine — and a service reached at `https://exchange.internal/…` cannot be
    /// spelled that way.
    ///
    /// **The connector still declares no destination.** This is a slot a host fills, exactly like
    /// `endpoint.origin`; the vendor does not choose your callback, you register it with them. What
    /// the connector supplies is the *question*, so an operator is asked for it during setup rather
    /// than discovering the mismatch on the vendor's error page after a failed grant.
    OAuthRedirectUri,
}

impl<'a> Binding<'a> {
    /// Who supplies this value. See [`Level`].
    /// **A pinned request value is *connection* level, and that is derivation rather than
    /// preference.** "Operator" here means *once per vendor, by whoever runs the product* — the app
    /// registration every tenant shares. A zone or a team is the opposite: one per tenant, chosen by
    /// the person connecting their own account, and two tenants of one deployment pin two different
    /// ones. Calling it operator level would put one customer's zone in front of every other
    /// customer, which is the same conflation in the same direction as asking an end user for a
    /// client secret.
    ///
    /// The story's phrase "operator-pinned" describes *when* the value is supplied — at install
    /// time, rather than on every call — not *which* of these two levels supplies it.
    pub fn level(self) -> Level {
        match self {
            Self::OAuthClientId | Self::OAuthClientSecret | Self::OAuthRedirectUri => {
                Level::Operator
            }
            Self::Endpoint { .. }
            | Self::Request { .. }
            | Self::ChannelQuery { .. }
            | Self::Credential { .. }
            | Self::Username { .. } => Level::Connection,
        }
    }

    /// The word this binding's destination is addressed by — `endpoint`, `path`, `query`, `header`,
    /// `username`, `credential` or `oauth`.
    ///
    /// The first half of the `(kind, target)` pair a host keys a stored value on, and the same
    /// vocabulary [`parse_binding`] accepts. It exists so that a consumer publishing or resolving a
    /// value does not re-split the `binds` string and get the `username.` / `credential.` prefix
    /// distinction subtly wrong.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Endpoint { .. } => "endpoint",
            Self::Request { position, .. } => position.word(),
            Self::ChannelQuery { .. } => "channel_query",
            Self::Credential { .. } => "credential",
            Self::Username { .. } => "username",
            Self::OAuthClientId | Self::OAuthClientSecret | Self::OAuthRedirectUri => "oauth",
        }
    }

    /// The name within [`kind`](Self::kind) — the template variable, the credential, the wire name
    /// of a pin, or the OAuth half.
    pub fn target(self) -> &'a str {
        match self {
            Self::Endpoint { variable } => variable,
            Self::Request { name, .. } | Self::Credential { name } | Self::Username { name } => {
                name
            }
            Self::ChannelQuery { parameter, .. } => parameter,
            Self::OAuthClientId => "client_id",
            Self::OAuthClientSecret => "client_secret",
            Self::OAuthRedirectUri => "redirect_uri",
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
            // may not. A pinned request value is the same category and for a sharper reason: it
            // travels in a URL or a header the module itself composes, where a secret must never be.
            Self::Endpoint { .. }
            | Self::Request { .. }
            | Self::ChannelQuery { .. }
            | Self::Username { .. }
            | Self::OAuthClientId
            | Self::OAuthRedirectUri => false,
        }
    }

    /// **Whether `value` can be substituted at this destination without reshaping the request.**
    ///
    /// One entry point over [`Position::validate_value`] and [`validate_host_value`], so that a
    /// field reaching several destinations (C-229) is checked against **each** of them from one
    /// loop rather than from a `match` repeated per caller — the shape in which "every position is
    /// validated, not only the first" is a property of the code instead of a claim about it.
    ///
    /// `Ok` for the three destinations that are not substituted into a request the emitter composes:
    /// a credential and an OAuth half travel through the secret side, and a Basic username is joined
    /// and base64-encoded rather than placed. Nothing here is a value this crate ever sees — the
    /// loader applies it to [`ConfigField::example`] and to every [`ConfigField::choices`] entry.
    ///
    /// # Errors
    ///
    /// The reason, phrased for whoever wrote the declaration.
    pub fn validate_value(self, value: &str) -> Result<(), String> {
        match self {
            Self::Endpoint { .. } => validate_host_value(value),
            Self::Request { position, .. } => position.validate_value(value),
            Self::ChannelQuery { .. } => Position::Query.validate_value(value),
            Self::Credential { .. }
            | Self::Username { .. }
            | Self::OAuthClientId
            | Self::OAuthClientSecret
            | Self::OAuthRedirectUri => Ok(()),
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
    if let Some(rest) = binds.strip_prefix("channel.") {
        let Some((channel, parameter)) = rest.split_once(".query.") else {
            return Err(format!(
                "{binds:?} is not a channel query binding; write \
                 `channel.<binding>.query.<parameter>`"
            ));
        };
        if channel.is_empty() || parameter.is_empty() {
            return Err(format!(
                "{binds:?} must name both a channel binding and a query parameter"
            ));
        }
        return Ok(Binding::ChannelQuery { channel, parameter });
    }
    for position in [Position::Path, Position::Query, Position::Header] {
        let Some(name) = binds
            .strip_prefix(position.word())
            .and_then(|rest| rest.strip_prefix('.'))
        else {
            continue;
        };
        if name.is_empty() {
            return Err(format!(
                "`{}.` names no {} value",
                position.word(),
                position.word()
            ));
        }
        return Ok(Binding::Request { position, name });
    }
    match binds {
        "oauth.client_id" => Ok(Binding::OAuthClientId),
        "oauth.client_secret" => Ok(Binding::OAuthClientSecret),
        "oauth.redirect_uri" => Ok(Binding::OAuthRedirectUri),
        _ => Err(format!(
            "{binds:?} is not a binding. A configuration value goes to exactly one of: \
             `endpoint.<variable>`, `path.<variable>`, `query.<name>`, `header.<name>`, \
             `channel.<binding>.query.<parameter>`, \
             `credential.<name>`, `username.<name>`, `oauth.client_id`, `oauth.client_secret`, \
             `oauth.redirect_uri`"
        )),
    }
}

/// One permitted value of a [closed set](ConfigField::choices), and the text a renderer shows for it.
///
/// **The label is mandatory, and that is the point of the type.** A bare list of strings is a
/// dropdown offering `api.newrelic.com` and `api.eu.newrelic.com` to someone who knows their account
/// is in Frankfurt, which moves the declaration without moving the benefit. `value` is what reaches
/// the wire; `label` is what the person choosing it reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    /// The value itself — what a host stores and what substitution puts on the request.
    pub value: String,
    /// The human name for it — "United States", not `api.newrelic.com`. Sentence case, no colon.
    pub label: String,
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
    ///
    /// **A [`secret`](Self::secret) field declares none, and the loader refuses one** (C-231). A
    /// token-shaped literal in a committed file has tripped GitHub push protection and blocked a
    /// release here; one that is a real token is a disclosed credential. It also buys nothing —
    /// nobody recognises their own secret from an example of someone else's — so the shape of the
    /// value belongs in [`help`](Self::help), as prose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// The value's shape, which decides the input type and the validation a form applies.
    #[serde(default, skip_serializing_if = "is_text")]
    pub format: Format,
    /// **The closed set of values this field permits** — empty when the field is open (C-225).
    ///
    /// A *narrowing* of [`format`](Self::format) rather than an alternative to it: every value here
    /// is validated against the field's own format at load, the example must be one of them, and a
    /// renderer that cannot draw a select still knows the input type. See the module docs for why
    /// the two questions stay separate and for what a host does with a stored value that later
    /// leaves the set.
    ///
    /// Order is the vendor's, and it is preserved end to end — a `Vec`, not a set, because "US
    /// first" is a fact about how the vendor names its regions and a re-sorted dropdown is a worse
    /// one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    /// Whether the connector cannot function without it.
    ///
    /// Defaults to `true`: a connector declaring a field it does not need is the rarer case, and the
    /// safe default is to ask.
    #[serde(default = "default_required", skip_serializing_if = "is_true")]
    pub required: bool,
    /// A literal used when an optional value is absent. Unlike [`example`](Self::example), this is
    /// sent on the wire and is therefore validated exactly like a supplied value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Additional policy required to activate a supplied value. `operator` means a non-default
    /// value is inert until the host records an operator approval for this connection.
    #[serde(default, skip_serializing_if = "is_no_approval")]
    pub approval: Approval,
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
    ///
    /// **Exactly one destination, and the head of the list when there are more** — see
    /// [`also_binds`](Self::also_binds). It is what [`slot`](Self::slot) reads, so it decides the
    /// `{placeholder}` the emitted module carries and the `(kind, name)` a host stores the value
    /// under.
    pub binds: String,
    /// **The further destinations this one value also reaches** — empty for all but a handful of
    /// vendors (C-229).
    ///
    /// Algolia's application id composes the hostname *and* travels as `X-Algolia-Application-Id` on
    /// every call. Declaring that as two fields ships two host-side slots for one answer and a
    /// second field with no honest `help`; declaring it as two fields under one name is refused as a
    /// shared slot, correctly. This is the one question those two rules leave unwritable.
    ///
    /// Each entry is a request position — `path.<variable>`, `query.<name>`, `header.<name>` — and
    /// nothing else. See the module docs for why the set is that narrow, why the `endpoint.`
    /// destination is the head rather than an entry here, and why the placeholder every destination
    /// carries is [`binds`](Self::binds)' target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_binds: Vec<String>,
    /// **The further services whose base URL this one value also fills** — empty for all but the
    /// vendors whose surfaces share a deployment (C-529).
    ///
    /// A self-managed GitLab serves its REST API at `{origin}/api/v4` and its OAuth2 authorize and
    /// token endpoints at `{origin}` — the same server, so the same fact. Modelled without this,
    /// that connector needs a second `[[config]]` field binding `endpoint.origin` for the OAuth
    /// service, and the operator is asked one question twice. Two slots that must agree and are not
    /// forced to is how a token exchange ends up pointed at a host the API never approved, and it is
    /// the Contentful defect the service dimension exists to prevent, running the other way.
    ///
    /// So this is **one address, filling several placeholders**. [`service`](Self::service) stays the
    /// head and remains the key the value is stored under; the services named here resolve their
    /// `{variable}` from that same address. One question, one value, one
    /// [`approval`](Self::approval).
    ///
    /// # Why it is explicit rather than derived
    ///
    /// "The same `{variable}` in two base URLs is one slot" would be a shorter rule and a wrong one.
    /// Contentful declares `delivery_space_id` and `management_space_id`, both binding
    /// `endpoint.space_id` in different services, and they are deliberately **two** values: keyed as
    /// one, a management write went to whichever space the delivery reads had been configured with —
    /// a `200` from a real server rather than a refusal. Sharing is therefore something an author
    /// states, and the default stays two slots.
    ///
    /// Only an `endpoint.` binding may carry entries here; the loader refuses any other. A
    /// credential or a request pin has no per-service placeholder for a second service to fill, so
    /// an entry on one would name a service without doing anything there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_services: Vec<String>,
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

    /// Whether this field permits a closed set of values rather than free text.
    ///
    /// The question a renderer asks to decide between a select and an input.
    pub fn is_closed(&self) -> bool {
        !self.choices.is_empty()
    }

    /// **Whether `value` is one this field permits**, with a refusal that names the field and lists
    /// what is legal.
    ///
    /// Always `Ok` on an open field — a field with no closed set permits anything its format
    /// accepts, and the format is a separate check that a renderer applies to a typed value.
    ///
    /// The refusal spells out every choice on purpose. "Invalid value" reproduces the diagnosis
    /// problem a closed set exists to remove: an operator who has just been told their region is
    /// wrong needs to be told what the regions *are*, in the same sentence, or they are back to
    /// guessing between an answer and a credential.
    ///
    /// **Called where a value is supplied, never where one is read back.** See the module docs: a
    /// vendor adding a region must not brick a connection configured before it existed.
    pub fn permits(&self, value: &str) -> Result<(), String> {
        if self.choices.is_empty() || self.choices.iter().any(|choice| choice.value == value) {
            return Ok(());
        }
        let permitted: Vec<String> = self
            .choices
            .iter()
            .map(|choice| format!("{:?} ({})", choice.value, choice.label))
            .collect();
        Err(format!(
            "configuration field {:?} permits only {}, and {value:?} is none of them",
            self.name,
            permitted.join(", ")
        ))
    }

    /// The request position and placeholder [`binds`](Self::binds) pins, when it pins one.
    ///
    /// **The head destination only.** A field that also reaches further positions (C-229) has more
    /// than one pin and a placeholder that is not its wire name, so anything composing a request
    /// wants [`pins`](Self::pins) instead; this stays the answer to the narrower question *"is this
    /// field's own binding a pin"*.
    pub fn pin(&self) -> Option<(Position, &str)> {
        match self.binding()? {
            Binding::Request { position, name } => Some((position, name)),
            _ => None,
        }
    }

    /// Every destination this field reaches, in declaration order, with [`binds`](Self::binds)
    /// first. `None` when any of them is malformed — which the loader refuses, so on a loaded
    /// connector this is always `Some`.
    pub fn bindings(&self) -> Option<Vec<Binding<'_>>> {
        std::iter::once(self.binds.as_str())
            .chain(self.also_binds.iter().map(String::as_str))
            .map(|binds| parse_binding(binds).ok())
            .collect()
    }

    /// **The one host-side slot every destination of this field reads** — the `name` half of the
    /// `(kind, name)` a host stores the value under.
    ///
    /// Always [`binds`](Self::binds)' own target, whatever else the field reaches. That is the whole
    /// answer to the question a multi-destination field forces: [`Binding::Request`]'s `name` is
    /// both the placeholder and the wire spelling, and when two destinations spell the value
    /// differently the placeholder is the head's and the rest contribute only what the vendor sees.
    /// A Basic username head is the one qualified exception: its request pins carry
    /// `username.<slot>` so the host resolves it through the username port rather than mistaking the
    /// credential name for an endpoint variable.
    pub fn slot(&self) -> Option<&str> {
        Some(self.binding()?.target())
    }
    /// **Every request position this field pins**, each carrying the field's host-resolvable
    /// placeholder.
    ///
    /// The question `connector-flux` asks of every field of an operation's service, so it is one
    /// call rather than a `matches!` repeated at each request position. An empty result means the
    /// field reaches no request position at all.
    pub fn pins(&self) -> Vec<Pin<'_>> {
        let Some(head) = self.binding() else {
            return Vec::new();
        };
        let variable = match head {
            Binding::Username { name } => Cow::Owned(format!("username.{name}")),
            _ => Cow::Borrowed(head.target()),
        };
        self.bindings()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|binding| match binding {
                Binding::Request { position, name } => Some(Pin {
                    position,
                    name,
                    variable: variable.clone(),
                }),
                _ => None,
            })
            .collect()
    }
}

/// **One request position a [`ConfigField`] pins, and the placeholder that reaches it.**
///
/// The two strings are the same for every field that binds exactly one destination, and that is why
/// [`Binding::Request`] carries only one: `name` is the placeholder *and* the wire spelling. A field
/// that also reaches a `base_url` variable (C-229) breaks the coincidence — Algolia's application id
/// is `app_id` in the host and `X-Algolia-Application-Id` on the wire — so an emitter needs both,
/// and the pair is spelled out here rather than left to whichever caller guessed right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin<'a> {
    /// Where on the request the value lands.
    pub position: Position,
    /// The name the vendor sees — the header, the query parameter, the path template variable.
    pub name: &'a str,
    /// The `{placeholder}` the emitted module carries for it. It is the field's
    /// [`slot`](ConfigField::slot), except that a Basic username is qualified as
    /// `username.<slot>` so a host resolves it through [`Binding::Username`].
    pub variable: Cow<'a, str>,
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

fn is_no_approval(approval: &Approval) -> bool {
    *approval == Approval::None
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

    /// The three request positions an operator can pin, and the two facts a pin must carry: it is
    /// connection level (one per tenant, derived) and it is never a secret.
    #[test]
    fn a_pinned_request_value_parses_and_is_connection_level_configuration() {
        for (binds, position, name) in [
            ("path.zone_id", Position::Path, "zone_id"),
            ("query.teamId", Position::Query, "teamId"),
            (
                "header.X-Algolia-Application-Id",
                Position::Header,
                "X-Algolia-Application-Id",
            ),
        ] {
            assert_eq!(
                parse_binding(binds),
                Ok(Binding::Request { position, name }),
                "{binds}"
            );
            assert_eq!(
                parse_binding(binds).map(Binding::level),
                Ok(Level::Connection),
                "{binds}: a zone or a team is one per tenant, not one per vendor"
            );
            assert_eq!(
                parse_binding(binds).map(Binding::is_secret),
                Ok(false),
                "{binds}: a pinned value is configuration an operator reads back, not a credential"
            );
        }

        for empty in ["path.", "query.", "header."] {
            assert!(parse_binding(empty).is_err(), "{empty}");
        }
        // A prefix that only *looks* like one of the three is still not a binding.
        assert!(parse_binding("pathzone_id").is_err());
        assert!(parse_binding("headers.X-Thing").is_err());
    }

    /// **A pinned path segment must not be able to escape its segment.** The value lands inside a
    /// URL a host composes, so a `/`, a `..` or a `%`-encoded slash does not fail — it addresses a
    /// different resource on the same host with the same credential.
    #[test]
    fn a_pinned_value_cannot_reshape_the_request_it_lands_in() {
        assert!(Position::Path
            .validate_value("023e105f4ecef8ad9ca31a8372d0c353")
            .is_ok());
        for escape in [
            "..",
            ".",
            "a/b",
            "../../accounts",
            "%2F..%2Fadmin",
            "zone?x=1",
            "zone#frag",
            "zone id",
            "a\\b",
            "",
        ] {
            assert!(
                Position::Path.validate_value(escape).is_err(),
                "a path pin holding {escape:?} would not stay in its segment"
            );
        }

        assert!(Position::Query.validate_value("team_abc123").is_ok());
        for injected in ["a&admin=1", "a=b", "a b", "a+b", "a%26b", "a#f", ""] {
            assert!(
                Position::Query.validate_value(injected).is_err(),
                "nothing percent-encodes a query value, so {injected:?} would add a parameter"
            );
        }

        assert!(Position::Header.validate_value("ABC123XYZ").is_ok());
        assert!(
            Position::Header.validate_value("two words").is_ok(),
            "an HTTP field value may contain an inner space"
        );
        for injected in ["a\r\nX-Evil: 1", "a\nb", " a", "a ", "café", ""] {
            assert!(
                Position::Header.validate_value(injected).is_err(),
                "a header pin holding {injected:?} is not an HTTP field value"
            );
        }

        // A brace is refused everywhere: substitution fills placeholders in emitted literals, so a
        // value spelling one would be filled in twice or reach the vendor as text.
        for position in [Position::Path, Position::Query, Position::Header] {
            assert!(position.validate_value("{subdomain}").is_err());
        }
    }

    /// **The host rule is the strict one of the family** (C-229), and this is why a field reaching a
    /// hostname *and* a request position cannot be checked against the request positions alone.
    ///
    /// `acme.example@evil.example` passes the path rule, the query rule and the header rule — none
    /// of those positions cares about an `@` — and substituted into an authority it moves the origin.
    #[test]
    fn a_host_value_is_refused_for_what_no_request_position_would_catch() {
        assert!(validate_host_value("B1G2GM9NG0").is_ok());
        assert!(validate_host_value("acme-corp").is_ok());
        assert!(validate_host_value("acme.freshdesk.com").is_ok());

        let moves_the_origin = "acme.example@evil.example";
        assert!(Position::Path.validate_value(moves_the_origin).is_ok());
        assert!(Position::Query.validate_value(moves_the_origin).is_ok());
        assert!(Position::Header.validate_value(moves_the_origin).is_ok());
        assert!(
            validate_host_value(moves_the_origin).is_err(),
            "the `@` turns everything before it into userinfo, and only the host rule sees that"
        );

        for bad in ["acme:8080", "a/b", "acme..example", "acme example", ""] {
            assert!(validate_host_value(bad).is_err(), "{bad:?}");
        }

        // And the dispatcher agrees with each of its arms, which is what lets the loader check every
        // destination of a field from one loop.
        assert_eq!(
            Binding::Endpoint { variable: "app_id" }.validate_value(moves_the_origin),
            validate_host_value(moves_the_origin)
        );
        assert_eq!(
            Binding::Request {
                position: Position::Header,
                name: "X-Algolia-Application-Id"
            }
            .validate_value("café"),
            Position::Header.validate_value("café")
        );
        // A destination that is not substituted into a request the emitter composes answers `Ok`:
        // a credential travels through the secret side, and a Basic username is joined and encoded.
        assert_eq!(
            Binding::Credential { name: "acme.token" }.validate_value("anything at all"),
            Ok(())
        );
    }

    /// One field, several destinations, **one placeholder** — the C-229 derivation, at the level of
    /// the type rather than of a provider file.
    #[test]
    fn a_multi_destination_field_carries_one_slot_into_every_pin() {
        let field = ConfigField {
            name: "app_id".to_owned(),
            service: default_service(),
            label: "Algolia application id".to_owned(),
            help: "Both the host prefix and a header".to_owned(),
            example: Some("B1G2GM9NG0".to_owned()),
            format: Format::Text,
            choices: Vec::new(),
            required: true,
            default: None,
            approval: Approval::None,
            secret: false,
            docs_url: None,
            binds: "endpoint.app_id".to_owned(),
            also_binds: vec!["header.X-Algolia-Application-Id".to_owned()],
            also_services: Vec::new(),
        };

        assert_eq!(
            field.bindings(),
            Some(vec![
                Binding::Endpoint { variable: "app_id" },
                Binding::Request {
                    position: Position::Header,
                    name: "X-Algolia-Application-Id"
                },
            ])
        );
        assert_eq!(field.slot(), Some("app_id"));
        assert_eq!(
            field.pins(),
            vec![Pin {
                position: Position::Header,
                name: "X-Algolia-Application-Id",
                variable: "app_id".into(),
            }]
        );
        // `binding`, `level` and `pin` still answer about the head, so every consumer that existed
        // before this landed reads exactly what it read before.
        assert_eq!(
            field.binding(),
            Some(Binding::Endpoint { variable: "app_id" })
        );
        assert_eq!(field.level(), Some(Level::Connection));
        assert_eq!(field.pin(), None);

        // A malformed destination makes the whole set unreadable rather than silently shorter.
        let broken = ConfigField {
            also_binds: vec!["cookie.session".to_owned()],
            also_services: Vec::new(),
            ..field
        };
        assert_eq!(broken.bindings(), None);
        assert!(broken.pins().is_empty());
    }

    #[test]
    fn a_username_head_qualifies_the_placeholder_of_its_request_pin() {
        let field = ConfigField {
            name: "account_sid".to_owned(),
            service: default_service(),
            label: "Account SID".to_owned(),
            help: "The same value authenticates and scopes every request".to_owned(),
            example: Some("AC00000000000000000000000000000000".to_owned()),
            format: Format::Token,
            choices: Vec::new(),
            required: true,
            default: None,
            approval: Approval::None,
            secret: false,
            docs_url: None,
            binds: "username.twilio.basic_auth".to_owned(),
            also_binds: vec!["path.AccountSid".to_owned()],
            also_services: Vec::new(),
        };

        assert_eq!(field.slot(), Some("twilio.basic_auth"));
        assert_eq!(
            field.pins(),
            vec![Pin {
                position: Position::Path,
                name: "AccountSid",
                variable: "username.twilio.basic_auth".into(),
            }]
        );
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

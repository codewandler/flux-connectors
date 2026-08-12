//! **The engine-free channel-handshake producer** (C-558): a connector's socket binding and the
//! bound tenant ports in, a complete RFC 6455 handshake plan out.
//!
//! This is the logic that lived in `connector-pack`'s `channel_plan` — plus
//! `Configuration::channel_snapshot` and `Credentials::resolve_channel` — relocated so a consumer
//! that never touches `connector-pack` and holds no flux `ToolContext` can compose a channel
//! handshake. `connector-pack`'s `channel_plan` now delegates here, adapting its flux-side
//! `Configuration`/`Credentials` into the bound [`ConfigPort`](crate::ConfigPort) and
//! [`SecretStore`](connector_secrets::SecretStore) ports, so the two are one implementation held to
//! the channel differential gate rather than two that could drift.
//!
//! What it applies, in the order the live path applied it: the channel service base URL substituted
//! from the tenant's endpoint config with the declared defaults overlaid, the composed authority
//! validated against the declared one via [`validate_templated_authority`](crate::validate_templated_authority),
//! the `connect.query` values placed (each validated as a query slot), the declared headers, and the
//! `connect.auth` credentials resolved and placed. A single unresolved variable is a refusal naming
//! the field, never a brace on the wire.
//!
//! # It touches no redactor, and that is the plan's posture rather than a returned set
//!
//! Unlike the operation-path assembler, this producer collects no redaction `Vec`: the
//! [`PreparedChannelPlan`] carries its secret-bearing fields — the URL and every header — as
//! [`SensitiveText`], so they do not reveal themselves through `Debug`, and no redactor is consulted
//! here. Where those values are scrubbed is the host's, exactly as it was on the flux-fed path.

use std::collections::BTreeMap;

use catalog::{ChannelTransport, Placement};
use connector_secrets::{InstanceId, SecretStore};

use crate::auth::{query_encode, Assembled};
use crate::config::{ConfigField, ConfigPort, ConfigValue};
use crate::credentials::resolve_channel;
use crate::plan::SensitiveText;
use crate::{validate_templated_authority, Error, Slot};

/// **A complete RFC 6455 handshake plan containing no client, socket, resolver or runtime.**
///
/// The unit a host wraps and dispatches; deriving one edits nothing that reaches a wire. Its `Debug`
/// reveals no credential: the [`url`](Self::url) and every [`headers`](Self::headers) value is a
/// [`SensitiveText`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PreparedChannelPlan {
    /// The connector this binding belongs to.
    pub provider: &'static str,
    /// The channel binding name.
    pub binding: &'static str,
    /// The service the binding's base URL and configuration are addressed under.
    pub service: &'static str,
    /// The declaration-level origin contract, before tenant endpoint substitution.
    pub declared_base_url: &'static str,
    /// The composed `wss://`/`ws://` URL, credentials placed — secret-bearing, so guarded.
    pub url: SensitiveText,
    /// The handshake request headers, each value guarded because any may carry a credential.
    pub headers: BTreeMap<String, SensitiveText>,
    /// The declared WebSocket subprotocols, in order.
    pub subprotocols: Vec<&'static str>,
    /// The local event names this binding delivers.
    pub events: &'static [&'static str],
    /// Exact vendor discriminator values mapped to the closed local event names.
    pub wire_events: BTreeMap<&'static str, &'static str>,
    /// How an inbound envelope names which event it carries.
    pub discriminator: Option<catalog::Selector>,
    /// Stable vendor delivery identity used for deduplication, when declared.
    pub delivery_id: Option<catalog::Selector>,
    /// The declared payload projection.
    pub payload: &'static [catalog::Pair],
    /// Whether the payload is the envelope root rather than a projected subset.
    pub payload_root: bool,
}

/// **Compose a generated socket channel from catalogue declarations and bound tenant ports** (C-558).
///
/// `provider` and `channel` are the resolved catalogue declarations — a caller that has only a
/// binding name resolves it with [`catalog::Provider::channel`] first. `secrets` resolves the
/// `connect.auth` credentials and `config` supplies the non-secret connection values; `tenant` and
/// `instance` address them.
///
/// This function may consult the supplied ports. It never resolves a host, constructs a client or
/// opens a socket; dispatch remains the consumer's.
///
/// # Errors
///
/// [`Error::NotSocketChannel`] for a non-socket binding and [`Error::VendorSocketChannel`] for a
/// vendor-specific socket with no generic `connect`; [`Error::MissingConfig`] and
/// [`Error::UnsafeConfig`] for the endpoint and query configuration; [`Error::Unbuildable`] and
/// [`Error::UnresolvedEndpoint`] for a base URL that will not compose; and every credential refusal
/// [`resolve_channel`] and [`apply_auth`] raise. Each refuses rather than composing a partial
/// handshake.
pub async fn channel_plan(
    provider: &'static catalog::Provider,
    channel: &'static catalog::Channel,
    tenant: &str,
    instance: Option<&InstanceId>,
    secrets: &dyn SecretStore,
    config: &dyn ConfigPort,
) -> Result<PreparedChannelPlan, Error> {
    let operation = format!("{}#{}", provider.id, channel.name);
    if channel.transport != ChannelTransport::Socket {
        return Err(Error::NotSocketChannel {
            provider: provider.id.to_owned(),
            binding: channel.name.to_owned(),
        });
    }
    let connect = channel
        .connect
        .as_ref()
        .ok_or_else(|| Error::VendorSocketChannel {
            provider: provider.id.to_owned(),
            binding: channel.name.to_owned(),
        })?;

    let settings = ChannelSettings {
        config,
        provider,
        service: channel.service,
        tenant,
        operation: &operation,
    };

    let mut base = substitute_endpoint(channel.base_url, &settings, &operation)?;
    if let Some(rest) = base.strip_prefix("https://") {
        base = format!("wss://{rest}");
    } else if let Some(rest) = base.strip_prefix("http://") {
        base = format!("ws://{rest}");
    } else {
        return Err(Error::Unbuildable {
            operation,
            message: "the channel service base URL is neither HTTP nor HTTPS".to_owned(),
        });
    }
    let mut url = format!("{}{}", base.trim_end_matches('/'), connect.path);
    for pair in connect.query {
        let value = if let Some(field_name) = pair
            .value
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        {
            let declaration = provider
                .config
                .iter()
                .find(|field| field.service == channel.service && field.name == field_name)
                .ok_or_else(|| Error::Unbuildable {
                    operation: operation.clone(),
                    message: format!(
                        "query template names undeclared configuration `{field_name}`"
                    ),
                })?;
            let prefix = format!("channel.{}.query.", channel.name);
            let parameter =
                declaration
                    .binds
                    .strip_prefix(&prefix)
                    .ok_or_else(|| Error::Unbuildable {
                        operation: operation.clone(),
                        message: format!(
                            "configuration `{field_name}` is not bound to this channel"
                        ),
                    })?;
            let raw = settings.require(
                ConfigField::ChannelQuery {
                    channel: channel.name,
                    parameter,
                },
                format!("channel.{}.query.{parameter}", channel.name),
            )?;
            Slot::Query
                .validate(&raw)
                .map_err(|reason| Error::UnsafeConfig {
                    operation: operation.clone(),
                    variable: field_name.to_owned(),
                    position: Slot::Query.word(),
                    reason,
                })?
        } else {
            query_encode(pair.value)
        };
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(pair.name);
        url.push('=');
        url.push_str(&value);
    }

    let mut headers = connect
        .headers
        .iter()
        .map(|pair| (pair.name.to_owned(), SensitiveText::new(pair.value)))
        .collect::<BTreeMap<_, _>>();
    let assembled = resolve_channel(
        &operation,
        provider,
        connect.auth,
        tenant,
        instance,
        secrets,
        &settings,
    )
    .await?;
    apply_auth(&operation, &mut url, &mut headers, assembled)?;

    let wire_events = channel
        .events
        .iter()
        .map(|name| {
            let event = provider
                .events
                .iter()
                .find(|event| event.name == *name)
                .ok_or_else(|| Error::Unbuildable {
                    operation: operation.clone(),
                    message: format!("channel names undeclared event `{name}`"),
                })?;
            Ok((event.wire_value.unwrap_or(event.name), event.name))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;

    Ok(PreparedChannelPlan {
        provider: provider.id,
        binding: channel.name,
        service: channel.service,
        declared_base_url: channel.base_url,
        url: SensitiveText::new(url),
        headers,
        subprotocols: connect.subprotocols.to_vec(),
        events: channel.events,
        wire_events,
        discriminator: channel.discriminator,
        delivery_id: channel.delivery_id,
        payload: channel.payload,
        payload_root: channel.payload_root,
    })
}

/// **A channel handshake's connection settings, with the declared defaults overlaid** (C-558).
///
/// The relocated body of `connector-pack`'s `Configuration::channel_snapshot`: it reads the tenant's
/// raw value through the bound [`ConfigPort`], drops an empty or all-whitespace one, and falls back
/// to the connector's declared default. Implementing [`ConfigPort`] itself is what lets the credential
/// resolver read the Basic user half through the same defaulting view rather than a second one that
/// could disagree about a default.
struct ChannelSettings<'a> {
    config: &'a dyn ConfigPort,
    provider: &'static catalog::Provider,
    service: &'static str,
    tenant: &'a str,
    operation: &'a str,
}

impl ChannelSettings<'_> {
    /// The tenant's value, an empty or all-whitespace one dropped, then the declared default — or
    /// `None` when the connector declares neither.
    fn value(&self, field: ConfigField<'_>) -> Option<String> {
        self.config
            .resolve(field)
            .filter(|resolved| !resolved.value().trim().is_empty())
            .map(|resolved| resolved.value().to_owned())
            .or_else(|| self.default_for(field).map(str::to_owned))
    }

    /// One field, or the refusal that names what is missing — keyed exactly as the flux-fed
    /// snapshot's `require` was, so an operator sees the same `binds` target.
    fn require(&self, field: ConfigField<'_>, binding: String) -> Result<String, Error> {
        self.value(field).ok_or_else(|| Error::MissingConfig {
            operation: self.operation.to_owned(),
            provider: self.provider.id.to_owned(),
            service: self.service.to_owned(),
            tenant: self.tenant.to_owned(),
            field: binding,
        })
    }

    /// The connector's declared default for `field`, looked up by its `binds` target for this
    /// channel's service.
    fn default_for(&self, field: ConfigField<'_>) -> Option<&'static str> {
        let binds = binds_of(field);
        self.provider
            .config
            .iter()
            .find(|declaration| declaration.service == self.service && declaration.binds == binds)
            .and_then(|declaration| declaration.default)
    }
}

impl ConfigPort for ChannelSettings<'_> {
    fn resolve(&self, field: ConfigField<'_>) -> Option<ConfigValue> {
        self.value(field).map(ConfigValue::proposed)
    }
}

/// The `binds` target one [`ConfigField`] addresses, as the connector's `[[config]]` spells it.
fn binds_of(field: ConfigField<'_>) -> String {
    match field {
        ConfigField::Endpoint(name) => format!("endpoint.{name}"),
        ConfigField::Username(name) => format!("username.{name}"),
        ConfigField::ChannelQuery { channel, parameter } => {
            format!("channel.{channel}.query.{parameter}")
        }
    }
}

/// Substitute the channel service base URL from the tenant's endpoint config, refusing any value
/// that would reshape the declared authority.
fn substitute_endpoint(
    template: &str,
    settings: &ChannelSettings,
    operation: &str,
) -> Result<String, Error> {
    let mut output = String::with_capacity(template.len());
    let mut remainder = template;
    let mut first_variable = None;
    while let Some(open) = remainder.find('{') {
        output.push_str(&remainder[..open]);
        let after = &remainder[open + 1..];
        let close = after.find('}').ok_or_else(|| Error::UnresolvedEndpoint {
            operation: operation.to_owned(),
            variable: "<malformed>".to_owned(),
            url: template.to_owned(),
        })?;
        let variable = &after[..close];
        first_variable.get_or_insert(variable.to_owned());
        let raw = settings.require(
            ConfigField::Endpoint(variable),
            format!("endpoint.{variable}"),
        )?;
        let value = Slot::Unplaced
            .validate(&raw)
            .map_err(|_| Error::UnsafeConfig {
                operation: operation.to_owned(),
                variable: variable.to_owned(),
                position: Slot::Host.word(),
                reason: "the configured value would reshape the channel authority declared by the connector"
                    .to_owned(),
            })?;
        output.push_str(&value);
        remainder = &after[close + 1..];
    }
    output.push_str(remainder);
    if output.contains(['{', '}']) {
        return Err(Error::UnresolvedEndpoint {
            operation: operation.to_owned(),
            variable: "<malformed>".to_owned(),
            url: output,
        });
    }
    if let Some(variable) = first_variable {
        let Some(template_authority) = url_authority(template) else {
            return Err(Error::Unbuildable {
                operation: operation.to_owned(),
                message: "the channel service base URL has no authority".to_owned(),
            });
        };
        let Some(composed_authority) = url_authority(&output) else {
            return Err(Error::Unbuildable {
                operation: operation.to_owned(),
                message: "the composed channel URL has no authority".to_owned(),
            });
        };
        validate_templated_authority(template_authority, composed_authority).map_err(|_| {
            Error::UnsafeConfig {
                operation: operation.to_owned(),
                variable,
                position: Slot::Host.word(),
                reason: "the configured value would reshape the channel authority declared by the connector"
                    .to_owned(),
            }
        })?;
    }
    Ok(output)
}

/// The authority of a URL — everything between `://` and the first `/`, `?` or `#`.
fn url_authority(url: &str) -> Option<&str> {
    url.split_once("://")
        .map(|(_, rest)| &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())])
}

/// Place the resolved `connect.auth` credentials onto the composed URL or headers.
fn apply_auth(
    operation: &str,
    url: &mut String,
    headers: &mut BTreeMap<String, SensitiveText>,
    assembled: Vec<Assembled>,
) -> Result<(), Error> {
    for value in assembled {
        match value.placement() {
            Placement::Header { name, prefix } => {
                if let Some(existing) = headers
                    .keys()
                    .find(|header| header.eq_ignore_ascii_case(name))
                {
                    return Err(Error::CredentialCollision {
                        operation: operation.to_owned(),
                        credential: value.credential().to_owned(),
                        header: existing.clone(),
                    });
                }
                headers.insert(
                    name.to_owned(),
                    SensitiveText::new(format!("{prefix}{}", value.expose_value())),
                );
            }
            Placement::Query { name } => {
                url.push(if url.contains('?') { '&' } else { '?' });
                url.push_str(name);
                url.push('=');
                url.push_str(&query_encode(value.expose_value()));
            }
            Placement::Inbound => {
                return Err(Error::InboundCredential {
                    operation: operation.to_owned(),
                    credential: value.credential().to_owned(),
                });
            }
        }
    }
    Ok(())
}

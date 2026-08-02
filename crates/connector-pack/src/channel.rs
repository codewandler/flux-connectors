//! Zero-transport composition of a generated connector socket binding.

use std::collections::BTreeMap;

use catalog::{ChannelTransport, Placement, ProviderKey};

use crate::auth::{query_encode, Assembled};
use crate::config::{Field, Snapshot};
use crate::request::Slot;
use crate::{Configuration, Credentials, Error};

/// Text that may contain credential material and therefore never reveals itself through `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveText(String);

impl SensitiveText {
    /// Deliberately expose the prepared wire value to the guarded WebSocket handshake.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SensitiveText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A complete RFC 6455 handshake plan containing no client, socket, resolver or runtime.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PreparedChannelPlan {
    pub provider: &'static str,
    pub binding: &'static str,
    pub service: &'static str,
    /// The declaration-level origin contract, before tenant endpoint substitution.
    pub declared_base_url: &'static str,
    pub url: SensitiveText,
    pub headers: BTreeMap<String, SensitiveText>,
    pub subprotocols: Vec<&'static str>,
    pub events: &'static [&'static str],
    /// Exact vendor discriminator values mapped to the closed local event names.
    pub wire_events: BTreeMap<&'static str, &'static str>,
    pub discriminator: Option<catalog::Selector>,
    pub payload: &'static [catalog::Pair],
    pub payload_root: bool,
}

/// Compose a generated socket channel from catalogue declarations and bound tenant ports.
///
/// This function may consult the supplied configuration and credential stores. It never resolves a
/// host, constructs a client or opens a socket; execution remains the selected Flux system's job.
pub async fn channel_plan(
    provider_id: &str,
    binding: &str,
    credentials: Credentials,
    configuration: Configuration,
) -> Result<PreparedChannelPlan, Error> {
    let operation = format!("{provider_id}#{binding}");
    if credentials.tenant() != configuration.tenant() {
        return Err(Error::TenantMismatch {
            operation: operation.clone(),
            credentials: credentials.tenant().to_owned(),
            configuration: configuration.tenant().to_owned(),
        });
    }
    let provider =
        catalog::provider(ProviderKey::id(provider_id)).ok_or_else(|| Error::UnknownProvider {
            provider: provider_id.to_owned(),
            available: catalog::providers().len(),
        })?;
    let channel = provider
        .channel(binding)
        .ok_or_else(|| Error::UnknownChannel {
            provider: provider_id.to_owned(),
            binding: binding.to_owned(),
        })?;
    if channel.transport != ChannelTransport::Socket {
        return Err(Error::NotSocketChannel {
            provider: provider_id.to_owned(),
            binding: binding.to_owned(),
        });
    }
    let connect = channel
        .connect
        .as_ref()
        .ok_or_else(|| Error::VendorSocketChannel {
            provider: provider_id.to_owned(),
            binding: binding.to_owned(),
        })?;
    let settings = configuration.channel_snapshot(provider, channel);
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
            let prefix = format!("channel.{binding}.query.");
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
                &operation,
                Field::ChannelQuery {
                    channel: binding,
                    parameter,
                },
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
        .map(|pair| (pair.name.to_owned(), SensitiveText(pair.value.to_owned())))
        .collect::<BTreeMap<_, _>>();
    let assembled = credentials
        .resolve_channel(&operation, provider, connect.auth, &settings)
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
        url: SensitiveText(url),
        headers,
        subprotocols: connect.subprotocols.to_vec(),
        events: channel.events,
        wire_events,
        discriminator: channel.discriminator,
        payload: channel.payload,
        payload_root: channel.payload_root,
    })
}

fn substitute_endpoint(
    template: &str,
    settings: &Snapshot,
    operation: &str,
) -> Result<String, Error> {
    let mut output = String::with_capacity(template.len());
    let mut remainder = template;
    while let Some(open) = remainder.find('{') {
        output.push_str(&remainder[..open]);
        let after = &remainder[open + 1..];
        let close = after.find('}').ok_or_else(|| Error::UnresolvedEndpoint {
            operation: operation.to_owned(),
            variable: "<malformed>".to_owned(),
            url: template.to_owned(),
        })?;
        let variable = &after[..close];
        let raw = settings.require(operation, Field::Endpoint(variable))?;
        let value = Slot::Unplaced
            .validate(&raw)
            .map_err(|reason| Error::UnsafeConfig {
                operation: operation.to_owned(),
                variable: variable.to_owned(),
                position: Slot::Host.word(),
                reason,
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
    Ok(output)
}

fn apply_auth(
    operation: &str,
    url: &mut String,
    headers: &mut BTreeMap<String, SensitiveText>,
    assembled: Vec<Assembled>,
) -> Result<(), Error> {
    for value in assembled {
        match value.place {
            Placement::Header { name, prefix } => {
                if let Some(existing) = headers
                    .keys()
                    .find(|header| header.eq_ignore_ascii_case(name))
                {
                    return Err(Error::CredentialCollision {
                        operation: operation.to_owned(),
                        credential: value.credential.to_owned(),
                        header: existing.clone(),
                    });
                }
                headers.insert(
                    name.to_owned(),
                    SensitiveText(format!("{prefix}{}", value.value)),
                );
            }
            Placement::Query { name } => {
                url.push(if url.contains('?') { '&' } else { '?' });
                url.push_str(name);
                url.push('=');
                url.push_str(&query_encode(&value.value));
            }
            Placement::Inbound => {
                return Err(Error::InboundCredential {
                    operation: operation.to_owned(),
                    credential: value.credential.to_owned(),
                });
            }
        }
    }
    Ok(())
}

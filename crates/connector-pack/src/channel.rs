//! Zero-transport composition of a generated connector socket binding.
//!
//! **The composition itself moved to `connector-resolve`** (C-558). What lived here — the base-URL
//! substitution, the `validate_templated_authority` check, the `connect.query` placement and the
//! `connect.auth` resolution — is now the engine-free [`connector_resolve::channel_plan`], so a host
//! that never touches `connector-pack` (Exchange, X-156) can derive a channel handshake with no flux
//! `ToolContext`. This file is the thin flux-side wrapper: it compares the two bound ports' tenants,
//! resolves the provider and binding, adapts the [`Configuration`] into the engine-free
//! [`ConfigPort`](connector_resolve::ConfigPort), and delegates. Its channel behaviour is unchanged.

use catalog::ProviderKey;

use crate::config::Field;
use crate::{Configuration, Credentials, Error};

// **`SensitiveText` and `PreparedChannelPlan` live in `connector-resolve` and are re-exported here.**
// The handshake plan is engine-free (it carries only `SensitiveText`, no flux type), and the producer
// that returns it is engine-free too, so both belong on that side; the surface a host imports from
// `connector-pack` is unchanged.
pub use connector_resolve::{PreparedChannelPlan, SensitiveText};

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
            operation,
            credentials: credentials.tenant().to_owned(),
            configuration: configuration.tenant().to_owned(),
        });
    }
    if credentials.instance() != configuration.instance() {
        return Err(Error::InstanceMismatch {
            operation,
            credentials: credentials.instance().map(|id| id.as_str().to_owned()),
            configuration: configuration.instance().map(|id| id.as_str().to_owned()),
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

    // The engine-free producer applies the declared defaults itself (relocated from
    // `channel_snapshot`), so this port hands it only what the tenant actually configured, keyed to
    // the channel's own service.
    let port = ConfigurationChannelPort {
        configuration: &configuration,
        provider: provider.id,
        service: channel.service,
    };
    let plan = connector_resolve::channel_plan(
        provider,
        channel,
        credentials.tenant(),
        credentials.instance(),
        credentials.store(),
        &port,
    )
    .await?;
    Ok(plan)
}

/// **Adapts a flux-side [`Configuration`] to the engine-free [`connector_resolve::ConfigPort`]** for
/// a channel handshake (C-558).
///
/// It reads the raw tenant value for the channel's service and leaves the declared defaults to the
/// producer, which is where `channel_snapshot`'s default logic now lives. Approval never gated the
/// channel path, so this reads the plain value rather than the operator-approved one.
struct ConfigurationChannelPort<'a> {
    configuration: &'a Configuration,
    provider: &'static str,
    service: &'static str,
}

impl connector_resolve::ConfigPort for ConfigurationChannelPort<'_> {
    fn resolve(
        &self,
        field: connector_resolve::ConfigField<'_>,
    ) -> Option<connector_resolve::ConfigValue> {
        let field = match field {
            connector_resolve::ConfigField::Endpoint(name) => Field::Endpoint(name),
            connector_resolve::ConfigField::Username(name) => Field::Username(name),
            connector_resolve::ConfigField::ChannelQuery { channel, parameter } => {
                Field::ChannelQuery { channel, parameter }
            }
        };
        self.configuration
            .channel_value(self.provider, self.service, field)
            .map(connector_resolve::ConfigValue::proposed)
    }
}

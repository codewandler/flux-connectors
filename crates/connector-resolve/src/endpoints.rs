//! **The engine-free endpoint resolver** (C-557): a tenant's configuration port in, the resolved
//! endpoint map [`resolve`](crate::resolve()) expects out.
//!
//! This is the logic that lived in `connector-pack`'s `Configuration::snapshot` and
//! `Operation::endpoints`/`endpoint`, relocated so a consumer that never touches `connector-pack` can
//! run it. `connector-pack`'s producers now delegate here, so it is one implementation held to the
//! whole-catalogue differential gate rather than two that could drift.
//!
//! The enforcement it carries, in the order the live path applied it: the tenant's value, an empty or
//! all-whitespace one treated as absent, then the connector's **declared default**, then — for a
//! field declared `format = "origin"` — [`HttpsOrigin`] normalisation, then the **operator-approval**
//! gate for an [`Approval::Operator`](catalog::Approval::Operator) field whose value is neither the
//! reviewed default nor operator-approved. A single unresolved variable is a refusal naming the field,
//! never a brace on the wire: total or refused, the same rule [`resolve`](crate::resolve()) holds for the request
//! it composes on top.

use std::collections::BTreeMap;

use connector_address::HttpsOrigin;

use crate::config::{ConfigField, ConfigPort, ConfigValue};
use crate::document::Operation;
use crate::Error;

/// **Resolve every endpoint variable `operation` names, or refuse** (C-557).
///
/// The map keys every name [`Operation::endpoint_variables`] reports and its resolved value, which is
/// the exact shape [`resolve`](crate::resolve()) and [`build_request`](crate::build_request) consume.
/// `tenant` names whose settings these are, for the refusal a missing value produces.
///
/// # Errors
///
/// [`Error::MissingConfig`] for a variable the port supplies nothing for and the connector declares
/// no default for; [`Error::UnsafeOrigin`] for an origin field whose value is not a canonical HTTPS
/// origin; [`Error::UnapprovedConfig`] for an operator-gated field bound to an unapproved non-default
/// value. Each refuses rather than composing a partial URL.
pub fn resolve_endpoints(
    operation: &Operation,
    provider: &catalog::Provider,
    tenant: &str,
    config: &dyn ConfigPort,
) -> Result<BTreeMap<String, String>, Error> {
    operation
        .endpoint_variables()
        .iter()
        .map(|variable| {
            resolve_endpoint(
                &operation.id,
                provider,
                &operation.service,
                tenant,
                variable,
                config,
            )
            .map(|value| (variable.clone(), value))
        })
        .collect()
}

/// **Resolve and validate one endpoint value** through the same path request construction and a
/// host's permission fallback both take (C-557).
///
/// Split out because the permission fallback resolves variables one at a time, best-effort, and must
/// honour the same refusals request construction does — an operator-gated proposal is inert data,
/// neither a request destination nor a subject that may enter an approval prompt, intent or evidence
/// record. An origin is normalised here, once, so the request destination, the permission subject and
/// the intent are the same value by construction rather than by two code paths agreeing.
///
/// # Errors
///
/// As [`resolve_endpoints`], for this one variable.
pub fn resolve_endpoint(
    operation: &str,
    provider: &catalog::Provider,
    service: &str,
    tenant: &str,
    variable: &str,
    config: &dyn ConfigPort,
) -> Result<String, Error> {
    let field = ConfigField::from_placeholder(variable);
    let declaration = endpoint_declaration(provider, service, variable);

    // The tenant's value, an empty or all-whitespace one dropped as if unset, then the connector's
    // declared default. A default is `proposed`, exactly as the live path inserts it: an
    // operator-gated default still passes the approval gate below because it is the reviewed value,
    // not because it is approved.
    let resolved = config
        .resolve(field)
        .filter(|resolved| !resolved.value().trim().is_empty())
        .or_else(|| {
            declaration
                .and_then(|declaration| declaration.default)
                .map(ConfigValue::proposed)
        })
        .ok_or_else(|| Error::MissingConfig {
            operation: operation.to_owned(),
            provider: provider.id.to_owned(),
            service: service.to_owned(),
            tenant: tenant.to_owned(),
            field: format!("endpoint.{variable}"),
        })?;

    let mut value = resolved.value().to_owned();
    if let Some(declaration) = declaration {
        let mut is_declared_default = declaration.default == Some(resolved.value());
        if declaration.format == "origin" {
            let origin =
                HttpsOrigin::parse(resolved.value()).map_err(|refusal| Error::UnsafeOrigin {
                    operation: operation.to_owned(),
                    provider: provider.id.to_owned(),
                    service: service.to_owned(),
                    field: declaration.name.to_owned(),
                    reason: refusal.to_string(),
                })?;
            // The declared default is canonical — the loader refuses a declaration that is not — so
            // parsing it cannot fail; a `None` here is a connector that declares no default, which is
            // not the reviewed destination either way.
            is_declared_default = declaration
                .default
                .and_then(|default| HttpsOrigin::parse(default).ok())
                .is_some_and(|default| default == origin);
            value = origin.into_string();
        }
        match declaration.approval {
            catalog::Approval::None => {}
            catalog::Approval::Operator
                if !is_declared_default && !resolved.is_operator_approved() =>
            {
                return Err(Error::UnapprovedConfig {
                    operation: operation.to_owned(),
                    provider: provider.id.to_owned(),
                    service: service.to_owned(),
                    field: declaration.name.to_owned(),
                });
            }
            catalog::Approval::Operator => {}
        }
    }
    Ok(value)
}

/// The `[[config]]` field of `provider` that binds `endpoint.<variable>` for `service`, if any.
///
/// The service is part of the match, not a hint: two services of one connector may bind the same
/// `{variable}` in their own base URLs, and keying without the service collapses them.
fn endpoint_declaration<'a>(
    provider: &'a catalog::Provider,
    service: &str,
    variable: &str,
) -> Option<&'a catalog::ConfigField> {
    let binding = format!("endpoint.{variable}");
    provider
        .config
        .iter()
        .find(|field| field.service == service && field.binds == binding)
}

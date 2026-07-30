//! One catalogue operation, as a thing a host's [`ToolRegistry`](flux_runtime::ToolRegistry) holds.

use async_trait::async_trait;
use flux_runtime::{Tool, ToolContext, ToolResult};
use flux_spec::ToolSpec;
use serde_json::Value;

use crate::{not_wired_yet, spec, Error};

/// A projected operation: its catalogue entry, and the spec a host resolves it by.
///
/// The spec is projected **once, at install time** rather than on each [`Tool::spec`] call. That is
/// not a cache — `spec()` returns a `ToolSpec` by value and cannot fail, so a projection error
/// discovered there would have nowhere to go but a panic inside a host's registration. Projecting
/// up front turns it into an error `pack` returns, which is the whole reason
/// [`ToolRegistry::try_register_all_from`](flux_runtime::ToolRegistry::try_register_all_from)
/// exists.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The catalogue entry this tool was projected from. Held for C-115, which builds the request
    /// from it; nothing here reads it yet beyond the projection above.
    entry: &'static catalog::Operation,
    /// The projected declaration, complete before the tool is ever registered.
    spec: ToolSpec,
}

impl Operation {
    /// Project `entry` and hold the result.
    ///
    /// # Errors
    ///
    /// Whatever [`spec::project`] refuses — an id with no dotted tool name, or an entry whose
    /// embedded Flux is not the single declaration a catalogue rendering is.
    pub fn project(entry: &'static catalog::Operation) -> Result<Self, Error> {
        Ok(Self {
            spec: spec::project(entry)?,
            entry,
        })
    }

    /// The catalogue entry behind this tool.
    pub fn entry(&self) -> &'static catalog::Operation {
        self.entry
    }
}

#[async_trait]
impl Tool for Operation {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    /// **Not wired yet — C-115.**
    ///
    /// The story that lands request construction also lands the mirrored network gate
    /// (`permission_subjects` returning the request URL, `intents` raising `NetworkFetch`). Until
    /// both arrive together, this returns an error naming the story: a panic reachable from a host
    /// process is strictly worse than a failed tool call, and a *successful* call that skipped the
    /// gate would be worse than either.
    async fn execute(&self, _ctx: &ToolContext, _params: Value) -> flux_core::Result<ToolResult> {
        Err(not_wired_yet(&self.spec.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalog::OperationKey;

    fn projected(id: &str) -> Operation {
        let entry = catalog::operation(OperationKey::id(id))
            .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
        Operation::project(entry).unwrap_or_else(|error| panic!("`{id}`: {error}"))
    }

    #[test]
    fn the_spec_is_the_projection_and_the_entry_is_kept() {
        let tool = projected("zendesk-ticket-show");

        assert_eq!(tool.spec().name, "zendesk.ticket.show");
        assert_eq!(tool.entry().id, "zendesk-ticket-show");
        assert_eq!(tool.entry().provider, "zendesk");
    }

    /// `spec()` is called by `try_register_from` and again by every dispatch. Returning a different
    /// value on a later call would mean a tool registered under one contract and dispatched under
    /// another.
    #[test]
    fn the_spec_does_not_change_between_calls() {
        let tool = projected("zendesk-ticket-comment-add");
        let first = serde_json::to_value(tool.spec()).expect("a spec serializes");
        let second = serde_json::to_value(tool.spec()).expect("a spec serializes");

        assert_eq!(first, second);
    }

    /// The trait's defaults are empty for `permission_subjects` and `intents`, which is only
    /// tolerable while `execute` cannot reach the network. This test states that dependency, so
    /// C-115 cannot land request delegation without also confronting it: the moment `execute`
    /// delegates to `HttpRequestTool`, this assertion is the one that has to be *inverted*, and it
    /// is here to be found rather than to be right forever.
    #[test]
    fn the_network_gate_is_unmirrored_only_because_execute_is_inert() {
        let tool = projected("zendesk-ticket-show");
        let params = serde_json::json!({ "ticket_id": 1 });

        assert!(
            tool.permission_subjects(&params).is_empty(),
            "a non-empty subject here means execute reaches the network — C-115 must mirror the \
             gate before that is true"
        );
        assert!(
            tool.intents(&params).intents.is_empty(),
            "an intent here means the same thing as a subject does"
        );
    }
}

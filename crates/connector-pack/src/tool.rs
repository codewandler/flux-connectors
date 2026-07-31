//! One catalogue operation, as a thing a host's [`ToolRegistry`](flux_runtime::ToolRegistry) holds.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use flux_lang::program::CompositeOpDecl;
use flux_runtime::{Tool, ToolContext, ToolResult};
use flux_spec::{
    Intent, IntentBehavior, IntentCertainty, IntentRole, IntentSet, IntentTarget, ToolSpec,
};
use serde_json::Value;

use crate::config::{Field, Snapshot};
use crate::dry_run::{DryRun, DryRunTransport, Transport};
use crate::request::{self, Request};
use crate::{auth, spec, Configuration, Credentials, Error};

/// **The connector's egress**: the tool every projected operation hands its request to.
///
/// # Why a newtype over `Arc<dyn Tool>`
///
/// This is flux's own `http.request` — `flux_web::http::HttpRequestTool` in a host that uses
/// flux-web — taken as an argument rather than constructed, so a host supplies the instance it has
/// already configured with its egress allow-list, its private-network grant and its audit sink.
///
/// It is typed as `dyn Tool` rather than as the concrete `HttpRequestTool`, and that is deliberate
/// twice over. It keeps this crate from linking `flux-web` — a whole HTTP client, a DNS resolver
/// and an SSRF guard — into a library whose entire claim is that it opens no socket, so the claim
/// stays structural rather than merely true today. And it is the seam a **non-vendor** transport
/// plugs into: a dry-run that renders the request instead of sending it, or a recorded fixture,
/// without either forking the request path.
///
/// **The named consequence:** `dyn Tool` cannot enforce that what it holds *is* `http.request`, and
/// a wrongly-wired host would send every connector's traffic somewhere else. Nothing in the type
/// system closes that, because the same openness is what the two transports above need. What this
/// wrapper buys is that the choice must be *stated* — `Egress::new(…)` at the call site rather than
/// an `Arc<dyn Tool>` coercing silently out of whatever tool was nearest.
///
/// # The contract a substitute must honour
///
/// Params are `{ url, method, headers?, body? }`, exactly [`Request::to_params`], and the result is
/// returned to the model unchanged. A stand-in that ignores `body`, or that resolves `url` against
/// some base of its own, is not a substitute — it is a different connector.
#[derive(Clone)]
pub struct Egress(Arc<dyn Tool>);

impl Egress {
    /// Declare `http` to be this pack's egress.
    pub fn new(http: Arc<dyn Tool>) -> Self {
        Self(http)
    }

    /// The tool itself.
    pub fn tool(&self) -> &Arc<dyn Tool> {
        &self.0
    }
}

/// `Arc<dyn Tool>` is not `Debug`; the tool's own name is the part worth seeing when a pack turns
/// out to have been wired to the wrong transport.
impl std::fmt::Debug for Egress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Egress").field(&self.0.spec().name).finish()
    }
}

/// A projected operation: its catalogue entry, the spec a host resolves it by, the declaration its
/// request is built from, and the transport it delegates to.
///
/// The spec and the declaration are both derived **once, at install time** rather than on each call.
/// That is not a cache — `spec()` returns a `ToolSpec` by value and cannot fail, so a projection
/// error discovered there would have nowhere to go but a panic inside a host's registration.
/// Deriving up front turns it into an error `pack` returns, which is the whole reason
/// [`ToolRegistry::try_register_all_from`](flux_runtime::ToolRegistry::try_register_all_from)
/// exists.
#[derive(Clone)]
pub struct Operation {
    /// The catalogue entry this tool was projected from — the declared hosts the network gate names
    /// come from here.
    entry: &'static catalog::Operation,
    /// The connector this operation belongs to: its authority, and the credentials it declares.
    /// Resolved once at install rather than looked up per call, for the same reason the spec is.
    provider: &'static catalog::Provider,
    /// The projected declaration, complete before the tool is ever registered.
    spec: ToolSpec,
    /// The operation's own emitted Flux, parsed. The request is **evaluated** from this rather than
    /// re-lowered from the IR, so the pack's request is the module's request by construction — see
    /// [`crate::request`].
    declaration: CompositeOpDecl,
    /// **flux's egress, not ours.** Every byte this pack sends leaves through this tool, which a
    /// host supplies pre-configured. This repository still opens no socket. See [`Egress`].
    http: Egress,
    /// **The credential port the host bound**, not a global. See [`Credentials`].
    credentials: Credentials,
    /// **This tenant's connection settings, read once** from the port the host bound — not the port
    /// itself, and deliberately so (C-198).
    ///
    /// `permission_subjects` and `execute` are two calls. Holding the [`Configuration`] would let
    /// each of them read a [`ConfigStore`](crate::ConfigStore) with interior mutability
    /// independently, and since this pack bypasses `Executor::dispatch` the first of those two reads
    /// *is* the host's egress gate — so a drifting store could gate one host and call another.
    /// Holding a [`Snapshot`] instead removes the second read rather than documenting it away.
    settings: Snapshot,
    /// The configuration variables this operation's own emitted Flux carries, derived once at
    /// install for the same reason the spec is: so a connector that cannot be configured is a
    /// diagnosable refusal at the first call rather than a brace discovered in a URL.
    endpoint_variables: Vec<String>,
}

/// Hand-written because [`CompositeOpDecl`]'s `Debug` is the whole parsed body, which buries the
/// three things a failure is actually read for: which operation, under which name, over which
/// transport.
impl std::fmt::Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Operation")
            .field("id", &self.entry.id)
            .field("name", &self.spec.name)
            .field("http", &self.http)
            .finish()
    }
}

impl Operation {
    /// Project `entry` and hold the result, delegating its egress to `http` (see [`Egress`]).
    ///
    /// # Errors
    ///
    /// Whatever [`spec::project`] refuses — an id with no dotted tool name, or an entry whose
    /// embedded Flux is not the single declaration a catalogue rendering is — plus
    /// [`Error::NoDeclaredHost`] for an entry that names no host, which is refused here so that
    /// [`Tool::permission_subjects`] can never return an empty answer, and
    /// [`Error::TenantMismatch`] for two ports bound to different tenants.
    pub fn project(
        entry: &'static catalog::Operation,
        http: Egress,
        credentials: Credentials,
        configuration: Configuration,
    ) -> Result<Self, Error> {
        // **Two ports, one tenant.** Nothing in the types stops a host from binding tenant A's
        // credentials beside tenant B's connection settings, and the result of that mistake is
        // specific and bad: one tenant's token sent to another tenant's host. Refused where it is
        // cheapest to notice, and where every other misconfiguration in this constructor is.
        if credentials.tenant() != configuration.tenant() {
            return Err(Error::TenantMismatch {
                operation: entry.id.to_owned(),
                credentials: credentials.tenant().to_owned(),
                configuration: configuration.tenant().to_owned(),
            });
        }

        // Refused at install rather than tolerated at dispatch. The gate below falls back to the
        // declared hosts when a request cannot be built, so an entry with no host would be a tool
        // whose subjects are empty exactly when they matter most — and "empty" is indistinguishable
        // from the default the trait hands out for free.
        if entry.hosts.is_empty() {
            return Err(Error::NoDeclaredHost {
                operation: entry.id.to_owned(),
            });
        }

        let provider =
            catalog::provider(catalog::ProviderKey::id(entry.provider)).ok_or_else(|| {
                Error::UnknownProvider {
                    provider: entry.provider.to_owned(),
                    available: catalog::providers().len(),
                }
            })?;

        let declaration = spec::declaration_of(entry.id, entry.flux)?;
        let spec = spec::project_declaration(entry.id, &declaration)?;
        let endpoint_variables = request::endpoint_variables(&declaration);

        // **The one read of the configuration port** (C-198). Everything this operation can ever ask
        // for is knowable here — the endpoint variables off its own emitted Flux, the Basic user
        // halves off its connector's declared credentials — so the port is consulted once and the
        // result is frozen. See [`Operation::settings`] for why holding the port instead would be a
        // hole through the host's egress gate.
        let mut fields: Vec<Field<'_>> = endpoint_variables
            .iter()
            .map(|variable| Field::Endpoint(variable.as_str()))
            .collect();
        push_user_half_fields(provider, &mut fields);
        // **Keyed by the operation's own service** (C-197). `entry.service` is what makes this one
        // service's settings rather than the connector's: `contentful-entry-get` reads `delivery`'s
        // `space_id` and `contentful-entry-create` reads `management`'s, because they are calls to
        // two different hosts against two different spaces.
        let settings = configuration.snapshot(provider.id, entry.service, fields);

        Ok(Self {
            spec,
            endpoint_variables,
            declaration,
            entry,
            provider,
            http,
            credentials,
            settings,
        })
    }

    /// The configuration variables this operation's URL carries, in stable order.
    ///
    /// Public because it is what a host needs in order to know *what to ask a tenant for* before
    /// C-87 publishes the configuration surface into the manifest. `zendesk-ticket-show` reports
    /// `["subdomain"]`; a `docusign` operation reports `["account_host", "account_id"]`; an
    /// operation on a connector with a literal base URL reports nothing.
    pub fn endpoint_variables(&self) -> &[String] {
        &self.endpoint_variables
    }

    /// Resolve every configuration variable this operation needs, or refuse.
    ///
    /// **Total or refused.** A partly-substituted base URL is not a degraded request, it is a
    /// request to a different host — the same rule [`crate::request`] already holds for its
    /// evaluator — so a single unbound variable stops the call here rather than putting a brace on
    /// the wire.
    fn endpoints(&self) -> Result<BTreeMap<String, String>, Error> {
        self.endpoint_variables
            .iter()
            .map(|variable| {
                let value = self
                    .settings
                    .require(self.entry.id, Field::Endpoint(variable))?;
                Ok((variable.clone(), value))
            })
            .collect()
    }

    /// The catalogue entry behind this tool.
    pub fn entry(&self) -> &'static catalog::Operation {
        self.entry
    }

    /// The connector this operation belongs to — its authority, and the credentials it declares.
    pub(crate) fn provider(&self) -> &'static catalog::Provider {
        self.provider
    }

    /// The dotted tool name a host resolves this operation by.
    pub(crate) fn spec_name(&self) -> &str {
        &self.spec.name
    }

    /// The transport this operation delegates to — the instance the host supplied.
    pub fn egress(&self) -> &Egress {
        &self.http
    }

    /// **What this operation would send, without sending it** (C-145).
    ///
    /// Answered by [`DryRunTransport`], which holds no client and therefore cannot reach a socket
    /// whatever it is asked to do. The credential port is not consulted: the request carries each
    /// declared credential's *reference* rather than a resolved value, so a dry run is safe to show
    /// before a token exists and answers identically over an empty store. See [`crate::dry_run`].
    ///
    /// # Errors
    ///
    /// Everything [`Operation::build_request`] refuses, plus the credential refusals that need no
    /// store — an inbound signing secret, a mechanism naming nothing, a credential the connector
    /// does not declare, and a header the operation's own module already sets. A dry run refuses
    /// rather than reporting a partly-built request, which would be a *different* call presented as
    /// the real one.
    pub fn dry_run(&self, params: &Value) -> Result<DryRun, Error> {
        DryRunTransport::new().dry_run(self, params)
    }

    /// **The request** this operation makes when called with `params`, before it is sent.
    ///
    /// Public because it is the honest thing to assert on. A test that reached for a real call
    /// would be testing flux's transport and a vendor's uptime; the two mistakes that actually ship
    /// — a body flattened out of its wire nesting, a query string missing its `?`/`&` — are visible
    /// here and nowhere else, because a vendor answers both with `200`.
    ///
    /// **No credential is applied** — this is the request the operation's own module describes, and
    /// nothing more. [`Operation::build_authenticated_request`] is the one that authenticates, and
    /// keeping the two apart is what lets [`Operation::permission_subjects`] name a URL that carries
    /// no secret.
    ///
    /// # Errors
    ///
    /// [`Error::MissingParameter`] when a declared parameter was not supplied,
    /// [`Error::Unbuildable`] when the operation's body contains something the pack does not
    /// evaluate, and [`Error::MissingConfig`] when the tenant has not supplied a value for a
    /// `{placeholder}` in the connector's base URL. All refuse rather than sending a
    /// partly-assembled call.
    pub fn build_request(&self, params: &Value) -> Result<Request, Error> {
        request::build(self.entry.id, &self.declaration, params, &self.endpoints()?)
    }

    /// **The request as it goes out**: built, then authenticated with the bound credential port.
    ///
    /// The order is the safety property, and it is the reverse of the obvious one. Every credential
    /// is resolved and **registered with `ctx.redactor` first**, before a request exists at all —
    /// `flux-web`'s `http.rs:248` is the precedent — so a failure anywhere between here and dispatch
    /// cannot surface a value the redactor has not been told about. Registering after building would
    /// leave exactly one window uncovered, and it is the window in which things go wrong.
    ///
    /// Public because it is the honest thing to assert on: a test that reached for a real call would
    /// be testing flux's transport and a vendor's uptime, while the mistakes that actually ship — a
    /// credential in the wrong half of the request, a prefix that is not there, a header the module
    /// already set — are visible here and nowhere else.
    ///
    /// # Errors
    ///
    /// Whatever [`Operation::build_request`] refuses, plus every credential refusal: no value
    /// stored, a store that could not answer, a connector with no address to look at, an inbound
    /// signing secret. **None of them sends the request**, which is the point — an unauthenticated
    /// call is a `401` that says nothing about what is missing.
    pub async fn build_authenticated_request(
        &self,
        ctx: &ToolContext,
        params: &Value,
    ) -> Result<Request, Error> {
        let credentials = self
            .credentials
            .resolve(ctx, self.entry, self.provider, &self.settings)
            .await?;

        let mut request = self.build_request(params)?;
        for credential in &credentials {
            auth::place(self.entry.id, credential, &mut request)?;
        }
        Ok(request)
    }

    /// Where this call would go, for the host's network policy to judge.
    ///
    /// The request URL when the request can be built, and the operation's **declared hosts** when it
    /// cannot. The fallback is the load-bearing half: [`Tool::permission_subjects`] returns a `Vec`
    /// and cannot fail, so without it the one call most likely to be malformed would also be the one
    /// call nobody gates. The hosts are the manifest's `http_hosts` (C-10) — declared data, read
    /// rather than re-derived by parsing a URL template a second time.
    ///
    /// **The URL here is the unauthenticated one**, deliberately. A permission subject is quoted in
    /// approval prompts, policy rules and the evidence log, and a query-placed credential would put
    /// a secret in all three — in the one place `Tool::permission_subjects` cannot fail and
    /// therefore cannot consult a redactor either. The named consequence is that a host writing an
    /// allow-list against a *full* URL sees the request without its `?api_key=…`; matching on host
    /// and path, which is what an egress policy is written against, is unaffected.
    ///
    /// # And it is the **substituted** host, on both paths (C-193)
    ///
    /// `entry.hosts` is the manifest's `http_hosts`, and for six connectors it is a template:
    /// `{subdomain}.zendesk.com`. Declaring that as the subject asks an allow-list to match a string
    /// no host resolves to, which is not a gate — it is a gate that always says no, or a rule an
    /// operator writes as a wildcard to make the connector work at all. So the fallback substitutes
    /// too, through the same port and the same values the request would have used.
    ///
    /// The fallback cannot fail, so it fills what it has and **leaves the rest verbatim**. That
    /// direction is deliberate: a subject still carrying `{subdomain}` matches nothing, so an
    /// unconfigured connector is refused by the host's own policy rather than admitted against a
    /// subject nobody can audit.
    fn subjects(&self, params: &Value) -> Vec<String> {
        match self.build_request(params) {
            Ok(request) => vec![request.url],
            Err(_) => self
                .entry
                .hosts
                .iter()
                .map(|&host| self.substituted_host(host))
                .collect(),
        }
    }

    /// A declared host with whatever configuration values the snapshot carries filled in.
    ///
    /// Best-effort by necessity — see [`Operation::subjects`] — and it reads through
    /// [`Snapshot::lookup`] rather than [`Operation::endpoints`] precisely because it must not be
    /// able to fail.
    fn substituted_host(&self, host: &str) -> String {
        let mut out = host.to_owned();
        for variable in &self.endpoint_variables {
            if let Some(value) = self.settings.lookup(Field::Endpoint(variable)) {
                out = out.replace(&format!("{{{variable}}}"), &value);
            }
        }
        out
    }
}

/// Add the [`Field::Username`] of every Basic credential `provider` declares.
///
/// The provider's whole set rather than one operation's, because a snapshot entry is cheap and the
/// alternative is a per-operation filter over `Operation::credentials` that would have to stay in
/// step with [`crate::credentials`]'s alternative-selection rule. A credential no mechanism of this
/// operation names contributes one unused entry; a credential the filter *missed* would be a field
/// the snapshot does not carry, and the honest way to answer that is a refusal rather than the live
/// store read C-198 removed.
fn push_user_half_fields(provider: &'static catalog::Provider, fields: &mut Vec<Field<'_>>) {
    for credential in provider.auth {
        if matches!(credential.acquire, catalog::Acquisition::BasicJoin { .. }) {
            fields.push(Field::Username(credential.name));
        }
    }
}

#[async_trait]
impl Tool for Operation {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    /// **The mirrored network gate**, half one.
    ///
    /// [`Tool::execute`] below delegates by calling `http.request`'s own `execute` directly, which
    /// **bypasses `Executor::dispatch`** — so `http.request`'s `permission_subjects` (flux-web's
    /// `http.rs:118`) is never consulted for the inner call. This is the same answer it would have
    /// given, declared here so that the answer is given at all.
    ///
    /// Omitting this would compile, register, execute and reach the vendor with every test still
    /// green. `tests/network_gate.rs` is what makes that not so.
    fn permission_subjects(&self, params: &Value) -> Vec<String> {
        self.subjects(params)
    }

    /// **The mirrored network gate**, half two — `http.request`'s `intents` (flux-web's
    /// `http.rs:126`), for the same reason and unreachable in the same way.
    fn intents(&self, params: &Value) -> IntentSet {
        let mut set = IntentSet::new();
        for subject in self.subjects(params) {
            set.push(Intent {
                behavior: IntentBehavior::NetworkFetch,
                target: IntentTarget::Url { url: subject },
                role: IntentRole::ReadTarget,
                certainty: IntentCertainty::Certain,
            });
        }
        set
    }

    /// Authenticate the request, build it, and hand it to flux.
    ///
    /// The **same** `ctx` travels through, so the redactor, the evidence log and the cancellation
    /// token the host bound are the ones the request is made under — and the redactor is the one the
    /// credential was registered with a moment earlier. The response comes back as `http.request`
    /// produced it — one flat string, `HTTP {status}\n{headers}\n{body}` — and is returned unshaped:
    /// it is a *result*, a 404 included, and field-selecting it needs `http.request` to return a
    /// record. That is a seam story on flux, filed rather than faked.
    ///
    /// It reads as one line because C-145 moved the two that were here behind
    /// [`Transport`](crate::Transport), unchanged. The live arm is still the only thing a registered
    /// operation dispatches through; the seam exists so that the dry run — which must run *before*
    /// a credential is resolved rather than after — is an implementation of the same interface
    /// instead of a second request path.
    async fn execute(&self, ctx: &ToolContext, params: Value) -> flux_core::Result<ToolResult> {
        self.http.carry(ctx, self, &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{empty_credentials, recording_http, test_configuration};
    use catalog::OperationKey;
    use serde_json::json;

    fn projected(id: &str) -> Operation {
        let entry = catalog::operation(OperationKey::id(id))
            .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
        Operation::project(
            entry,
            recording_http(),
            empty_credentials(),
            test_configuration(),
        )
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
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

    /// **The inverted tripwire.** C-114 asserted that `permission_subjects` and `intents` were the
    /// trait's empty defaults, and said in as many words that C-115 would have to invert it: the
    /// moment `execute` delegates to `http.request`, an empty answer stops being tolerable and
    /// becomes a hole through the host's network policy. This is that assertion, positive.
    ///
    /// **Inverted a second time by C-193.** It used to assert the subject
    /// `https://{subdomain}.zendesk.com/…`, which is what the pack genuinely declared and which no
    /// egress allow-list can match — a gate consulted with a string that names no host. The subject
    /// is now the substituted one, and `test_configuration` is where `acme` comes from.
    #[test]
    fn the_network_gate_is_mirrored_because_execute_reaches_the_network() {
        let tool = projected("zendesk-ticket-show");
        let params = json!({ "ticket_id": 1 });

        assert_eq!(
            tool.permission_subjects(&params),
            vec!["https://acme.zendesk.com/api/v2/tickets/1.json".to_string()],
            "the subject must be the URL `http.request` would have declared for itself"
        );
        assert_eq!(
            tool.intents(&params).intents,
            vec![Intent {
                behavior: IntentBehavior::NetworkFetch,
                target: IntentTarget::Url {
                    url: "https://acme.zendesk.com/api/v2/tickets/1.json".to_string(),
                },
                role: IntentRole::ReadTarget,
                certainty: IntentCertainty::Certain,
            }],
        );
    }

    /// A parameter the caller omitted is refused rather than interpolated away. Left alone,
    /// `zendesk-ticket-show` without `ticket_id` would request `/api/v2/tickets/{ticket_id}.json` —
    /// a URL the vendor answers, with a 404 that says nothing about the real mistake.
    #[test]
    fn an_omitted_parameter_is_refused_rather_than_left_in_the_url() {
        let tool = projected("zendesk-ticket-show");
        let error = tool
            .build_request(&json!({}))
            .expect_err("a missing path parameter is not a request");

        assert!(
            matches!(&error, Error::MissingParameter { parameter, .. } if parameter == "ticket_id"),
            "{error}"
        );
    }

    /// An entry with no declared host cannot be gated, so it does not install. The fallback in
    /// [`Operation::subjects`] is what makes this necessary rather than tidy: without a host to fall
    /// back to, a request that failed to build would produce an empty subject.
    #[test]
    fn an_operation_with_no_declared_host_is_refused() {
        let mut entry = *catalog::operation(OperationKey::id("zendesk-ticket-show"))
            .expect("the shipped catalogue carries zendesk-ticket-show");
        entry.hosts = &[];
        // `project` takes a `&'static` entry, which a doctored copy is not — leaking one is the
        // cheapest way to make a corrupt-catalogue case testable at all, and it is one allocation in
        // one test.
        let entry: &'static catalog::Operation = Box::leak(Box::new(entry));

        assert!(matches!(
            Operation::project(
                entry,
                recording_http(),
                empty_credentials(),
                test_configuration()
            ),
            Err(Error::NoDeclaredHost { .. })
        ));
    }
}

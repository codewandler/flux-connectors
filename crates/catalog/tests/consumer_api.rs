//! **The consumer-side compile test of C-537: the published API, exercised as a consumer would.**
//!
//! `codewandler-connector-catalog` became a shim over the embedded pack with *no breaking change
//! to its public API* — and "no breaking change" is a claim about consumers, so it is stated from
//! the consumer's side: an integration test links this crate exactly as an external dependent
//! does, and this file uses every promised piece of the surface. A field removed, a signature
//! moved, a variant renamed, or a re-export dropped stops this file *compiling*, which turns an
//! API break from a release incident into a red test. `publish_closure.rs` holds the publishing
//! half of the same line.
//!
//! Two deliberate properties of how it is written:
//!
//! - **Closed enums are matched exhaustively, with no wildcard arm.** `Placement`, `Acquisition`,
//!   `OAuthGrant`, `Subject`, `AuthHazard`, `Approval`, `Runtime` and `CredentialRequirement` are
//!   documented as closed so a consumer's exhaustive match is told when the set grows; this file
//!   *is* that consumer, so a new variant fails here first, in-repo.
//! - **Assertions are about shape, not the catalogue's contents.** What ships is
//!   `embedded_operations.rs`'s business; this file must stay green across catalogue changes or
//!   it is not testing the API.

use catalog::{
    Acquisition, Approval, AuthHazard, Channel, ChannelTransport, Choice, ConfigChoices,
    ConfigField, Credential, CredentialRequirement, Event, Idempotency, OAuth2, OAuthGrant,
    OAuthRedirect, Operation, OperationDirection, OperationKey, Pair, Placement, Provider,
    ProviderKey, Risk, Runtime, Selector, SocketConnect, Subject,
};

/// The lookups, and every field of [`Operation`] — including [`Operation::flux`], which is the
/// field the pack deliberately does not carry and the reason the generated tables remain until
/// C-540.
#[test]
fn the_lookup_surface_and_every_operation_field_are_reachable() {
    let providers: &'static [&'static Provider] = catalog::providers();
    assert!(!providers.is_empty());

    let provider: &'static Provider =
        catalog::provider(ProviderKey::id(providers[0].id)).expect("a listed provider resolves");
    let operation: &'static Operation = catalog::operations()
        .next()
        .expect("a shipped catalogue has operations");
    let by_key: Option<&'static Operation> = catalog::operation(OperationKey::id(operation.id));
    assert_eq!(by_key.map(|op| op.id), Some(operation.id));
    let of_provider: &'static [Operation] = catalog::operations_of(ProviderKey::id(provider.id));
    assert_eq!(of_provider.len(), provider.operations.len());

    // Every public field, read — a removed or retyped field stops this compiling.
    let Operation {
        id,
        provider: owner,
        service,
        direction,
        description,
        risk,
        idempotency,
        semantic_effects,
        credentials,
        credential_requirement,
        hosts,
        flux,
        ..
    } = *operation;
    let _: (&str, &str, &str, &str) = (id, owner, service, description);
    let _: OperationDirection = direction;
    let _: (&[&str], &[&[&str]], &[&str]) = (semantic_effects, credentials, hosts);
    assert!(!flux.is_empty(), "the Flux text is still published API");
    assert!(!risk.as_str().is_empty());
    assert!(!idempotency.as_str().is_empty());
    assert!(!direction.as_str().is_empty());
    assert!(!credential_requirement.as_str().is_empty());

    // The provider's own methods and fields.
    assert!(provider.operation(OperationKey::id(operation.id)).is_some() || owner != provider.id);
    let _: Option<&'static Credential> = provider.credential("no-such-credential");
    let _: Option<&'static Channel> = provider.channel("no-such-channel");
    let _: Option<&'static ConfigChoices> = provider.choices_for("default", "endpoint", "host");
    let Provider {
        id: pid,
        vendor,
        description: pdesc,
        authority,
        runtime,
        base_url,
        auth,
        operations,
        config,
        verify,
        events,
        channels,
        config_choices,
        ..
    } = *provider;
    let _: (&str, &str, &str, &str) = (pid, vendor, pdesc, base_url);
    let _: Option<&str> = authority;
    let _: Runtime = runtime;
    let _: (&[Credential], &[Operation]) = (auth, operations);
    let _: (&[ConfigField], &[Event], &[Channel]) = (config, events, channels);
    let _: (Option<&str>, &[ConfigChoices]) = (verify, config_choices);
}

/// The closed vocabularies, matched the way their documentation instructs consumers to match
/// them: exhaustively. A new variant is a compile error here before it is a surprise anywhere.
#[test]
fn the_closed_vocabularies_match_exhaustively() {
    fn place(placement: Placement) -> &'static str {
        match placement {
            Placement::Header { name: _, prefix: _ } => "header",
            Placement::Query { name: _ } => "query",
            Placement::Inbound => "inbound",
        }
    }
    fn acquire(acquisition: Acquisition) -> &'static str {
        match acquisition {
            Acquisition::Static => "static",
            Acquisition::Minted { by: _, from: _ } => "minted",
            Acquisition::BasicJoin {
                user_env: _,
                user_suffix: _,
            } => "basic",
            Acquisition::OAuth2(spec) => {
                let OAuth2 {
                    endpoint: _,
                    authorize_path: _,
                    token_path: _,
                    client_id: _,
                    scopes: _,
                    grants,
                    redirect,
                } = *spec;
                for grant in grants {
                    match grant {
                        OAuthGrant::AuthorizationCode => {}
                        OAuthGrant::Password => {}
                        OAuthGrant::RefreshToken => {}
                        OAuthGrant::ClientCredentials => {}
                    }
                }
                if let Some(OAuthRedirect { port: _, path: _ }) = redirect {}
                "oauth2"
            }
        }
    }
    fn judge(subject: Subject, hazard: Option<AuthHazard>) -> bool {
        let reviewed = match subject {
            Subject::Unstated => false,
            Subject::App | Subject::User => true,
        };
        let hazardous = match hazard {
            Some(AuthHazard::ResourceOwnerSecretShared) => true,
            None => false,
        };
        reviewed && !hazardous
    }
    fn admit(runtime: Runtime) -> bool {
        match runtime {
            Runtime::Http | Runtime::Remote => true,
            Runtime::Socket | Runtime::Process | Runtime::Container | Runtime::Plugin => false,
        }
    }
    fn transport(channel: &Channel) -> &'static str {
        match channel.transport {
            ChannelTransport::Webhook => "webhook",
            ChannelTransport::Socket => "socket",
            ChannelTransport::Poll => "poll",
        }
    }
    fn activation(approval: Approval) -> &'static str {
        match approval {
            Approval::None => approval.as_str(),
            Approval::Operator => approval.as_str(),
        }
    }
    fn why(requirement: CredentialRequirement) -> &'static str {
        match requirement {
            CredentialRequirement::Declared => requirement.as_str(),
            CredentialRequirement::NoneRequired => requirement.as_str(),
            CredentialRequirement::Withheld => requirement.as_str(),
        }
    }

    // A consumer constructs a `Credential` in a test of its own placement code — documented as
    // supported, so it is exercised here.
    let credential = Credential {
        name: "acme.token",
        leaf: "token",
        acquire: Acquisition::Static,
        place: Placement::Header {
            name: "Authorization",
            prefix: "Bearer ",
        },
        subject: Subject::default(),
        hazard: None,
    };
    assert_eq!(place(credential.place), "header");
    assert_eq!(acquire(credential.acquire), "static");
    assert!(!judge(credential.subject, credential.hazard));
    assert_eq!(
        AuthHazard::ResourceOwnerSecretShared.word(),
        "resource_owner_secret_shared"
    );

    for provider in catalog::providers() {
        assert!(!activation(Approval::default()).is_empty());
        let _ = admit(provider.runtime);
        for credential in provider.auth {
            let _ = place(credential.place);
            let _ = acquire(credential.acquire);
        }
        for channel in provider.channels {
            let _ = transport(channel);
        }
        for operation in provider.operations {
            let _ = why(operation.credential_requirement);
            match operation.direction {
                OperationDirection::Read | OperationDirection::Write => {}
            }
            match operation.risk {
                Risk::Low | Risk::Medium | Risk::High | Risk::Destructive => {}
            }
            match operation.idempotency {
                Idempotency::Idempotent | Idempotency::NonIdempotent | Idempotency::Conditional => {
                }
            }
        }
        for ConfigChoices { choices, .. } in provider.config_choices {
            for Choice { value, label } in *choices {
                let _: (&str, &str) = (value, label);
            }
        }
    }
}

/// The C-537 addition itself, from the consumer's side: the reader is re-exported as
/// `catalog::reader`, the embedded pack serves the same catalogue as the tables, and the
/// load-from-a-path constructor with its named refusals is reachable.
#[test]
fn the_reader_reexport_serves_the_same_catalogue() {
    let table_ids: Vec<&str> = catalog::providers()
        .iter()
        .map(|provider| provider.id)
        .collect();
    let pack_ids: Vec<&str> = catalog::reader::providers()
        .map(|provider| provider.id())
        .collect();
    assert_eq!(
        table_ids, pack_ids,
        "the generated tables and the embedded pack describe the same catalogue"
    );

    for operation in catalog::operations() {
        let record = catalog::reader::operation(operation.id)
            .unwrap_or_else(|| panic!("`{}` is in the tables but not the pack", operation.id));
        assert_eq!(record.provider(), operation.provider);
        assert_eq!(record.service(), operation.service);
    }

    // The path constructor and its refusal vocabulary, spelled as a consumer spells them. The
    // enum is `#[non_exhaustive]`, so the consumer's match carries a wildcard arm — that is the
    // documented contract, and this is the compile-time proof it suffices.
    match catalog::reader::Pack::load("/no/such/path/catalog.pack") {
        Ok(_) => panic!("a missing file cannot load"),
        Err(catalog::reader::Error::Io(_)) => {}
        Err(other) => panic!("a missing file is an Io refusal, got {other}"),
    }
}

/// The inbound and configuration surfaces — [`Event`], [`Channel`] with its [`SocketConnect`],
/// [`Selector`] and [`Pair`] members, and [`ConfigField`] — read field by field, with no `..`
/// rest pattern anywhere.
///
/// None of these types is `#[non_exhaustive]`, so the pin works in both directions: a removed or
/// retyped field breaks the constructions below, an added field breaks the full destructures. The
/// C-537 review found the first version of this file never read them, so a break confined to
/// these three types would have compiled straight through the test whose whole job is holding the
/// no-breaking-change line.
#[test]
fn the_inbound_and_configuration_surfaces_are_reachable() {
    fn read_event(event: Event) {
        let Event {
            name,
            service,
            description,
            wire_value,
            default,
            group,
            schema,
            declaration_json,
        } = event;
        let _: (&str, &str, &str, &str) = (name, service, description, group);
        let _: (Option<&str>, Option<&str>) = (wire_value, schema);
        let _: (bool, &str) = (default, declaration_json);
    }

    fn read_channel(channel: Channel) {
        let Channel {
            name,
            service,
            base_url,
            description,
            transport,
            events,
            connect,
            discriminator,
            delivery_id,
            payload,
            payload_root,
            declaration_json,
        } = channel;
        let _: (&str, &str, &str, &str) = (name, service, base_url, description);
        match transport {
            ChannelTransport::Webhook | ChannelTransport::Socket | ChannelTransport::Poll => {}
        }
        let _: &[&str] = events;
        if let Some(SocketConnect {
            path,
            query,
            headers,
            auth,
            subprotocols,
        }) = connect
        {
            let _: &str = path;
            for &Pair { name, value } in query.iter().chain(headers) {
                let _: (&str, &str) = (name, value);
            }
            let _: (&[&[&str]], &[&str]) = (auth, subprotocols);
        }
        for Selector { source, name } in [discriminator, delivery_id].into_iter().flatten() {
            let _: (&str, &str) = (source, name);
        }
        for &Pair { name, value } in payload {
            let _: (&str, &str) = (name, value);
        }
        let _: (bool, &str) = (payload_root, declaration_json);
    }

    fn read_config(field: ConfigField) {
        let ConfigField {
            name,
            service,
            label,
            help,
            example,
            format,
            required,
            default,
            approval,
            secret,
            docs_url,
            binds,
            also_binds,
            also_services,
            declaration_json,
        } = field;
        let _: (&str, &str, &str, &str) = (name, service, label, help);
        let _: (Option<&str>, Option<&str>, Option<&str>) = (example, default, docs_url);
        let _: (&str, &str) = (format, binds);
        let _: (&[&str], &[&str]) = (also_binds, also_services);
        let _: (bool, bool, &str) = (required, secret, declaration_json);
        assert!(!approval.as_str().is_empty());
    }

    // Constructed once each, as a consumer's own test fixture would be — the direction the full
    // destructure cannot cover.
    read_event(Event {
        name: "acme.ping",
        service: "default",
        description: "The vendor signals liveness.",
        wire_value: Some("ping"),
        default: true,
        group: "system",
        schema: None,
        declaration_json: "{}",
    });
    read_channel(Channel {
        name: "acme-events",
        service: "default",
        base_url: "https://api.acme.example",
        description: "The vendor's event socket.",
        transport: ChannelTransport::Socket,
        events: &["acme.ping"],
        connect: Some(SocketConnect {
            path: "/ws",
            query: &[Pair {
                name: "v",
                value: "1",
            }],
            headers: &[],
            auth: &[&["acme.token"]],
            subprotocols: &[],
        }),
        discriminator: Some(Selector {
            source: "payload",
            name: "type",
        }),
        delivery_id: None,
        payload: &[],
        payload_root: false,
        declaration_json: "{}",
    });
    read_config(ConfigField {
        name: "subdomain",
        service: "default",
        label: "Acme subdomain",
        help: "The part of your Acme URL before the dot.",
        example: Some("acme"),
        format: "subdomain",
        required: true,
        default: None,
        approval: Approval::None,
        secret: false,
        docs_url: None,
        binds: "endpoint.subdomain",
        also_binds: &[],
        also_services: &[],
        declaration_json: "{}",
    });

    // And the shipped catalogue's own instances, through the same readers, so the fields are
    // exercised against real data whatever the catalogue holds.
    for provider in catalog::providers() {
        for event in provider.events {
            read_event(*event);
        }
        for channel in provider.channels {
            read_channel(*channel);
        }
        for field in provider.config {
            read_config(*field);
        }
    }
}

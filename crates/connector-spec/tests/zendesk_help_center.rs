//! C-463: Zendesk Help Center is a named, spec-backed sibling of the published Support service.

use std::path::Path;

use connector_spec::credential::TenantInstances;
use connector_spec::{
    sha256_hex, HttpMethod, Idempotency, Layout, Risk, Tag, TenantLayout, DEFAULT_SERVICE,
};

#[path = "support/shipped_provider.rs"]
mod shipped_provider;

const SUPPORT_OPERATIONS: [&str; 13] = [
    "zendesk-test",
    "zendesk-ticket-search",
    "zendesk-ticket-show",
    "zendesk-ticket-comment-list",
    "zendesk-ticket-update",
    "zendesk-ticket-comment-add",
    "zendesk-ticket-tag-add",
    "zendesk-ticket-audit-list",
    "zendesk-incremental-ticket-list",
    "zendesk-incremental-user-list",
    "zendesk-incremental-organization-list",
    "zendesk-incremental-ticket-event-list",
    "zendesk-custom-object-list",
];

const HELP_CENTER_OPERATIONS: [(&str, HttpMethod, &str); 7] = [
    (
        "zendesk-help-center-category-list",
        HttpMethod::Get,
        "/api/v2/help_center/categories",
    ),
    (
        "zendesk-help-center-section-list",
        HttpMethod::Get,
        "/api/v2/help_center/sections",
    ),
    (
        "zendesk-help-center-article-list",
        HttpMethod::Get,
        "/api/v2/help_center/articles",
    ),
    (
        "zendesk-help-center-article-get",
        HttpMethod::Get,
        "/api/v2/help_center/articles/{article_id}",
    ),
    (
        "zendesk-help-center-translation-list",
        HttpMethod::Get,
        "/api/v2/help_center/articles/{article_id}/translations",
    ),
    (
        "zendesk-help-center-article-incremental-list",
        HttpMethod::Get,
        "/api/v2/help_center/incremental/articles",
    ),
    (
        "zendesk-help-center-article-create",
        HttpMethod::Post,
        "/api/v2/help_center/sections/{section_id}/articles",
    ),
];

const SUPPORT_HASHES: [(&str, &str); 13] = [
    (
        "crates/catalog/ops/zendesk/zendesk-custom-object-list.flux",
        "3dbbc14f46c868dd4258f458309d67f32549cfa284ff5e756dd50e6e5a9b5218",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-incremental-organization-list.flux",
        "c505ca40dd42432b33ff5253a5d15b7cd46415fa8bb81a77e330e0ba1938a02b",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-incremental-ticket-event-list.flux",
        "a51207b84c61a8d4d073f5757cf0e379ef41fa908167d6575a85ceb6d47ea171",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-incremental-ticket-list.flux",
        "a9bb1e84d935251f12378839916f726b6b1598b4c9e404a51125eea1778d5bfc",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-incremental-user-list.flux",
        "fdddb2e407569ffb8d370c6305f44908864ce8d6d1cb4aa6c5bfa222e019bea7",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-test.flux",
        "56686f0978ad6ea218e6808c7adbe2032b0deb6a7ed571c68db11a48c38e3faf",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-audit-list.flux",
        "753452bcac1bcb16bb03eec70c55491d779b82f849c37803e25e0c28618263c0",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-comment-add.flux",
        "659c209e8f5e50dc2a44204edc65595fb9ae83df5b6a59f743fb8d1a1cc10087",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-comment-list.flux",
        "ee341766bc0adb93909690db2dff45e0d8863486fa19ef85097ae06739468ccf",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-search.flux",
        "16af6fb77f78869db2104e76c3a3cb349a0e707a4968568de8f1b4fdc483b6e6",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-show.flux",
        "021b276e0e8253ea81449ba4c7c6ffc47b518de9c19b3d2d097ed436d12370a7",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-tag-add.flux",
        "38375cfba5205a43e8359384090d9fef76e6c1c2e68bc91d3e5b11e21e472c8d",
    ),
    (
        "crates/catalog/ops/zendesk/zendesk-ticket-update.flux",
        "99daea2b462448a66ab1a1290cc596f53b405bb80d35b05e9681281c212faf2a",
    ),
];

#[test]
fn help_center_is_a_named_service_without_moving_support() {
    let loaded = shipped_provider::load("zendesk");
    let connector = &loaded.connector;
    assert!(
        connector
            .service_names()
            .starts_with(&[DEFAULT_SERVICE, "help-center"]),
        "Support and Help Center moved when a later named sibling landed"
    );

    let support = connector.service(DEFAULT_SERVICE).expect("legacy Support");
    assert!(support.legacy);
    assert_eq!(support.tags, [Tag::Support]);
    assert_eq!(connector.verify.as_deref(), Some("zendesk-test"));
    let credential = connector
        .credential_ref_for("tenant-1", "zendesk.api_token", TenantInstances::sole())
        .expect("Zendesk keeps a valid credential address")
        .expect("Zendesk declares its authority");
    assert_eq!(
        TenantLayout.render(&credential),
        "tenants/tenant-1/com.zendesk.api/api_token"
    );

    let help_center = connector.service("help-center").expect("Help Center");
    assert_eq!(
        help_center.description,
        "Zendesk Help Center knowledge base: read categories, sections, articles and translations, and publish articles"
    );
    assert_eq!(
        help_center.base_url.as_deref(),
        Some("https://{subdomain}.zendesk.com")
    );
    assert_eq!(help_center.api_version.as_deref(), Some("v2"));
    assert_eq!(help_center.tags, [Tag::KnowledgeBase]);
    assert_eq!(
        connector.gid_of("help-center").map(|gid| gid.to_string()),
        Some("com.zendesk.api/help-center:v2".to_owned())
    );

    let support_ids: Vec<&str> = connector
        .operations_of(DEFAULT_SERVICE)
        .map(|operation| operation.id.as_str())
        .collect();
    assert!(
        support_ids.starts_with(&SUPPORT_OPERATIONS),
        "the pre-Help-Center Support operation prefix moved"
    );
    for id in SUPPORT_OPERATIONS {
        let operation = connector
            .operation(id)
            .expect("published Support operation");
        assert_eq!(operation.service, DEFAULT_SERVICE);
        assert_eq!(
            connector.oip_of(operation).map(|oip| oip.to_string()),
            Some(format!("com.zendesk.api:v2#{id}"))
        );
    }

    let help_center_ids: Vec<&str> = connector
        .operations_of("help-center")
        .map(|operation| operation.id.as_str())
        .collect();
    assert_eq!(
        help_center_ids,
        HELP_CENTER_OPERATIONS
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>()
    );
    for (id, method, path) in HELP_CENTER_OPERATIONS {
        let operation = connector
            .operation(id)
            .expect("published Help Center operation");
        assert_eq!(operation.service, "help-center");
        assert_eq!(operation.method, method);
        assert_eq!(operation.path, path);
        assert_eq!(
            connector.oip_of(operation).map(|oip| oip.to_string()),
            Some(format!("com.zendesk.api/help-center:v2#{id}"))
        );
        assert!(
            operation.response_schema.is_some(),
            "{id} lost its response schema"
        );
    }
}

#[test]
fn seven_exact_help_center_operations_are_selected_and_inexpressible_writes_stay_absent() {
    let loaded = shipped_provider::load("zendesk");
    let expected = [
        ("ListCategoriesNoLocale", &["sort_by", "sort_order"][..]),
        ("ListSectionsNoLocale", &["sort_by", "sort_order"][..]),
        (
            "ListArticlesNoLocale",
            &["sort_by", "sort_order", "label_names"][..],
        ),
        ("ShowArticleNoLocale", &[][..]),
        ("ListTranslations", &["locales", "outdated", "draft"][..]),
        (
            "ListArticlesIncremental",
            &["sort_by", "sort_order", "label_names"][..],
        ),
        ("CreateArticleBySection", &[][..]),
    ];

    let selected: Vec<_> = loaded
        .patch
        .operations
        .iter()
        .filter(|patch| patch.service.as_deref() == Some("help-center"))
        .collect();
    assert_eq!(selected.len(), expected.len());
    for (patch, (operation_id, omitted_query)) in selected.into_iter().zip(expected) {
        assert_eq!(patch.select, operation_id);
        assert_eq!(patch.omit.query, omitted_query);
    }

    let help_center = loaded
        .ingested_for("help-center")
        .expect("the pinned Help Center document");
    let update = help_center
        .ingested
        .operation("UpdateArticleNoLocale")
        .expect("the pinned document still declares UpdateArticleNoLocale");
    assert!(
        update.params.body.is_empty() && update.params.body_schema.is_none(),
        "UpdateArticleNoLocale gained the request body required to make it expressible"
    );
    for deferred in [
        "UpdateArticleNoLocale",
        "CreateTranslation",
        "UpdateTranslation",
    ] {
        assert!(
            loaded
                .patch
                .operations
                .iter()
                .all(|patch| patch.select != deferred),
            "{deferred} must remain deferred while its body is absent"
        );
    }
}

#[test]
fn help_center_queries_writes_configuration_and_verification_read_are_bounded() {
    let loaded = shipped_provider::load("zendesk");
    let connector = &loaded.connector;

    for id in [
        "zendesk-help-center-category-list",
        "zendesk-help-center-section-list",
    ] {
        let operation = connector.operation(id).expect("selected read");
        assert!(
            operation.params.is_empty(),
            "{id} must be a zero-argument verification read"
        );
        assert_eq!(operation.risk, Risk::Low);
        assert_eq!(operation.idempotency, Idempotency::Idempotent);
    }

    for id in [
        "zendesk-help-center-article-list",
        "zendesk-help-center-article-incremental-list",
    ] {
        let operation = connector.operation(id).expect("selected article list");
        assert_eq!(
            operation
                .params
                .query
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            ["start_time"]
        );
        assert_eq!(operation.params.query[0].schema["type"], "integer");
    }

    let translations = connector
        .operation("zendesk-help-center-translation-list")
        .expect("translation list");
    assert!(translations.params.query.is_empty());

    let create = connector
        .operation("zendesk-help-center-article-create")
        .expect("article create");
    assert_eq!(create.risk, Risk::High);
    assert_eq!(create.idempotency, Idempotency::NonIdempotent);
    assert_eq!(create.params.path[0].name, "section_id");
    assert_eq!(create.params.path[0].schema["type"], "integer");
    assert_eq!(
        create
            .params
            .body
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["article"]
    );
    assert!(create.params.body[0].required);

    let help_config: Vec<_> = connector
        .config_of("help-center")
        .map(|field| (field.name.as_str(), field.binds.as_str()))
        .collect();
    assert_eq!(
        help_config,
        [
            ("help_center_subdomain", "endpoint.subdomain"),
            ("help_center_email", "username.zendesk.api_token"),
            ("help_center_api_token", "credential.zendesk.api_token"),
        ]
    );
    for operation in connector.operations_of("help-center") {
        assert_eq!(
            connector.effective_auth(operation),
            connector.default_auth,
            "{} stopped reusing Support authentication",
            operation.id
        );
    }
}

#[test]
fn every_pre_help_center_support_operation_rendering_is_byte_identical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, expected) in SUPPORT_HASHES {
        let path = root.join(relative);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        assert_eq!(sha256_hex(&bytes), expected, "{} moved", path.display());
    }
}

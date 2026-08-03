//! C-508's live seam: a real TLS socket at a custom origin, behind connector-pack's `Egress` port.
//!
//! `connector-pack` is forbidden to link an HTTP client, and flux-web's production
//! `HttpRequestTool` intentionally has no test-root injection. The test transport therefore lives
//! in the host crate: it sends the pack-built request with reqwest to a loopback TLS server whose
//! one test CA is trusted explicitly. Nothing in shipped host construction accepts that CA.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use catalog::{OperationKey, ProviderKey};
use connector_pack::{
    Configuration, CredentialRef, Credentials, Egress, MemoryConfig, MemoryStore, Operation,
    Secret, SecretStore, DEFAULT_SERVICE,
};
use flux_runtime::{Tool, ToolContext};
use flux_system::{System, Workspace};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde_json::{json, Value};
use tokio_rustls::TlsAcceptor;

const TENANT: &str = "t-gitlab-live-origin";
const TOKEN: &str = "SENTINEL-NOT-A-REAL-GITLAB-TOKEN";

struct GitLabFixture {
    origin: String,
    certificate: Vec<u8>,
    paths: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for GitLabFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl GitLabFixture {
    async fn start() -> Self {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
                .expect("ephemeral localhost certificate");
        let cert = cert.der().to_vec();
        let key = signing_key.serialize_der();
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("supported TLS versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
        )
        .expect("test certificate and key agree");
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let port = listener.local_addr().expect("bound listener").port();
        let paths = Arc::new(Mutex::new(Vec::new()));
        let server_paths = Arc::clone(&paths);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let acceptor = acceptor.clone();
                let paths = Arc::clone(&server_paths);
                tokio::spawn(async move {
                    let Ok(stream) = acceptor.accept(stream).await else {
                        return;
                    };
                    let service = service_fn(move |request| respond(request, Arc::clone(&paths)));
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        Self {
            origin: format!("https://localhost:{port}"),
            certificate: cert,
            paths,
            task,
        }
    }
}

async fn respond(
    request: Request<Incoming>,
    paths: Arc<Mutex<Vec<String>>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = request.uri().path().to_owned();
    paths.lock().expect("not poisoned").push(path.clone());
    let body = if path == "/api/v4/user" {
        json!({"id": 1, "username": "fixture", "state": "active"})
    } else {
        json!([])
    };
    Ok(Response::builder()
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .expect("fixture response"))
}

fn live_egress(certificate: &[u8]) -> Egress {
    let certificate = reqwest::Certificate::from_der(certificate).expect("ephemeral test CA");
    let client = reqwest::Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("test HTTPS client");
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "test-only HTTPS transport".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        move |params: Value| {
            let client = client.clone();
            async move {
                let method = params["method"].as_str().expect("request method");
                let url = params["url"].as_str().expect("request URL");
                let mut request = client.request(method.parse().expect("HTTP method"), url);
                if let Some(headers) = params["headers"].as_object() {
                    for (name, value) in headers {
                        request = request.header(name, value.as_str().expect("header value"));
                    }
                }
                if let Some(body) = params.get("body").and_then(Value::as_str) {
                    request = request.body(body.to_owned());
                }
                let response = request.send().await.map_err(|error| error.to_string())?;
                response
                    .json::<Value>()
                    .await
                    .map_err(|error| error.to_string())
            }
        },
    ))
}

#[tokio::test]
async fn verify_and_an_ordinary_operation_reach_the_operator_pinned_https_origin() {
    let fixture = GitLabFixture::start().await;
    let store = Arc::new(MemoryStore::new());
    let reference = CredentialRef::new(TENANT, "com.gitlab.api", DEFAULT_SERVICE, "token")
        .expect("GitLab credential address");
    store
        .put(&reference, &Secret::new(TOKEN))
        .await
        .expect("fixture credential");
    let configuration = Configuration::new(
        Arc::new(MemoryConfig::new().with_approved_endpoint(
            TENANT,
            "gitlab",
            DEFAULT_SERVICE,
            "origin",
            &fixture.origin,
        )),
        TENANT,
    )
    .expect("configuration");
    let credentials = Credentials::new(store, TENANT).expect("credentials");
    let transport = live_egress(&fixture.certificate);
    let provider = catalog::provider(ProviderKey::id("gitlab")).expect("shipped GitLab provider");
    let verify_entry = provider
        .verify
        .and_then(|id| provider.operation(OperationKey::id(id)))
        .expect("GitLab's embedded verify operation");
    let project = |entry| {
        Operation::project(
            entry,
            transport.clone(),
            credentials.clone(),
            configuration.clone(),
        )
        .expect("projected operation")
    };
    let context = ToolContext::new(Arc::new(System::new(
        Workspace::new(env!("CARGO_MANIFEST_DIR")).expect("crate root"),
    )));

    let verify = project(verify_entry)
        .execute(&context, json!({}))
        .await
        .expect("declared verify call reaches the fixture");
    assert!(!verify.is_error, "verify failed: {}", verify.content);
    let ordinary = project(
        provider
            .operation(OperationKey::id("gitlab-issue-list"))
            .expect("shipped ordinary GitLab operation"),
    )
        .execute(
            &context,
            json!({"project_id": 7, "state": null, "page": null, "per_page": null}),
        )
        .await
        .expect("ordinary call reaches the fixture");
    assert!(
        !ordinary.is_error,
        "ordinary call failed: {}",
        ordinary.content
    );

    assert_eq!(
        *fixture.paths.lock().expect("not poisoned"),
        ["/api/v4/user", "/api/v4/projects/7/issues"]
    );
}

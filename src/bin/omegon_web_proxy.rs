#[cfg(target_arch = "wasm32")]
fn main() {
    panic!("omegon_web_proxy is a native-only development helper");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use axum::{
        body::Bytes,
        extract::{
            ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
            Path as AxumPath,
            State,
        },
        http::{HeaderMap, Method, StatusCode},
        response::{IntoResponse, Response},
        routing::any,
        Router,
    };
    use futures_util::{SinkExt, StreamExt};
    use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
    use serde::{Deserialize, Serialize};
    use std::{
        fs,
        net::SocketAddr,
        path::{Path, PathBuf},
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::sync::Mutex;
    use tokio_tungstenite::{connect_async, tungstenite::Message as UpstreamWsMessage};

    const DEFAULT_BIND: &str = "127.0.0.1:9311";
    const DEFAULT_OMEGON_BASE: &str = "http://127.0.0.1:8080";

    const STRIPPED_INBOUND_HEADERS: &[&str] = &[
        "authorization",
        "omegon-principal-issuer",
        "omegon-principal-subject",
        "omegon-principal-role",
        "omegon-principal-display-name",
        "omegon-principal-session-id",
        "omegon-principal-client-id",
        "omegon-back-url",
        "auspex-proxy-identity-fingerprint",
        "host",
        "connection",
        "upgrade",
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-protocol",
        "sec-websocket-extensions",
    ];

    #[derive(Clone)]
    struct ProxyState {
        client: reqwest::Client,
        omegon_base: String,
        token: Arc<Mutex<Option<String>>>,
        identity: Option<AuthorityIdentity>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AuthorityIdentity {
        schema_version: u8,
        subject: String,
        fingerprint: String,
        created_at_unix: u64,
        strict_daemon_identity: bool,
    }

    impl AuthorityIdentity {
        fn create() -> Self {
            let id = uuid::Uuid::new_v4().to_string();
            let fingerprint = uuid::Uuid::new_v4().simple().to_string();
            Self {
                schema_version: 1,
                subject: format!("styrene:local-operator:{id}"),
                fingerprint,
                created_at_unix: now_unix(),
                strict_daemon_identity: false,
            }
        }
    }

    #[derive(Debug, Deserialize)]
    struct StartupResponse {
        token: Option<String>,
    }

    #[derive(Debug, Serialize)]
    struct ProxyStatusResponse {
        schema_version: u8,
        mode: &'static str,
        browser_tls: BrowserTlsStatus,
        daemon: DaemonProxyStatus,
        identity: IdentityProxyStatus,
        websocket: WebSocketProxyStatus,
    }

    #[derive(Debug, Serialize)]
    struct BrowserTlsStatus {
        enabled: bool,
        trusted_local_ca: bool,
    }

    #[derive(Debug, Serialize)]
    struct DaemonProxyStatus {
        base_url: String,
        reachable: bool,
        token_cached: bool,
    }

    #[derive(Debug, Serialize)]
    struct IdentityProxyStatus {
        configured: bool,
        subject: Option<String>,
        fingerprint: Option<String>,
        strict_daemon_identity: bool,
    }

    #[derive(Debug, Serialize)]
    struct WebSocketProxyStatus {
        surface_stream_proxy: bool,
    }

    #[tokio::main]
    pub async fn main() -> anyhow::Result<()> {
        let identity_path = identity_path()?;
        if std::env::args().any(|arg| arg == "--init-identity") {
            return init_identity(&identity_path);
        }
        let bind =
            std::env::var("AUSPEX_WEB_PROXY_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
        let omegon_base = std::env::var("AUSPEX_OMEGON_BASE")
            .unwrap_or_else(|_| DEFAULT_OMEGON_BASE.to_string())
            .trim_end_matches('/')
            .to_string();
        let identity = load_identity(&identity_path)?;
        let state = ProxyState {
            client: reqwest::Client::new(),
            omegon_base,
            token: Arc::new(Mutex::new(None)),
            identity,
        };
        let app = Router::new()
            .route("/_auspex/proxy/status", axum::routing::get(proxy_status))
            .route("/api/*path", any(proxy_api))
            .with_state(state);
        let addr: SocketAddr = bind.parse()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        eprintln!("auspex web proxy listening on http://{addr}");
        axum::serve(listener, app).await?;
        Ok(())
    }

    fn identity_path() -> anyhow::Result<PathBuf> {
        let path = std::env::var("AUSPEX_WEB_PROXY_IDENTITY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".auspex/identity/web-proxy.json"));
        Ok(path)
    }

    fn init_identity(path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let identity = AuthorityIdentity::create();
        let json = serde_json::to_string_pretty(&identity)?;
        fs::write(path, format!("{json}\n"))?;
        eprintln!("initialized Auspex web proxy identity: {}", path.display());
        eprintln!("subject: {}", identity.subject);
        eprintln!("fingerprint: {}", identity.fingerprint);
        Ok(())
    }

    fn load_identity(path: &Path) -> anyhow::Result<Option<AuthorityIdentity>> {
        match fs::read_to_string(path) {
            Ok(raw) => Ok(Some(serde_json::from_str(&raw)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default()
    }

    async fn proxy_status(State(state): State<ProxyState>) -> axum::Json<ProxyStatusResponse> {
        let startup_url = format!("{}/api/startup", state.omegon_base);
        let reachable = state.client.get(startup_url).send().await.is_ok();
        let token_cached = state.token.lock().await.is_some();
        let strict_daemon_identity = if reachable {
            daemon_requires_proxy_identity(&state).await.unwrap_or(false)
        } else {
            false
        };
        axum::Json(ProxyStatusResponse {
            schema_version: 1,
            mode: "proxy-mediated",
            browser_tls: BrowserTlsStatus {
                enabled: false,
                trusted_local_ca: false,
            },
            daemon: DaemonProxyStatus {
                base_url: state.omegon_base.clone(),
                reachable,
                token_cached,
            },
            identity: IdentityProxyStatus {
                configured: state.identity.is_some(),
                subject: state.identity.as_ref().map(|identity| identity.subject.clone()),
                fingerprint: state
                    .identity
                    .as_ref()
                    .map(|identity| identity.fingerprint.clone()),
                strict_daemon_identity,
            },
            websocket: WebSocketProxyStatus {
                surface_stream_proxy: true,
            },
        })
    }

    async fn daemon_requires_proxy_identity(state: &ProxyState) -> Result<bool, String> {
        let Some(token) = current_token(state).await? else {
            return Ok(false);
        };
        let probe_url = format!("{}/api/sessions/default/surfaces", state.omegon_base);
        let response = state
            .client
            .get(probe_url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .map_err(|error| format!("strict identity probe failed: {error}"))?;
        Ok(matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ))
    }

    async fn proxy_api(
        State(state): State<ProxyState>,
        method: Method,
        AxumPath(path): AxumPath<String>,
        headers: HeaderMap,
        ws: Option<WebSocketUpgrade>,
        body: Bytes,
    ) -> Response {
        if let Some(ws) = ws {
            return proxy_websocket(ws, state, path).await;
        }
        let upstream = format!("{}/api/{}", state.omegon_base, path);
        match forward_once(&state, &method, &upstream, &headers, body.clone(), true).await {
            Ok(response) if response.status() == StatusCode::UNAUTHORIZED => {
                let mut token = state.token.lock().await;
                *token = None;
                drop(token);
                match forward_once(&state, &method, &upstream, &headers, body, true).await {
                    Ok(response) => response,
                    Err(error) => proxy_error(error),
                }
            }
            Ok(response) => response,
            Err(error) => proxy_error(error),
        }
    }

    async fn proxy_websocket(
        ws: WebSocketUpgrade,
        state: ProxyState,
        path: String,
    ) -> Response {
        match current_token(&state).await {
            Ok(Some(token)) => {
                let ws_base = state
                    .omegon_base
                    .strip_prefix("https://")
                    .map(|rest| format!("wss://{rest}"))
                    .or_else(|| {
                        state
                            .omegon_base
                            .strip_prefix("http://")
                            .map(|rest| format!("ws://{rest}"))
                    })
                    .unwrap_or_else(|| state.omegon_base.clone());
                let separator = if path.contains('?') { '&' } else { '?' };
                let upstream = format!("{ws_base}/api/{path}{separator}token={token}");
                ws.on_upgrade(move |socket| bridge_websocket(socket, upstream))
            }
            Ok(None) => proxy_error("startup discovery returned no token".to_string()),
            Err(error) => proxy_error(error),
        }
    }

    async fn bridge_websocket(socket: WebSocket, upstream_url: String) {
        let Ok((upstream, _response)) = connect_async(&upstream_url).await else {
            return;
        };
        let (mut browser_tx, mut browser_rx) = socket.split();
        let (mut upstream_tx, mut upstream_rx) = upstream.split();

        let browser_to_upstream = async {
            while let Some(Ok(message)) = browser_rx.next().await {
                let mapped = match message {
                    AxumWsMessage::Text(text) => UpstreamWsMessage::Text(text.into()),
                    AxumWsMessage::Binary(bytes) => UpstreamWsMessage::Binary(bytes.into()),
                    AxumWsMessage::Ping(bytes) => UpstreamWsMessage::Ping(bytes.into()),
                    AxumWsMessage::Pong(bytes) => UpstreamWsMessage::Pong(bytes.into()),
                    AxumWsMessage::Close(frame) => {
                        let mapped = frame.map(|frame| {
                            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                code: frame.code.into(),
                                reason: frame.reason.to_string().into(),
                            }
                        });
                        let _ = upstream_tx.send(UpstreamWsMessage::Close(mapped)).await;
                        break;
                    }
                };
                if upstream_tx.send(mapped).await.is_err() {
                    break;
                }
            }
        };

        let upstream_to_browser = async {
            while let Some(Ok(message)) = upstream_rx.next().await {
                let mapped = match message {
                    UpstreamWsMessage::Text(text) => AxumWsMessage::Text(text.to_string()),
                    UpstreamWsMessage::Binary(bytes) => AxumWsMessage::Binary(bytes.to_vec()),
                    UpstreamWsMessage::Ping(bytes) => AxumWsMessage::Ping(bytes.to_vec()),
                    UpstreamWsMessage::Pong(bytes) => AxumWsMessage::Pong(bytes.to_vec()),
                    UpstreamWsMessage::Close(frame) => {
                        let mapped = frame.map(|frame| axum::extract::ws::CloseFrame {
                            code: frame.code.into(),
                            reason: frame.reason.to_string().into(),
                        });
                        let _ = browser_tx.send(AxumWsMessage::Close(mapped)).await;
                        break;
                    }
                    UpstreamWsMessage::Frame(_) => continue,
                };
                if browser_tx.send(mapped).await.is_err() {
                    break;
                }
            }
        };

        tokio::select! {
            _ = browser_to_upstream => {},
            _ = upstream_to_browser => {},
        }
    }

    async fn forward_once(
        state: &ProxyState,
        method: &Method,
        upstream: &str,
        headers: &HeaderMap,
        body: Bytes,
        inject_authority: bool,
    ) -> Result<Response, String> {
        let mut request = state.client.request(method.clone(), upstream);
        for (name, value) in headers {
            if should_forward_header(name.as_str()) {
                request = request.header(name, value);
            }
        }
        if inject_authority {
            if let Some(token) = current_token(state).await? {
                request = request.header(AUTHORIZATION, format!("Bearer {token}"));
            }
            let principal_subject = state
                .identity
                .as_ref()
                .map(|identity| identity.subject.as_str())
                .unwrap_or("local-operator");
            let identity_fingerprint = state
                .identity
                .as_ref()
                .map(|identity| identity.fingerprint.as_str());
            request = request
                .header("Omegon-Principal-Issuer", "auspex")
                .header("Omegon-Principal-Subject", principal_subject)
                .header("Omegon-Principal-Role", "operator")
                .header("Omegon-Principal-Client-Id", "auspex-web")
                .header("Omegon-Back-Url", "http://127.0.0.1:9310/");
            if let Some(fingerprint) = identity_fingerprint {
                request = request.header("Auspex-Proxy-Identity-Fingerprint", fingerprint);
            }
        }
        if !body.is_empty() {
            request = request.body(body);
        }
        let upstream_response = request
            .send()
            .await
            .map_err(|error| format!("upstream request failed: {error}"))?;
        let status = upstream_response.status();
        let headers = upstream_response.headers().clone();
        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|error| format!("upstream body failed: {error}"))?;
        let mut response = (status, bytes).into_response();
        for (name, value) in headers.iter() {
            if should_return_header(name.as_str()) {
                response.headers_mut().insert(name.clone(), value.clone());
            }
        }
        Ok(response)
    }

    async fn current_token(state: &ProxyState) -> Result<Option<String>, String> {
        if let Some(token) = state.token.lock().await.clone() {
            return Ok(Some(token));
        }
        let startup_url = format!("{}/api/startup", state.omegon_base);
        let startup = state
            .client
            .get(startup_url)
            .send()
            .await
            .map_err(|error| format!("startup discovery failed: {error}"))?
            .json::<StartupResponse>()
            .await
            .map_err(|error| format!("startup decode failed: {error}"))?;
        let mut slot = state.token.lock().await;
        *slot = startup.token.clone();
        Ok(startup.token)
    }

    fn should_forward_header(name: &str) -> bool {
        !STRIPPED_INBOUND_HEADERS
            .iter()
            .any(|stripped| name.eq_ignore_ascii_case(stripped))
    }

    fn should_return_header(name: &str) -> bool {
        !matches!(
            name.to_ascii_lowercase().as_str(),
            "connection" | "transfer-encoding" | "content-length"
        )
    }

    fn proxy_error(error: String) -> Response {
        let mut response = (StatusCode::BAD_GATEWAY, error).into_response();
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    native::main()
}

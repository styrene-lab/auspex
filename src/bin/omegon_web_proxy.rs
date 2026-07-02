#[cfg(target_arch = "wasm32")]
fn main() {
    panic!("omegon_web_proxy is a native-only development helper");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use axum::{
        body::Bytes,
        extract::{Path, State},
        http::{HeaderMap, Method, StatusCode},
        response::{IntoResponse, Response},
        routing::any,
        Router,
    };
    use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
    use serde::Deserialize;
    use std::{net::SocketAddr, sync::Arc};
    use tokio::sync::Mutex;

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
    }

    #[derive(Debug, Deserialize)]
    struct StartupResponse {
        token: Option<String>,
    }

    #[tokio::main]
    pub async fn main() -> anyhow::Result<()> {
        let bind =
            std::env::var("AUSPEX_WEB_PROXY_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
        let omegon_base = std::env::var("AUSPEX_OMEGON_BASE")
            .unwrap_or_else(|_| DEFAULT_OMEGON_BASE.to_string())
            .trim_end_matches('/')
            .to_string();
        let state = ProxyState {
            client: reqwest::Client::new(),
            omegon_base,
            token: Arc::new(Mutex::new(None)),
        };
        let app = Router::new()
            .route("/api/*path", any(proxy_api))
            .with_state(state);
        let addr: SocketAddr = bind.parse()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        eprintln!("auspex web proxy listening on http://{addr}");
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn proxy_api(
        State(state): State<ProxyState>,
        method: Method,
        Path(path): Path<String>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
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
            request = request
                .header("Omegon-Principal-Issuer", "auspex")
                .header("Omegon-Principal-Subject", "local-operator")
                .header("Omegon-Principal-Role", "operator")
                .header("Omegon-Principal-Client-Id", "auspex-web")
                .header("Omegon-Back-Url", "http://127.0.0.1:9310/");
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

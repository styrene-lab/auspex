# Omegon Web Proxy Tech Stack

## Purpose

The Omegon web proxy is the local authority boundary between a browser UI and a local Omegon daemon. Its job is not to become a general auth service; it is a single-operator bridge that makes browser access practical without leaking daemon bearer tokens or trusting browser-supplied identity headers.

The accepted architecture is proxy-mediated local identity:

```text
Browser UI
  │
  │ HTTP(S) + WebSocket to stable Auspex origin
  ▼
Auspex local web proxy
  │
  │ daemon bearer + trusted principal headers + local identity proof
  ▼
Omegon daemon 0.27+
```

Terminology note: do not overfit this design to the term `mTLS`. The identity proof may use mTLS, a signed Styrene identity assertion, IPC-paired local credentials, or a combination. The invariant is that the browser never gets to assert daemon authority directly; the proxy does.

## Goals

1. Stable browser URL: no rotating `?token=` copy/paste.
2. Browser never sees the daemon bearer in normal proxy mode.
3. Proxy proves single-operator/local authority to Omegon.
4. Proxy strips spoofable browser auth/principal headers before injecting its own.
5. HTTP and WebSocket surface traffic work through the same authority boundary.
6. First-run setup is explicit, inspectable, and resettable.
7. Local development remains low-friction with an insecure loopback mode.

## Non-goals

- Multi-user web auth.
- Internet-exposed OAuth/OIDC session management.
- A full certificate-management product.
- Kubernetes deployed WebUI control-plane identity. That is covered by deployed WebUI/operator TLS and Kubernetes identity docs.
- Browser client certificate UX as the primary path. Browser client cert enrollment is a poor MVP operator experience.

## Current Implemented Slice

As of the local MVP:

- `src/bin/omegon_web_proxy.rs` runs a native local proxy on `127.0.0.1:9311`.
- `web/Trunk.toml` proxies browser `/api/` calls to `127.0.0.1:9311/api/`.
- The proxy discovers the current Omegon daemon bearer from `GET /api/startup`.
- The proxy injects `Authorization: Bearer <token>` upstream.
- The proxy injects trusted local principal headers upstream.
- The proxy strips browser-supplied auth/principal headers.
- The proxy bridges WebSocket surface streams without exposing the token to browser code.
- The browser can open `http://127.0.0.1:9310/` without a token query parameter.

This is a correct shape for the MVP, but its identity proof is still only “trusted loopback process plus daemon bearer discovery.” The next design step is to replace implicit loopback trust with explicit first-run local identity material.

## Ownership and Dependency Boundaries

The proxy stack has three separately owned layers. Keep these boundaries hard; do not collapse them for convenience.

```text
Auspex Web UI  ──depends on──► Auspex Web Proxy API ──depends on──► Omegon Web API
      │                                │                              │
      │                                └──depends on──► Styrene identity primitives
      │
      └── must not depend on daemon bearer, daemon internals, or Styrene private keys
```

### Ownership Matrix

| Layer | Code owner | Owns | May depend on | Must not depend on |
|---|---|---|---|---|
| Browser UI | Auspex web bundle | Rendering, composer UX, surface/action client calls, proxy-status display | Stable HTTP/WS API paths, public JSON contracts, proxy status schema | Daemon bearer, private identity material, Omegon internal Rust types, filesystem trust store |
| Auspex web proxy | Auspex native helper | Browser authority boundary, bearer discovery/cache, trusted principal injection, WS bridging, proxy diagnostics | Omegon HTTP/WS contracts, Styrene/local identity primitives, TLS/runtime networking crates | UI component code, Omegon in-process runtime internals, provider/model execution code |
| Omegon daemon | Omegon runtime | Source-of-truth session state, action execution, daemon bearer issuance, strict local authority validation | Its own web contracts, Styrene identity verification primitives | Auspex UI implementation details, Trunk dev-server behavior, browser storage conventions |
| Styrene identity | Styrene identity library/tooling | Local identity keys, signing/verification, fingerprints, trust-material format | Cryptographic primitives and filesystem/keychain storage | Auspex UI state, Omegon session/action logic |
| Dev server / Trunk | Development tooling | WASM serving and dev-time proxy routing | Static bundle and configured proxy target | Auth, identity, principal injection, token handling |

### Dependency Direction Rules

1. Browser UI depends on **contracts**, not implementations.
   - Allowed: `/api/sessions/default/surfaces`, `/api/sessions/default/actions`, `/api/sessions/default/surfaces/stream`, `/_auspex/proxy/status`.
   - Forbidden: reading daemon tokens, local trust files, private keys, or importing native proxy modules.

2. Auspex proxy depends on Omegon **wire contracts**, not Omegon runtime internals.
   - Allowed: `GET /api/startup`, session/action/surface/stream routes, documented strict-authority attach route once added.
   - Forbidden: linking against Omegon daemon crates to call internal session state directly.

3. Omegon daemon validates authority; it does not know Auspex UI layout.
   - Allowed: validating a proxy identity proof and mapping it to a principal.
   - Forbidden: special-casing Auspex component names, CSS state, Trunk routes, or browser-local conventions.

4. Styrene identity stays below both Auspex proxy and Omegon daemon.
   - The proxy may sign with local identity material.
   - The daemon may verify identity material.
   - Neither side should reimplement signing/verification ad hoc.

5. Trunk is not an authority component.
   - It may forward `/api/` during development.
   - It must not inject principal headers, discover daemon tokens, sign identity assertions, or own trust state.

### Boundary Contracts

#### Browser UI ↔ Auspex Proxy

Owned by Auspex. Browser-facing contract should stay small and stable:

- `GET /api/...` and `POST /api/...` are forwarded daemon-compatible routes.
- `WS /api/sessions/default/surfaces/stream` is forwarded stream transport.
- `GET /_auspex/proxy/status` reports proxy posture.

Browser-to-proxy auth is intentionally local and simple in MVP. When browser-facing HTTPS is enabled, that is a transport property of the proxy; the browser still does not become the daemon identity principal.

#### Auspex Proxy ↔ Omegon Daemon

Joint Auspex/Omegon contract. This is the actual authority boundary:

- Proxy forwards daemon bearer or successor attach credential.
- Proxy injects `Omegon-Principal-*` only after stripping browser-supplied copies.
- Proxy attaches future `Auspex-Identity-*` / Styrene proof.
- Daemon validates bearer today; strict mode validates proxy identity before trusting principal headers.

This contract should be documented as a wire protocol, not hidden in proxy code.

#### Proxy ↔ Styrene Identity

Owned by Styrene identity primitives. The proxy should consume a narrow API:

```rust
trait LocalIdentitySigner {
    fn subject(&self) -> &str;
    fn fingerprint(&self) -> &str;
    fn sign_request(&self, request: CanonicalProxyRequest<'_>) -> SignatureEnvelope;
}
```

The daemon should consume the verifier side:

```rust
trait LocalIdentityVerifier {
    fn verify_request(&self, request: CanonicalProxyRequest<'_>, signature: &SignatureEnvelope)
        -> Result<VerifiedPrincipal, IdentityError>;
}
```

Do not let proxy code own crypto policy beyond selecting the configured identity.

### Code Placement Rules

Current short-term placement:

```text
src/bin/omegon_web_proxy.rs          # thin executable / temporary MVP shell
src/omegon_web_contract.rs           # browser-side daemon/proxy wire DTOs
src/omegon_web_mock.rs               # UI rendering and browser interactions
web/Trunk.toml                       # dev proxy routing only
```

Near-term extraction target:

```text
crates/auspex-web-proxy/
  src/lib.rs                         # proxy router assembly
  src/http.rs                        # HTTP forwarding
  src/ws.rs                          # WebSocket bridge
  src/authority.rs                   # bearer cache + principal injection
  src/status.rs                      # /_auspex/proxy/status DTO
  src/identity.rs                    # adapter to Styrene identity signer
  src/config.rs                      # bind/upstream/trust config
  src/bin/auspex-web-proxy.rs        # CLI wrapper

crates/auspex-web-contract/
  src/proxy_status.rs                # browser-visible proxy DTOs
  src/omegon_surface.rs              # daemon surface DTOs if they remain Auspex-owned
```

Extraction trigger: once the proxy has status, trust-store loading, or identity signing. Keeping all of that in `src/bin/omegon_web_proxy.rs` would blur ownership and make tests harder.

### Testing Ownership

- Browser UI tests assert rendering behavior and request construction only.
- Proxy tests assert header stripping/injection, token refresh, WS bridging, and status output.
- Omegon daemon tests assert authority validation and rejection of spoofed principal headers.
- Styrene identity tests assert signing/verification and key/trust-file handling.

No test should need to spin up all layers unless it is explicitly an integration test.

## Components

### Browser UI

Owner: Auspex web bundle.

Responsibilities:

- Render Omegon surfaces.
- Submit prompts and slash commands to `/api/sessions/default/actions`.
- Subscribe to `/api/sessions/default/surfaces/stream`.
- Display proxy/auth posture surfaced by diagnostics.

Must not:

- Store the daemon bearer.
- Inject `Omegon-Principal-*` headers as a source of truth.
- Treat `GET /api/startup` token as the normal auth path in proxy mode.

Direct-token mode may exist for raw daemon development, but should be visibly marked as degraded/direct.

### Auspex Local Web Proxy

Owner: Auspex native helper.

Responsibilities:

- Serve as the browser-facing local authority.
- Discover or receive daemon authority material.
- Strip inbound spoofable authority headers:
  - `Authorization`
  - `Omegon-Principal-*`
  - `Omegon-Back-Url`
  - WebSocket hop-by-hop headers when proxying HTTP.
- Inject upstream authority:
  - daemon bearer or successor daemon-local credential
  - trusted principal headers
  - back URL
  - future local identity proof header/signature when not using TLS client auth
- Bridge WebSocket upgrades.
- Expose a local status endpoint for UI diagnostics.

The proxy is the correct place to absorb token rotation. Browser code should never need to know that a token rotated.

### Omegon Daemon

Owner: Omegon runtime.

Responsibilities:

- Expose browser-compatible surfaces/actions/streams.
- Continue supporting bearer bootstrap for direct local development.
- Add a strict local-web-authority mode that trusts principal headers only when accompanied by a configured local proxy identity proof.
- In strict mode, do not expose daemon bearer to unauthenticated browser-readable startup routes.

### Local Identity Store

Owner: Auspex, with Styrene identity semantics.

A first-run setup creates inspectable local material under a project-local or user-local directory. Proposed default:

```text
.auspex/web-identity/
  trust.json
  local-authority.json
  proxy.identity.json
  proxy.identity.key
  daemon.peer.json
```

If TLS certificates are used:

```text
.auspex/web-identity/
  local-ca.pem
  local-ca.key
  proxy-cert.pem
  proxy-cert.key
  daemon-client-cert.pem
  daemon-client-key.pem
```

The exact file names can change, but the store needs three conceptual records:

1. local authority root
2. proxy identity
3. daemon peer/trust binding

## First-Run Setup Flow

Command proposal:

```bash
auspex web trust init
```

Behavior:

1. Detect existing trust store.
2. If absent, create local authority root and proxy identity.
3. Register an Omegon daemon peer binding for the current workspace.
4. Write `trust.json` with fingerprints and creation metadata.
5. Print status and next command.

Example `trust.json`:

```json
{
  "schema_version": 1,
  "mode": "single_operator_local",
  "identity": {
    "kind": "styrene-local-operator",
    "subject": "local-operator",
    "machine_id": "...",
    "fingerprint": "..."
  },
  "authority": {
    "fingerprint": "...",
    "created_at": "2026-07-02T00:00:00Z"
  },
  "proxy": {
    "fingerprint": "...",
    "bind": "127.0.0.1:9311"
  },
  "daemon": {
    "base_url": "http://127.0.0.1:8080",
    "strict_identity": false
  }
}
```

Status command:

```bash
auspex web trust status
```

Reset command:

```bash
auspex web trust reset
```

Reset must be explicit and destructive; it invalidates the local authority material.

## Identity Proof Options

### Option A — TLS server cert to browser, signed assertion to daemon

Browser side:

- Proxy serves HTTPS with a local CA or self-signed cert.
- Browser trust-store enrollment is optional in early MVP; loopback HTTP remains the low-friction mode.

Daemon side:

- Proxy sends an assertion header:

```text
Auspex-Identity: styrene:<subject>
Auspex-Identity-Timestamp: <unix-ms>
Auspex-Identity-Signature: <base64 signature over method/path/body-hash/timestamp>
```

Daemon validates the signature against configured local trust.

Pros:

- Avoids browser client cert UX.
- Easy to debug with headers.
- Works over HTTP loopback and HTTPS.
- Good fit for Styrene identity semantics.

Cons:

- Need replay protection and request canonicalization.
- Body hashing must be precise for streamed requests; initial MVP can sign method/path/timestamp for control-plane routes and rely on loopback for body integrity.

### Option B — mTLS proxy to daemon

Browser side:

- Same as Option A.

Daemon side:

- Daemon serves HTTPS/WSS on loopback.
- Proxy presents client cert.
- Daemon maps cert SAN/fingerprint to local operator identity.

Pros:

- Standard transport-layer identity.
- Avoids custom signature verification.

Cons:

- More certificate plumbing.
- Harder hot-reload of local cert material.
- Potentially invasive to Omegon daemon server setup.

### Option C — IPC pairing token between proxy and daemon

Browser side:

- Same as Option A.

Daemon side:

- Proxy proves identity via local IPC socket pairing or a daemon-generated pair secret stored in the trust file.

Pros:

- Very simple for localhost.
- Avoids cert handling.

Cons:

- Less portable across process boundaries.
- Easier to accidentally reduce to “just another bearer token.”
- Weak story if the proxy ever needs non-loopback operation.

## Recommended MVP Choice

Use Option A first: HTTPS-capable proxy plus signed Styrene identity assertion to the daemon.

Rationale:

- It preserves the accepted proxy boundary.
- It avoids browser client certificate UX.
- It can start with HTTP loopback and upgrade to HTTPS without changing browser API paths.
- It gives Omegon a daemon-verifiable identity proof before investing in transport TLS changes.
- It composes with mTLS later if desired.

MVP daemon validation can be scoped to local-web routes:

- `/api/sessions/*`
- `/api/events*`
- `/api/web/*`
- `/ws` if used from proxy

## Proxy Runtime Stack

Current stack:

- Rust native binary.
- `axum` for HTTP server and WebSocket upgrade handling.
- `reqwest` for upstream HTTP to Omegon.
- `tokio-tungstenite` for upstream WebSocket bridge.
- `tokio` runtime.
- Trunk dev proxy forwards browser `/api/` to this proxy.

Near-term additions:

- `auspex-web-identity` module for trust file load/generate/status.
- Signature helper using existing Styrene identity primitives if available; otherwise `ed25519-dalek` or the project’s canonical signing crate.
- Optional TLS listener:
  - `rustls`
  - `tokio-rustls`
  - browser-facing cert from local authority store

Avoid adding a database. The trust store is small structured files with private-key permissions.

## Request Flow

### HTTP snapshot/action flow

```text
Browser → GET /api/sessions/default/surfaces
Proxy:
  strip browser auth/principal headers
  load current daemon token
  add Authorization: Bearer <token>
  add Omegon-Principal-* headers
  add Auspex-Identity-* proof (future)
  forward to Omegon
Omegon:
  validate bearer today
  validate proxy identity in strict mode later
  return response
Proxy → Browser
```

### WebSocket surface stream flow

```text
Browser → WS /api/sessions/default/surfaces/stream
Proxy:
  accept browser WS upgrade
  discover daemon token
  open upstream WS /api/sessions/default/surfaces/stream?token=<token>
  add future identity proof via query/header depending on daemon support
  bridge frames bidirectionally
```

For WebSocket identity proof, the clean choices are:

- query parameter carrying a signed short-lived attach assertion, or
- upstream request headers if the WS client library and daemon accept them.

Do not pass long-lived private material to browser JavaScript.

## Proxy Status API

Add a local-only proxy status route outside the daemon API namespace:

```text
GET /_auspex/proxy/status
```

Response:

```json
{
  "schema_version": 1,
  "mode": "proxy-mediated",
  "browser_tls": {
    "enabled": false,
    "trusted_local_ca": false
  },
  "daemon": {
    "base_url": "http://127.0.0.1:8080",
    "reachable": true,
    "token_cached": true,
    "last_token_refresh_at": "..."
  },
  "identity": {
    "configured": false,
    "subject": null,
    "fingerprint": null,
    "strict_daemon_identity": false
  },
  "websocket": {
    "surface_stream_proxy": true
  }
}
```

The web UI should render this in the BACKEND card instead of guessing from URL tokens.

## Strict Mode

Strict mode means:

- Browser-supplied bearer/principal headers are ignored by proxy.
- Omegon ignores principal headers unless identity proof validates.
- Omegon does not expose daemon bearer to browser-readable startup routes.
- Proxy token acquisition uses one of:
  - daemon IPC
  - signed local identity attach
  - mTLS-protected startup/attach route

Proposed flags:

```bash
omegon serve --web-authority .auspex/web-identity/trust.json --require-web-authority
cargo run --bin omegon_web_proxy -- --trust .auspex/web-identity/trust.json
```

The exact CLI shape can change; the lifecycle needs `init`, `status`, `reset`, and `serve/dev` integration.

## Threat Model

### Defended

- Browser JS cannot steal daemon bearer in proxy mode.
- Browser JS cannot spoof principal headers because proxy strips them.
- Token rotation is absorbed by proxy.
- Accidental direct URL/token copy/paste is eliminated from normal workflow.
- Future strict mode prevents arbitrary localhost clients from asserting identity headers.

### Not fully defended until strict identity mode

- A malicious local process may call Omegon directly if it can obtain the bearer from public `/api/startup`.
- A malicious local process may impersonate the proxy if daemon trusts headers without identity proof.
- Browser traffic is not encrypted if using HTTP loopback mode.

### Accepted for MVP

- Loopback HTTP is acceptable for development convenience.
- Public `/api/startup` token is acceptable only in direct/local bootstrap mode.
- First identity slice may be assertion-based rather than full mTLS.

## Implementation Slices

### Slice 1 — Proxy status surface

- Add `/_auspex/proxy/status` to `omegon_web_proxy`.
- Add web contract type and loader.
- Render proxy status in BACKEND card.
- Tests: proxy status JSON unit/integration check.

### Slice 2 — Trust store skeleton

- Add trust file structs.
- Add generate/status/reset commands or helper functions.
- Proxy loads trust file if present.
- No daemon enforcement yet.
- UI shows identity configured/unconfigured.

### Slice 3 — Signed local identity assertion

- Proxy signs upstream requests.
- Daemon validates for web/principal routes when configured.
- Add replay window check.
- Add explicit failure diagnostics.

### Slice 4 — Strict daemon mode

- Omegon stops honoring browser-visible startup bearer in strict mode.
- Proxy acquires authority through identity-backed attach path.
- Principal headers require validated proxy identity.

### Slice 5 — Optional browser-facing HTTPS

- Proxy serves HTTPS with local generated cert.
- Provide install instructions/status for local CA.
- Keep HTTP loopback as non-strict dev mode.

## Open Questions

1. Where should single-operator identity material live by default: project-local `.auspex/` or user-local `~/.config/auspex/`?
2. Should trust be per workspace, per machine, or per Omegon install?
3. Which Styrene identity crate/API is canonical for local signing today?
4. Does Omegon strict mode prefer signed headers or an mTLS-only peer certificate check?
5. Should Trunk remain in the loop for development, or should the proxy serve the WASM bundle directly in `auspex web dev`?

## Decision Summary

- Use proxy-mediated local identity as the architectural boundary.
- Keep browser tokenless in normal mode.
- Treat mTLS as an implementation option, not a design requirement.
- Implement signed Styrene/local identity assertion first unless Omegon server TLS support is already trivial.
- Add strict daemon enforcement only after proxy status and trust-store diagnostics exist.

## Transition Plan: Reference Adapter to Caddy / Traefik / Envoy

The hand-rolled proxy is a reference adapter, not the target infrastructure product. End users should see the same browser origin and the same authority semantics regardless of whether the adapter is the built-in Rust helper, Caddy, Traefik, or Envoy.

### Stable Adapter Contract

Any replacement must implement these externally observable routes:

| Route | Browser-facing behavior | Upstream owner |
|---|---|---|
| `/` and static assets | Serve or reverse-proxy the Auspex UI bundle | Auspex UI / static server |
| `/_auspex/proxy/status` | Return proxy posture JSON | Authority adapter |
| `/api/*` | Forward daemon-compatible HTTP routes | Omegon daemon |
| `/api/sessions/default/surfaces/stream` | WebSocket upgrade and frame bridge | Omegon daemon |

And these authority rules:

1. Strip inbound browser `Authorization`, `Omegon-Principal-*`, and `Auspex-Proxy-*` headers.
2. Discover or receive the daemon bearer server-side; never expose it to browser JavaScript or URL state.
3. Inject trusted principal headers only after local identity verification.
4. Preserve WebSocket upgrade semantics.
5. Publish the effective posture through `/_auspex/proxy/status`.

### What Breaks During Migration

| Breakpoint | Why it breaks | Mitigation |
|---|---|---|
| Header spoofing | Generic reverse proxies forward browser headers by default | Explicit strip middleware before any injection |
| Bearer discovery | Caddy/Traefik config cannot call `/api/startup` and cache tokens safely by itself | Use sidecar/plugin/ext_authz; keep Rust adapter as sidecar until plugin exists |
| WebSocket stream | Some proxy configs need explicit upgrade support | Add WS upgrade config and integration test `wss://.../surfaces/stream` |
| Browser TLS trust | Self-signed certs trigger warnings until local CA is trusted | First-run trust install or Caddy `tls internal`; status reports `trusted_local_ca` |
| Back URL / origin | Daemon actions may use origin/back-url for operator flows | Adapter injects `Omegon-Back-Url` from configured `public_origin` |
| Status endpoint | Stock proxies do not know Styrene identity posture | Keep a tiny sidecar for `/_auspex/proxy/status` or add plugin endpoint |
| Token rotation | Daemon restart rotates bearer | Adapter must refresh on 401 and before WS connect |

### Recommended User-Facing Migration Stages

1. **Reference mode** — built-in Rust adapter owns all authority behavior. Best default for MVP.
2. **Sidecar mode** — Caddy/Traefik/Envoy owns browser TLS/static serving; Rust adapter remains mounted under `/_auspex/*` and `/api/*` for token/identity injection.
3. **Plugin mode** — production proxy plugin or ext_authz service implements token discovery, identity signing, and status directly.
4. **Strict deployed mode** — no public startup bearer, daemon accepts only verified adapter identity.

Do not jump from reference mode directly to stock reverse proxy config. A stock reverse proxy can handle TLS and routing, but not the authority contract.

### Example Configs

Skeletons live in `examples/web-proxy/`:

- `Caddyfile` — route shape and `tls internal` sketch.
- `traefik-dynamic.yml` — routers/services and header-stripping middleware sketch.
- `envoy.yaml` — route shape with websocket upgrade enabled.

These files are deliberately labelled sketches. The missing piece in all stock configs is server-side bearer discovery plus Styrene identity proof. Until that exists as plugin/ext_authz, the Rust adapter remains the authority sidecar.

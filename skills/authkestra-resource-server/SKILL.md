---
name: authkestra-resource-server
description: Protect Rust Axum or Actix-web API endpoints with Authkestra Resource Server (authkestra-resource) - configuring ValidationConfig, background JWKS caching, multi-issuer trust maps, DPoP (RFC 9449), mTLS certificate binding (RFC 8705), chained authentication strategies, and the Auth<I> extractor. Use when validating JWT access tokens at an API edge or MCP server.
---

# Protecting APIs with `authkestra-resource`

Targets `authkestra` **0.13.x**.

Enable `features = ["axum", "resource"]` (or `["actix", "resource"]`) on `authkestra` to validate external OIDC/OAuth2 JWTs via `Guard<I>` and `JwtStrategy<I>`.

## 1. Configuring `ValidationConfig` & `JwtStrategy<I>`

```rust
use authkestra::axum::{Auth, AxumState};
use authkestra_resource::{jwt::JwtStrategy, jwt::ValidationConfig, Guard};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize)]
struct ApiPrincipal {
    sub: String,
    scope: Option<String>,
}

let config = ValidationConfig::builder()
    .jwks_url("https://idp.example.com/.well-known/jwks.json")
    .issuer("https://idp.example.com")
    .audiences(["https://api.example.com"])
    .require_kid(true)
    .build();

let jwt_strategy = JwtStrategy::<ApiPrincipal>::new(config);
let guard = Arc::new(Guard::builder().strategy(jwt_strategy).build());

#[derive(Clone, AxumState)]
struct AppState {
    #[authkestra(store)]
    guard: Arc<Guard<ApiPrincipal>>,
}

async fn protected_route(Auth(principal): Auth<ApiPrincipal>) -> String {
    format!("Hello, {}", principal.sub)
}
```

## 2. Startup Panic Traps on `ValidationConfig::builder().build()`

`build()` intentionally panics at startup if misconfigured so a server never boots with broken validation:
1. **No key source**: You MUST set either `.jwks_url(...)` OR at least one `.trusted_issuer(iss, jwks_url)`.
2. **Malformed `dpop_resource_origin`**: If set, `.dpop_resource_origin("https://api.example.com")` MUST be a bare `scheme://host` (with optional port) — **no path, no query string, and no trailing slash**.

## 3. Multi-Issuer Validation (Strict Trust Map)

To accept tokens from multiple identity providers without issuer confusion:

```rust
let config = ValidationConfig::builder()
    .trusted_issuer("https://tenant-a.example", "https://tenant-a.example/.well-known/jwks.json")
    .trusted_issuer("https://tenant-b.example", "https://tenant-b.example/.well-known/jwks.json")
    .audience("my-api")
    .build();
```

Once any `.trusted_issuer(...)` is added, tokens with an unknown `iss` (or missing `iss`) are rejected immediately.

## 4. Sender-Constrained Tokens: DPoP (RFC 9449) & mTLS (RFC 8705)

- **DPoP (`require_dpop(true)`)**: Requires a `DPoP` proof header matching the token's `cnf.jkt` and `ath`.
  - **Crucial Trap:** `JwtStrategy` defaults to `NoDpopReplayStore`, which **rejects every DPoP proof**. You MUST wire `.with_dpop_replay_store(Arc::new(your_replay_store))` on `JwtStrategy` when enabling `require_dpop(true)`!
- **mTLS (`require_cert_binding(true)`)**: Verifies `cnf.x5t#S256` against the client TLS certificate.

## 5. Clock Skew (`leeway`)

By default, `ValidationConfig` applies a 60-second clock-skew leeway (`authkestra_engine::token::DEFAULT_LEEWAY_SECS`) to `exp` and `nbf`. In unit tests that check immediate token expiration, chain `.leeway(0)` so a just-expired token is not accepted during the 60s grace window.

## 6. Reference Examples (`authkestra-v0.13.0`)

- Single-strategy resource server: `crates/authkestra/examples/axum_resource_server.rs`
- Chained strategies (`Guard`): `crates/authkestra/examples/axum_resource_server_strategy.rs`

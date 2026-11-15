---
name: authkestra-resource-server
description: Protect Rust Axum or Actix-web API endpoints with Authkestra Resource Server (authkestra-resource) - configuring ValidationConfig, background JWKS caching, multi-issuer trust maps, DPoP (RFC 9449), mTLS certificate binding (RFC 8705), chained authentication strategies, and the Auth<I> extractor. Use when validating JWT access tokens at an API edge or MCP server.
---

# Protecting APIs with `authkestra-resource`, `policy`, and `ssf`

Targets `authkestra` **0.15.x** (`0.15.1`).

Enable `features = ["axum", "resource"]` (or `["actix", "resource"]`) on `authkestra` to validate external OIDC/OAuth2 JWTs via `Guard<I>` and `JwtStrategy<I>`. Add `"policy"` for Cedar ABAC authorization or `"ssf"` for OpenID CAEP / RFC 8935 continuous session revocation.

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
#[authkestra(crate = ::authkestra::axum)]
struct AppState {
    #[authkestra(store)]
    guard: Arc<Guard<ApiPrincipal>>,
}

async fn protected_route(Auth(principal): Auth<ApiPrincipal>) -> String {
    format!("Hello, {}", principal.sub)
}
```

> **Refusal Envelopes (`0.15.1+`)**: `Auth<I>` calls `Guard::authenticate_with_refusal` and automatically renders `ResourceRefusal` as RFC 6750 / 9449 / 9470 JSON (`{"error": "...", ...}`) with `WWW-Authenticate`, `DPoP-Nonce` (when present), `Cache-Control: no-store`, and `X-Content-Type-Options: nosniff` (redacting `>= 500` internal error descriptions).

## 2. Startup Panic Traps on `ValidationConfig::builder().build()`

`build()` intentionally panics at startup if misconfigured so a server never boots with broken validation:
1. **No key source**: Set `.jwks_url(...)`, `.static_jwks(jwks)`, OR at least one `.trusted_issuer(iss, jwks_url)`.
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

Once any `.trusted_issuer(...)` is added (or `.issuer(...)` is configured in single-issuer mode, `0.15.0+`), tokens with an unknown `iss` or missing `iss` are rejected immediately (`MissingIssuer` / `UntrustedIssuer`).

## 4. Sender-Constrained Tokens: DPoP (`DPoP-Nonce`) & mTLS

- **DPoP (`require_dpop(true)`)**: Requires a `DPoP` proof header matching the token's `cnf.jkt` and `ath`.
  - **Crucial Trap:** `JwtStrategy` defaults to `NoDpopReplayStore`, which **rejects every DPoP proof** (`503 temporarily_unavailable` via `authenticate_with_refusal`). Always wire `.with_dpop_replay_store(Arc::new(your_replay_store))` when enabling `require_dpop(true)`!
  - **`dpop_resource_origin` (`htu` check):** Leaving `dpop_resource_origin` unset skips `htu` validation, allowing a captured valid proof to be replayed against another accepting service with its own replay store. Configure `.dpop_resource_origin("https://api.example.com")` to the externally visible origin whenever that origin can be determined reliably.
- **Resource-Server `DPoP-Nonce` (`0.15.1+`, RFC 9449 §9)**: Chain `.with_dpop_nonces(DpopNonces::new(secret, DEFAULT_NONCE_WINDOW))` onto `JwtStrategy`. Proofs missing a current server nonce receive `401 WWW-Authenticate: DPoP error="use_dpop_nonce"` + a fresh `DPoP-Nonce` header.
- **mTLS (`require_cert_binding(true)`)**: Verifies `cnf.x5t#S256` against the client TLS certificate.

## 5. RFC 9470 Step-Up Authentication Challenges (`0.15.1+`)

Enforce `acr_values` and/or `max_age` either globally on `JwtStrategy::with_step_up(...)` or per route using `StepUpRequirement` + `resource_refusal_response`:

```rust
use authkestra::axum::resource_refusal_response;
use authkestra_resource::{AuthScheme, StepUpRequirement};

let step_up = StepUpRequirement::new()
    .with_acr_values("urn:mfa")
    .with_max_age(300);

// Inside a handler holding Claims (or extracted acr/auth_time via check_values):
if let Err(refusal) = step_up.check(&claims, AuthScheme::Bearer) {
    return resource_refusal_response(&refusal); // 401 WWW-Authenticate: Bearer error="insufficient_user_authentication", ...
}
```

## 6. Cedar ABAC Policy (`features = ["policy"]`) & CAEP/SSF (`features = ["ssf"]`)

- **Cedar Policy (`policy`)**: Build a `CedarPolicyEngine` (`authkestra::policy::PolicyEngine`), wrap it in `Arc::new(PolicyGuard::new(engine))`, put it in state (`#[authkestra(store)]`), and extract `AuthPolicyGuard` in Axum/Actix handlers.
  - Note (`0.15.0+` #434): `DefaultPrincipalMapper` formats entity UIDs as `User::"{encoded_provider}:{encoded_external_id}"` (`:` and `%` percent-encoded).
- **CAEP / SSF Receiver (`ssf`)**: Wire `SetVerifier` -> `PushReceiver::new(verifier).with_handler(Arc::new(SessionRevocationHandler::new(session_store)))` and mount `ssf_axum_router(receiver)` (or `ssf_actix_scope`). Implement `SessionSubjectResolver` (`.with_resolver(...)`) to revoke user-scoped subjects (`iss_sub`, `email`), because `SessionRevocationHandler` fails closed on user-scoped events without a resolver.

## 7. Clock Skew (`leeway`) & `nbf`

`ValidationConfig` enforces `nbf` validation (`0.15.0+` #426) and applies a 60-second clock-skew leeway (`authkestra_engine::token::DEFAULT_LEEWAY_SECS`) to `exp` and `nbf`. In unit tests checking immediate token expiration, chain `.leeway(0)`.

## 8. Reference Examples (`authkestra-v0.15.1`)

- Single-strategy resource server: `crates/authkestra/examples/axum_resource_server.rs`
- Chained strategies (`Guard`): `crates/authkestra/examples/axum_resource_server_strategy.rs`

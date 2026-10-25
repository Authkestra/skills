---
name: authkestra-social-login
description: Add GitHub, Google, Discord, or custom OAuth2 and OIDC login to a Rust app with Authkestra, configuring stateful session cookies or stateless JWT callbacks and understanding the encrypted-cookie state/nonce contract. Use when integrating OAuth2 or OIDC providers.
---

# OAuth2, OIDC & Social Login in Authkestra

Targets `authkestra` **0.13.x**.

Authkestra ships concrete providers for GitHub, Google, and Discord (`features = ["github", "google", "discord"]`), a background-cached OIDC Discovery client (`features = ["oidc"]`), and a simple `OAuthProvider` trait for any custom OAuth2 provider.

## 1. The Stateless Handshake Contract

Regardless of whether your post-login session is stateful (`SessionStore`) or stateless (`TokenManager` JWT), **the OAuth2 handshake itself is always stateless**:
- `GET /auth/login/{provider}` generates a random `state` (and PKCE verifier / OIDC `nonce`) and stores it in an **encrypted, HTTP-only cookie** (`CookieManagerLayer::new()` on Axum).
- `GET /auth/callback/{provider}` reads and verifies that cookie against the callback query parameters.
- **Do not create a database table for OAuth `state` or `nonce`.** And on Axum, never forget `.layer(CookieManagerLayer::new())` on your `Router`, or the callback will fail because the encrypted state cookie was never set.

## 2. Stateful Session Mode vs. Stateless JWT Mode

### Mode A: Stateful Sessions (`AkWebAppEngine` + `AxumExt`)

```rust
use authkestra::axum::{AuthSession, AxumError, AxumExt, AxumState};
use authkestra::core::{AkWebAppEngine, OAuth2Flow, SessionStore};
use authkestra::providers::github::GithubProvider;
use authkestra::store::memory::MemoryStore;
use authkestra::Authkestra;
use axum::Router;
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;

#[derive(Clone, AxumState)]
struct AppState {
    #[authkestra(engine)]
    auth: AkWebAppEngine,
}

let github = GithubProvider::new(client_id, client_secret, "http://localhost:3000/auth/callback/github".into());
let session_store: Arc<dyn SessionStore> = Arc::new(MemoryStore::default());

let engine = Authkestra::builder()
    // Always wrap the provider in `OAuth2Flow::new(...)`!
    .provider(OAuth2Flow::new(github))
    .session_store(session_store)
    .build();

let app = Router::new()
    .merge(engine.axum_router())
    .layer(CookieManagerLayer::new())
    .with_state(AppState { auth: engine });
```

### Mode B: Stateless JWT Callback (`AkApiEngine` + `AxumStatelessExt`)

When you omit `.session_store(...)` and supply `.jwt_secret(b"at-least-32-bytes-long-secret-key")`, use `AxumStatelessExt::axum_router_stateless()` (or `ActixStatelessExt::actix_scope_stateless()`):

```rust
use authkestra::axum::{AxumState, AxumStatelessExt};
use authkestra::core::{AkApiEngine, OAuth2Flow};
use authkestra::providers::github::GithubProvider;
use authkestra::Authkestra;

#[derive(Clone, AxumState)]
struct AppState {
    #[authkestra(engine)]
    auth: AkApiEngine,
}

let engine = Authkestra::builder()
    .provider(OAuth2Flow::new(github))
    .jwt_secret(b"your-256-bit-secret-key-at-least-32-bytes-long")
    .build();

let app = axum::Router::new()
    .merge(engine.axum_router_stateless())
    .layer(tower_cookies::CookieManagerLayer::new())
    .with_state(AppState { auth: engine });
```

## 3. Generic OIDC Discovery (`OidcProvider`)

For Google, Okta, Auth0, Keycloak, or any OIDC-compliant issuer (`features = ["oidc"]`), `OidcProvider::discover` fetches `/.well-known/openid-configuration` and refreshes keys in a background `tokio::spawn` task:

```rust
use authkestra::oidc::OidcProvider;
use std::time::Duration;

let oidc = OidcProvider::discover(
    client_id,
    client_secret,
    "http://localhost:3000/auth/callback/oidc".to_string(),
    "https://accounts.google.com",
    Duration::from_secs(3600), // fallback cache TTL
).await?;

let engine = Authkestra::builder()
    .provider(OAuth2Flow::new(oidc))
    .session_store(session_store)
    .build();
```

## 4. Reference Examples (`authkestra-v0.13.0`)

- Stateful GitHub OAuth: `crates/authkestra/examples/axum_oauth2_github.rs`
- Stateless OAuth (JWT): `crates/authkestra/examples/axum_oauth_stateless.rs`
- Google OIDC: `crates/authkestra/examples/axum_oidc_google.rs`

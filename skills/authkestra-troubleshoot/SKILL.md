---
name: authkestra-troubleshoot
description: Troubleshoot runtime errors and unexpected behavior in Authkestra - rustls CryptoProvider panics, lost OAuth state cookies on callback, invalid_grant on OP token exchange, DPoP proof rejections, ReproofRequirement failures in tests, and JWT clock skew. Use when an Authkestra app compiles but panics or rejects valid authentication flows at runtime.
---

# Troubleshooting Authkestra Runtime Issues

Targets `authkestra` **0.13.x**.

First: turn on `tracing` for `authkestra` at `debug` level (`RUST_LOG=info,authkestra=debug`). Every handler and branch in Authkestra emits structured `tracing` events explaining why a request was rejected.

## 1. Symptom → Cause → Fix

### Panic at startup or first request: `Could not automatically determine the process-level CryptoProvider`
- **Cause**: You enabled `rustls-no-provider` (with `default-features = false`) to avoid `aws-lc-rs`, but did not install a `rustls` crypto provider before constructing an HTTP client (`OidcProvider`, `OAuth2Flow`, `JwksCache`, `CaptchaVerifier`).
- **Fix**: Call this at the very top of `main()`:
  ```rust
  rustls::crypto::ring::default_provider()
      .install_default()
      .expect("failed to install rustls crypto provider");
  ```

### Panic at startup: `ValidationConfig::builder().build()`
- **Cause**: Either no key source (`jwks_url` or `trusted_issuer`) was configured, OR `dpop_resource_origin` contained a path, query, or trailing slash (e.g. `"https://api.example.com/"`).
- **Fix**: Supply `jwks_url` / `trusted_issuer`, and strip any trailing slash or path from `dpop_resource_origin` (`"https://api.example.com"`).

### OAuth callback fails with missing or invalid `state` cookie
- **Causes & Fixes**:
  1. **Missing `CookieManagerLayer` on Axum**: `axum_router()` and `axum_router_stateless()` rely on `tower_cookies::CookieManagerLayer::new()`. Add `.layer(CookieManagerLayer::new())` to your `Router`.
  2. **Plain HTTP in local dev with `secure: true`**: `SessionConfig::default()` sets `secure: true`. On `http://localhost`, cookies marked `Secure` are sometimes dropped or mishandled across redirects. Set `SessionConfig { secure: false, ..Default::default() }` in local HTTP development.
  3. **Cross-site frontend/backend deployment**: If your frontend (`app.example.com`) and API (`api.other.com`) are on different registrable domains, `SameSite::Lax` cookies are not sent on cross-site fetches. Put both on the same registrable domain or configure `SameSite::None` with `secure: true`.

### OP `/token` always returns `invalid_grant` even right after `/authorize`
- **Cause**: Custom `OpStore` implemented `#[derive(Clone)]` over an in-memory `HashMap` without `Arc<Mutex<_>>` / `Arc<RwLock<_>>`. Because `Op` clones the store per request, the authorization code written during `/authorize` was dropped when that request finished.
- **Fix**: Wrap in-memory state in `Arc<RwLock<_>>` (or use `MemoryStore` / `RedisStore` / `SqlxOpStore`) so `Clone` shares the underlying state.

### Every DPoP or `private_key_jwt` request is rejected with 401 / `invalid_dpop_proof`
- **Cause**: Both `OpStore` and `JwtStrategy` default to fail-closed replay stores (`NoDpopReplayStore`, `NoClientAssertionStore`) that reject 100% of proofs until a real replay store is wired.
- **Fix**: Wire `.with_dpop_replay_store(...)` / `.with_client_assertion_store(...)` on `CompositeOpStore` or `JwtStrategy`.

### `start_register`, `register_totp`, or `RecoveryCodeAuthMethod::generate` fails in unit tests with `AuthError::Credentials`
- **Cause**: `ReproofRequirement::check` fails closed (`ReproofFailure::FreshnessUnknown`) if the `Identity` passed to it was manually constructed without `IDENTITY_ATTR_AUTH_TIME` in `identity.attributes`.
- **Fix**: Either obtain the `Identity` from `Engine::authenticate`, or stamp `IDENTITY_ATTR_AUTH_TIME` in your test helper:
  ```rust
  use authkestra_engine::auth::IDENTITY_ATTR_AUTH_TIME;
  identity.attributes.insert(
      IDENTITY_ATTR_AUTH_TIME.to_string(),
      chrono::Utc::now().timestamp().to_string(),
  );
  ```

### Expired JWT test passes when it should fail
- **Cause**: `TokenManager` and `ValidationConfig` allow 60 seconds of clock skew (`DEFAULT_LEEWAY_SECS`) by default.
- **Fix**: Chain `.leeway(0)` on `ValidationConfig::builder()` in expiration unit tests.

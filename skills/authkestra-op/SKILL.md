---
name: authkestra-op
description: Run an embedded OpenID Connect Provider (OP) and OAuth 2.0 Authorization Server in Rust using authkestra-op — configuring OpConfig, OpStore, asymmetric signing keys, PKCE, DPoP, private_key_jwt, Device Flow, RFC 8693 Token Exchange, and device/service attestation issuance. Use when building an identity provider or token broker with Authkestra.
---

# Running an OpenID Connect Provider (`authkestra-op`)

Targets `authkestra` **0.13.x**.

`authkestra-op` turns your Rust binary into an OpenID Connect Provider (OP) and OAuth 2.0 Authorization Server. Enable `features = ["axum", "op"]` (or `["actix", "op"]`) on `authkestra`.

## 1. Critical Rules for `authkestra-op`

1. **Asymmetric Signing Algorithms Only**: Your `Engine`'s `TokenManager` and `OpConfig.id_token_signing_alg` MUST use an asymmetric algorithm (`EdDSA`, `ES256`, `ES384`, `RS256`, etc.). Symmetric `HS256` is rejected because Resource Servers validate OP tokens via the public `/jwks.json` endpoint.
2. **`OpConfig` Has No `Default`**: You must specify all fields explicitly, including the TTLs and `token_exchange_enabled`:
   ```rust
   use authkestra_op::config::OpConfig;

   let config = OpConfig {
       issuer: "https://auth.example.com".to_string(), // NO trailing slash!
       scopes_supported: vec!["openid".into(), "profile".into(), "email".into()],
       response_types_supported: vec!["code".into()],
       grant_types_supported: vec![
           "authorization_code".into(),
           "refresh_token".into(),
           "client_credentials".into(),
           "urn:ietf:params:oauth:grant-type:device_code".into(),
       ],
       id_token_signing_alg: "EdDSA".to_string(),
       authorization_code_ttl_secs: 60,
       access_token_ttl_secs: 3600,
       device_code_ttl_secs: 600,
       token_exchange_enabled: false, // Set `true` only when using RFC 8693 Token Exchange
   };
   ```
3. **`CloneableOpStore` Must Share State**: `Op::builder().store(Arc::new(op_store))` clones the store per request so `&mut self` methods don't contend on a global lock. Use `SqlxOpStore` (`authkestra-store-sqlx`), `RedisStore`, or `CompositeOpStore` over `Arc`-backed stores.
4. **Replay Guards Default to Fail-Closed**: `OpStore::record_client_assertion_jti` (`private_key_jwt`) and `OpStore::check_and_record_dpop_jti` (RFC 9449 DPoP) default to **rejecting every proof**. When using `CompositeOpStore`, wire `.with_client_assertion_store(MemoryClientAssertionStore::new())` and `.with_dpop_replay_store(...)` (or use `SqlxOpStore`, which implements `oauth_dpop_jti`).

## 2. Wiring OP Routes in Axum & Actix

```rust
use authkestra_axum::op::{OpExt, OpState};
use authkestra_op::Op;
use axum::Router;
use std::sync::Arc;

let op = Op::builder()
    .engine(auth_engine)
    .config(config)
    .store(Arc::new(op_store))
    .build();

let app = Router::new()
    // Mounts /.well-known/openid-configuration, /jwks.json, /authorize,
    // /token, /userinfo, /device_authorization, and /device/verify
    .merge(op.op_axum_router())
    .with_state(OpState(op));
```

## 3. RFC 8693 Token Exchange (Agent & Microservice Delegation)

When `token_exchange_enabled: true` is set on `OpConfig`:
- Clients submit `grant_type=urn:ietf:params:oauth:grant-type:token-exchange` with a `subject_token`.
- **Strict Audience Binding**: The presenting `client_id` must be in the `subject_token`'s `aud` (or be the original recipient), and any requested downstream `audience` must be listed in the client's `ClientRegistration.allowed_audiences`.
- **Strict Scope Narrowing**: The issued token's scope is the intersection of the requested scope, the client's allowed scopes, and the `subject_token`'s existing scopes. Privilege escalation is structurally impossible.

## 4. Device/Service Attestation Issuance (`POST /enrol`, `/enrol/complete`, `/reissue`)

To mint `cnf.jkt`-bound attestations for `authkestra-devsig`, supply the three attestation builder slots on `Op::builder()` and merge `op.op_axum_attestation_router()`:

```rust
let op = Op::builder()
    .engine(auth_engine)
    .config(config)
    .store(Arc::new(op_store))
    .challenge_store(Arc::new(MemoryStore::<EnrolmentChallenge>::new()))
    .second_factor_verifier(Arc::new(MySecondFactorVerifier))
    .attestation_config(AttestationConfig::new())
    .build();

let app = Router::new()
    .merge(op.op_axum_router())
    .merge(op.op_axum_attestation_router())
    .with_state(OpState(op));
```

## 5. Reference Examples (`authkestra-v0.13.0`)

- In-memory OP server: `crates/authkestra/examples/axum_op_server.rs`
- SQLx-backed OP server: `crates/authkestra/examples/axum_op_server_sqlx.rs`
- Device attestation OP: `crates/authkestra/examples/axum_op_server_attestation.rs`
- Custom grant OP: `crates/authkestra/examples/axum_op_server_custom_grant.rs`

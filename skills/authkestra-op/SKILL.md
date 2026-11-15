---
name: authkestra-op
description: Run an embedded OpenID Connect Provider (OP) and OAuth 2.0 Authorization Server in Rust using authkestra-op — configuring OpConfig, OpStore, asymmetric signing keys, PKCE, DPoP, private_key_jwt, Device Flow, RFC 8693 Token Exchange, and device/service attestation issuance. Use when building an identity provider or token broker with Authkestra.
---

# Running an OpenID Connect Provider (`authkestra-op`)

Targets `authkestra` **0.15.x** (`0.15.1`).

`authkestra-op` turns your Rust binary into an OpenID Connect Provider (OP) and OAuth 2.0 Authorization Server. Enable `features = ["axum", "op"]` (or `["actix", "op"]`) on `authkestra`.

## 1. Critical Rules for `authkestra-op`

1. **Asymmetric Signing for Public `/jwks.json`**: Use `TokenManager::new_asymmetric(...)` (`RS256`) or `TokenManager::new_ed25519(...)` (`EdDSA`) paired with matching `OpConfig.id_token_signing_alg` (`"RS256"` / `"EdDSA"`) so `/jwks.json` publishes a verifiable JWK (`public_jwk()`). `TokenManager::new(secret, issuer)` is symmetric `HS256` with `public_jwk() == None`, which external Resource Servers cannot validate via JWKS.
2. **`OpConfig` Has No `Default`**: Specify all fields explicitly, including TTLs and `token_exchange_enabled`:
   ```rust
   use authkestra::op::config::OpConfig;

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
3. **`ClientRegistration` (`0.14+` / OAuth 2.1)**: `allowed_audiences: Vec<String>` configures audiences a client may request/target (`0.14.0` #422); `token_endpoint_auth_method` + inline `jwks` lock down `private_key_jwt`; `require_pkce` is deprecated because PKCE is mandatory unconditionally for every client; loopback IP redirect URIs (`http://127.0.0.1/...`, `http://[::1]/...`) match any ephemeral port per RFC 8252 §7.3 (never `localhost`).
4. **`CloneableOpStore` Must Share State**: `Op::builder().store(Arc::new(op_store))` clones the store per request so `&mut self` methods don't contend on a global lock. Prefer `SqlxOpStore` (`authkestra-store-sqlx`), `RedisStore`, or `CompositeOpStore` over `Arc`-backed stores.
5. **Replay Guards Default to Fail-Closed**: `OpStore::record_client_assertion_jti` (`private_key_jwt`) and `OpStore::check_and_record_dpop_jti` (RFC 9449 DPoP) default to **rejecting every proof**. `SqlxOpStore` overrides `check_and_record_dpop_jti` only (`oauth_dpop_jti`) and does **not** track `private_key_jwt` assertion JTIs, so `private_key_jwt` fails on a bare `SqlxOpStore`. On `CompositeOpStore`, wire `.with_client_assertion_store(...)` (`MemoryClientAssertionStore::new()` is single-process only; use a shared atomic `ClientAssertionStore` across multi-replica deployments) and `.with_dpop_replay_store(...)` (or forward DPoP checks to `SqlxOpStore`). Also opt into DPoP in discovery via `OidcDiscovery::with_dpop_support`.

## 2. Wiring OP Routes in Axum & Actix

Either pass `OpState(op)` (from `Op::builder()...build()`) or derive `AxumState` on your own struct (`#[authkestra(engine)] auth: AkEngine`, `#[authkestra(store)] op_store: Arc<dyn CloneableOpStore>`, `#[authkestra(store)] config: OpConfig`):

```rust
use authkestra::axum::op::{OpExt, OpState};
use authkestra::op::Op;
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

Note (`0.15.0+` #431): `POST /device/verify` rejects `Sec-Fetch-Site: cross-site`, checks `Origin` against `OpConfig::issuer`, and supports session-bound HMAC CSRF tokens.

## 3. Opt-in `DPoP-Nonce` at `/token` (`0.15.0+`, RFC 9449 §8)

DPoP nonces prevent pre-generated proofs from staying usable across a full `iat` window:

```rust
use authkestra::core::token::dpop_nonce::{DpopNonces, DEFAULT_NONCE_WINDOW};

let nonces = DpopNonces::new(b"32-byte-shared-cluster-secret...", DEFAULT_NONCE_WINDOW);
let op_store = CompositeOpStore::new(clients, auth_codes, refresh_tokens, device_codes)
    .with_dpop_replay_store(dpop_replay)
    .with_dpop_nonces(nonces);
```

- Proofs without a current nonce are refused `400 {"error":"use_dpop_nonce"}` with a fresh `DPoP-Nonce` header **before** consuming the proof's `jti` (successful responses also return `DPoP-Nonce`).
- **Browser / CORS Trap**: Cross-origin clients cannot read `DPoP-Nonce` unless you expose it in CORS (`Access-Control-Expose-Headers: DPoP-Nonce`).
- **`private_key_jwt` Retry Trap**: A `client_assertion` *is* consumed before the nonce check fails, so a retrying client must mint a fresh `client_assertion` alongside the retried `DPoP` proof.

## 4. `amr` / `acr`, Audience Issuance & RFC 8693 Token Exchange

- **Access Token Audiences (`0.14.0+`)**: Access tokens honor configured client `allowed_audiences` (`ClientRegistration::allowed_audiences` and the SQLx `allowed_audiences` column).
- **OIDC `acr` / `amr` (`0.15.1+`)**: `authkestra_op::amr_acr_extra_claims(&identity)` maps engine auth bookkeeping to standard `acr` (`ACR_SINGLE_FACTOR` / `ACR_MFA`), `amr`, and `auth_time`; `/userinfo` enforces `cnf.jkt` DPoP binding when the access token is DPoP-bound.
- **RFC 8693 Token Exchange**: With `token_exchange_enabled: true`, clients submit `grant_type=urn:ietf:params:oauth:grant-type:token-exchange` with a `subject_token`. The presenting `client_id` must match the `subject_token`'s `aud`, any requested downstream `audience` must appear in `ClientRegistration.allowed_audiences`, and issued scopes are narrowed to the intersection of requested, client-allowed, and `subject_token` scopes.

## 5. Device/Service Attestation Issuance (`POST /enrol`, `/enrol/complete`, `/reissue`)

To mint `cnf.jkt`-bound attestations for `authkestra-devsig`, supply the attestation builder slots on `Op::builder()` and merge `op.op_axum_attestation_router()` (and optionally `.status_provider(...)` so revoked principals are refused before renewal):

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

## 6. Reference Examples (`authkestra-v0.15.1`)

- In-memory/Redis OP server: `crates/authkestra/examples/axum_op_server.rs`
- SQLx-backed OP server: `crates/authkestra/examples/axum_op_server_sqlx.rs`
- Device attestation OP: `crates/authkestra/examples/axum_op_server_attestation.rs`
- Custom grant OP: `crates/authkestra/examples/axum_op_server_custom_grant.rs`

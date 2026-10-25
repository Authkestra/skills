---
name: authkestra-quickstart
description: Add Authkestra authentication to a Rust axum or actix-web application - choosing facade feature flags, picking a TLS backend, satisfying the typestate builder, selecting a session store, and wiring the framework extractors. Use when someone wants to add login, sessions, OAuth or OIDC to a Rust app with authkestra, or is starting an authkestra integration from scratch.
---

# Adding Authkestra to a Rust app

Targets `authkestra` **0.13.x**. For older code — especially anything importing `authkestra-core`,
`authkestra-flow`, `authkestra-token`, `authkestra-session`, or enabling the removed `"session"` /
`"token"` Cargo features — use the `authkestra-upgrade` skill first.

Work through the five steps in order. Each one constrains the next, and skipping ahead produces
compiler errors that look unrelated to their cause.

## 1. Pick the facade features

Depend on the `authkestra` facade crate. In `0.13.x`, it re-exports sub-crates, auth methods, and
storage backends behind feature flags:

```toml
[dependencies]
authkestra = { version = "0.13", features = ["axum", "memory", "github"] }
```

| Feature Category | Features | Gives you |
| --- | --- | --- |
| **Core** | `engine` (default) | `Engine`, session & token machinery (`authkestra::core`) |
| **Web frameworks** | `axum`, `actix` | Framework adapters, extractors (`AuthSession`, `AuthToken`, `Auth`), state macros |
| **Storage backends** | `memory`, `redis`, `sql-postgres`, `sql-mysql`, `sql-sqlite` | Built-in `MemoryStore`, `RedisStore`, and `SqlxCredentialStore` (`authkestra::store`) |
| **OAuth / OIDC** | `oidc`, `github`, `google`, `discord` | OIDC discovery client and concrete OAuth2 providers |
| **Passwordless & MFA** | `webauthn`, `totp`, `magic-link`, `otp`, `recovery-codes`, `captcha` | First-party authentication methods and CAPTCHA verifiers |
| **Servers & Policy** | `resource`, `op`, `devsig`, `policy`, `ssf` | Resource server `Guard`, OIDC Provider (`authkestra-op`), device signatures, Cedar policy, CAEP/SSF |

Two rules that save a debugging session:

- **There is NO `session` or `token` feature in `0.13`.** Both were removed (#375, #377) because
  session and token machinery are now unconditional in `authkestra-engine`. Pick a storage backend
  feature (`memory`, `redis`, `sql-*`) instead.
- **`full` includes every auth capability, but NO storage backend and NO TLS choice.** If you use
  `features = ["full"]`, still name your store alongside it (e.g., `features = ["full", "memory"]`).

## 2. Choose a TLS backend

Authkestra talks to identity providers over HTTPS, and the crypto provider is chosen by feature flag
rather than inherited from `reqwest`'s defaults.

- `rustls-aws-lc-rs` — **the default.** Compiles C and assembly, so the build host needs a C toolchain.
- `rustls-no-provider` — for pure-Rust builds (`ring`), `*-unknown-linux-musl` targets, or a `cargo-deny`
  policy that bans `aws-lc-rs`.

Choosing the second means disabling default features and installing a provider yourself, **before any
Authkestra call that builds an HTTP client** — otherwise `reqwest` panics at client construction:

```rust
rustls::crypto::ring::default_provider()
    .install_default()
    .expect("failed to install rustls crypto provider");
```

Cargo features are additive, so one crate anywhere in the graph enabling `rustls-aws-lc-rs` brings
`aws-lc-rs` back. Verify with `cargo tree -i aws-lc-rs -e features`.

## 3. Build the engine (typestate)

`Authkestra<S, T>` is a type alias for `Engine<S, T>`, and both type parameters default to `Missing`.
This is deliberate: **methods do not exist until their prerequisite is supplied.** `create_session`
and `axum_router()` are absent until `.session_store(...)` is set; `axum_router_stateless()` and
`AuthToken` are absent until `.jwt_secret(...)` or `.token_manager(...)` is set.

```rust
use authkestra::core::{AkWebAppEngine, OAuth2Flow, SessionStore};
use authkestra::providers::github::GithubProvider;
use authkestra::store::memory::MemoryStore;
use authkestra::Authkestra;
use std::sync::Arc;

let github_provider = GithubProvider::new(client_id, client_secret, redirect_uri);
let session_store: Arc<dyn SessionStore> = Arc::new(MemoryStore::default());

let engine: AkWebAppEngine = Authkestra::builder()
    .provider(OAuth2Flow::new(github_provider))
    .session_store(session_store)
    .build();
```

Use the engine type aliases in your router state struct:
- `AkWebAppEngine`: `SessionStore` configured, no `TokenManager`
- `AkApiEngine`: `TokenManager` configured, no `SessionStore`
- `AkEngine`: both configured

## 4. Choose a store

Authkestra never enforces a schema. Data access is defined entirely by traits — `KvStore`,
`SessionStore`, `CredentialStore`, and `OpStore` for the OP.

- **Prototyping**: `MemoryStore` (`features = ["memory"]`).
- **Redis**: `RedisStore` (`features = ["redis"]`).
- **SQL**: `SqlxCredentialStore` (`features = ["sql-postgres" | "sql-mysql" | "sql-sqlite"]`) for
  credentials, and `authkestra-store-sqlx` (`SqlxOpStore`) for OP tables.
- **Your own database or ORM**: implement the traits yourself — use the `authkestra-store` skill,
  which also covers validating the implementation against `authkestra-store-testsuite`.

**There is no `UserStore` and no user table.** The application owns user and account data; Authkestra
orchestrates authentication over it.

## 5. Wire the framework adapter

**axum** — extractors and traits from `authkestra::axum`:

- `AuthSession(pub Session)` — the authenticated session for a request (requires `AkWebAppEngine` or `AkEngine`).
- `AuthToken(pub Claims)` — validated JWT bearer token (requires `AkApiEngine` or `AkEngine`).
- `Auth<I>(pub I)` — resource-server `Guard<I>` identity validation.
- `#[derive(AxumState)]` with `#[authkestra(engine)]` — generates the `FromRef` impls your engine typestate supports. When depending only on the `authkestra` facade crate (not `authkestra-axum` directly), add `#[authkestra(crate = ::authkestra::axum)]` to your state struct so the macro resolves paths through the facade (#332).
- `AxumExt::axum_router()` (stateful sessions, requires `CookieManagerLayer::new()`) and `AxumStatelessExt::axum_router_stateless()` (stateless JWT callback).

**actix-web** — from `authkestra::actix`:

- `AuthSession`, `AuthToken`, `Auth<I>`, `#[derive(ActixState)]` (requires `"macros"` feature on `authkestra` and `#[authkestra(crate = ::authkestra::actix)]` when depending only on the facade).
- `ActixExt::actix_scope()` and `ActixStatelessExt::actix_scope_stateless()`, paired with `.configure(move |cfg| state.configure_authkestra(cfg))`.

OAuth `state` and `nonce` live in **encrypted cookies, never in the database**. This keeps the
deployment horizontally scalable with no shared session state during the handshake.

## Runnable examples (`authkestra-v0.13.0`)

Every example lives under `crates/authkestra/examples/` in
<https://github.com/marcjazz/authkestra/tree/authkestra-v0.13.0/crates/authkestra/examples>:

| Goal | Command |
| --- | --- |
| Minimal axum setup | `cargo run -p authkestra --example axum_basic_setup --all-features` |
| Minimal actix setup | `cargo run -p authkestra --example actix_basic_setup --all-features` |
| GitHub OAuth | `cargo run -p authkestra --example axum_oauth2_github --all-features` |
| Google OIDC | `cargo run -p authkestra --example axum_oidc_google --all-features` |
| Stateless OAuth (JWT callback) | `cargo run -p authkestra --example axum_oauth_stateless --all-features` |
| Redis-backed sessions | `cargo run -p authkestra --example axum_session_redis --all-features` |
| MFA (TOTP + Passkeys) | `cargo run -p authkestra --example axum_mfa_server --all-features` |

## Where to go next

| Goal | Skill |
| --- | --- |
| Missing method or trait bound error | `authkestra-features` |
| Social login, or a custom provider | `authkestra-social-login` |
| Your own database behind the store traits | `authkestra-store` |
| Run your own identity provider (OP) | `authkestra-op` |
| Validate tokens at an API edge | `authkestra-resource-server` |
| Passkeys, TOTP, Magic Link, OTP, Recovery Codes, `devsig` | `authkestra-mfa` |
| Upgrading from older versions | `authkestra-upgrade` |
| Runtime panics, lost state, clock skew | `authkestra-troubleshoot` |

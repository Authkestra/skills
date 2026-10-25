---
name: authkestra-features
description: Diagnose compiler errors in Authkestra caused by missing Cargo feature flags versus unmet typestate builder prerequisites (Engine<S, T>, FromRef<AppState>, AxumExt vs AxumStatelessExt). Use when an Authkestra method, module, or extractor fails to compile.
---

# Diagnosing Authkestra Compiler Errors: Cargo Features vs. Typestate

Targets `authkestra` **0.13.x**.

In Authkestra, two distinct mechanisms hide methods at compile time:
1. **Cargo feature flags** (`#[cfg(feature = "...")]`)
2. **Typestate builder parameters** (`Engine<S, T>`, where `S` and `T` start as `Missing`)

Both surface as *"no method named `...` found"* or *"the trait bound `...: FromRef<AppState>` is not satisfied"*. Use the table below to tell them apart immediately.

## 1. Compiler Error → Exact Fix

| Compiler Symptom | Cause | Fix |
| --- | --- | --- |
| `package authkestra does not have feature session` or `token` | Removed in `0.13` (#375, #377); session and token machinery are now unconditional in `engine`. | Remove `"session"` and `"token"` from `Cargo.toml`. Pick a storage backend (`"memory"`, `"redis"`, `"sql-sqlite"`, etc.) instead. |
| `no method named create_session` or `axum_router` on `Engine<Missing, _>` | Typestate: no `SessionStore` was supplied to `Authkestra::builder()`. | Chain `.session_store(session_store)` before `.build()`, and use `AkWebAppEngine` or `AkEngine`. |
| `no method named axum_router_stateless` on `Engine<Configured<_>, Missing>` | Typestate or missing trait import. | 1. Chain `.jwt_secret(...)` or `.token_manager(...)` on the builder.<br>2. Import `authkestra::axum::AxumStatelessExt` (it is NOT on `AxumExt`). |
| `the trait bound Result<Arc<TokenManager>, AxumError>: FromRef<AppState> is not satisfied` | Handler uses `AuthToken`, but `AppState` holds `AkWebAppEngine` (`T = Missing`). | Supply `.jwt_secret(...)` / `.token_manager(...)` on the builder and change the state field type to `AkApiEngine` or `AkEngine`. |
| `the trait bound Result<Arc<dyn SessionStore>, AxumError>: FromRef<AppState> is not satisfied` | Handler uses `AuthSession`, but `AppState` holds `AkApiEngine` (`S = Missing`). | Supply `.session_store(...)` on the builder and change the state field type to `AkWebAppEngine` or `AkEngine`. |
| `no method named with_webauthn` or `with_totp` on `EngineBuilder` | Missing Cargo feature. | Add `"webauthn"` or `"totp"` to `authkestra` features in `Cargo.toml`. |
| `unresolved import authkestra::store::memory` or `redis` | No storage feature enabled. | Note that `features = ["full"]` deliberately excludes storage backends! Add `"memory"`, `"redis"`, or `"sql-*"` explicitly. |
| `compile_error!("authkestra-engine needs a TLS backend...")` | `default-features = false` was set without choosing a TLS feature. | Enable either `"rustls-aws-lc-rs"` or `"rustls-no-provider"`. |

## 2. Complete `authkestra` 0.13 Facade Feature Map

```toml
[dependencies]
authkestra = { version = "0.13", features = [
    # Web framework adapters
    "axum",           # authkestra-axum (macros, session, token, resource)
    # "actix",        # authkestra-actix (session, token, resource)
    # "macros",       # needed explicitly on actix for #[derive(ActixState)]

    # Storage backends (NOT included in `full` — always pick one!)
    "memory",         # authkestra_engine::store::memory::MemoryStore
    # "redis",        # authkestra_engine::store::redis::RedisStore (+ authkestra-op/redis)
    # "sql-postgres", # authkestra_engine::store::sql::SqlxCredentialStore (Postgres)
    # "sql-mysql",    # authkestra_engine::store::sql::SqlxCredentialStore (MySQL)
    # "sql-sqlite",   # authkestra_engine::store::sql::SqlxCredentialStore (SQLite)

    # OAuth2 / OIDC providers
    "github",
    "google",
    "discord",
    "oidc",

    # First-party passwordless & MFA methods
    "webauthn",
    "totp",
    "magic-link",
    "otp",
    "recovery-codes",
    "captcha",

    # Server roles, device signatures, policy & continuous trust
    "resource",       # authkestra-resource (Guard, JwtStrategy)
    "op",             # authkestra-op + adapter OP routes
    "devsig",         # authkestra-devsig + adapter middleware/extractors
    "policy",         # authkestra-policy (AWS Cedar engine)
    "ssf",            # authkestra-ssf (OpenID CAEP / RFC 8417 SET receiver)
] }
```

## 3. Typestate Engine Aliases (`authkestra_engine::aliases`)

Never write raw `Engine<Configured<Arc<dyn SessionStore>>, Missing>` in your `AppState`. Use the three canonical aliases exported by `authkestra::core` / `authkestra_engine`:

| Alias | Expansion | Supports `AuthSession` & `axum_router()` | Supports `AuthToken` & `axum_router_stateless()` |
| --- | --- | --- | --- |
| `AkWebAppEngine` | `Engine<Configured<Arc<dyn SessionStore>>, Missing>` | Yes | No |
| `AkApiEngine` | `Engine<Missing, Configured<Arc<TokenManager>>>` | No | Yes |
| `AkEngine` | `Engine<Configured<Arc<dyn SessionStore>>, Configured<Arc<TokenManager>>>` | Yes | Yes |

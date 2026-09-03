---
name: authkestra-quickstart
description: Add Authkestra authentication to a Rust axum or actix-web application - choosing facade feature flags, picking a TLS backend, satisfying the typestate builder, selecting a session store, and wiring the framework extractors. Use when someone wants to add login, sessions, OAuth or OIDC to a Rust app with authkestra, or is starting an authkestra integration from scratch.
---

# Adding Authkestra to a Rust app

Targets `authkestra` **0.7.x**. For older code — especially anything importing `authkestra-core`,
`authkestra-flow`, `authkestra-token` or `authkestra-session`, which no longer exist — use the
`authkestra-upgrade` skill first.

Work through the five steps in order. Each one constrains the next, and skipping ahead produces
compiler errors that look unrelated to their cause.

## 1. Pick the facade features

Depend on the `authkestra` facade, not the individual crates. It re-exports everything behind
feature flags:

```toml
[dependencies]
authkestra = { version = "0.7", features = ["axum", "session", "github"] }
```

| Feature | Gives you |
| --- | --- |
| `session` | Session management and the session stores |
| `token` | `TokenManager`, JWT issuance and validation |
| `oidc` | OIDC discovery and provider support |
| `resource` | Resource-server validation (`Guard`, `ValidationConfig`) |
| `axum` / `actix` | The framework adapter, incl. its extractors and macros |
| `github` / `google` / `discord` | Concrete OAuth providers |
| `full` | All of the above |

Two rules that save a debugging session:

- **Adapters have their own features.** `authkestra-axum` and `authkestra-actix` carry `op`,
  `devsig`, `captcha`, `macros`, `session`, `token` and `resource` flags of their own. The facade's
  `axum`/`actix` features turn on a useful subset, not all of them. Reaching for OP or device-signature
  functionality means enabling the adapter's feature too.
- **A missing feature usually reports as a missing method**, not as a missing crate. If a method the
  docs promise does not exist, suspect features before suspecting your code — see the
  `authkestra-features` skill.

## 2. Choose a TLS backend

Authkestra talks to identity providers over HTTPS, and the crypto provider is chosen by feature flag
rather than inherited from `reqwest`'s defaults.

- `rustls-aws-lc-rs` — **the default.** Compiles C and assembly, so the build host needs a C toolchain.
- `rustls-no-provider` — for pure-Rust builds, `*-unknown-linux-musl` targets, or a `cargo-deny` policy
  that bans `aws-lc-rs`.

Choosing the second means installing a provider yourself, **before any Authkestra call that builds an
HTTP client** — otherwise `reqwest` panics at client construction:

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
is absent until a `session_store` is set; token methods are absent until a `TokenManager` is.

```rust
use authkestra::Authkestra;
use authkestra_engine::store::memory::MemoryStore;
use authkestra_engine::{OAuth2Flow, SessionStore};
use authkestra_providers::github::GithubProvider;
use std::sync::Arc;

let github_provider = GithubProvider::new(client_id, client_secret, redirect_uri);
let session_store: Arc<dyn SessionStore> = Arc::new(MemoryStore::default());

let engine = Authkestra::builder()
    .provider(OAuth2Flow::new(github_provider))
    .session_store(session_store)
    .build();
```

So "no method named `create_session`" is not a bug — it is the builder reporting that no session
store was provided. Read it as a missing prerequisite, not a missing import.

## 4. Choose a store

Authkestra never enforces a schema. Data access is defined entirely by traits — `KvStore`,
`SessionStore`, `CredentialStore`, and `OpStore` for the OP.

- **Prototyping**: `MemoryStore`.
- **Redis**: the engine's `redis` feature.
- **SQL**: `authkestra-store-sqlx` (Postgres, MySQL, SQLite).
- **Your own database or ORM**: implement the traits yourself — use the `authkestra-store` skill,
  which also covers validating the implementation against `authkestra-store-testsuite`.

**There is no `UserStore` and no user table.** The application owns user and account data; Authkestra
orchestrates authentication over it. Do not go looking for a user trait to implement — its absence is
the design.

## 5. Wire the framework adapter

**axum** — extractors from `authkestra::axum`:

- `AuthSession(pub Session)` — the authenticated session for a request.
- `Auth<I>(pub I)` — the authenticated identity, for resource-server style validation.
- `#[derive(AxumState)]` — wires engine components into application state.
- `OpExt` — route registration when running an OP.

**actix-web** — from `authkestra::actix`:

- `Auth<I>(pub I)`, `#[derive(ActixState)]`, `OpExt`.
- Ready-made handlers: `actix_login_handler`, `actix_callback_handler`, `actix_logout_handler`.

OAuth `state` and `nonce` live in **encrypted cookies, never in the database**. This keeps the
deployment horizontally scalable with no shared session state, and it is why there is no table to
create for the OAuth handshake. Do not "fix" this by persisting them.

## Verify

`cargo check` after each step rather than at the end — the typestate builder localises mistakes well,
but only if few things changed since the last successful check.

If it fails: a missing method points at step 1 or step 3 (`authkestra-features` skill); a panic at
client construction points at step 2; a trait-not-implemented on your database type points at step 4.

## Runnable examples

Every example lives in the facade crate of the main repository
(<https://github.com/marcjazz/authkestra>, under `crates/authkestra/examples/`), and each one's module
docs list the environment variables it needs:

| Goal | Command |
| --- | --- |
| Minimal axum setup | `cargo run -p authkestra --example axum_basic_setup --all-features` |
| Minimal actix setup | `cargo run -p authkestra --example actix_basic_setup --all-features` |
| GitHub OAuth | `cargo run -p authkestra --example axum_oauth2_github --all-features` |
| Google OIDC | `cargo run -p authkestra --example axum_oidc_google --all-features` |
| Stateless OAuth (JWT callback) | `cargo run -p authkestra --example axum_oauth_stateless --all-features` |
| Redis-backed sessions | `cargo run -p authkestra --example axum_session_redis --all-features` |

## Where to go next

| Goal | Skill |
| --- | --- |
| Social login, or a custom provider | `authkestra-social-login` |
| Your own database behind the store traits | `authkestra-store` |
| Run your own identity provider | `authkestra-op` |
| Validate tokens at an API edge | `authkestra-resource-server` |
| Passkeys, TOTP, device-bound auth | `authkestra-mfa` |
| It compiles nowhere / it panics | `authkestra-features`, `authkestra-troubleshoot` |

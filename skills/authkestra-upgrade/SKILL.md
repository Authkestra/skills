---
name: authkestra-upgrade
description: Upgrade existing Authkestra codebases across breaking releases through v0.13 - migrating from dissolved RFC-001 crates (authkestra-core, authkestra-flow, authkestra-token, authkestra-session), removing dropped session/token Cargo features, and updating TOTP/WebAuthn enrolment calls to ReproofRequirement. Use when upgrading Authkestra versions or fixing deprecated imports.
---

# Upgrading Authkestra Through `v0.13`

Targets `authkestra` **0.13.x**.

## 1. Upgrading to `0.13` (From `0.8` – `0.12`)

### A. Remove `"session"` and `"token"` Cargo Features
In `0.13` (#375, #377), the `session` and `token` Cargo features were removed from both `authkestra` and `authkestra-engine` because session and token machinery are now compiled unconditionally into `authkestra-engine`.
- **Before (`<= 0.12`)**:
  ```toml
  authkestra = { version = "0.9", features = ["axum", "session", "token", "github"] }
  authkestra-engine = { version = "0.9", features = ["session", "memory"] }
  ```
- **After (`0.13`)**:
  ```toml
  authkestra = { version = "0.13", features = ["axum", "memory", "github"] }
  ```

### B. Update Passkey & TOTP Enrolment to `ReproofRequirement` (RFC-009)
`TotpAuthMethod::register_totp`, `WebAuthnAuthMethod::start_register`, `start_register_with_handle`, and `finish_register` changed signatures in `0.13` to enforce re-proof gating and bind the subject into `PasskeyEnrolment`:

- **TOTP Before**:
  ```rust
  let (secret, uri) = totp.register_totp(&user.id, "Acme", &user.email).await?;
  ```
- **TOTP After (`0.13`)**:
  ```rust
  use authkestra_engine::auth::{AuthMethod, ReproofRequirement};

  let gate = ReproofRequirement::new(300)
      .require_step_up(totp.has_enrolled(&session.identity.external_id).await?);
  let (secret, uri) = totp
      .register_totp(&session.identity, &gate, "Acme", &user.email)
      .await?;
  ```

- **WebAuthn Before**:
  ```rust
  let (challenge, reg_state) = webauthn.start_register(&user.id, &user.username)?;
  let passkey = webauthn.finish_register(&user.id, reg_response, reg_state).await?;
  ```
- **WebAuthn After (`0.13`)**:
  ```rust
  let gate = ReproofRequirement::new(300)
      .require_step_up(webauthn.has_enrolled(&session.identity.external_id).await?);
  let (challenge, enrolment) = webauthn.start_register(&session.identity, &gate, &user.username)?;
  let passkey = webauthn.finish_register(enrolment, reg_response).await?;
  ```

### C. `CredentialStore::delete_credential` Return Contract
If you implemented a custom `CredentialStore`, `delete_credential` now returns `Result<bool, AuthError>` where `Ok(true)` means **this specific call** removed the row (`rows_affected == 1`). Validate your store with `authkestra-store-testsuite`.

---

## 2. Migrating Pre-RFC-001 Crates (`<= 0.5`)

Four crates were dissolved and merged into `authkestra-engine`:

| Dissolved Crate | Replacement in `0.13` |
| --- | --- |
| `authkestra-core` | `authkestra_engine` (or `authkestra::core` via the facade) |
| `authkestra-flow` | `authkestra_engine::flow` (`OAuth2Flow`, `ClientCredentialsFlow`, `DeviceFlow`) |
| `authkestra-session` | `authkestra_engine::auth::session` + `authkestra_engine::store` |
| `authkestra-token` | `authkestra_engine::token` (`TokenManager`) |
| Generic `SqlKvStore` | Removed in Epic #289. Use `RedisStore` / `MemoryStore` for KV sessions, `SqlxCredentialStore` for credentials, and `authkestra_store_sqlx::SqlxOpStore` for OP tables. |

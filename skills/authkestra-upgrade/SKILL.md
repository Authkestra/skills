---
name: authkestra-upgrade
description: Upgrade existing Authkestra codebases across breaking releases through v0.15.1 - handling devsig profile v1, CredentialStore::claim_totp_step, SessionConfig::default CSPRNG keys, OP ClientRegistration allowed_audiences, Cedar PrincipalMapper colon encoding, dissolved RFC-001 crates, and ReproofRequirement enrolment. Use when upgrading Authkestra versions or fixing deprecated/broken APIs.
---

# Upgrading Authkestra Through `v0.15.1`

Targets `authkestra` **0.15.x** (`0.15.1`).

## 1. Upgrading from `0.13` / `0.14` to `0.15` (`0.15.1`)

### A. `authkestra-devsig` Profile v1 (`0.15.0` Breaking, #440)
- **Refusal wire format**: Adapter refusal bodies changed from plain text (`"key_not_bound"`) to JSON `{"error":"<code>"}` (`application/json`) plus a `WWW-Authenticate: DeviceSignature error="<code>"` header on `401`.
- **Strict `qsh` / `bdh` & `att` validation**: Stray `qsh` on a queryless request or `bdh` on a bodyless request is now refused (`query_mismatch` / `body_mismatch`); a non-object `att` claim returns `malformed` (was `device_not_active`); an empty credential header returns `missing_credential` (was `malformed`).
- **Config & Key Roles**: `VerifierConfig` gained `device_algs` (`.with_device_algs(...)`) and `VerifierConfig::profile_v1(issuers, audience)`. `DeviceIdentity::key_role()` inspects `att.role` (`KeyRole::UserPresent` vs `KeyRole::Ambient`).

### B. Implement `CredentialStore::claim_totp_step` (`0.15.0`, #432)
If you implemented a custom `CredentialStore` and serve TOTP, override `claim_totp_step(&self, user_id: &str, credential_id: &str, matched_step: u64) -> Result<bool, AuthError>` with atomic compare-and-advance semantics (`matched_step > last_used_step`). Inheriting the default causes TOTP logins to fail with `AuthError::Unsupported`. Ensure `save_credential` also replaces existing credentials sharing the same `credential_id`/`id`.

### C. Share `SessionConfig` Key Across Replicas (`0.15.0`, #431)
`SessionConfig::default()` now generates a fresh random 32-byte `state_encryption_key` on every call instead of a static literal. Create the config once and clone it across routes, or call `SessionConfig::new(shared_key)` when multiple instances or restarts must decrypt in-flight OAuth state cookies.

### D. `ClientRegistration::allowed_audiences` & `at+jwt` (`0.14.0` Breaking, #422)
Struct literals constructing `ClientRegistration` must include `allowed_audiences: Vec<String>` (and SQLx client inserts must populate the `allowed_audiences` column). When `allowed_audiences` is non-empty (for example `["api-a"]`), user access tokens set `aud` to the configured resource audience(s) instead of `client_id`. In addition, `0.14+` user access tokens always carry `typ: at+jwt` (RFC 9068) even when `allowed_audiences` is empty. Update resource-server validation that expects `client_id` in `aud` or `typ: JWT` before upgrading.

### E. `Guard` / `Auth<I>` Structured Refusals (`0.15.1`, #448)
`Auth<I>` now calls `Guard::authenticate_with_refusal` and returns RFC 6750 / 9449 / 9470 JSON responses with `WWW-Authenticate`, `DPoP-Nonce`, and `Cache-Control: no-store` instead of plain-text 401 strings. Tests asserting raw plain-text bodies on `Auth<I>` rejections should parse JSON `{"error": ...}` instead.

### F. Cedar `PolicyGuard` Entity Format (`0.15.0`, #434) & MCP Header Default
- `DefaultPrincipalMapper` / `format_principal_uid_str` formats Cedar principals as `User::"{provider}:{external_id}"` (`:` and `%` percent-encoded) rather than `User::"provider|id"`. Update any hardcoded Cedar policy literals.
- `McpRouter` now ignores `x-authkestra-identity` by default; pass verified extensions from auth middleware or call `.allow_unverified_identity_header(true)` in tests only.

---

## 2. Upgrading from `0.8` – `0.12` to `0.13+`

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

## 3. Migrating Pre-RFC-001 Crates (`<= 0.5`)

Four crates were dissolved and merged into `authkestra-engine`:

| Dissolved Crate | Replacement in `0.13` |
| --- | --- |
| `authkestra-core` | `authkestra_engine` (or `authkestra::core` via the facade) |
| `authkestra-flow` | `authkestra_engine::flow` (`OAuth2Flow`, `ClientCredentialsFlow`, `DeviceFlow`) |
| `authkestra-session` | `authkestra_engine::auth::session` + `authkestra_engine::store` |
| `authkestra-token` | `authkestra_engine::token` (`TokenManager`) |
| Generic `SqlKvStore` | Removed in Epic #289. Use `RedisStore` / `MemoryStore` for KV sessions, `SqlxCredentialStore` for credentials, and `authkestra_store_sqlx::SqlxOpStore` for OP tables. |

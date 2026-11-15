---
name: authkestra-mfa
description: Add Passkeys (WebAuthn), TOTP, Magic Links, Email/SMS OTP, Recovery Codes, Re-proof gating (ReproofRequirement), and device-bound request signatures (authkestra-devsig) to a Rust app with Authkestra. Use when implementing passwordless login, step-up MFA, factor enrolment, or proof-of-possession device signatures.
---

# Passwordless, Step-Up MFA, Re-Proof Gating & Device Signatures

Targets `authkestra` **0.15.x** (`0.15.1`).

## 1. Re-Proof Gating (`ReproofRequirement` — Mandatory for Enrolment in `0.13+`)

Enrolling a passkey, TOTP secret, or recovery code set requires `&Identity` and `&ReproofRequirement` instead of a bare `user_id: &str`. This prevents stolen/stale sessions from enrolling an attacker's factor and prevents confused-deputy bugs:

```rust
use authkestra::core::auth::{AuthMethod, ReproofRequirement};

// Require authentication within the last 300s, and step-up if the user already has this factor enrolled
let gate = ReproofRequirement::new(300)
    .require_step_up(method.has_enrolled(&session.identity.external_id).await?);
```

- **Note for tests**: An `Identity` constructed manually in a test must have `IDENTITY_ATTR_AUTH_TIME` set in `identity.attributes` (to a recent Unix timestamp string), or `gate.check(&identity)` fails closed with `ReproofFailure::FreshnessUnknown`!

## 2. Passkeys (`features = ["webauthn"]`)

```rust
use authkestra::core::auth::{webauthn::WebAuthnAuthMethod, AuthInput, ReproofRequirement};

let method = WebAuthnAuthMethod::new(Arc::new(webauthn), cred_store.clone());

// Registration (two-step ceremony; subject is bound inside `PasskeyEnrolment`)
let (challenge, enrolment) = method.start_register(&session.identity, &gate, "alice")?;
// Stash `enrolment` in server-side session, never trust client-supplied subject!
let passkey = method.finish_register(enrolment, register_response).await?;

// Authentication
let (req_challenge, auth_state) = engine.start_webauthn(&user_passkeys)?;
let result = engine.authenticate(AuthInput::WebAuthnAuthentication {
    user_id: "user_123".into(),
    credential_id,
    client_data_json,
    authenticator_data,
    signature,
    user_handle: None,
    auth_state_json: Some(serde_json::to_string(&auth_state)?),
}).await?;
```

## 3. TOTP (`features = ["totp"]`)

Register TOTP as step-up-only via `.with_mfa_method(TotpAuthMethod::new(cred_store))` (use `.with_totp(cred_store)` only if you want TOTP allowed as a primary factor):

```rust
use authkestra::core::auth::{totp::TotpAuthMethod, AuthInput};

let (secret_b32, otpauth_uri) = totp_method
    .register_totp(&session.identity, &gate, "MyApp", "alice@example.com")
    .await?;

// Completing an MfaRequired step-up challenge:
let result = engine.authenticate(AuthInput::MfaChallenge {
    mfa_token,
    challenge_input: Box::new(AuthInput::Totp {
        user_id: "user_123".into(),
        code: "123456".into(),
    }),
}).await?;
```

- **Replay & Rotation Rules (`0.15.0+` #432)**:
  - `TotpAuthMethod::authenticate` calls `CredentialStore::claim_totp_step` atomically on every valid login so a TOTP code cannot be replayed in the same 30s window. Any custom `CredentialStore` storing TOTP credentials **must** implement `claim_totp_step` (default returns `AuthError::Unsupported`).
  - `register_totp` aborts if retiring superseded TOTP credentials fails.
  - `MfaTokenClaims` binds `allowed_methods` and rejects MFA continuation using the same method as the primary factor.

## 4. Magic Links, Email/SMS OTP & Recovery Codes

- **Magic Links (`features = ["magic-link"]`)**:
  - `MagicLinkAuthMethod::new(kv_store)` where `kv_store: KvStore<MagicLinkRecord> + AtomicConsume<MagicLinkRecord>`.
  - `method.mint("user_123", Duration::from_secs(900), MagicLinkBinding::AnyContext)` (or `MagicLinkBinding::SameContext(cookie_nonce)`).
  - Verify with `AuthInput::MagicLink { token, binding }`.
- **Email/SMS OTP (`features = ["otp"]`)**:
  - `OtpAuthMethod::new(kv_store).with_resend_cooldown(Duration::from_secs(60))` where `kv_store` implements `KvStore<OtpChallenge> + AtomicConsume + AtomicDecrement + AtomicInsert`.
  - `method.mint("user_123", Duration::from_secs(300), OtpChannel::Email, 5)`. Note (`0.15.0+` #432): zero and sub-second TTLs are rejected, and attempt counters bind to a per-challenge UUID `challenge_id`.
  - Verify with `AuthInput::Otp { subject, code }`.
- **Recovery Codes (`features = ["recovery-codes"]`)**:
  - Factor-agnostic look-up secret authenticator (`RecoveryCodeAuthMethod::new(cred_store)`).
  - `method.generate(&session.identity, &gate, 10).await?` returns 10 single-use codes (`XXXX-XXXX-XXXX-XXXX`).
  - Redeem with `AuthInput::RecoveryCode { user_id, code }`.

## 5. Device-Bound Signatures (`features = ["devsig"]`, Profile v1 in `0.15+`)

`authkestra-devsig` verifies per-request proof-of-possession (`X-Signature` signed by the device private key over method, path, query hash `qsh`, and SHA-256 body digest `bdh`, paired with `X-Attestation` signed by `authkestra-op`).

In `0.15.0+` (#440), `devsig` implements **profile v1** (`docs/rfc-014-devsig-profile-v1.md`) with shared conformance vectors:

```rust
use authkestra::axum::devsig::{refusal_response, AuthDeviceSignature, DeviceSignatureLayer};
use authkestra::devsig::{DevSig, InMemoryReplayStore, IssuerJwks, KeyRole, Refusal, VerifierConfig};
use std::sync::Arc;

let devsig = DevSig::builder()
    .config(VerifierConfig::profile_v1(["https://op.example.com"], "https://api.example.com"))
    .jwks(Arc::new(IssuerJwks::new()))
    .replay_store(Arc::new(InMemoryReplayStore::new()))
    .build();
```

- **Key Roles & Step-Up**: `att` is a JSON object (`{"status": "active", "role": "user_present" | "ambient"}`). Check `identity.key_role()`: only exact `"user_present"` returns `KeyRole::UserPresent`; absent or unknown values fall back safely to `KeyRole::Ambient`. High-value handlers can reject ambient keys with `refusal_response(Refusal::insufficient_user_authentication())` (`401` + `WWW-Authenticate: DeviceSignature error="insufficient_user_authentication"`), or revoked devices with `refusal_response(Refusal::device_revoked())` (`403`).
- **Strict `qsh` / `bdh` Rule**: `qsh` and `bdh` must be present *if and only if* the request has a query or body — stray `qsh` or `bdh` is refused (`query_mismatch` / `body_mismatch`).
- **Wire Refusals**: All adapter refusals emit JSON `{"error":"<code>"}` (`application/json`) plus `WWW-Authenticate: DeviceSignature error="<code>"` on `401`.
- **Example (`authkestra-v0.15.1`)**: `crates/authkestra/examples/axum_devsig/main.rs` and `crates/authkestra/examples/axum_mfa_server.rs`.

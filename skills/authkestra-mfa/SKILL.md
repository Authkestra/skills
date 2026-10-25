---
name: authkestra-mfa
description: Add Passkeys (WebAuthn), TOTP, Magic Links, Email/SMS OTP, Recovery Codes, Re-proof gating (ReproofRequirement), and device-bound request signatures (authkestra-devsig) to a Rust app with Authkestra. Use when implementing passwordless login, step-up MFA, factor enrolment, or proof-of-possession device signatures.
---

# Passwordless, Step-Up MFA, Re-Proof Gating & Device Signatures

Targets `authkestra` **0.13.x**.

## 1. Re-Proof Gating (`ReproofRequirement` — Mandatory for Enrolment in `0.13+`)

Enrolling a passkey, TOTP secret, or recovery code set requires `&Identity` and `&ReproofRequirement` instead of a bare `user_id: &str`. This prevents stolen/stale sessions from enrolling an attacker's factor and prevents confused-deputy bugs:

```rust
use authkestra_engine::auth::{AuthMethod, ReproofRequirement};

// Require authentication within the last 300s, and step-up if the user already has this factor enrolled
let gate = ReproofRequirement::new(300)
    .require_step_up(method.has_enrolled(&session.identity.external_id).await?);
```

- **Note for tests**: An `Identity` constructed manually in a test must have `IDENTITY_ATTR_AUTH_TIME` set in `identity.attributes` (to a recent Unix timestamp string), or `gate.check(&identity)` fails closed with `ReproofFailure::FreshnessUnknown`!

## 2. Passkeys (`features = ["webauthn"]`)

```rust
use authkestra_engine::auth::{webauthn::WebAuthnAuthMethod, AuthInput, ReproofRequirement};

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
use authkestra_engine::auth::{totp::TotpAuthMethod, AuthInput};

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

## 4. Magic Links, Email/SMS OTP & Recovery Codes (`0.13+`)

- **Magic Links (`features = ["magic-link"]`)**:
  - `MagicLinkAuthMethod::new(kv_store)` where `kv_store: KvStore<MagicLinkRecord> + AtomicConsume<MagicLinkRecord>`.
  - `method.mint("user_123", Duration::from_secs(900), MagicLinkBinding::AnyContext)` (or `MagicLinkBinding::SameContext(cookie_nonce)`).
  - Verify with `AuthInput::MagicLink { token, binding }`.
- **Email/SMS OTP (`features = ["otp"]`)**:
  - `OtpAuthMethod::new(kv_store).with_resend_cooldown(Duration::from_secs(60))` where `kv_store` implements `KvStore<OtpChallenge> + AtomicConsume + AtomicDecrement + AtomicInsert`.
  - `method.mint("user_123", Duration::from_secs(300), OtpChannel::Email, 5)`.
  - Verify with `AuthInput::Otp { subject, code }`.
- **Recovery Codes (`features = ["recovery-codes"]`)**:
  - Factor-agnostic look-up secret authenticator (`RecoveryCodeAuthMethod::new(cred_store)`).
  - `method.generate(&session.identity, &gate, 10).await?` returns 10 single-use codes (`XXXX-XXXX-XXXX-XXXX`).
  - Redeem with `AuthInput::RecoveryCode { user_id, code }`.

## 5. Device-Bound Signatures (`features = ["devsig"]`)

`authkestra-devsig` verifies per-request proof-of-possession (`X-Signature` signed by the device/agent private key over method, path, query hash, and SHA-256 body digest `bdh`, paired with `X-Attestation` signed by `authkestra-op`):

```rust
use authkestra_axum::devsig::{AuthDeviceSignature, DeviceSignatureLayer};
use authkestra_devsig::{DevSig, InMemoryReplayStore, IssuerJwks, VerifierConfig};

let devsig = DevSig::builder()
    .config(VerifierConfig::new(
        ["https://op.example.com"],
        [jsonwebtoken::Algorithm::ES256, jsonwebtoken::Algorithm::EdDSA],
        Duration::from_secs(5),
        Duration::from_secs(60),
        "https://api.example.com",
    ))
    .jwks(Arc::new(IssuerJwks::new()))
    .replay_store(Arc::new(InMemoryReplayStore::new()))
    .build();
```

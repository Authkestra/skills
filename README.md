# Authkestra Agent Skills Pack

[![Agent Skills](https://img.shields.io/badge/agent%20skills-Authkestra%2Fskills-informational)](https://github.com/Authkestra/skills)
[![Landing Page](https://img.shields.io/badge/landing-authkestra.com-f4845f)](https://authkestra.com)
[![Documentation](https://img.shields.io/badge/docs-docs.authkestra.com-e85d2f)](https://docs.authkestra.com)
[![crates.io](https://img.shields.io/crates/v/authkestra.svg?color=ce422b)](https://crates.io/crates/authkestra)
[![Single-File Corpus](https://img.shields.io/badge/corpus-llms--full.txt-orange)](https://docs.authkestra.com/llms-full.txt)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/Authkestra/authkestra)

Official Claude Code and AI agent skill pack for integrating and orchestrating **[Authkestra](https://authkestra.com)** — The Compile-Time Safe Identity & AI Agent Authorization Engine for Rust.

Authkestra is an embedded composition library: nothing works until you have picked the right Cargo feature flags, satisfied the typestate builder (`Engine<S, T>`), implemented storage traits against your own database, and wired adapter extractors. These skills equip coding agents with exact architectural knowledge, eliminating hallucinations and runtime security pitfalls.

---

## 📦 Installation

### Claude Code Plugin Marketplace
```bash
/plugin marketplace add Authkestra/skills
/plugin install authkestra@authkestra
```

### Direct Agent Ingestion
The skills are structured as standard Markdown files with YAML frontmatter located in `skills/`. Point your agent directly at `skills/` or reference the single-file corpus:
- **LLM Context Summary**: [`https://authkestra.com/llms.txt`](https://authkestra.com/llms.txt)
- **Comprehensive Corpus**: [`https://docs.authkestra.com/llms-full.txt`](https://docs.authkestra.com/llms-full.txt)

---

## 🛠️ Included Skills

| Skill | Description & Key Topics |
| :--- | :--- |
| `authkestra-quickstart` | "Add auth to my Axum or Actix app." Feature flags, TLS backend selection, typestate builder setup, store choice, extractor wiring. |
| `authkestra-features` | "Why doesn't this compile?" Distinguishes missing Cargo features from unmet typestate builder prerequisites. |
| `authkestra-store` | "Use my own Postgres / Diesel / SeaORM store." Implements `KvStore`, `SessionStore`, `CredentialStore`, and `OpStore` validated via `authkestra-store-testsuite`. |
| `authkestra-social-login` | GitHub, Google, Discord, or custom OIDC providers, plus the stateless OAuth2 encrypted-cookie contract. |
| `authkestra-mcp` | Model Context Protocol tool security (`authkestra-mcp`), 3 security tiers (`Public`, `Scoped`, `HumanApproved`), RFC 8785 JCS parameter challenge digests, and `DownstreamTokenBroker` credential vaulting. |
| `authkestra-op` | "Be my own identity provider." Full OpenID Provider, RFC 8693 Token Exchange & audience binding, RFC 8628 Device Flow, DPoP (`with_dpop_nonces`), discovery/JWKS, `amr`/`acr`, and attestation issuance. |
| `authkestra-resource-server` | Token validation at the API edge: `Guard`, multi-issuer JWKS, RFC 9449 DPoP (`DPoP-Nonce`), RFC 9470 `StepUpRequirement`, RFC 8705 mTLS binding, Cedar ABAC (`PolicyGuard`), and CAEP/SSF (`PushReceiver`). |
| `authkestra-mfa` | FIDO2 Passkeys, TOTP (atomic `claim_totp_step`), Magic Links, Email/SMS OTP, Recovery Codes, mandatory `ReproofRequirement` enrolment gating, and `authkestra-devsig` profile v1 (`KeyRole`, JSON refusals). |
| `authkestra-upgrade` | Migrating code through `0.15.x` (`devsig` profile v1, `claim_totp_step`, `SessionConfig::default`, `allowed_audiences`) and from pre-0.13 crates (`RFC-001`). |
| `authkestra-troubleshoot` | Diagnosing runtime symptoms: `rustls` crypto provider panics, ephemeral `SessionConfig::default()` keys, `Unsupported` TOTP stores, `use_dpop_nonce`, clock skew, and `devsig` signature mismatches. |

---

## 🔄 Version Alignment

This pack's versioning directly tracks the core `authkestra` crate: **`skills` 0.15.x targets `authkestra` 0.15.x (`0.15.1`)**. Every skill states the version it was written against and cites examples in the core repository by git tag.

| Skills Pack | `authkestra` Crate Version |
| :--- | :--- |
| **0.15.x** | **0.15.x (`0.15.1`, Active)** |
| 0.13.x | 0.13.x |
| 0.7.x | 0.7.x |

---

## 🧪 Automated Verification

To prevent documentation drift, the `fixtures/` directory contains minimal applications that depend on `authkestra` directly from **crates.io**. CI builds these fixtures on every push and nightly, ensuring that published crate changes never break skill instructions.

---

## 🌐 Ecosystem Links

- **Authkestra Landing**: [authkestra.com](https://authkestra.com)
- **Documentation**: [docs.authkestra.com](https://docs.authkestra.com)
- **Interactive Playground**: [play.authkestra.com](https://play.authkestra.com)
- **Core Rust Workspace**: [github.com/Authkestra/authkestra](https://github.com/Authkestra/authkestra)
- **crates.io**: [crates.io/crates/authkestra](https://crates.io/crates/authkestra)

---

## 📜 License

Dual-licensed under **MIT** or **Apache-2.0**, at your option — matching Authkestra itself.

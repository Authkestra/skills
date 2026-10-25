---
name: authkestra-store
description: Implement or configure Authkestra storage traits (KvStore, AtomicConsume, AtomicDecrement, AtomicInsert, IndexedKvStore, SessionStore, CredentialStore, OpStore) for Postgres, MySQL, SQLite, Redis, SeaORM, or Diesel, and validate custom stores with authkestra-store-testsuite. Use when wiring database persistence in Authkestra.
---

# Authkestra Storage Traits & Custom Database Backends

Targets `authkestra` **0.13.x**.

Authkestra never enforces a user schema and **deliberately has no `UserStore` trait** — your application owns user and account tables. Authkestra persists only protocol and credential state through four families of traits.

## 1. Which Built-in Store Exists for What?

| State | Trait | Built-in Memory | Built-in Redis | Built-in SQL (`sqlx`) |
| --- | --- | --- | --- | --- |
| Sessions, Magic Links, OTP, Enrolment Challenges | `KvStore<T>` + atomicity traits (`SessionStore` is blanket-implemented for `KvStore<Session>`) | `MemoryStore<T>` (`features = ["memory"]`) | `RedisStore` (`features = ["redis"]`) | *None* (implement `KvStore` on your DB/ORM, or use Redis/Memory) |
| Passkeys, TOTP, Recovery Codes | `CredentialStore` | `MemoryCredentialStore` (`features = ["memory"]`) | — | `SqlxCredentialStore` (`features = ["sql-postgres" \| "sql-mysql" \| "sql-sqlite"]` on `authkestra`) |
| OIDC Provider (Clients, Auth Codes, Refresh Tokens, Device Codes, DPoP/Assertion JTIs) | `OpStore` (`CloneableOpStore`) | `CompositeOpStore` over `MemoryStore` | `authkestra_op::redis_store` (`features = ["op", "redis"]`) | `authkestra_store_sqlx::SqlxOpStore` (`authkestra-store-sqlx` crate) |

> **Trap to avoid:** There is no generic `SqlKvStore` in `authkestra-engine` (it was removed in Epic #289). SQL in the main crate is specifically `SqlxCredentialStore` (for `CredentialStore`) and `authkestra-store-sqlx`'s `SqlxOpStore` (for `OpStore`).

## 2. Implementing Custom `KvStore` + Atomicity Traits

When implementing storage against your own ORM (see `crates/authkestra-example-seaorm` and `crates/authkestra-example-diesel` at tag `authkestra-v0.13.0`), basic `get`/`set`/`delete` is **not** enough for single-use tokens or rate-limited OTPs:

1. **`KvStore<V>`**: `get(&self, key: &str)`, `set(&self, key: &str, value: V, ttl: Duration)`, `delete(&self, key: &str)`.
2. **`AtomicConsume<V>`** (Required for Magic Links, Auth Codes, PKCE states, Enrolment Challenges):
   - `consume(&self, key: &str) -> Result<Option<V>, StoreError>`
   - **MUST be atomic** (`DELETE FROM ... WHERE key = $1 AND expires_at > NOW() RETURNING value` in SQL, or `GETDEL` in Redis). Never implement `consume` as `get()` followed by `delete()` — concurrent requests (such as email link prefetchers) will race and both succeed.
3. **`AtomicDecrement<V>` & `AtomicInsert<V>`** (Required for `OtpAuthMethod`):
   - `AtomicDecrement` maintains the per-challenge attempt budget without read-modify-write races.
   - `AtomicInsert::insert_if_absent` enforces the per-subject resend cooldown.

## 3. Implementing `CredentialStore` (`0.13` Contract)

In `0.13`, `CredentialStore::delete_credential` tightened its return contract for single-use Recovery Codes:

```rust
#[async_trait::async_trait]
pub trait CredentialStore: Send + Sync {
    async fn save_credential(&self, user_id: &str, cred_type: &str, data: serde_json::Value) -> Result<(), AuthError>;
    async fn get_credentials(&self, user_id: &str, cred_type: &str) -> Result<Vec<serde_json::Value>, AuthError>;
    async fn update_credential(&self, credential_id: &str, data: serde_json::Value) -> Result<(), AuthError>;
    /// MUST return `Ok(true)` ONLY if *this specific call* removed the credential (`rows_affected == 1`).
    /// Two concurrent calls naming the same recovery code must result in one `Ok(true)` and one `Ok(false)`.
    async fn delete_credential(&self, user_id: &str, cred_type: &str, credential_id: &str) -> Result<bool, AuthError>;
    async fn delete_credentials(&self, user_id: &str, cred_type: &str) -> Result<u64, AuthError>;
}
```

## 4. Implementing `OpStore` (`Clone` Must Share State!)

`Op::builder().store(...)` takes `Arc<dyn CloneableOpStore>`, where `OpStore` methods take `&mut self`. Every incoming HTTP request clones the store handle.
- **Critical Rule:** `Clone` on your `OpStore` MUST clone a handle to shared state (like `sqlx::Pool`, `redis::Client`, or `Arc<RwLock<...>>`), **never** a deep copy of an in-memory `HashMap` (which would cause `/authorize` writes to vanish before `/token` reads them).
- **Replay Guards:** `OpStore::record_client_assertion_jti` and `OpStore::check_and_record_dpop_jti` default to **refusing every request**. Override them (or supply `.with_client_assertion_store(...)` and `.with_dpop_replay_store(...)` on `CompositeOpStore`) if you use `private_key_jwt` or DPoP.

## 5. Validating with `authkestra-store-testsuite`

Never ship a custom store without running the official conformance suite against it in your `#[tokio::test]` module:

```toml
[dev-dependencies]
authkestra-store-testsuite = "0.13"
```

It tests concurrent `consume` races, TTL expiry, `AtomicDecrement` under contention, and `CredentialStore::delete_credential` single-winner semantics.

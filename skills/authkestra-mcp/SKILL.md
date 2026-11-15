---
name: authkestra-mcp
description: Secure Model Context Protocol (MCP) servers and AI agent tools in Rust using authkestra-mcp — configuring McpRouter, Public/Scoped/HumanApproved security tiers, RFC 8785 JCS parameter-bound step-up approval, EncryptedKvTokenVault downstream credential brokering, RFC 9728 Protected Resource Metadata, and Cedar policy enforcement. Use when building an authenticated MCP server or securing AI tool calls with Authkestra.
---

# Securing AI Agent Tools with `authkestra-mcp`

Targets `authkestra` **0.15.x** (`0.15.1`).

Enable `features = ["axum", "mcp", "memory"]` (or `["actix", "mcp", "memory"]`) on `authkestra`, and add `"policy"` if you also want Cedar ABAC tool enforcement.

## 1. Three Composable Security Tiers (`McpSecurityTier`)

Every `ToolDefinition` declares one of three security tiers:

1. **`McpSecurityTier::Public`**: Callable without an authenticated `Identity` (still receives optional identity context if present).
2. **`McpSecurityTier::Scoped { required_scopes }`**: Requires an authenticated `Identity` whose scopes include every required scope; missing/insufficient scope returns JSON-RPC `RPC_UNAUTHORIZED` (`-32002`) plus an RFC 9728 `WWW-Authenticate` header.
3. **`McpSecurityTier::human_approved(required_scopes, ReproofRequirement, ttl_secs)`**: High-risk actions. Checks scopes first, then canonicalizes tool arguments via **RFC 8785 JSON Canonicalization Scheme (JCS)** and hashes them with SHA-256 (`compute_challenge_digest`) into a pending challenge. The human approves the exact digest via `GET/POST /mcp/approve`; the resulting single-use execution token is redeemed atomically via `AtomicConsume` (`execution_store`), and mutating any argument after approval is rejected with `RPC_PARAMETER_MISMATCH`.
   Clients must send the same `x-session-id` or `mcp-session-id` header from the initial call through approval redemption (otherwise `axum_mcp_router` generates a fresh UUID per request and the session comparison fails, consuming the execution token):
   - Initial `POST /mcp` (`tools/call`) with `x-session-id: <sid>` -> returns `RPC_STEP_UP_REQUIRED` (`-32001`) + approval URL
   - Human approval via `GET/POST /mcp/approve` (preserving the bound session context). **Security boundaries:**
     - `GET /mcp/approve` performs no identity or reproof check (anyone with `approval_id` can fetch `PendingApprovalChallenge`), and returns only `approval_id`, `session_id`, `tool_name`, `challenge_digest`, and timestamps/reproof metadata (not the original arguments or requester identity). Protect `GET /mcp/approve` with access control when pending challenge details must stay private, and retain trusted original request arguments + requester identity in your app, verify they match `challenge_digest`, and render them to the human before approval.
     - `POST /mcp/approve` only checks that a submitting `Identity` is present and satisfies `ReproofRequirement`; it does not verify human UI consent or check whether that identity may approve the caller's request. Applications must enforce approver authorization (role/ownership/separation-of-duties checks) and human confirmation around this endpoint so an agent or caller cannot self-approve.
   - Approved retry `POST /mcp` (`tools/call`) with the same `x-session-id: <sid>`, identical arguments, and the returned `execution_token` supplied either in the `x-authkestra-approval-token` HTTP header (always include `"arguments": {}` in `params`, even for no-argument tools, or the Axum adapter skips header injection) or as `_authkestra_approval_token` inside `arguments` -> executes the tool. **Identity binding:** `ApprovedExecutionRecord` binds the token to `session_id`, `tool_name`, and `challenge_digest`, not to the requesting `Identity`; applications must verify outside `McpRouter` that the retry's caller identity matches the identity that initiated approval.

```rust
use authkestra::core::auth::ReproofRequirement;
use authkestra::mcp::{McpSecurityTier, ToolCallResult, ToolContext, ToolDefinition};
use serde_json::json;

let transfer_tool = ToolDefinition::new(
    "execute_transfer",
    json!({
        "type": "object",
        "properties": {
            "recipient": { "type": "string" },
            "amount": { "type": "number" }
        },
        "required": ["recipient", "amount"]
    }),
    McpSecurityTier::human_approved(
        vec!["scope:transfer"],
        ReproofRequirement::new(60).require_step_up(true),
        120,
    ),
    |args, ctx: ToolContext| async move {
        let approver = ctx
            .approver_identity
            .map(|i| i.external_id)
            .unwrap_or_else(|| "unrecorded".to_string());
        Ok(ToolCallResult::text(format!(
            "Transferred {} to {} (approved by {approver})",
            args["amount"], args["recipient"]
        )))
    },
);
```

## 2. Zero-Trust Downstream Token Vault (`EncryptedKvTokenVault`)

Never pass downstream API tokens (GitHub, Stripe, internal APIs) into LLM prompts or tool arguments. Store them encrypted at rest with a 256-bit AES-GCM key in `EncryptedKvTokenVault` and resolve them inside the tool handler from the caller's verified `Identity`:

```rust
use authkestra::core::store::memory::MemoryStore;
use authkestra::mcp::{DownstreamTokenBroker, EncryptedKvTokenVault, McpError};
use std::sync::Arc;

let vault = Arc::new(EncryptedKvTokenVault::new(
    Arc::new(MemoryStore::new()),
    vault_encryption_key, // [u8; 32]
));

// Inside a tool handler:
let token = ctx
    .get_downstream_token("weather_service", &["weather:read"])
    .await
    .map_err(|e| McpError::Execution(format!("vault error: {e}")))?;
```

Tokens are namespaced by identity (`provider_id` + `external_id`) so cross-identity leakage is prevented (`0.15.0` #424).

## 3. Wiring `McpRouter`, RFC 9728 Metadata & Axum/Actix

`McpRouter::builder(pending_store, execution_store)` requires two `Arc<dyn AtomicConsume<_>>` stores so pending challenges and approved execution tokens cannot suffer TOCTOU replay races (`0.15.0` #434):

```rust
use authkestra::axum::mcp_axum_router;
use authkestra::core::store::memory::MemoryStore;
use authkestra::mcp::{McpRouter, ProtectedResourceMetadata};
use axum::Router;
use std::sync::Arc;

let metadata = ProtectedResourceMetadata::new("https://api.example.com/mcp")
    .with_auth_server("https://auth.example.com")
    .with_scopes(vec!["scope:read", "scope:transfer"]);

let router = McpRouter::builder(
    Arc::new(MemoryStore::new()),
    Arc::new(MemoryStore::new()),
)
.tool(transfer_tool)
.token_broker(vault)
.resource_metadata(metadata)
// Optional when `features = ["policy"]` is enabled:
// .with_policy_engine(cedar_engine)
.build();

// Mounts POST /mcp, GET /.well-known/oauth-protected-resource, and GET/POST /mcp/approve
let app = Router::new()
    .merge(mcp_axum_router::<Arc<McpRouter>>("/mcp"))
    .with_state(Arc::new(router));
```

On Actix-web, use `authkestra::actix::mcp_actix_scope("/mcp")` (or `McpExt::mcp_actix_scope`).

## 4. Crucial Security Default (`0.15.0+`): Verified Extensions Only

By default, `allow_unverified_identity_header` is **`false`**. The Axum/Actix MCP adapters resolve the caller's `Identity` strictly from verified request extensions (`Extension<Identity>`, `Extension<Arc<Identity>>`, or `Extension<AuthSession>`) populated by your auth middleware/extractor, and **ignore** raw `x-authkestra-identity` headers.

Only call `.allow_unverified_identity_header(true)` on `McpRouter::builder(...)` in isolated local development or test harnesses — never in production.

## 5. Reference Example (`authkestra-v0.15.1`)

- Full Axum MCP server (Public + Scoped + HumanApproved + Vault + RFC 9728): `crates/authkestra/examples/axum_mcp_server.rs`

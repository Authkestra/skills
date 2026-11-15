//! Compiles the v0.15.x APIs documented in authkestra-op, authkestra-resource-server,
//! authkestra-mcp, and authkestra-mfa (devsig profile v1) against crates.io.

use std::sync::Arc;

use authkestra::axum::{
    devsig::{refusal_response, AuthDeviceSignature, DeviceSignatureLayer},
    mcp_axum_router, resource_refusal_response, ssf_axum_router, Auth, AuthPolicyGuard,
    AxumState, OpExt,
};
use authkestra::core::auth::ReproofRequirement;
use authkestra::core::store::memory::MemoryStore;
use authkestra::core::token::dpop_nonce::{DpopNonces, DEFAULT_NONCE_WINDOW};
use authkestra::core::token::refusal::{AuthScheme, StepUpRequirement};
use authkestra::core::SessionConfig;
use authkestra::devsig::{
    DevSig, InMemoryReplayStore, IssuerJwks, KeyRole, Refusal, VerifierConfig,
};
use authkestra::mcp::{
    EncryptedKvTokenVault, McpRouter, McpSecurityTier, ProtectedResourceMetadata, ToolCallResult,
    ToolContext, ToolDefinition,
};
use authkestra::op::{
    client::{ClientRegistration, GrantType},
    config::OpConfig,
    store::CompositeOpStore,
    Op,
};
use authkestra::policy::{MemoryResourceLoader, PolicyEngine};
use authkestra::ssf::{InMemorySetReplayGuard, PushReceiver, SessionRevocationHandler, SetVerifier};
use authkestra::Authkestra;
use authkestra_resource::{jwt::JwtStrategy, jwt::ValidationConfig, Guard, PolicyGuard};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Debug, Deserialize)]
struct ApiPrincipal {
    sub: String,
}

#[derive(Clone, AxumState)]
#[authkestra(crate = ::authkestra::axum)]
struct ResourceState {
    #[authkestra(store)]
    guard: Arc<Guard<ApiPrincipal>>,
    #[authkestra(store)]
    policy_guard: Arc<PolicyGuard>,
}

fn main() {
    // 1. SessionConfig::new vs default CSPRNG key
    let _session_cfg = SessionConfig::new([7u8; 32]);

    // 2. OP + DPoP-Nonce + ClientRegistration.allowed_audiences
    let nonces = DpopNonces::new(b"0123456789abcdef0123456789abcdef", DEFAULT_NONCE_WINDOW);
    #[allow(deprecated)]
    let _client = ClientRegistration {
        client_id: "c1".into(),
        client_secret_hash: None,
        redirect_uris: vec!["http://127.0.0.1/cb".into()],
        require_pkce: true,
        scopes: vec!["openid".into()],
        grant_types: vec![GrantType::AuthorizationCode],
        allowed_audiences: vec!["https://api.example.com".into()],
        token_endpoint_auth_method: None,
        jwks: None,
    };
    let op_store = CompositeOpStore::new(
        MemoryStore::new(),
        MemoryStore::new(),
        MemoryStore::new(),
        MemoryStore::new(),
    )
    .with_dpop_nonces(nonces.clone());

    let engine = Authkestra::builder()
        .session_store(Arc::new(MemoryStore::new()))
        .jwt_secret(b"0123456789abcdef0123456789abcdef")
        .build();

    let op = Op::builder()
        .engine(engine)
        .config(OpConfig {
            issuer: "https://auth.example.com".into(),
            scopes_supported: vec!["openid".into()],
            response_types_supported: vec!["code".into()],
            grant_types_supported: vec!["authorization_code".into()],
            id_token_signing_alg: "EdDSA".into(),
            authorization_code_ttl_secs: 60,
            access_token_ttl_secs: 3600,
            device_code_ttl_secs: 600,
            token_exchange_enabled: false,
        })
        .store(Arc::new(op_store))
        .build();

    let _op_router: Router<()> = Router::new()
        .merge(op.op_axum_router())
        .with_state(authkestra::axum::op::OpState(op));

    // 3. Resource server + DPoP-Nonce + StepUpRequirement + Cedar PolicyGuard + SSF
    let validation = ValidationConfig::builder()
        .jwks_url("https://auth.example.com/jwks.json")
        .issuer("https://auth.example.com")
        .audiences(["https://api.example.com"])
        .require_kid(true)
        .build();
    let strategy = JwtStrategy::<ApiPrincipal>::new(validation)
        .with_dpop_nonces(nonces)
        .with_step_up(
            StepUpRequirement::new()
                .with_acr_values("urn:mfa")
                .with_max_age(300),
        );
    let guard = Arc::new(Guard::builder().strategy(strategy).build());

    let policy_engine = Arc::new(
        PolicyEngine::builder()
            .policies("permit(principal, action, resource);")
            .loader(MemoryResourceLoader::new())
            .build()
            .expect("valid cedar policy"),
    );
    let policy_guard = Arc::new(PolicyGuard::new(policy_engine.clone()));

    let verifier = SetVerifier::builder("https://auth.example.com")
        .audience("https://api.example.com/ssf")
        .replay_guard(Arc::new(InMemorySetReplayGuard::new(
            std::time::Duration::from_secs(3600),
        )))
        .build()
        .expect("valid set verifier");
    let ssf_receiver = Arc::new(
        PushReceiver::new(Arc::new(verifier))
            .with_handler(Arc::new(SessionRevocationHandler::new(Arc::new(
                MemoryStore::new(),
            )))),
    );

    let _api_router: Router<()> = Router::new()
        .route("/protected", get(protected))
        .merge(ssf_axum_router(ssf_receiver))
        .with_state(ResourceState {
            guard,
            policy_guard,
        });

    // 4. DevSig Profile v1
    let devsig = DevSig::builder()
        .config(VerifierConfig::profile_v1(
            ["https://auth.example.com"],
            "https://api.example.com",
        ))
        .jwks(Arc::new(IssuerJwks::new()))
        .replay_store(Arc::new(InMemoryReplayStore::new()))
        .build();
    let _devsig_router: Router<()> = Router::new()
        .route("/signed", post(signed_action))
        .layer(DeviceSignatureLayer::from(devsig));

    // 5. MCP Router + EncryptedKvTokenVault + PolicyEngine
    let vault = Arc::new(EncryptedKvTokenVault::new(
        Arc::new(MemoryStore::new()),
        [1u8; 32],
    ));
    let tool = ToolDefinition::new(
        "transfer",
        json!({"type": "object"}),
        McpSecurityTier::human_approved(
            vec!["scope:transfer"],
            ReproofRequirement::new(60).require_step_up(true),
            120,
        ),
        |_args, _ctx: ToolContext| async { Ok(ToolCallResult::text("ok")) },
    );
    let mcp = McpRouter::builder(Arc::new(MemoryStore::new()), Arc::new(MemoryStore::new()))
        .tool(tool)
        .token_broker(vault)
        .resource_metadata(ProtectedResourceMetadata::new(
            "https://api.example.com/mcp",
        ))
        .with_policy_engine(policy_engine)
        .build();
    let _mcp_router: Router<()> = Router::new()
        .merge(mcp_axum_router::<Arc<McpRouter>>("/mcp"))
        .with_state(Arc::new(mcp));
}

async fn protected(
    Auth(principal): Auth<ApiPrincipal>,
    AuthPolicyGuard(_policy): AuthPolicyGuard,
) -> impl IntoResponse {
    let req = StepUpRequirement::new().with_max_age(60);
    let _ = req.check_values(Some("urn:mfa"), Some(1_700_000_000), AuthScheme::Bearer);
    let _ = resource_refusal_response;
    principal.sub
}

async fn signed_action(AuthDeviceSignature(id): AuthDeviceSignature) -> impl IntoResponse {
    if id.key_role() != KeyRole::UserPresent {
        return refusal_response(Refusal::insufficient_user_authentication());
    }
    axum::http::StatusCode::OK.into_response()
}

//! Compiles the code `authkestra-quickstart` tells an integrator to write.
//!
//! Evidence, not a demo: it proves the facade paths, the feature set and the
//! typestate builder in the skill are the real ones. It never listens on a
//! port.

use std::sync::Arc;

use authkestra::axum::{AuthSession, AxumError, AxumExt, AxumState};
use authkestra::core::{AkWebAppEngine, OAuth2Flow, SessionStore};
use authkestra::providers::github::GithubProvider;
use authkestra::store::memory::MemoryStore;
use authkestra::Authkestra;

#[derive(Clone, AxumState)]
#[authkestra(crate = ::authkestra::axum)]
struct AppState {
    #[authkestra(engine)]
    auth: AkWebAppEngine,
}

fn main() {
    let provider = GithubProvider::new(
        "client-id".to_string(),
        "client-secret".to_string(),
        "http://localhost:3000/auth/callback/github".to_string(),
    );

    let session_store: Arc<dyn SessionStore> = Arc::new(MemoryStore::default());

    // `create_session` does not exist until `session_store` is supplied. That
    // is the typestate, and it is the claim this fixture pins.
    let engine = Authkestra::builder()
        .session_store(session_store)
        .provider(OAuth2Flow::new(provider))
        .build();

    let _app: axum::Router = axum::Router::new()
        .merge(engine.axum_router())
        .with_state(AppState {
            auth: engine.clone(),
        });
}

/// The extractor an integrator reaches for first.
#[allow(dead_code)]
async fn me(session: Result<AuthSession, AxumError>) -> String {
    match session {
        Ok(AuthSession(s)) => s.identity.external_id,
        Err(_) => "anonymous".to_string(),
    }
}

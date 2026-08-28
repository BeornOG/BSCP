pub mod admin;
pub mod auth;
pub mod calls;
pub mod chats;
pub mod federation;
pub mod invites;
pub mod misc;
pub mod uploads;
pub mod users;
pub mod webhooks;

use crate::state::AppState;
use axum::extract::DefaultBodyLimit;
use axum::Router;
use tower_http::trace::TraceLayer;

const MAX_BODY_BYTES: usize = 1024 * 1024 * 1024; // 1 GiB

pub fn build(state: AppState) -> Router {
    #[allow(unused_mut)]
    let mut router: Router<AppState> = Router::new()
        .merge(auth::router())
        .merge(users::router())
        .merge(chats::router())
        .merge(uploads::router())
        .merge(invites::router())
        .merge(webhooks::router())
        .merge(admin::router())
        .merge(federation::router())
        .merge(calls::router())
        .merge(misc::router());

    #[cfg(debug_assertions)]
    {
        router = crate::openapi::mount(router);
    }

    let router = router.fallback(misc::spa_fallback);

    #[cfg(debug_assertions)]
    let router = router.layer(tower_http::cors::CorsLayer::permissive());

    router
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

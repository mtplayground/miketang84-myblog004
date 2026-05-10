use axum::{Router, routing::get};
use tower_http::trace::TraceLayer;

pub fn app() -> Router {
    Router::new()
        .route("/", get(healthcheck))
        .layer(TraceLayer::new_for_http())
}

async fn healthcheck() -> &'static str {
    "myblog004 is running"
}

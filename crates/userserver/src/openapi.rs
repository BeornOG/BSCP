//! Swagger UI / OpenAPI document — mounted only in debug builds.

use crate::state::AppState;
use axum::Router;

pub fn mount(router: Router<AppState>) -> Router<AppState> {
    let spec: serde_json::Value =
        serde_json::from_str(include_str!("../openapi.json")).expect("valid openapi.json");
    let swagger = utoipa_swagger_ui::SwaggerUi::new("/api/docs")
        .external_url_unchecked("/api/docs/openapi.json", spec);
    router.merge(swagger)
}

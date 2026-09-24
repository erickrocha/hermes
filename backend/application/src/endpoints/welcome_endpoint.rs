use axum::response::{IntoResponse, Redirect};

#[utoipa::path(
  get,
  path = "/",
  responses(
    (status = 200, description = "Welcome message", body = String)
  )
)]
pub async fn welcome() -> impl IntoResponse {
  Redirect::to("/swagger-ui/index.html")
}

#[utoipa::path(
  get,
  path = "/",
  responses(
    (status = 200, description = "Welcome message", body = String)
  )
)]
pub async fn welcome() -> String {
    "Hermes application!".to_string()
}
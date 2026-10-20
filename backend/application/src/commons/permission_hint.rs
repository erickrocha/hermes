use axum::body::{Body, to_bytes};
use axum::extract::{MatchedPath, Request};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use business::domain::permission;

/// `HRMS-859` (`TRM-891`): a refusal names the permission the operation asks for -- a `permission` field in
/// the 403 body and the same code at the end of the message -- so the person knows what to ask the owner for.
/// Runs inside the router (`Router::layer`), where the matched route template is known.
pub async fn name_missing_permission(method: Method, path: Option<MatchedPath>, request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    // An unmatched route (the router's fallback) has no template.
    if response.status() != StatusCode::FORBIDDEN || path.is_none() {
        return response;
    }
    let Some(code) = path.and_then(|p| permission::for_operation(method.as_str(), p.as_str())) else {
        return response;
    };
    let (parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, 64 * 1024).await else {
        return Response::from_parts(parts, Body::empty());
    };
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    // An endpoint that already named a finer permission than the route's (`ExceptionResponse::ForbiddenPermission`) keeps it.
    if json.get("permission").is_some() {
        return Response::from_parts(parts, Body::from(bytes));
    }
    if let Some(object) = json.as_object_mut() {
        if let Some(message) = object.get("message").and_then(|m| m.as_str()) {
            let named = format!("{message} ({code})");
            object.insert("message".into(), named.into());
        }
        object.insert("permission".into(), code.into());
    }
    let body = serde_json::to_vec(&json).unwrap_or_else(|_| bytes.to_vec());
    let mut parts = parts;
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(body))
}

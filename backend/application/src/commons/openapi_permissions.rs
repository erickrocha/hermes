use business::domain::permission;
use utoipa::openapi::OpenApi;
use utoipa::openapi::path::Operation;

/// `EPIC-AC-02-S04` (`HRMS-856`): every operation of the published contract says which catalogue permission it
/// asks for, next to who may reach it (`**Roles:**`). The permission comes from the one operation table the 403
/// naming (`HRMS-859`) already reads, so the document and the refusal can never name different permissions.
/// A public operation (login, refresh, accept-invite) asks for none and says so.
pub fn with_permissions(mut openapi: OpenApi) -> OpenApi {
    for (path, item) in openapi.paths.paths.iter_mut() {
        for (method, operation) in [
            ("GET", &mut item.get),
            ("PUT", &mut item.put),
            ("POST", &mut item.post),
            ("DELETE", &mut item.delete),
        ] {
            if let Some(operation) = operation {
                append_permission(operation, permission::for_operation(method, path));
            }
        }
    }
    openapi
}

fn append_permission(operation: &mut Operation, code: Option<&'static str>) {
    let statement = match code {
        Some(code) => format!("**Permission:** `{code}`"),
        None => "**Permission:** none (a public operation, or one no profile can grant)".to_string(),
    };
    let described = match operation.description.take() {
        Some(text) if !text.trim().is_empty() => format!("{text}\n\n{statement}"),
        _ => statement,
    };
    operation.description = Some(described);
}

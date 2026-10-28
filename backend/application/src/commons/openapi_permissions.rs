use business::domain::permission;
use utoipa::openapi::OpenApi;
use utoipa::openapi::path::Operation;

/// `EPIC-AC-02-S04` (`HRMS-856`): every operation of the published contract says which catalogue permission it
/// asks for, next to who may reach it (`**Roles:**`). The permission comes from the one operation table the 403
/// naming (`HRMS-859`) already reads, so the document and the refusal can never name different permissions.
/// A public operation (login, refresh, accept-invite) asks for none and says so.
///
/// `C-035`: the permission is what decides; the `**Roles:**` line names the system profiles that hold it by
/// default, and the statement below spells those out from the catalogue so the two cannot drift.
pub fn with_permissions(mut openapi: OpenApi) -> OpenApi {
    let note = "Access is decided by permissions (`C-035`): a caller reaches an operation when their effective \
                grants -- their access profile's, their own extra permissions and the baseline -- hold the \
                operation's **Permission**, inside their tenant. **Roles:** lines name the built-in system \
                profiles (`SysAdmin`, `TenantOwner`, `TenantUser`, `Driver`, `Mechanic`) that hold it by default; \
                any profile may be granted it. A grant written `code@own` reaches the caller's own records only.";
    openapi.info.description = Some(match openapi.info.description.take() {
        Some(text) if !text.trim().is_empty() => format!("{text}\n\n{note}"),
        _ => note.to_string(),
    });
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
        Some(code) => {
            let holders = default_holders(code);
            format!("**Permission:** `{code}` -- held by default by {holders}")
        }
        None => "**Permission:** none (a public operation, or one every signed-in account reaches)".to_string(),
    };
    let described = match operation.description.take() {
        Some(text) if !text.trim().is_empty() => format!("{text}\n\n{statement}"),
        _ => statement,
    };
    operation.description = Some(described);
}

/// The system profiles whose default grants hold `code`, as prose.
fn default_holders(code: &str) -> String {
    let Some(entry) = permission::find(code) else {
        return "no built-in profile".to_string();
    };
    match entry.scope {
        permission::Scope::Baseline => "every signed-in account".to_string(),
        permission::Scope::Platform => "the platform administrator only".to_string(),
        permission::Scope::Tenant => {
            let mut names = vec!["the platform administrator", "the tenant owner"];
            names.extend(permission::default_for(code).into_iter().map(|key| match key {
                permission::TENANT_USER => "the tenant user",
                permission::DRIVER => "the driver",
                permission::MECHANIC => "the mechanic",
                other => other,
            }));
            let own = permission::default_for(code).iter().any(|key| {
                permission::Grants::from_codes(permission::system_grants(key)).only_own(code)
            });
            let mut text = names.join(", ");
            if own {
                text.push_str(" (over own records where the profile says `@own`)");
            }
            text
        }
    }
}

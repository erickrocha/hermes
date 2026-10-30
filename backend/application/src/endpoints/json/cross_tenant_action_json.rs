use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `C-036` (`HRMS-877`): one request a tenant made inside a descendant of its own.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrossTenantActionJson {
    pub uuid: Option<String>,
    pub actor_user_id: i64,
    pub actor_email: String,
    /// The acting tenant -- the ancestor.
    pub actor_tenant_id: i64,
    /// The tenant the request acted in -- the descendant.
    pub affected_tenant_id: i64,
    /// `read` for a safe method, `write` for any other.
    pub kind: String,
    pub method: String,
    /// The route template, e.g. `/vehicle/uuid/{uuid}`.
    pub operation: String,
    /// The concrete path, which names the record.
    pub path: String,
    pub occurred_at: chrono::NaiveDateTime,
}

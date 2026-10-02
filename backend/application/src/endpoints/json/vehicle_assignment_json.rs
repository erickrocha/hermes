use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// EPIC-FO-03-S03 (HRMS-933): an assignment as the API shows it -- by UUID,
/// never by sequential id (HRMS-204/AD-010).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehicleAssignmentJson {
    pub uuid: Option<String>,
    pub vehicle_uuid: Option<String>,
    pub driver_uuid: Option<String>,
    pub driver_name: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Absent while the assignment is live (D-23(d): at most one per vehicle).
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// `POST /vehicle/uuid/{uuid}/assignment`.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignDriverRequest {
    /// A user of the vehicle's own tenant holding the `Driver` role (HRMS-932).
    pub driver_uuid: String,
}

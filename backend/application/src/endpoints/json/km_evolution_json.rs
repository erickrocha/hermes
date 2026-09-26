use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-CK-01-S01` (`HRMS-650`): a kilometre-evolution entry's HTTP shape.
/// `recordedByUuid` is the user who took the reading, resolved at the
/// endpoint; omitted means the caller took it themselves.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KmEvolutionJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub km: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<chrono::NaiveDateTime>,
    /// One of `InitialRegistration`, `Manual`, `WorkOrder`,
    /// `DriverChecklist`, `Garage`, `TechnicalInspection`, `Adjustment`
    /// (`TRM-151`).
    pub origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_entity_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_by_uuid: Option<String>,
}

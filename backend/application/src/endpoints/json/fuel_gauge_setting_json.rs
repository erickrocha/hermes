use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-FU-06-S03` (`HRMS-953`): a tenant's tank-gauge thresholds, the legacy
/// `[TC]` values being only the defaults. On `GET`, `uuid` is absent while the
/// tenant is still on the defaults.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelGaugeSettingJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    /// Default 1.05 (`TRM-596`): implied use above this multiple of the capacity is `Suspect`.
    pub suspect_margin_ratio: f64,
    /// Default 8 (`TRM-589`): litres the distance must imply before a fuelling is judged a top-off.
    pub set_aside_min_expected_liters: f64,
    /// Default 0.5 (`TRM-589`): a full tank below this share of the implied litres is a top-off.
    pub set_aside_ratio: f64,
    /// Default 0.5 (`TRM-592`): a fuelling of at least this share of the capacity is never set aside.
    pub large_fuelling_ratio: f64,
}

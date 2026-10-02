use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-FU-01-S01` (`HRMS-942`): a vehicle's fuelling, manual entry only.
/// `origin` is server-derived (`Manual`) and ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelEntryJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub vehicle_uuid: String,
    pub recorded_at: NaiveDateTime,
    pub volume_liters: f64,
    pub value_cents: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odometer_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    #[serde(default)]
    pub full_tank: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// `EPIC-FU-02-S01` (`HRMS-944`): what one `POST /fuel-entry/sync` call did.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelSyncOutcomeJson {
    pub fetched: usize,
    pub imported: usize,
    pub updated: usize,
    pub reconciled: usize,
    pub unmatched: usize,
}

/// `EPIC-FU-08-S01` (`HRMS-945`): the fuelling report -- rows plus the totals
/// `TRM-1553` names.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelReportJson {
    pub entries: Vec<FuelEntryJson>,
    pub total_liters: f64,
    pub total_value_cents: i64,
}

/// `EPIC-FU-05-S01` (`HRMS-946`, `TRM-563…568`): a vehicle's own consumption
/// average. `kmPerLiter` is absent until `segments` reaches 3 (`mature`), so a
/// short history never presents a confidently wrong figure.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsumptionAverageJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub km_per_liter: Option<f64>,
    pub segments: usize,
    pub mature: bool,
    /// `EPIC-FU-06-S04` (`TRM-569`/`570`): the consumption to use for this vehicle -- its own once
    /// mature, else its registered reference, else the median of comparable peers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_km_per_liter: Option<f64>,
    /// One of `Own`, `Reference`, `PeerMedian`; anything but `Own` is an estimate (`TRM-571`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_source: Option<String>,
}

/// `EPIC-FU-03-S01` (`HRMS-948`, `TRM-520…543`): a fuelling the driver has
/// **confirmed on screen** from a receipt (`TRM-521`). `fullTank` has no
/// default (`TRM-537`). `driverUuid` is required for an owner or mechanic
/// reporting on a driver's behalf and ignored for a driver, who reports as
/// themselves. `overrideOdometer` is the driver's explicit confirmation of a
/// reading outside its neighbours (`TRM-540`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelReceiptJson {
    pub vehicle_uuid: String,
    pub vehicle_prefix: String,
    pub recorded_at: NaiveDateTime,
    pub volume_liters: f64,
    pub value_cents: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odometer_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    pub full_tank: bool,
    #[serde(default)]
    pub override_odometer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_uuid: Option<String>,
}

/// `EPIC-FU-04-S02` (`HRMS-950`, `TRM-554`): unify two fuellings of one vehicle
/// and day. The target is kept and absorbs the source.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelUnifyJson {
    pub target_uuid: String,
    pub source_uuid: String,
}

/// `EPIC-FU-06-S02` (`HRMS-952`, `TRM-580`/`581`/`597`): a vehicle's estimated
/// tank level. Either the reading (`percent`, `remainingLiters`, `rangeKm`,
/// `consumptionKmPerLiter`, `anchoredAt`) or the `reason` there is none.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TankGaugeJson {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_liters: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumption_km_per_liter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchored_at: Option<NaiveDateTime>,
    /// `TRM-571`: true when the consumption behind the reading is a reference or peer estimate.
    #[serde(default)]
    pub estimated: bool,
    /// `TRM-589`: fuellings flagged full that were judged top-offs and credited as partials.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_aside_at: Vec<NaiveDateTime>,
    /// One of `NoTankRegistered`, `NoReferenceFuelling`, `AwaitingCalibration`, `Suspect`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

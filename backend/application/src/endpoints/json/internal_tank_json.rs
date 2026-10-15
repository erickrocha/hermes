use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// `EPIC-FU-07-S01` (`HRMS-943`): the operator's own diesel tank, at most
/// one per tenant. `PUT` upserts the caller's own row.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InternalTankJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub capacity_liters: f64,
    pub reference_stock_liters: f64,
    pub reference_at: NaiveDateTime,
    pub alert_level_liters: f64,
    pub reserve_level_liters: f64,
}

/// Lets an **unbound** `SysAdmin` name which tenant's singleton row a bare
/// `GET /internal-tank` means -- see `InternalTankGateway::find_current`'s
/// own doc comment. Ignored for a tenant-bound caller, whose own tenant
/// always wins.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct InternalTankQuery {
    pub tenant_id: Option<i64>,
}

/// `EPIC-FU-07-S02` (`HRMS-947`, `TRM-1541…1544`): the tank's current stock.
/// `currentStockLiters` is absent when `suppressed` says why (`TRM-1543`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TankStockJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_stock_liters: Option<f64>,
    pub delivery_alert: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppressed: Option<String>,
}

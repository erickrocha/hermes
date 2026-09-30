use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::internal_tank_json::InternalTankQuery;
use crate::endpoints::json::tenant_rule_setting_json::TenantRuleSettingJson;
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::domain::authorization::can_configure_fuel_gauge;
use business::domain::tenant_rule_setting::{RuleSettings, TenantRuleSetting};
use business::domain::user::User;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::use_cases::tenant_rule_setting_use_case::TenantRuleSettingUseCase;

fn use_case(state: &AppState) -> TenantRuleSettingUseCase {
    TenantRuleSettingUseCase::new(TenantRuleSettingGateway::new(state.conn.as_ref().clone()))
}

fn json(tenant_id: Option<i64>, uuid: Option<String>, s: RuleSettings) -> TenantRuleSettingJson {
    TenantRuleSettingJson {
        uuid,
        tenant_id,
        avg_max_segment_km: s.avg_max_segment_km,
        avg_min_km_per_liter: s.avg_min_km_per_liter,
        avg_max_km_per_liter: s.avg_max_km_per_liter,
        avg_recent_window_days: s.avg_recent_window_days,
        avg_fallback_segments: s.avg_fallback_segments,
        avg_median_min_segments: s.avg_median_min_segments,
        avg_median_tolerance: s.avg_median_tolerance,
        avg_median_min_survivors: s.avg_median_min_survivors,
        avg_min_segments_for_own_average: s.avg_min_segments_for_own_average,
        receipt_duplicate_volume_tolerance_liters: s.receipt_duplicate_volume_tolerance_liters,
        receipt_duplicate_window_minutes: s.receipt_duplicate_window_minutes,
        reconciliation_window_hours: s.reconciliation_window_hours,
        reconciliation_volume_tolerance_liters: s.reconciliation_volume_tolerance_liters,
        reconciliation_volume_tolerance_ratio: s.reconciliation_volume_tolerance_ratio,
        preventive_attention_km_margin: s.preventive_attention_km_margin,
        preventive_attention_days_margin: s.preventive_attention_days_margin,
        preventive_max_extension_km: s.preventive_max_extension_km,
        avg_peer_capacity_tolerance: s.avg_peer_capacity_tolerance,
        avg_peer_min_count: s.avg_peer_min_count,
        garage_validity_hours: s.garage_validity_hours,
        garage_min_trip_absence_minutes: s.garage_min_trip_absence_minutes,
        garage_fuel_pending_below_percent: s.garage_fuel_pending_below_percent,
        garage_trip_fuel_exempt_percent: s.garage_trip_fuel_exempt_percent,
        garage_alert_low_percent: s.garage_alert_low_percent,
        garage_alert_trip_percent: s.garage_alert_trip_percent,
    }
}

#[utoipa::path(
    put,
    tag = "TenantRuleSetting",
    path = "/tenant-rule-settings",
    request_body = TenantRuleSettingJson,
    responses(
        (status = 200, description = "The caller's own tenant's rule thresholds are set -- created on first use, updated in place after. They replace the legacy defaults for that tenant alone: the fuel consumption average, the receipt and reconciliation windows, and the preventive-plan margins. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = TenantRuleSettingJson),
        (status = 400, description = "A non-positive value, a share outside its range, or a minimum km/l not below the maximum", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not configure rule thresholds", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn configure(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(p): Json<TenantRuleSettingJson>,
) -> HttpResponse<Json<TenantRuleSettingJson>> {
    if !can_configure_fuel_gauge(&current_user, p.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelGaugeSettingsForbidden));
    }
    let tenant_id = current_user.tenant_id.or(p.tenant_id);
    let settings = RuleSettings {
        avg_max_segment_km: p.avg_max_segment_km,
        avg_min_km_per_liter: p.avg_min_km_per_liter,
        avg_max_km_per_liter: p.avg_max_km_per_liter,
        avg_recent_window_days: p.avg_recent_window_days,
        avg_fallback_segments: p.avg_fallback_segments,
        avg_median_min_segments: p.avg_median_min_segments,
        avg_median_tolerance: p.avg_median_tolerance,
        avg_median_min_survivors: p.avg_median_min_survivors,
        avg_min_segments_for_own_average: p.avg_min_segments_for_own_average,
        receipt_duplicate_volume_tolerance_liters: p.receipt_duplicate_volume_tolerance_liters,
        receipt_duplicate_window_minutes: p.receipt_duplicate_window_minutes,
        reconciliation_window_hours: p.reconciliation_window_hours,
        reconciliation_volume_tolerance_liters: p.reconciliation_volume_tolerance_liters,
        reconciliation_volume_tolerance_ratio: p.reconciliation_volume_tolerance_ratio,
        preventive_attention_km_margin: p.preventive_attention_km_margin,
        preventive_attention_days_margin: p.preventive_attention_days_margin,
        preventive_max_extension_km: p.preventive_max_extension_km,
        avg_peer_capacity_tolerance: p.avg_peer_capacity_tolerance,
        avg_peer_min_count: p.avg_peer_min_count,
        garage_validity_hours: p.garage_validity_hours,
        garage_min_trip_absence_minutes: p.garage_min_trip_absence_minutes,
        garage_fuel_pending_below_percent: p.garage_fuel_pending_below_percent,
        garage_trip_fuel_exempt_percent: p.garage_trip_fuel_exempt_percent,
        garage_alert_low_percent: p.garage_alert_low_percent,
        garage_alert_trip_percent: p.garage_alert_trip_percent,
    };
    match use_case(&state).configure(TenantRuleSetting::new(tenant_id, settings)).await {
        Ok(saved) => Ok(Json(json(saved.tenant_id, saved.uuid.clone(), saved.settings()))),
        Err(_) => Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)),
    }
}

#[utoipa::path(
    get,
    tag = "TenantRuleSetting",
    path = "/tenant-rule-settings",
    params(InternalTankQuery),
    responses(
        (status = 200, description = "The caller's own tenant's rule thresholds, or the legacy defaults (no `uuid`) while it has set none. `tenantId` names the tenant for an unbound `SysAdmin`. **Roles:** any authenticated role; own tenant only.", body = TenantRuleSettingJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_current(
    state: State<AppState>,
    Query(query): Query<InternalTankQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<TenantRuleSettingJson>> {
    let target = current_user.tenant_id.or(query.tenant_id);
    match use_case(&state).current(target).await {
        Ok(settings) => Ok(Json(json(target, None, settings))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

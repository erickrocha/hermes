use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::fuel_gauge_setting_json::FuelGaugeSettingJson;
use crate::endpoints::json::internal_tank_json::InternalTankQuery;
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::domain::authorization::can_configure_fuel_gauge;
use business::domain::fuel_gauge_setting::FuelGaugeSetting;
use business::domain::user::User;
use business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway;
use business::use_cases::fuel_gauge_setting_use_case::FuelGaugeSettingUseCase;
use business::use_cases::fuel_gauge_use_case::GaugeSettings;

fn use_case(state: &AppState) -> FuelGaugeSettingUseCase {
    FuelGaugeSettingUseCase::new(FuelGaugeSettingGateway::new(state.conn.as_ref().clone()))
}

fn json(tenant_id: Option<i64>, uuid: Option<String>, s: GaugeSettings) -> FuelGaugeSettingJson {
    FuelGaugeSettingJson {
        uuid,
        tenant_id,
        suspect_margin_ratio: s.suspect_margin_ratio,
        set_aside_min_expected_liters: s.set_aside_min_expected_liters,
        set_aside_ratio: s.set_aside_ratio,
        large_fuelling_ratio: s.large_fuelling_ratio,
    }
}

#[utoipa::path(
    put,
    tag = "FuelGaugeSetting",
    path = "/fuel-gauge-settings",
    request_body = FuelGaugeSettingJson,
    responses(
        (status = 200, description = "The caller's own tenant's tank-gauge thresholds are set -- created on first use, updated in place after. They apply to every vehicle's gauge of the tenant. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = FuelGaugeSettingJson),
        (status = 400, description = "A suspect margin below 1, a negative litres floor, or a share outside (0, 1]", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not configure the gauge", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn configure(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<FuelGaugeSettingJson>,
) -> HttpResponse<Json<FuelGaugeSettingJson>> {
    if !can_configure_fuel_gauge(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelGaugeSettingsForbidden));
    }
    let tenant_id = current_user.tenant_id.or(payload.tenant_id);
    let setting = FuelGaugeSetting {
        id: None,
        uuid: None,
        tenant_id,
        suspect_margin_ratio: payload.suspect_margin_ratio,
        set_aside_min_expected_liters: payload.set_aside_min_expected_liters,
        set_aside_ratio: payload.set_aside_ratio,
        large_fuelling_ratio: payload.large_fuelling_ratio,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    match use_case(&state).configure(setting).await {
        Ok(saved) => {
            let settings = GaugeSettings {
                suspect_margin_ratio: saved.suspect_margin_ratio,
                set_aside_min_expected_liters: saved.set_aside_min_expected_liters,
                set_aside_ratio: saved.set_aside_ratio,
                large_fuelling_ratio: saved.large_fuelling_ratio,
            };
            Ok(Json(json(saved.tenant_id, saved.uuid, settings)))
        }
        Err(_) => Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)),
    }
}

#[utoipa::path(
    get,
    tag = "FuelGaugeSetting",
    path = "/fuel-gauge-settings",
    params(InternalTankQuery),
    responses(
        (status = 200, description = "The caller's own tenant's tank-gauge thresholds, or the platform defaults (no `uuid`) while it has set none. `tenantId` names the tenant for an unbound `SysAdmin`. **Roles:** any authenticated role; own tenant only.", body = FuelGaugeSettingJson),
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
) -> HttpResponse<Json<FuelGaugeSettingJson>> {
    let target = current_user.tenant_id.or(query.tenant_id);
    match use_case(&state).current(target).await {
        Ok(settings) => Ok(Json(json(target, None, settings))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

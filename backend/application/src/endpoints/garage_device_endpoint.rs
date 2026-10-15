use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::garage_device_json::{
    GarageDeviceCredentialJson, GarageDeviceJson, GarageDevicePairingCodeJson,
    IssueGarageDeviceJson, PairGarageDeviceJson,
};
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use business::domain::business_error::BusinessError;
use business::domain::user::User;
use business::gateway::garage_device_gateway::GarageDeviceGateway;
use business::use_cases::garage_device_use_case::{
    DEVICE_CREDENTIAL_INVALID, DEVICE_NAME_REQUIRED, DEVICE_NOT_FOUND, DEVICE_OWNER_REQUIRED,
    GarageDeviceUseCase, INVALID_PAIRING_CODE,
};

fn failure(locale: Locale, error: BusinessError) -> ExceptionResponse {
    match error.message.as_str() {
        DEVICE_OWNER_REQUIRED => {
            ExceptionResponse::Forbidden(locale, ErrorKey::DeviceManagementForbidden)
        }
        DEVICE_NAME_REQUIRED => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        DEVICE_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::GarageDeviceNotFound),
        INVALID_PAIRING_CODE | DEVICE_CREDENTIAL_INVALID => {
            ExceptionResponse::Unauthorized(locale, ErrorKey::BadCredentials)
        }
        _ => ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError),
    }
}

fn use_case(state: &AppState) -> GarageDeviceUseCase<GarageDeviceGateway> {
    GarageDeviceUseCase::new(GarageDeviceGateway::new(state.conn.as_ref().clone()))
}

#[utoipa::path(
    get,
    tag = "GarageDevice",
    path = "/garage-device",
    responses(
        (status = 200, description = "The caller tenant's wall display devices. **Roles:** TenantOwner only.", body = [GarageDeviceJson]),
        (status = 403, description = "Only a tenant owner may manage devices", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(owner): Extension<User>,
) -> HttpResponse<Json<Vec<GarageDeviceJson>>> {
    use_case(&state)
        .list(&owner)
        .await
        .map(|devices| Json(devices.into_iter().map(GarageDeviceJson::from).collect()))
        .map_err(|error| failure(locale, error))
}

#[utoipa::path(
    post,
    tag = "GarageDevice",
    path = "/garage-device/pairing-code",
    request_body = IssueGarageDeviceJson,
    responses(
        (status = 201, description = "A short-lived, one-time code to pair a wall display. **Roles:** TenantOwner only.", body = GarageDevicePairingCodeJson),
        (status = 400, description = "Invalid device name", body = crate::endpoints::json::error_response_json::BadRequestErrorJson),
        (status = 403, description = "Only a tenant owner may manage devices", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn issue_pairing_code(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(owner): Extension<User>,
    Json(payload): Json<IssueGarageDeviceJson>,
) -> HttpResponse<(StatusCode, Json<GarageDevicePairingCodeJson>)> {
    use_case(&state)
        .issue_pairing_code(&owner, payload.name)
        .await
        .map(|pairing| (StatusCode::CREATED, Json(pairing.into())))
        .map_err(|error| failure(locale, error))
}

#[utoipa::path(
    post,
    tag = "GarageDevice",
    path = "/garage-device/uuid/{uuid}/pairing-code",
    params(("uuid" = String, Path, description = "Public device uuid")),
    responses(
        (status = 201, description = "A replacement one-time pairing code. Previous credentials are invalidated. **Roles:** TenantOwner only.", body = GarageDevicePairingCodeJson),
        (status = 404, description = "Device not found", body = crate::endpoints::json::error_response_json::NotFoundErrorJson),
        (status = 403, description = "Only a tenant owner may manage devices", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn reissue_pairing_code(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(owner): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<(StatusCode, Json<GarageDevicePairingCodeJson>)> {
    use_case(&state)
        .reissue_pairing_code(&owner, uuid)
        .await
        .map(|pairing| (StatusCode::CREATED, Json(pairing.into())))
        .map_err(|error| failure(locale, error))
}

#[utoipa::path(
    post,
    tag = "GarageDevice",
    path = "/garage-device/pair",
    request_body = PairGarageDeviceJson,
    responses(
        (status = 201, description = "The device's long-lived read-only credential. **Roles:** public; the one-time pairing code authorizes this call.", body = GarageDeviceCredentialJson),
        (status = 401, description = "Pairing code is invalid or expired", body = crate::endpoints::json::error_response_json::UnauthorizedErrorJson)
    )
)]
pub async fn pair(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Json(payload): Json<PairGarageDeviceJson>,
) -> HttpResponse<(StatusCode, Json<GarageDeviceCredentialJson>)> {
    use_case(&state)
        .pair(payload.code)
        .await
        .map(|credential| (StatusCode::CREATED, Json(credential.into())))
        .map_err(|error| failure(locale, error))
}

#[utoipa::path(
    delete,
    tag = "GarageDevice",
    path = "/garage-device/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Public device uuid")),
    responses(
        (status = 204, description = "Device credential revoked. **Roles:** TenantOwner only."),
        (status = 404, description = "Device not found", body = crate::endpoints::json::error_response_json::NotFoundErrorJson),
        (status = 403, description = "Only a tenant owner may manage devices", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn revoke(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(owner): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<StatusCode> {
    use_case(&state)
        .revoke(&owner, uuid)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(|error| failure(locale, error))
}

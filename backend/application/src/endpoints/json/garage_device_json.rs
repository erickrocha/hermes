use business::domain::garage_device::GarageDevice;
use business::use_cases::garage_device_use_case::{DeviceCredential, PairingCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueGarageDeviceJson {
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PairGarageDeviceJson {
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageDeviceJson {
    pub uuid: String,
    pub name: String,
    pub paired: bool,
    pub revoked: bool,
    pub paired_at: Option<String>,
    pub revoked_at: Option<String>,
}

impl From<GarageDevice> for GarageDeviceJson {
    fn from(device: GarageDevice) -> Self {
        Self {
            uuid: device.uuid.unwrap_or_default(),
            name: device.name,
            paired: device.paired_at.is_some() && device.revoked_at.is_none(),
            revoked: device.revoked_at.is_some(),
            paired_at: device
                .paired_at
                .map(|value| value.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            revoked_at: device
                .revoked_at
                .map(|value| value.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageDevicePairingCodeJson {
    pub device_uuid: String,
    pub device_name: String,
    pub code: String,
    pub expires_at: String,
}

impl From<PairingCode> for GarageDevicePairingCodeJson {
    fn from(pairing: PairingCode) -> Self {
        Self {
            device_uuid: pairing.device_uuid,
            device_name: pairing.device_name,
            code: pairing.code,
            expires_at: pairing.expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageDeviceCredentialJson {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: i64,
    pub device_uuid: String,
    pub device_name: String,
}

impl From<DeviceCredential> for GarageDeviceCredentialJson {
    fn from(credential: DeviceCredential) -> Self {
        Self {
            access_token: credential.access_token,
            token_type: credential.token_type,
            expires_at: credential.expires_at,
            device_uuid: credential.device_uuid,
            device_name: credential.device_name,
        }
    }
}

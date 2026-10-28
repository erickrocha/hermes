use crate::domain::access_token::{DeviceClaims, PrincipalType};
use crate::domain::business_error::BusinessError;
use crate::domain::permission::GARAGE_DEVICE_MANAGE;
use crate::domain::garage_device::GarageDevice;
use crate::domain::user::User;
use crate::gateway::garage_device_gateway::GarageDeviceStore;
use chrono::{Duration, NaiveDateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use std::env;
use uuid::Uuid;

const PAIRING_CODE_MINUTES: i64 = 10;
const DEVICE_CREDENTIAL_DAYS: i64 = 365;

pub const DEVICE_NAME_REQUIRED: &str = "A device name is required";
pub const DEVICE_NOT_FOUND: &str = "Garage device not found";
pub const INVALID_PAIRING_CODE: &str = "Pairing code is invalid or expired";
pub const DEVICE_NOT_PAIRED: &str = "Garage device is not paired";
pub const DEVICE_CREDENTIAL_INVALID: &str = "Device credential is invalid or revoked";
pub const DEVICE_OWNER_REQUIRED: &str = "Only a tenant account holding garage.device.manage may manage garage devices";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode {
    pub device_uuid: String,
    pub device_name: String,
    pub code: String,
    pub expires_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCredential {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: i64,
    pub device_uuid: String,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePrincipal {
    pub id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub name: String,
}

pub struct GarageDeviceUseCase<S: GarageDeviceStore> {
    store: S,
}

impl<S: GarageDeviceStore> GarageDeviceUseCase<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn issue_pairing_code(
        &self,
        owner: &User,
        name: String,
    ) -> Result<PairingCode, BusinessError> {
        let tenant_id = tenant_owner_id(owner)?;
        let name = name.trim();
        if name.is_empty() || name.len() > 120 {
            return Err(BusinessError::new(DEVICE_NAME_REQUIRED.to_string()));
        }
        let now = Utc::now();
        let expires_at = now + Duration::minutes(PAIRING_CODE_MINUTES);
        let code = new_pairing_code();
        let device = self
            .store
            .save(GarageDevice {
                id: None,
                uuid: None,
                tenant_id: Some(tenant_id),
                name: name.to_string(),
                credential_version: 1,
                pairing_code: Some(code.clone()),
                pairing_code_expires_at: Some(expires_at.naive_utc()),
                paired_at: None,
                revoked_at: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await?;
        Ok(pairing_code(&device, code, expires_at.naive_utc()))
    }

    pub async fn reissue_pairing_code(
        &self,
        owner: &User,
        uuid: String,
    ) -> Result<PairingCode, BusinessError> {
        tenant_owner_id(owner)?;
        let current = self.find_by_uuid(uuid).await?;
        let now = Utc::now();
        let expires_at = now + Duration::minutes(PAIRING_CODE_MINUTES);
        let code = new_pairing_code();
        let device = self
            .store
            .save(GarageDevice {
                credential_version: next_version(current.credential_version)?,
                pairing_code: Some(code.clone()),
                pairing_code_expires_at: Some(expires_at.naive_utc()),
                paired_at: None,
                revoked_at: None,
                ..current
            })
            .await?;
        Ok(pairing_code(&device, code, expires_at.naive_utc()))
    }

    pub async fn pair(&self, code: String) -> Result<DeviceCredential, BusinessError> {
        let current = self
            .store
            .find_by_pairing_code(code.clone())
            .await?
            .ok_or_else(|| BusinessError::new(INVALID_PAIRING_CODE.to_string()))?;
        let now = Utc::now().naive_utc();
        if current.paired_at.is_some()
            || current.revoked_at.is_some()
            || current
                .pairing_code_expires_at
                .is_none_or(|expiry| expiry <= now)
        {
            return Err(BusinessError::new(INVALID_PAIRING_CODE.to_string()));
        }
        let id = current
            .id
            .filter(|id| *id > 0)
            .ok_or_else(|| BusinessError::new(DEVICE_NOT_FOUND.to_string()))?;
        let uuid = current
            .uuid
            .clone()
            .ok_or_else(|| BusinessError::new(DEVICE_NOT_FOUND.to_string()))?;
        let tenant_id = current
            .tenant_id
            .ok_or_else(|| BusinessError::new(DEVICE_NOT_FOUND.to_string()))?;
        let expires_at = (Utc::now() + Duration::days(DEVICE_CREDENTIAL_DAYS)).timestamp();
        let access_token = mint_device_token(&current, id, &uuid, tenant_id, expires_at)?;
        let saved = self
            .store
            .consume_pairing_code(code, now)
            .await?
            .ok_or_else(|| BusinessError::new(INVALID_PAIRING_CODE.to_string()))?;
        Ok(DeviceCredential {
            access_token,
            token_type: "Bearer".to_string(),
            expires_at,
            device_uuid: uuid,
            device_name: saved.name,
        })
    }

    pub async fn validate_credential(
        &self,
        token: String,
    ) -> Result<DevicePrincipal, BusinessError> {
        let secret = access_token_secret()?;
        let claims = decode::<DeviceClaims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::new(Algorithm::HS512),
        )
        .map_err(|_| BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()))?
        .claims;
        if claims.principal_type != PrincipalType::Device
            || claims.device_id <= 0
            || claims.credential_version <= 0
        {
            return Err(BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()));
        }
        let device = self
            .store
            .find_for_authentication(claims.device_id)
            .await?
            .ok_or_else(|| BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()))?;
        if device.id != Some(claims.device_id)
            || device.uuid.as_deref() != Some(claims.device_uuid.as_str())
            || device.tenant_id != Some(claims.tenant_id)
            || device.credential_version != claims.credential_version
            || device.paired_at.is_none()
            || device.revoked_at.is_some()
        {
            return Err(BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()));
        }
        Ok(DevicePrincipal {
            id: claims.device_id,
            uuid: claims.device_uuid,
            tenant_id: claims.tenant_id,
            name: device.name,
        })
    }

    pub async fn list(&self, owner: &User) -> Result<Vec<GarageDevice>, BusinessError> {
        tenant_owner_id(owner)?;
        self.store.list().await
    }

    pub async fn revoke(&self, owner: &User, uuid: String) -> Result<(), BusinessError> {
        tenant_owner_id(owner)?;
        let current = self.find_by_uuid(uuid).await?;
        if current.revoked_at.is_some() {
            return Ok(());
        }
        self.store
            .save(GarageDevice {
                credential_version: next_version(current.credential_version)?,
                revoked_at: Some(Utc::now().naive_utc()),
                pairing_code: None,
                pairing_code_expires_at: None,
                ..current
            })
            .await?;
        Ok(())
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<GarageDevice, BusinessError> {
        self.store
            .find_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(DEVICE_NOT_FOUND.to_string()))
    }
}

fn tenant_owner_id(owner: &User) -> Result<i64, BusinessError> {
    if owner.is_platform() || !owner.allows(GARAGE_DEVICE_MANAGE) {
        return Err(BusinessError::new(DEVICE_OWNER_REQUIRED.to_string()));
    }
    owner
        .tenant_id
        .filter(|id| *id > 0)
        .ok_or_else(|| BusinessError::new(DEVICE_OWNER_REQUIRED.to_string()))
}

fn next_version(current: i64) -> Result<i64, BusinessError> {
    current
        .checked_add(1)
        .ok_or_else(|| BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()))
}

/// Eight digits, so it can be typed on a TV remote. The 10-minute lifetime and single use bound guessing.
fn new_pairing_code() -> String {
    format!("{:08}", Uuid::new_v4().as_u128() % 100_000_000)
}

fn pairing_code(device: &GarageDevice, code: String, expires_at: NaiveDateTime) -> PairingCode {
    PairingCode {
        device_uuid: device.uuid.clone().unwrap_or_default(),
        device_name: device.name.clone(),
        code,
        expires_at,
    }
}

fn mint_device_token(
    device: &GarageDevice,
    id: i64,
    uuid: &str,
    tenant_id: i64,
    expires_at: i64,
) -> Result<String, BusinessError> {
    let claims = DeviceClaims {
        sub: format!("device:{uuid}"),
        exp: expires_at,
        device_id: id,
        device_uuid: uuid.to_string(),
        tenant_id,
        credential_version: device.credential_version,
        principal_type: PrincipalType::Device,
    };
    let secret = access_token_secret()?;
    encode(
        &Header::new(Algorithm::HS512),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()))
}

fn access_token_secret() -> Result<String, BusinessError> {
    env::var("ACCESS_TOKEN_SECRET")
        .map_err(|_| BusinessError::new(DEVICE_CREDENTIAL_INVALID.to_string()))
}

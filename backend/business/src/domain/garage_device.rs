use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::garage_device_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// A wall monitor's registered identity. The credential is never stored;
/// `credential_version` invalidates every token issued before a revoke/re-pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarageDevice {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub credential_version: i64,
    pub pairing_code: Option<String>,
    pub pairing_code_expires_at: Option<NaiveDateTime>,
    pub paired_at: Option<NaiveDateTime>,
    pub revoked_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageDeviceEntityMapper;

impl EntityMapper<GarageDevice, Model, ActiveModel> for GarageDeviceEntityMapper {
    fn build_active_model(device: GarageDevice) -> ActiveModel {
        ActiveModel {
            id: device.id.map_or(NotSet, Set),
            uuid: device
                .uuid
                .map(|uuid| Set(string_to_bytes(&uuid)))
                .unwrap_or(NotSet),
            tenant_id: Set(device.tenant_id),
            name: Set(device.name),
            credential_version: Set(device.credential_version),
            pairing_code: Set(device.pairing_code),
            pairing_code_expires_at: set_utc(device.pairing_code_expires_at),
            paired_at: set_utc(device.paired_at),
            revoked_at: set_utc(device.revoked_at),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(model: Model) -> GarageDevice {
        GarageDevice {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            tenant_id: model.tenant_id,
            name: model.name,
            credential_version: model.credential_version,
            pairing_code: model.pairing_code,
            pairing_code_expires_at: model.pairing_code_expires_at.map(|value| value.naive_utc()),
            paired_at: model.paired_at.map(|value| value.naive_utc()),
            revoked_at: model.revoked_at.map(|value| value.naive_utc()),
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
        }
    }

    fn from_active_model(mut active: ActiveModel) -> GarageDevice {
        use sea_orm::TryIntoModel;
        match active.clone().try_into_model() {
            Ok(model) => Self::from_model(model),
            Err(_) => GarageDevice {
                id: active.id.take(),
                uuid: active.uuid.take().map(bytes_para_string),
                tenant_id: active.tenant_id.take().flatten(),
                name: active.name.take().unwrap_or_default(),
                credential_version: active.credential_version.take().unwrap_or(1),
                pairing_code: active.pairing_code.take().flatten(),
                pairing_code_expires_at: active
                    .pairing_code_expires_at
                    .take()
                    .flatten()
                    .map(|value| value.naive_utc()),
                paired_at: active
                    .paired_at
                    .take()
                    .flatten()
                    .map(|value| value.naive_utc()),
                revoked_at: active
                    .revoked_at
                    .take()
                    .flatten()
                    .map(|value| value.naive_utc()),
                created_at: active.created_at.take().map(|value| value.naive_utc()),
                created_by: active.created_by.take().flatten(),
                updated_at: active.updated_at.take().map(|value| value.naive_utc()),
                updated_by: active.updated_by.take().flatten(),
            },
        }
    }
}

fn set_utc(
    value: Option<NaiveDateTime>,
) -> sea_orm::ActiveValue<Option<chrono::DateTime<chrono::Utc>>> {
    Set(value.map(|value| value.and_utc()))
}

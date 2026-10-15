use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::fuel_gauge_setting_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-FU-06-S03` (`HRMS-953`): a tenant's tank-gauge thresholds. At most
/// one per tenant.
#[derive(Debug, Clone, PartialEq)]
pub struct FuelGaugeSetting {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub suspect_margin_ratio: f64,
    pub set_aside_min_expected_liters: f64,
    pub set_aside_ratio: f64,
    pub large_fuelling_ratio: f64,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct FuelGaugeSettingEntityMapper {}

impl EntityMapper<FuelGaugeSetting, Model, ActiveModel> for FuelGaugeSettingEntityMapper {
    fn build_active_model(d: FuelGaugeSetting) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            tenant_id: Set(d.tenant_id),
            suspect_margin_ratio: Set(d.suspect_margin_ratio),
            set_aside_min_expected_liters: Set(d.set_aside_min_expected_liters),
            set_aside_ratio: Set(d.set_aside_ratio),
            large_fuelling_ratio: Set(d.large_fuelling_ratio),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> FuelGaugeSetting {
        FuelGaugeSetting {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            suspect_margin_ratio: e.suspect_margin_ratio,
            set_aside_min_expected_liters: e.set_aside_min_expected_liters,
            set_aside_ratio: e.set_aside_ratio,
            large_fuelling_ratio: e.large_fuelling_ratio,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> FuelGaugeSetting {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => FuelGaugeSetting {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                suspect_margin_ratio: e.suspect_margin_ratio.take().unwrap_or_default(),
                set_aside_min_expected_liters: e.set_aside_min_expected_liters.take().unwrap_or_default(),
                set_aside_ratio: e.set_aside_ratio.take().unwrap_or_default(),
                large_fuelling_ratio: e.large_fuelling_ratio.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

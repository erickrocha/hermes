use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, normalize_name, string_to_bytes};
use crate::domain::enums::GarageServiceGroup;
use chrono::NaiveDateTime;
use entity::garage_service_model_entity::{ActiveModel, Model};
use serde::{Deserialize, Serialize};
use sea_orm::{NotSet, Set};
use std::str::FromStr;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "scope", content = "values", rename_all = "camelCase")]
pub enum GarageServiceApplicability {
    All,
    VehicleTypes(Vec<String>),
    Vehicles(Vec<String>),
}

impl Default for GarageServiceApplicability {
    fn default() -> Self {
        Self::All
    }
}

impl GarageServiceApplicability {
    pub fn applies_to(&self, vehicle_type: Option<&str>, vehicle_uuid: &str) -> bool {
        match self {
            Self::All => true,
            Self::VehicleTypes(types) => vehicle_type.is_some_and(|vehicle_type| {
                let vehicle_type = normalize_name(vehicle_type);
                types.iter().any(|candidate| normalize_name(candidate) == vehicle_type)
            }),
            Self::Vehicles(uuids) => uuids.iter().any(|candidate| candidate.eq_ignore_ascii_case(vehicle_uuid)),
        }
    }
}

/// `EPIC-GA-01-S01` (`HRMS-956`): a garage service the tenant's yard performs
/// (`TRM-430`).
#[derive(Debug, Clone, PartialEq)]
pub struct GarageServiceModel {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub name_key: String,
    pub display_order: i32,
    pub active: bool,
    pub service_group: GarageServiceGroup,
    pub required_for_departure: bool,
    pub governed_by_tank: bool,
    pub applicability: GarageServiceApplicability,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageServiceModelEntityMapper {}

impl EntityMapper<GarageServiceModel, Model, ActiveModel> for GarageServiceModelEntityMapper {
    fn build_active_model(d: GarageServiceModel) -> ActiveModel {
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
            name: Set(d.name),
            name_key: Set(d.name_key),
            display_order: Set(d.display_order),
            active: Set(d.active),
            service_group: Set(d.service_group.to_string()),
            required_for_departure: Set(d.required_for_departure),
            governed_by_tank: Set(d.governed_by_tank),
            applicability: Set(serde_json::to_string(&d.applicability).unwrap_or_else(|_| {
                "{\"scope\":\"vehicles\",\"values\":[]}".to_string()
            })),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> GarageServiceModel {
        GarageServiceModel {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            name: e.name,
            name_key: e.name_key,
            display_order: e.display_order,
            active: e.active,
            service_group: GarageServiceGroup::from_str(&e.service_group).unwrap_or(GarageServiceGroup::External),
            required_for_departure: e.required_for_departure,
            governed_by_tank: e.governed_by_tank,
            applicability: serde_json::from_str(&e.applicability)
                .unwrap_or_else(|_| GarageServiceApplicability::Vehicles(Vec::new())),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> GarageServiceModel {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => GarageServiceModel {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                name: e.name.take().unwrap_or_default(),
                name_key: e.name_key.take().unwrap_or_default(),
                display_order: e.display_order.take().unwrap_or_default(),
                active: e.active.take().unwrap_or_default(),
                service_group: e
                    .service_group
                    .take()
                    .and_then(|v| GarageServiceGroup::from_str(&v).ok())
                    .unwrap_or(GarageServiceGroup::External),
                required_for_departure: e.required_for_departure.take().unwrap_or_default(),
                governed_by_tank: e.governed_by_tank.take().unwrap_or_default(),
                applicability: e
                    .applicability
                    .take()
                    .and_then(|value| serde_json::from_str(&value).ok())
                    .unwrap_or_else(|| GarageServiceApplicability::Vehicles(Vec::new())),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GarageServiceApplicability;

    #[test]
    fn all_services_apply_to_every_vehicle() {
        assert!(GarageServiceApplicability::All.applies_to(None, "vehicle-1"));
    }

    #[test]
    fn vehicle_type_matching_uses_the_existing_free_text_type() {
        let applicability = GarageServiceApplicability::VehicleTypes(vec!["Ônibus Urbano".into()]);
        assert!(applicability.applies_to(Some("onibus urbano"), "vehicle-1"));
        assert!(!applicability.applies_to(Some("Van"), "vehicle-1"));
        assert!(!applicability.applies_to(None, "vehicle-1"));
    }

    #[test]
    fn explicit_vehicle_matching_uses_the_stable_uuid() {
        let applicability = GarageServiceApplicability::Vehicles(vec!["ABC-123".into()]);
        assert!(applicability.applies_to(Some("Van"), "abc-123"));
        assert!(!applicability.applies_to(Some("Van"), "other-vehicle"));
    }
}

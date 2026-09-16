use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use entity::city_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set, TryIntoModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct City {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub province_id: i64,
    pub name: String,
}

pub struct CityEntityMapper {}

impl EntityMapper<City, Model, ActiveModel> for CityEntityMapper {
    fn build_active_model(d: City) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            province_id: Set(d.province_id),
            name: Set(d.name),
        }
    }

    fn from_model(e: Model) -> City {
        City {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            province_id: e.province_id,
            name: e.name,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> City {
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => City {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                province_id: e.province_id.take().unwrap_or_default(),
                name: e.name.take().unwrap_or_default(),
            },
        }
    }
}

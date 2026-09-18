use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use entity::province_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set, TryIntoModel};

/// ISO 3166-1 alpha-2, normalizado. Vive aqui e não no endpoint porque desde
/// PD-027 há dois chamadores — a consulta por país e a gravação/importação —
/// e duas cópias da mesma regra divergem.
pub fn normalize_country_code(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_uppercase();
    (normalized.len() == 2 && normalized.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .then_some(normalized)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Province {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub acronym: String,
    pub name: String,
    pub country_code: String,
}

pub struct ProvinceEntityMapper {}

impl EntityMapper<Province, Model, ActiveModel> for ProvinceEntityMapper {
    fn build_active_model(d: Province) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            acronym: Set(d.acronym),
            name: Set(d.name),
            country_code: Set(d.country_code),
        }
    }

    fn from_model(e: Model) -> Province {
        Province {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            acronym: e.acronym,
            name: e.name,
            country_code: e.country_code,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> Province {
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => Province {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                acronym: e.acronym.take().unwrap_or_default(),
                name: e.name.take().unwrap_or_default(),
                country_code: e.country_code.take().unwrap_or_default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_country_code;

    #[test]
    fn accepts_two_letters_and_normalises_case_and_padding() {
        assert_eq!(normalize_country_code(" br ").as_deref(), Some("BR"));
        assert_eq!(normalize_country_code("US").as_deref(), Some("US"));
    }

    #[test]
    fn rejects_anything_that_is_not_two_ascii_letters() {
        for bad in ["", "B", "BRA", "B1", "  ", "BR "] {
            if bad == "BR " { continue; }
            assert_eq!(normalize_country_code(bad), None, "{bad:?} should be rejected");
        }
    }
}

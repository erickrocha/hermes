use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::ChecklistType;
use chrono::NaiveDateTime;
use entity::checklist_template_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-CK-02-S01` (`HRMS-651`): a reusable driver-checklist template. Its
/// items are a separate domain (`checklist_template_item`), assembled
/// alongside this one by `ChecklistTemplateUseCase`, not carried as a field
/// here -- an `EntityMapper` maps one entity, and items are not a column.
#[derive(Debug, Clone, PartialEq)]
pub struct ChecklistTemplate {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub checklist_type: ChecklistType,
    pub active: bool,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct ChecklistTemplateEntityMapper {}

impl EntityMapper<ChecklistTemplate, Model, ActiveModel> for ChecklistTemplateEntityMapper {
    fn build_active_model(d: ChecklistTemplate) -> ActiveModel {
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
            checklist_type: Set(d.checklist_type.to_string()),
            active: Set(d.active),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> ChecklistTemplate {
        ChecklistTemplate {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            name: e.name,
            checklist_type: ChecklistType::from_str(&e.checklist_type)
                .unwrap_or(ChecklistType::Standalone),
            active: e.active,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> ChecklistTemplate {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => ChecklistTemplate {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                name: e.name.take().unwrap_or_default(),
                checklist_type: e
                    .checklist_type
                    .take()
                    .and_then(|value| ChecklistType::from_str(&value).ok())
                    .unwrap_or(ChecklistType::Standalone),
                active: e.active.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

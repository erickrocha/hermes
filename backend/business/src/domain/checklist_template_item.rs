use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::checklist_template_item_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-CK-02-S01` (`HRMS-651`): one question of a `ChecklistTemplate`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChecklistTemplateItem {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub checklist_template_id: i64,
    pub description: String,
    pub generates_work_order: bool,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct ChecklistTemplateItemEntityMapper {}

impl EntityMapper<ChecklistTemplateItem, Model, ActiveModel> for ChecklistTemplateItemEntityMapper {
    fn build_active_model(d: ChecklistTemplateItem) -> ActiveModel {
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
            checklist_template_id: Set(d.checklist_template_id),
            description: Set(d.description),
            generates_work_order: Set(d.generates_work_order),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> ChecklistTemplateItem {
        ChecklistTemplateItem {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            checklist_template_id: e.checklist_template_id,
            description: e.description,
            generates_work_order: e.generates_work_order,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> ChecklistTemplateItem {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => ChecklistTemplateItem {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                checklist_template_id: e.checklist_template_id.take().unwrap_or_default(),
                description: e.description.take().unwrap_or_default(),
                generates_work_order: e.generates_work_order.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

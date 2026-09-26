use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::AnswerStatus;
use chrono::NaiveDateTime;
use entity::checklist_answer_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-CK-04-S01` (`HRMS-654`): a driver's answer to one template item.
#[derive(Debug, Clone, PartialEq)]
pub struct ChecklistAnswer {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub checklist_run_id: i64,
    pub checklist_template_item_id: i64,
    pub status: AnswerStatus,
    pub observation: Option<String>,
    pub work_order_id: Option<i64>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct ChecklistAnswerEntityMapper {}

impl EntityMapper<ChecklistAnswer, Model, ActiveModel> for ChecklistAnswerEntityMapper {
    fn build_active_model(d: ChecklistAnswer) -> ActiveModel {
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
            checklist_run_id: Set(d.checklist_run_id),
            checklist_template_item_id: Set(d.checklist_template_item_id),
            status: Set(d.status.to_string()),
            observation: Set(d.observation),
            work_order_id: Set(d.work_order_id),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> ChecklistAnswer {
        ChecklistAnswer {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            checklist_run_id: e.checklist_run_id,
            checklist_template_item_id: e.checklist_template_item_id,
            status: AnswerStatus::from_str(&e.status).unwrap_or(AnswerStatus::NonConforming),
            observation: e.observation,
            work_order_id: e.work_order_id,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> ChecklistAnswer {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => ChecklistAnswer {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                checklist_run_id: e.checklist_run_id.take().unwrap_or_default(),
                checklist_template_item_id: e.checklist_template_item_id.take().unwrap_or_default(),
                status: e
                    .status
                    .take()
                    .and_then(|value| AnswerStatus::from_str(&value).ok())
                    .unwrap_or(AnswerStatus::NonConforming),
                observation: e.observation.take().flatten(),
                work_order_id: e.work_order_id.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

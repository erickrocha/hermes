use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::maintenance_notification_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-MT-06` (`C-026`, `TRM-616`): an in-app notice that a part a work
/// order was waiting for has arrived. `part_name` is a copy taken when the
/// notice is raised; `read_at` is `None` while the notice is unread.
#[derive(Debug, Clone, PartialEq)]
pub struct MaintenanceNotification {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub part_id: i64,
    pub part_name: String,
    pub quantity: f64,
    pub read_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct MaintenanceNotificationEntityMapper {}

impl EntityMapper<MaintenanceNotification, Model, ActiveModel>
    for MaintenanceNotificationEntityMapper
{
    fn build_active_model(d: MaintenanceNotification) -> ActiveModel {
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
            work_order_id: Set(d.work_order_id),
            part_id: Set(d.part_id),
            part_name: Set(d.part_name),
            quantity: Set(d.quantity),
            read_at: Set(d.read_at.map(|at| at.and_utc())),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> MaintenanceNotification {
        MaintenanceNotification {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            work_order_id: e.work_order_id,
            part_id: e.part_id,
            part_name: e.part_name,
            quantity: e.quantity,
            read_at: e.read_at.map(|at| at.naive_utc()),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> MaintenanceNotification {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => MaintenanceNotification {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                work_order_id: e.work_order_id.take().unwrap_or_default(),
                part_id: e.part_id.take().unwrap_or_default(),
                part_name: e.part_name.take().unwrap_or_default(),
                quantity: e.quantity.take().unwrap_or_default(),
                read_at: e.read_at.take().flatten().map(|at| at.naive_utc()),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

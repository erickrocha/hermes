use sea_orm::entity::prelude::*;

/// `EPIC-GA-02-S02` (`HRMS-959`): the independent audit row written with every service marking (`TRM-440`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_service_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub attendance_id: i64,
    pub vehicle_id: i64,
    pub service_model_id: i64,
    pub name_key: String,
    pub new_state: String,
    pub acted_by_user_id: Option<i64>,
    pub acted_at: DateTimeUtc,
    pub origin: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

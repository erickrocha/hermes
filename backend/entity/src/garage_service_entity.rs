use sea_orm::entity::prelude::*;

/// `EPIC-GA-02-S01` (`HRMS-958`): one service of a triage, identified by the triage and the normalised service name (`TRM-433`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_service")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub attendance_id: i64,
    pub service_model_id: i64,
    pub name_key: String,
    pub state: String,
    pub performed_at: Option<DateTimeUtc>,
    pub marked_at: Option<DateTimeUtc>,
    pub forced_pending_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

use sea_orm::entity::prelude::*;

/// `EPIC-GA-05-S02` (`HRMS-963`, `TRM-483`): a manager's manual call of a
/// vehicle to base. Active until the vehicle's physical arrival is later than
/// `called_at` -- never cleared by a tag (`TRM-484`) -- or cancelled as made in
/// error (`TRM-486`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_call")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub called_at: DateTimeUtc,
    pub called_by_user_id: Option<i64>,
    pub cancelled_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

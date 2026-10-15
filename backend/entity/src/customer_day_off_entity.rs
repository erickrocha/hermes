use sea_orm::entity::prelude::*;

/// `EPIC-SC-01-S02` (`HRMS-601`, `C-024`): a day a customer is closed, so a
/// recurring line due that day is suppressed instead of dispatched against a
/// customer who cannot receive it (`entity-inventory.md` §1 "Folgas de
/// cliente").
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "customer_day_off")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub customer_id: i64,
    pub date: Date,
    pub reason: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

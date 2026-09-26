use sea_orm::entity::prelude::*;

/// `EPIC-SC-02-S02` (`HRMS-604`, `C-024`): a driver and vehicle allocated to
/// a transport demand over a date range (`entity-inventory.md` §2 "Linhas /
/// alocações"). Unlike `transport_demand`'s own optional `specific_driver_
/// id`/`specific_vehicle_id` (a *preference* recorded on the demand), this
/// is the actual crew -- both are required here.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "transport_demand_allocation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub demand_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    pub days_of_week: Option<String>,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub active: bool,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

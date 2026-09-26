use sea_orm::entity::prelude::*;

/// `EPIC-SC-01-S01` (`HRMS-600`, `C-024`): the customer registry -- the first
/// of `C-024`'s eight parity tables, and the only one with no dependency but
/// the tenant. `Necessidades` (transport demand) references a customer by
/// id, so this lands first (`entity-inventory.md` §1 "Clientes").
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "customer")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    /// Free text, like `vehicle.vehicle_type` (`C-023`): operacao-trm's
    /// `entity-inventory.md` names this field but states no vocabulary for
    /// it, so nothing here invents one.
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

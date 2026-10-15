use sea_orm::entity::prelude::*;

/// `EPIC-MT-01-S02` (`HRMS-701`, `C-026`): one individually tracked
/// pendency of a work order -- `entity-inventory.md` §4 "Itens de OS".
///
/// `foto` (photo evidence), `preventiva_id`, `origem_itens`/`inspecao_ids`
/// (consolidation/inspection links) and `prorrogacao_preventiva` are
/// deliberately absent: each names a producer (file storage, `C-027`,
/// consolidation) that does not exist in hermes yet -- see this Change's
/// plan doc. `purchase_order_id`/`is_purchase_placeholder` were added by
/// `EPIC-SP-03-S02` (`C-030`, `TRM-644…650`): the pendency a purchase order
/// names, and whether this item is a synthetic placeholder that only
/// represents the purchase itself (`TRM-645`) rather than a real repair
/// pendency (`TRM-644`).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "work_order_item")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub description: String,
    pub item_type: Option<String>,
    pub status: String,
    pub observation: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTimeUtc>,
    pub resolution_description: Option<String>,
    pub purchase_order_id: Option<i64>,
    pub is_purchase_placeholder: bool,
    pub preventive_plan_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

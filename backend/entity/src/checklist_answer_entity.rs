use sea_orm::entity::prelude::*;

/// `EPIC-CK-04-S01` (`HRMS-654`, `C-025`): a driver's answer to one item of
/// the template a `checklist_run` was submitted against --
/// `entity-inventory.md` §5 "Itens de checklist". `foto` (photo evidence) is
/// deliberately not a column here -- hermes has no file-storage mechanism
/// yet; see this Change's plan doc for why one is not stubbed in either.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "checklist_answer")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub checklist_run_id: i64,
    pub checklist_template_item_id: i64,
    /// Persisted as an `AnswerStatus` name -- `TRM-115`/`TRM-116`'s own
    /// repeated term "non-conforming" (and its implied opposite) is this
    /// two-value vocabulary's source, not an invented one.
    pub status: String,
    pub observation: Option<String>,
    /// `EPIC-CK-03-S02`: set only when this flagged, non-conforming answer
    /// actually opened a work order (`TRM-115`/`TRM-116`).
    pub work_order_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

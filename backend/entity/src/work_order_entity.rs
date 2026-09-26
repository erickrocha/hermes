use sea_orm::entity::prelude::*;

/// `EPIC-MT-01-S01` (`HRMS-700`, `C-026`): a maintenance work order --
/// `entity-inventory.md` §4 "Ordens de serviço". The human-facing number
/// (`TRM-221`'s "the number" in a shareable summary) is this row's `id`
/// rendered `OS-{id}` at the API boundary -- no separate counter to race,
/// unlike legacy's `numero_os` (`U-004`, fixed by design; see this Change's
/// plan doc).
///
/// `origin` is stamped `"Manual"` by every caller today (`EPIC-MT-01-S01`'s
/// own scope); `service_type` is free text until the `tiposServico`
/// catalogue (`EPIC-MT-05`) exists -- both documented in the plan doc, not
/// invented here.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "work_order")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub opened_at: DateTimeUtc,
    pub odometer_km: f64,
    pub origin: String,
    /// `EPIC-CK-03-S02`: set only for a `Checklist`-origin order, linking
    /// it back to the checklist that opened it (`TRM-115`).
    pub checklist_run_id: Option<i64>,
    /// `EPIC-MT-03-S01`: the maintenance window this order is scheduled
    /// into, if any. A work order belongs to at most one *active* plan at a
    /// time (`TRM-237`) -- not the `os_id`/`os_ids[]` pair legacy carries on
    /// the plan side.
    pub maintenance_plan_id: Option<i64>,
    pub service_type: Option<String>,
    pub description: String,
    pub responsible: Option<String>,
    pub status: String,
    pub observation: Option<String>,
    pub external_service: bool,
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    pub invoice_value_cents: Option<i64>,
    pub invoice_date: Option<Date>,
    pub concluded_at: Option<Date>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

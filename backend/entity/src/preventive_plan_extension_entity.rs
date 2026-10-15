use sea_orm::entity::prelude::*;

/// `EPIC-MT-07-S05` (`HRMS-710`, `C-027`): one auditable extension of a
/// preventive plan (`TRM-317`) -- retained forever, never edited. The plan's
/// previous last-service km/date are kept so the entry alone can restore the
/// baseline (`TRM-321`). The responsible party and instant are the audit
/// columns (`created_by`/`created_at`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "preventive_plan_extension")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub preventive_plan_id: i64,
    pub work_order_item_id: i64,
    pub inspection_km: f64,
    pub granted_km: f64,
    pub resulting_limit_km: f64,
    pub description: String,
    pub previous_last_service_km: Option<f64>,
    pub previous_last_service_date: Option<Date>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);

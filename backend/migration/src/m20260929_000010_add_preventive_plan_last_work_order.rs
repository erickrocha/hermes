use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S07` (`HRMS-712`, `C-027`): `preventive_plan.last_work_order_id`
/// -- the work order that last serviced the plan (`TRM-309`), or that a
/// manual new cycle names as its origin (`TRM-330`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PreventivePlan::Table)
                    .add_column(integer_null(PreventivePlan::LastWorkOrderId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_preventive_plan_last_work_order")
                    .from(PreventivePlan::Table, PreventivePlan::LastWorkOrderId)
                    .to(WorkOrder::Table, WorkOrder::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PreventivePlan::Table)
                    .drop_foreign_key(Alias::new("fk_preventive_plan_last_work_order"))
                    .drop_column(PreventivePlan::LastWorkOrderId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PreventivePlan {
    Table,
    LastWorkOrderId,
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    Id,
}

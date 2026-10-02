use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S02` (`HRMS-707`, `C-027`): `work_order.preventive_plan_id`
/// names the plan an automatically generated preventive work order serves
/// (`TRM-306`). Nullable: only a `WorkOrderOrigin::Preventive` order sets it.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrder::Table)
                    .add_column(integer_null(WorkOrder::PreventivePlanId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_work_order_preventive_plan")
                    .from(WorkOrder::Table, WorkOrder::PreventivePlanId)
                    .to(PreventivePlan::Table, PreventivePlan::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrder::Table)
                    .drop_foreign_key(Alias::new("fk_work_order_preventive_plan"))
                    .drop_column(WorkOrder::PreventivePlanId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    PreventivePlanId,
}

#[derive(DeriveIden)]
enum PreventivePlan {
    Table,
    Id,
}

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S04` (`HRMS-709`, `C-027`): `work_order_item.preventive_plan_id`
/// (`TRM-310`) -- the plan a resolved item serviced, so one work order can
/// close several plans without mixing their cycles.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrderItem::Table)
                    .add_column(integer_null(WorkOrderItem::PreventivePlanId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_work_order_item_preventive_plan")
                    .from(WorkOrderItem::Table, WorkOrderItem::PreventivePlanId)
                    .to(PreventivePlan::Table, PreventivePlan::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrderItem::Table)
                    .drop_foreign_key(Alias::new("fk_work_order_item_preventive_plan"))
                    .drop_column(WorkOrderItem::PreventivePlanId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum WorkOrderItem {
    Table,
    PreventivePlanId,
}

#[derive(DeriveIden)]
enum PreventivePlan {
    Table,
    Id,
}

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SP-03-S02` (`HRMS-802`, `C-030`, `TRM-644…650`): the two-way link
/// between a purchase order and the pendency it awaits a part for --
/// `purchase_order.work_order_item_id` (which item) and
/// `work_order_item.purchase_order_id`/`is_purchase_placeholder` (which
/// order, and whether the item is a synthetic placeholder or a real
/// pendency).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PurchaseOrder::Table)
                    .add_column(integer_null(PurchaseOrder::WorkOrderItemId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_purchase_order_work_order_item")
                    .from(PurchaseOrder::Table, PurchaseOrder::WorkOrderItemId)
                    .to(WorkOrderItem::Table, WorkOrderItem::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrderItem::Table)
                    .add_column(integer_null(WorkOrderItem::PurchaseOrderId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_work_order_item_purchase_order")
                    .from(WorkOrderItem::Table, WorkOrderItem::PurchaseOrderId)
                    .to(PurchaseOrder::Table, PurchaseOrder::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrderItem::Table)
                    .add_column(boolean(WorkOrderItem::IsPurchasePlaceholder).default(false))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrderItem::Table)
                    .drop_column(WorkOrderItem::IsPurchasePlaceholder)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrderItem::Table)
                    .drop_foreign_key(Alias::new("fk_work_order_item_purchase_order"))
                    .drop_column(WorkOrderItem::PurchaseOrderId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PurchaseOrder::Table)
                    .drop_foreign_key(Alias::new("fk_purchase_order_work_order_item"))
                    .drop_column(PurchaseOrder::WorkOrderItemId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PurchaseOrder {
    Table,
    Id,
    WorkOrderItemId,
}

#[derive(DeriveIden)]
enum WorkOrderItem {
    Table,
    Id,
    PurchaseOrderId,
    IsPurchasePlaceholder,
}

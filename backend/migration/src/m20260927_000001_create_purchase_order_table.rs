use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SP-03-S01` (`HRMS-802`, `C-030`): purchase orders. `part_id` is a
/// real FK -- see `entity::purchase_order_entity`'s doc comment. No
/// `work_order_item_id` column yet -- `EPIC-SP-03-S02`'s own field.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PurchaseOrder::Table)
                    .if_not_exists()
                    .col(pk_auto(PurchaseOrder::Id).integer())
                    .col(binary_len_uniq(PurchaseOrder::Uuid, 16))
                    .col(integer(PurchaseOrder::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_purchase_order_tenant")
                            .from(PurchaseOrder::Table, PurchaseOrder::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(big_integer(PurchaseOrder::PartId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_purchase_order_part")
                            .from(PurchaseOrder::Table, PurchaseOrder::PartId)
                            .to(Part::Table, Part::Id),
                    )
                    .col(double(PurchaseOrder::Quantity).not_null())
                    .col(string_len_null(PurchaseOrder::SuggestedSupplier, 255))
                    .col(string_len_null(PurchaseOrder::Observation, 500))
                    .col(string_len(PurchaseOrder::Status, 20).not_null())
                    .col(integer_null(PurchaseOrder::WorkOrderId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_purchase_order_work_order")
                            .from(PurchaseOrder::Table, PurchaseOrder::WorkOrderId)
                            .to(WorkOrder::Table, WorkOrder::Id),
                    )
                    .col(integer_null(PurchaseOrder::VehicleId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_purchase_order_vehicle")
                            .from(PurchaseOrder::Table, PurchaseOrder::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date_null(PurchaseOrder::OrderedAt))
                    .col(date_null(PurchaseOrder::ExpectedDeliveryDate))
                    .col(date_time(PurchaseOrder::CreatedAt).null())
                    .col(string_len(PurchaseOrder::CreatedBy, 50).null())
                    .col(date_time(PurchaseOrder::UpdatedAt).null())
                    .col(string_len(PurchaseOrder::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_purchase_order_part_work_order")
                    .table(PurchaseOrder::Table)
                    .col(PurchaseOrder::PartId)
                    .col(PurchaseOrder::WorkOrderId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PurchaseOrder::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum PurchaseOrder {
    Table,
    Id,
    Uuid,
    TenantId,
    PartId,
    Quantity,
    SuggestedSupplier,
    Observation,
    Status,
    WorkOrderId,
    VehicleId,
    OrderedAt,
    ExpectedDeliveryDate,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Part {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

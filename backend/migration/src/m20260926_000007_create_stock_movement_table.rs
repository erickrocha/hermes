use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SP-02-S01` (`HRMS-801`, `C-030`): the stock ledger. No stored
/// stock figure anywhere -- see `entity::part_entity`'s doc comment.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StockMovement::Table)
                    .if_not_exists()
                    .col(pk_auto(StockMovement::Id).integer())
                    .col(binary_len_uniq(StockMovement::Uuid, 16))
                    .col(integer(StockMovement::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_stock_movement_tenant")
                            .from(StockMovement::Table, StockMovement::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(big_integer(StockMovement::PartId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_stock_movement_part")
                            .from(StockMovement::Table, StockMovement::PartId)
                            .to(Part::Table, Part::Id),
                    )
                    .col(string_len(StockMovement::MovementType, 20).not_null())
                    .col(double(StockMovement::Quantity).not_null())
                    .col(big_integer_null(StockMovement::UnitValueCents))
                    .col(big_integer_null(StockMovement::TotalValueCents))
                    .col(string_len(StockMovement::CostSource, 30).not_null())
                    .col(string_len_null(StockMovement::Supplier, 255))
                    .col(string_len_null(StockMovement::InvoiceNumber, 50))
                    .col(date_null(StockMovement::EntryDate))
                    .col(date_time(StockMovement::CreatedAt).null())
                    .col(string_len(StockMovement::CreatedBy, 50).null())
                    .col(date_time(StockMovement::UpdatedAt).null())
                    .col(string_len(StockMovement::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_stock_movement_part")
                    .table(StockMovement::Table)
                    .col(StockMovement::PartId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(StockMovement::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum StockMovement {
    Table,
    Id,
    Uuid,
    TenantId,
    PartId,
    MovementType,
    Quantity,
    UnitValueCents,
    TotalValueCents,
    CostSource,
    Supplier,
    InvoiceNumber,
    EntryDate,
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
enum Tenant {
    Table,
    Id,
}

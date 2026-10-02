use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SP-04-S01` (`HRMS-803`, `C-030`): direct vehicle expenses. No
/// `plate` column -- see `entity::vehicle_expense_entity`'s doc comment.
/// `uq_vehicle_expense_invoice` (`TRM-661`) is a plain unique index on
/// `(vehicle_id, category, invoice_number)`: MySQL treats each `NULL`
/// `invoice_number` as distinct, so the constraint only ever engages once an
/// invoice number is actually given -- the same "live" scope the legacy
/// partial index needed a `where` clause for, here for free.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(VehicleExpense::Table)
                    .if_not_exists()
                    .col(pk_auto(VehicleExpense::Id).integer())
                    .col(binary_len_uniq(VehicleExpense::Uuid, 16))
                    .col(integer(VehicleExpense::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_expense_tenant")
                            .from(VehicleExpense::Table, VehicleExpense::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(VehicleExpense::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_expense_vehicle")
                            .from(VehicleExpense::Table, VehicleExpense::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len(VehicleExpense::Category, 100).not_null())
                    .col(string_len(VehicleExpense::CompetencePeriod, 20).not_null())
                    .col(date(VehicleExpense::IssueDate).not_null())
                    .col(string_len_null(VehicleExpense::Supplier, 255))
                    .col(string_len_null(VehicleExpense::InvoiceNumber, 100))
                    .col(string_len_null(VehicleExpense::Description, 500))
                    .col(big_integer(VehicleExpense::ValueCents).not_null())
                    .col(string_len(VehicleExpense::Origin, 20).not_null())
                    .col(date_time(VehicleExpense::CreatedAt).null())
                    .col(string_len(VehicleExpense::CreatedBy, 50).null())
                    .col(date_time(VehicleExpense::UpdatedAt).null())
                    .col(string_len(VehicleExpense::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_vehicle_expense_invoice")
                    .table(VehicleExpense::Table)
                    .col(VehicleExpense::VehicleId)
                    .col(VehicleExpense::Category)
                    .col(VehicleExpense::InvoiceNumber)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VehicleExpense::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum VehicleExpense {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    Category,
    CompetencePeriod,
    IssueDate,
    Supplier,
    InvoiceNumber,
    Description,
    ValueCents,
    Origin,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
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

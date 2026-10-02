use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SP-01-S01` (`HRMS-800`, `C-030`): the parts catalogue. No stock
/// column -- see `entity::part_entity`'s doc comment.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Part::Table)
                    .if_not_exists()
                    .col(pk_auto(Part::Id).integer())
                    .col(binary_len_uniq(Part::Uuid, 16))
                    .col(integer(Part::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_part_tenant")
                            .from(Part::Table, Part::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Part::Name, 255).not_null())
                    .col(string_len_null(Part::Category, 255))
                    .col(string_len_null(Part::Application, 255))
                    .col(double(Part::MinimumStock).not_null())
                    .col(string_len(Part::Unit, 20).not_null())
                    .col(big_integer_null(Part::UnitValueCents))
                    .col(string_len_null(Part::DefaultSupplier, 255))
                    .col(string_len_null(Part::Location, 255))
                    .col(string_len_null(Part::Observation, 500))
                    .col(big_integer_null(Part::MovingAverageCostCents))
                    .col(big_integer_null(Part::LastPurchasePriceCents))
                    .col(date_time(Part::CreatedAt).null())
                    .col(string_len(Part::CreatedBy, 50).null())
                    .col(date_time(Part::UpdatedAt).null())
                    .col(string_len(Part::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Part::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
pub enum Part {
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    Category,
    Application,
    MinimumStock,
    Unit,
    UnitValueCents,
    DefaultSupplier,
    Location,
    Observation,
    MovingAverageCostCents,
    LastPurchasePriceCents,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

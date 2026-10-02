use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-07-S01` (`HRMS-943`, `C-029`): the operator's own diesel tank,
/// at most one row per tenant.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InternalTank::Table)
                    .if_not_exists()
                    .col(pk_auto(InternalTank::Id).integer())
                    .col(binary_len_uniq(InternalTank::Uuid, 16))
                    .col(integer(InternalTank::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_internal_tank_tenant")
                            .from(InternalTank::Table, InternalTank::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(double(InternalTank::CapacityLiters).not_null())
                    .col(double(InternalTank::ReferenceStockLiters).not_null())
                    .col(date_time(InternalTank::ReferenceAt).not_null())
                    .col(double(InternalTank::AlertLevelLiters).not_null())
                    .col(double(InternalTank::ReserveLevelLiters).not_null())
                    .col(date_time(InternalTank::CreatedAt).null())
                    .col(string_len(InternalTank::CreatedBy, 50).null())
                    .col(date_time(InternalTank::UpdatedAt).null())
                    .col(string_len(InternalTank::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_internal_tank_tenant")
                    .table(InternalTank::Table)
                    .col(InternalTank::TenantId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InternalTank::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum InternalTank {
    Table,
    Id,
    Uuid,
    TenantId,
    CapacityLiters,
    ReferenceStockLiters,
    ReferenceAt,
    AlertLevelLiters,
    ReserveLevelLiters,
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

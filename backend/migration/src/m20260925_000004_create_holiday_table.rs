use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-01-S03` (`HRMS-602`, `C-024`): tenant-configured holidays.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Holiday::Table)
                    .if_not_exists()
                    .col(pk_auto(Holiday::Id).integer())
                    .col(binary_len_uniq(Holiday::Uuid, 16))
                    .col(integer(Holiday::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_holiday_tenant")
                            .from(Holiday::Table, Holiday::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(date(Holiday::Date).not_null())
                    .col(string_len(Holiday::Name, 200).not_null())
                    .col(date_time(Holiday::CreatedAt).null())
                    .col(string_len(Holiday::CreatedBy, 50).null())
                    .col(date_time(Holiday::UpdatedAt).null())
                    .col(string_len(Holiday::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // A tenant records one holiday per date -- two different names on
        // the same date is a correction, not a second holiday.
        manager
            .create_index(
                Index::create()
                    .name("uq_holiday_tenant_date")
                    .table(Holiday::Table)
                    .col(Holiday::TenantId)
                    .col(Holiday::Date)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Holiday::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Holiday {
    Table,
    Id,
    Uuid,
    TenantId,
    Date,
    Name,
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

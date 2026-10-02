use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-01-S01` (`HRMS-600`, `C-024`): the customer registry, the first
/// tenant-owned table of the scheduling domain -- three-part change
/// (`AD-015`), same shape `EPIC-FO-01` established for `vehicle`.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Customer::Table)
                    .if_not_exists()
                    .col(pk_auto(Customer::Id).integer())
                    .col(binary_len_uniq(Customer::Uuid, 16))
                    .col(integer(Customer::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customer_tenant")
                            .from(Customer::Table, Customer::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Customer::Name, 200).not_null())
                    .col(string_len(Customer::Status, 50).not_null())
                    .col(string_len_null(Customer::Notes, 1000))
                    .col(date_time(Customer::CreatedAt).null())
                    .col(string_len(Customer::CreatedBy, 50).null())
                    .col(date_time(Customer::UpdatedAt).null())
                    .col(string_len(Customer::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Customer::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Customer {
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    Status,
    Notes,
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

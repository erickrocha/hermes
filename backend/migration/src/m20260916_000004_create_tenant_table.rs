use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tenant::Table)
                    .if_not_exists()
                    .col(pk_auto(Tenant::Id).integer())
                    .col(binary_len_uniq(Tenant::Uuid, 16))
                    .col(string_len(Tenant::BusinessName, 500).not_null())
                    .col(string_len(Tenant::CompanyName, 500).null())
                    .col(string_len(Tenant::TaxId, 100).not_null())
                    .col(string_len(Tenant::Email, 500).null())
                    .col(string_len(Tenant::Phone, 50).null())
                    .col(string_len(Tenant::WebSite, 500).null())
                    .col(string_len(Tenant::AddressLine1, 500).null())
                    .col(string_len(Tenant::AddressLine2, 500).null())
                    .col(string_len(Tenant::Locality, 200).null())
                    .col(string_len(Tenant::AdministrativeArea, 200).null())
                    .col(string_len(Tenant::PostalCode, 32).null())
                    .col(string_len(Tenant::CountryCode, 2).null())
                    .col(date_time(Tenant::CreatedAt).null())
                    .col(string_len(Tenant::CreatedBy, 50).null())
                    .col(date_time(Tenant::UpdatedAt).null())
                    .col(string_len(Tenant::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Tenant::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Tenant {
    Table,
    Id,
    Uuid,
    BusinessName,
    CompanyName,
    TaxId,
    Email,
    Phone,
    WebSite,
    AddressLine1,
    AddressLine2,
    Locality,
    AdministrativeArea,
    PostalCode,
    CountryCode,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

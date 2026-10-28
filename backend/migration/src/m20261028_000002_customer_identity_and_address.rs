use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-01-S04`: a customer is a legal person -- taxpayer document (CPF or CNPJ, digits only), legal
/// name and an address in the tenant's own shape (`tenant.address_line1` ... `country_code`). `name` stays
/// as the trade name the office knows the customer by. The document is unique inside a tenant when present.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Customer::Table)
                    .add_column(ColumnDef::new(Customer::LegalName).string_len(200).null())
                    .add_column(ColumnDef::new(Customer::TaxId).string_len(100).null())
                    .add_column(ColumnDef::new(Customer::AddressLine1).string_len(500).null())
                    .add_column(ColumnDef::new(Customer::AddressLine2).string_len(500).null())
                    .add_column(ColumnDef::new(Customer::Locality).string_len(200).null())
                    .add_column(ColumnDef::new(Customer::AdministrativeArea).string_len(200).null())
                    .add_column(ColumnDef::new(Customer::PostalCode).string_len(32).null())
                    .add_column(ColumnDef::new(Customer::CountryCode).string_len(2).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_customer_tenant_tax_id")
                    .table(Customer::Table)
                    // `tax_id` leads so the index cannot be taken over by `fk_customer_tenant`, which would
                    // make it undroppable on the way down.
                    .col(Customer::TaxId)
                    .col(Customer::TenantId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("uq_customer_tenant_tax_id").table(Customer::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Customer::Table)
                    .drop_column(Customer::LegalName)
                    .drop_column(Customer::TaxId)
                    .drop_column(Customer::AddressLine1)
                    .drop_column(Customer::AddressLine2)
                    .drop_column(Customer::Locality)
                    .drop_column(Customer::AdministrativeArea)
                    .drop_column(Customer::PostalCode)
                    .drop_column(Customer::CountryCode)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Customer {
    Table,
    TenantId,
    LegalName,
    TaxId,
    AddressLine1,
    AddressLine2,
    Locality,
    AdministrativeArea,
    PostalCode,
    CountryCode,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // EPIC-TP-01-S04 (PD-022): country is now required. Any pre-existing
        // row without one (only possible on a dev database that predates
        // this change -- there is no production deployment yet) is
        // backfilled to a harmless placeholder rather than left to break the
        // NOT NULL constraint below; 'US' carries no policy meaning here, it
        // is simply the country the schema happened to seed first.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE tenant SET country_code = 'US' WHERE country_code IS NULL",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    .modify_column(ColumnDef::new(Tenant::CountryCode).string_len(2).not_null())
                    .to_owned(),
            )
            .await?;

        // EPIC-TP-02-S06 (HRMS-211): unique *within its country*, not
        // globally -- two different countries' tax authorities can't collide
        // on the same document, so the constraint is scoped to match. Must
        // follow app-level normalisation (this session's tenant_use_case.rs
        // change) rather than precede it: the column may otherwise hold the
        // same document in two spellings, which a unique index would refuse
        // outright instead of silently missing.
        manager
            .create_index(
                Index::create()
                    .name("uq_tenant_country_tax_id")
                    .table(Tenant::Table)
                    .col(Tenant::CountryCode)
                    .col(Tenant::TaxId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_tenant_country_tax_id")
                    .table(Tenant::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    .modify_column(ColumnDef::new(Tenant::CountryCode).string_len(2).null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    CountryCode,
    TaxId,
}

use sea_orm_migration::prelude::*;

/// DEF-XF-01: `create_tenant_table` named two columns `social_name` and
/// `web_site`, while the entity, domain, API and console all say
/// `company_name` and `website`. SeaORM selects by the entity's names, so every
/// tenant read and write failed on a freshly migrated database.
///
/// Fixed forward with a rename rather than by editing the applied migration
/// (HRMS-026): databases that already ran `000004` get corrected, and a fresh
/// one ends up identical.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    .rename_column(Tenant::SocialName, Tenant::CompanyName)
                    .rename_column(Tenant::WebSite, Tenant::Website)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    .rename_column(Tenant::CompanyName, Tenant::SocialName)
                    .rename_column(Tenant::Website, Tenant::WebSite)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    SocialName,
    WebSite,
    CompanyName,
    Website,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// The root tenant's taxpayer id (CNPJ 31.856.380/0001-94, digits only). A valid CNPJ by `D-37`/`HRMS-873`.
const ROOT_TAX_ID: &str = "31856380000194";
const ROOT_NAME: &str = "Erocha Soluções em TI";

/// `C-036` (`D-37`, `HRMS-871`/`872`/`874`): every tenant has a parent but the root, and every user has a tenant.
/// Seeds the root tenant, hangs the existing tenants under it, and moves the platform administrators (who had no
/// tenant) into it. Declarative: no data is lost, and no user's effective permissions change.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    .add_column(ColumnDef::new(Tenant::ParentTenantId).integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tenant_parent_tenant")
                    .from(Tenant::Table, Tenant::ParentTenantId)
                    .to(Tenant::Table, Tenant::Id)
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(&format!(
            "INSERT INTO `tenant` (`uuid`, `business_name`, `company_name`, `tax_id`, `country_code`, \
             `created_at`, `created_by`, `updated_at`, `updated_by`) \
             SELECT UNHEX(REPLACE(UUID(), '-', '')), '{ROOT_NAME}', '{ROOT_NAME}', '{ROOT_TAX_ID}', 'BR', \
             NOW(), 'system', NOW(), 'system' \
             WHERE NOT EXISTS (SELECT 1 FROM `tenant` WHERE `tax_id` = '{ROOT_TAX_ID}' AND `country_code` = 'BR')"
        ))
        .await?;
        db.execute_unprepared(&format!(
            "UPDATE `tenant` SET `parent_tenant_id` = (SELECT `id` FROM (SELECT `id` FROM `tenant` \
             WHERE `tax_id` = '{ROOT_TAX_ID}' AND `country_code` = 'BR') r) \
             WHERE `tax_id` <> '{ROOT_TAX_ID}' OR `country_code` <> 'BR'"
        ))
        .await?;
        db.execute_unprepared(&format!(
            "UPDATE `user` SET `tenant_id` = (SELECT `id` FROM `tenant` WHERE `tax_id` = '{ROOT_TAX_ID}' \
             AND `country_code` = 'BR') WHERE `tenant_id` IS NULL"
        ))
        .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .modify_column(ColumnDef::new(User::TenantId).integer().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .modify_column(ColumnDef::new(User::TenantId).integer().null())
                    .to_owned(),
            )
            .await?;
        // The platform administrators go back to belonging to no tenant.
        db.execute_unprepared(
            "UPDATE `user` u JOIN `access_profile` p ON p.`id` = u.`profile_id` \
             SET u.`tenant_id` = NULL WHERE p.`scope` = 'Platform'",
        )
        .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_tenant_parent_tenant").table(Tenant::Table).to_owned())
            .await?;
        manager
            .alter_table(Table::alter().table(Tenant::Table).drop_column(Tenant::ParentTenantId).to_owned())
            .await?;
        db.execute_unprepared(&format!(
            "DELETE FROM `tenant` WHERE `tax_id` = '{ROOT_TAX_ID}' AND `country_code` = 'BR' \
             AND NOT EXISTS (SELECT 1 FROM `user` WHERE `tenant_id` = `tenant`.`id`)"
        ))
        .await
        .map(|_| ())
    }
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
    ParentTenantId,
}

#[derive(DeriveIden)]
enum User {
    Table,
    TenantId,
}

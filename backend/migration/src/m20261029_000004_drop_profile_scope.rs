use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-036` (`D-37` 4, `HRMS-875`): a profile has no scope. The `Platform` profiles a tenant-less account held
/// are gone with the tenant-less account; one not held by anyone is deleted, one that is held stays as an
/// ordinary profile (its holders already moved to the root tenant by `m20261029_000001`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DELETE FROM `access_profile` WHERE `scope` = 'Platform' AND `system` = 0 \
             AND `id` NOT IN (SELECT `profile_id` FROM `user`)",
        )
        .await?;
        manager
            .alter_table(Table::alter().table(AccessProfile::Table).drop_column(AccessProfile::Scope).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AccessProfile::Table)
                    .add_column(ColumnDef::new(AccessProfile::Scope).string_len(16).not_null().default("Tenant"))
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("UPDATE `access_profile` SET `scope` = 'Platform' WHERE `key` = 'SystemAdministrator'")
            .await
            .map(|_| ())
    }
}

#[derive(DeriveIden)]
enum AccessProfile {
    Table,
    Scope,
}

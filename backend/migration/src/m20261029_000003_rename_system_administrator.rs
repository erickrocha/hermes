use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-036` (`D-37` 3, `HRMS-874`): the platform administrator is the system profile *Administrador do Sistema*,
/// holding every permission, in the root tenant. There is no `SysAdmin` role any more -- the profile keeps its
/// row and its holders and takes the new key and name.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE `access_profile` SET `key` = 'SystemAdministrator', `name` = 'Administrador do Sistema', \
                 `name_key` = 'administrador do sistema' WHERE `key` = 'SysAdmin'",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE `access_profile` SET `key` = 'SysAdmin', `name` = 'Platform administrator', \
                 `name_key` = 'platform administrator' WHERE `key` = 'SystemAdministrator'",
            )
            .await
            .map(|_| ())
    }
}

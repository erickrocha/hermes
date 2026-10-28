use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-035`: roles become access profiles. The five roles are seeded as system profiles (no tenant,
/// `system = 1`, keyed by the old role name); every user without a profile receives the system profile
/// of their role; then `user.role`, `user.principal_function` and `access_profile.base_role` go. The
/// system profiles' permissions are left empty here and derived from the catalogue at boot
/// (`AccessProfileUseCase::sync_system_profiles`), so the migration crate never duplicates it.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Profile::Table)
                    .modify_column(ColumnDef::new(Profile::TenantId).integer().null())
                    .add_column(ColumnDef::new(Profile::Scope).string_len(10).not_null().default("Tenant"))
                    .add_column(ColumnDef::new(Profile::System).boolean().not_null().default(false))
                    .add_column(ColumnDef::new(Profile::Key).string_len(30).null())
                    .drop_column(Profile::BaseRole)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_access_profile_key")
                    .table(Profile::Table)
                    .col(Profile::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        for (key, name, scope) in [
            ("SysAdmin", "Platform administrator", "Platform"),
            ("TenantOwner", "Tenant owner", "Tenant"),
            ("TenantUser", "Tenant user", "Tenant"),
            ("Driver", "Driver", "Tenant"),
            ("Mechanic", "Mechanic", "Tenant"),
        ] {
            let uuid = uuid::Uuid::new_v4();
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(Profile::Table)
                        .columns([
                            Profile::Uuid,
                            Profile::TenantId,
                            Profile::Name,
                            Profile::NameKey,
                            Profile::Scope,
                            Profile::System,
                            Profile::Key,
                            Profile::Active,
                            Profile::Permissions,
                            Profile::CreatedAt,
                            Profile::CreatedBy,
                            Profile::UpdatedAt,
                            Profile::UpdatedBy,
                        ])
                        .values_panic([
                            uuid.as_bytes().to_vec().into(),
                            Option::<i64>::None.into(),
                            name.into(),
                            name.to_lowercase().into(),
                            scope.into(),
                            true.into(),
                            key.into(),
                            true.into(),
                            "[]".into(),
                            Expr::current_timestamp(),
                            "system".into(),
                            Expr::current_timestamp(),
                            "system".into(),
                        ])
                        .to_owned(),
                )
                .await?;
        }

        // Every user keeps a custom profile they already held; the rest take their role's system profile.
        db.execute_unprepared(
            "UPDATE `user` u \
             JOIN `access_profile` p ON p.`key` = u.`role` \
             SET u.`profile_id` = p.`id` \
             WHERE u.`profile_id` IS NULL",
        )
        .await?;

        // PD-013's "a SysAdmin has no tenant" lived in a CHECK on `role`; the invariant now lives in the
        // profile's scope (`authorization::lacks_required_tenant`).
        db.execute_unprepared("ALTER TABLE `user` DROP CONSTRAINT chk_sysadmin_has_no_tenant").await?;
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .modify_column(ColumnDef::new(User::ProfileId).integer().not_null())
                    .add_column(ColumnDef::new(User::ExtraPermissions).text().not_null())
                    .drop_column(User::Role)
                    .drop_column(User::PrincipalFunction)
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared("UPDATE `user` SET `extra_permissions` = '[]'").await.map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(Profile::Table)
                    .add_column(ColumnDef::new(Profile::BaseRole).string_len(20).not_null().default("TenantUser"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .add_column(ColumnDef::new(User::Role).string_len(50).not_null().default("TenantUser"))
                    .add_column(ColumnDef::new(User::PrincipalFunction).string_len(120).null())
                    .drop_column(User::ExtraPermissions)
                    .modify_column(ColumnDef::new(User::ProfileId).integer().null())
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "UPDATE `user` u JOIN `access_profile` p ON p.`id` = u.`profile_id` \
             SET u.`role` = COALESCE(p.`key`, 'TenantUser')",
        )
        .await?;
        db.execute_unprepared(
            "UPDATE `user` u JOIN `access_profile` p ON p.`id` = u.`profile_id` \
             SET u.`profile_id` = NULL WHERE p.`system` = 1",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE `user` ADD CONSTRAINT chk_sysadmin_has_no_tenant \
             CHECK (role <> 'SysAdmin' OR tenant_id IS NULL)",
        )
        .await?;
        db.execute_unprepared("DELETE FROM `access_profile` WHERE `system` = 1").await?;
        manager
            .drop_index(Index::drop().name("uq_access_profile_key").table(Profile::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Profile::Table)
                    .drop_column(Profile::Scope)
                    .drop_column(Profile::System)
                    .drop_column(Profile::Key)
                    .modify_column(ColumnDef::new(Profile::TenantId).integer().not_null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Profile {
    #[sea_orm(iden = "access_profile")]
    Table,
    Uuid,
    TenantId,
    Name,
    NameKey,
    BaseRole,
    Scope,
    System,
    Key,
    Active,
    Permissions,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Role,
    ProfileId,
    ExtraPermissions,
    PrincipalFunction,
}

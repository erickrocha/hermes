use sea_orm_migration::prelude::*;

/// D-08: "tenant-bound SysAdmin" is an impossible state by design. Until now
/// that was true only because five independent call sites each remembered to
/// require `role == SysAdmin && tenant_id IS NULL`; nothing in the schema said
/// so, and `PUT /user/{id}` on a self-updating SysAdmin never checked. The
/// owner's decision was to enforce it **once** — so it moves here, where no
/// future endpoint can forget it.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // A dev database may already hold the state the constraint forbids
        // (nothing in the app produced it, but nothing prevented it either).
        // Detach rather than delete: an administrator who wrongly carries a
        // tenant should lose the tenant, not the account.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE user SET tenant_id = NULL WHERE role = 'SysAdmin' AND tenant_id IS NOT NULL",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE user ADD CONSTRAINT chk_sysadmin_has_no_tenant \
                 CHECK (role <> 'SysAdmin' OR tenant_id IS NULL)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE user DROP CONSTRAINT chk_sysadmin_has_no_tenant")
            .await?;
        Ok(())
    }
}

use sea_orm_migration::{prelude::*, schema::*};

/// DEF-XF-07: three schema changes had been made by editing or deleting
/// migrations that were already applied, against HRMS-026:
///
/// - `blocked_reason` was added to `000001_create_table_user` (defect D-1);
/// - `first_login` was removed from the same file (HRM-045 descoped);
/// - `000003_business_plan_tiers` was deleted (HRM-014/015/016 descoped).
///
/// `000005_create_tenant_plan_table` had also been deleted (PD-021); it is
/// restored too. On a fresh database `m20260917_000001` carries its data onto
/// `tenant.business_plan_id` and drops it. On a database that ran `m20260917_000001`
/// *before* `000005` was restored, `000005` now creates it afresh -- always empty,
/// because the data carry-over already happened -- so it is dropped here.
///
/// Those files are restored to what was originally applied, and the changes
/// are made here instead. Depending on which version of the files a database
/// saw, any combination of the three may already be in place, so every step
/// checks first -- the result is the same schema from every starting point.
///
/// The one edit deliberately kept is `m20260917_000001`'s BIGINT -> INT fix
/// (defect D-13): the original could never be applied on MariaDB (errno 150),
/// so no database has it recorded as run, and there is nothing to protect.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("user", "blocked_reason").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(User::Table)
                        .add_column(string_len(User::BlockedReason, 32).null())
                        .to_owned(),
                )
                .await?;
        }

        if manager.has_column("user", "first_login").await? {
            manager
                .alter_table(Table::alter().table(User::Table).drop_column(User::FirstLogin).to_owned())
                .await?;
        }

        if manager.has_table("tenant_plan").await? {
            manager
                .drop_table(Table::drop().table(TenantPlan::Table).to_owned())
                .await?;
        }

        if manager.has_table("business_plan_tier").await? {
            manager
                .drop_table(Table::drop().table(BusinessPlanTier::Table).to_owned())
                .await?;
        }

        Ok(())
    }

    /// Restores `first_login` only. The tier table and its data are not
    /// recreated: the feature is descoped, and an empty table would be a lie.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("user", "first_login").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(User::Table)
                        .add_column(boolean(User::FirstLogin).default(true))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    BlockedReason,
    FirstLogin,
}

#[derive(DeriveIden)]
enum BusinessPlanTier {
    Table,
}

#[derive(DeriveIden)]
enum TenantPlan {
    Table,
}

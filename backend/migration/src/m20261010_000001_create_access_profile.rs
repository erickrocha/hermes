use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-032` (`HRMS-851`/`852`, `D-27`): a tenant's access profiles -- a named set of permissions for one
/// of the roles below the owner -- and the optional profile of a user. A user with no profile holds the
/// built-in default of their role, so existing users change in no way.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Profile::Table)
                    .if_not_exists()
                    .col(pk_auto(Profile::Id).integer())
                    .col(binary_len_uniq(Profile::Uuid, 16))
                    .col(integer(Profile::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_access_profile_tenant")
                            .from(Profile::Table, Profile::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Profile::Name, 120).not_null())
                    .col(string_len(Profile::NameKey, 120).not_null())
                    .col(string_len(Profile::BaseRole, 20).not_null())
                    .col(boolean(Profile::Active).not_null())
                    .col(text(Profile::Permissions).not_null())
                    .col(date_time(Profile::CreatedAt).null())
                    .col(string_len(Profile::CreatedBy, 50).null())
                    .col(date_time(Profile::UpdatedAt).null())
                    .col(string_len(Profile::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_access_profile_tenant_name")
                    .table(Profile::Table)
                    .col(Profile::TenantId)
                    .col(Profile::NameKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .add_column(ColumnDef::new(User::ProfileId).integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_access_profile")
                    .from(User::Table, User::ProfileId)
                    .to(Profile::Table, Profile::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_user_access_profile").table(User::Table).to_owned())
            .await?;
        manager
            .alter_table(Table::alter().table(User::Table).drop_column(User::ProfileId).to_owned())
            .await?;
        manager.drop_table(Table::drop().table(Profile::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Profile {
    #[sea_orm(iden = "access_profile")]
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    NameKey,
    BaseRole,
    Active,
    Permissions,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum User {
    Table,
    ProfileId,
}

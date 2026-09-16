use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(pk_auto(User::Id).integer())
                    .col(binary_len_uniq(User::Uuid,16))
                    .col(string_len(User::Name, 500).null())
                    .col(string_len(User::Email, 500).unique_key())
                    .col(string_len(User::Password, 500).not_null())
                    .col(boolean(User::FirstLogin).default(true))
                    .col(boolean(User::Enabled).default(true))
                    .col(integer(User::TenantId).null())
                    .col(string_len(User::Role,50).not_null())
                    .col(string_len(User::BlockedReason,32).null())
                    .col(date_time(User::CreatedAt).null())
                    .col(string_len(User::CreatedBy,50).null())
                    .col(date_time(User::UpdatedAt).null())
                    .col(string_len(User::UpdatedBy,50).null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum User {
    Table,
    Id,
    Uuid,
    Name,
    Email,
    Password,
    FirstLogin,
    Enabled,
    TenantId,
    Role,
    BlockedReason,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy
}

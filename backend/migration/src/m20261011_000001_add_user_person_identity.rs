use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-032` (`HRMS-861`, `D-27` 6): the person's CPF and principal function on the user. Both optional;
/// the CPF is stored digits-only and unique within a tenant when present (NULLs do not collide).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .add_column(ColumnDef::new(User::Cpf).string_len(20).null())
                    .add_column(ColumnDef::new(User::PrincipalFunction).string_len(120).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_user_tenant_cpf")
                    .table(User::Table)
                    .col(User::TenantId)
                    .col(User::Cpf)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("uq_user_tenant_cpf").table(User::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .drop_column(User::Cpf)
                    .drop_column(User::PrincipalFunction)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    TenantId,
    Cpf,
    PrincipalFunction,
}

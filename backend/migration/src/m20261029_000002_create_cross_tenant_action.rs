use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-036` (`HRMS-877`, `EPIC-AC-08-S06`): one row per request a tenant makes inside a descendant of its own --
/// who, from which tenant, into which, which operation, which record, read or write, and when. Append-only: no
/// row is ever updated or deleted by the service. The affected tenant reads it back.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CrossTenantAction::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(CrossTenantAction::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(CrossTenantAction::Uuid).binary_len(16).not_null().unique_key())
                    .col(ColumnDef::new(CrossTenantAction::ActorUserId).integer().not_null())
                    .col(ColumnDef::new(CrossTenantAction::ActorEmail).string_len(255).not_null())
                    .col(ColumnDef::new(CrossTenantAction::ActorTenantId).integer().not_null())
                    .col(ColumnDef::new(CrossTenantAction::AffectedTenantId).integer().not_null())
                    .col(ColumnDef::new(CrossTenantAction::Kind).string_len(8).not_null())
                    .col(ColumnDef::new(CrossTenantAction::Method).string_len(8).not_null())
                    .col(ColumnDef::new(CrossTenantAction::Route).string_len(300).not_null())
                    .col(ColumnDef::new(CrossTenantAction::Path).string_len(500).not_null())
                    .col(ColumnDef::new(CrossTenantAction::OccurredAt).date_time().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cross_tenant_action_actor_tenant")
                            .from(CrossTenantAction::Table, CrossTenantAction::ActorTenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cross_tenant_action_affected_tenant")
                            .from(CrossTenantAction::Table, CrossTenantAction::AffectedTenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_cross_tenant_action_affected")
                    .table(CrossTenantAction::Table)
                    .col(CrossTenantAction::AffectedTenantId)
                    .col(CrossTenantAction::OccurredAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(CrossTenantAction::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum CrossTenantAction {
    Table,
    Id,
    Uuid,
    ActorUserId,
    ActorEmail,
    ActorTenantId,
    AffectedTenantId,
    Kind,
    Method,
    Route,
    Path,
    OccurredAt,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

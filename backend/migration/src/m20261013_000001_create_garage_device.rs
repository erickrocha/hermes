use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// HRMS-512/515: device identity and one-time pairing state are separate from
/// human accounts. The credential itself is never persisted; its version is
/// checked on each monitor request so revocation takes effect immediately.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Device::Table)
                    .if_not_exists()
                    .col(pk_auto(Device::Id).integer())
                    .col(binary_len_uniq(Device::Uuid, 16))
                    .col(integer(Device::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_device_tenant")
                            .from(Device::Table, Device::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Device::Name, 120).not_null())
                    .col(integer(Device::CredentialVersion).not_null().default(1))
                    .col(string_len(Device::PairingCode, 32).null().unique_key())
                    .col(date_time(Device::PairingCodeExpiresAt).null())
                    .col(date_time(Device::PairedAt).null())
                    .col(date_time(Device::RevokedAt).null())
                    .col(date_time(Device::CreatedAt).null())
                    .col(string_len(Device::CreatedBy, 255).null())
                    .col(date_time(Device::UpdatedAt).null())
                    .col(string_len(Device::UpdatedBy, 255).null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Device::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Device {
    #[sea_orm(iden = "garage_device")]
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    CredentialVersion,
    PairingCode,
    PairingCodeExpiresAt,
    PairedAt,
    RevokedAt,
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

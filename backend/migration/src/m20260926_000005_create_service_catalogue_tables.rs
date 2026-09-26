use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-05-S01` (`HRMS-704`, `C-026`): the service-type and
/// priced-service catalogues.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ServiceType::Table)
                    .if_not_exists()
                    .col(pk_auto(ServiceType::Id).integer())
                    .col(binary_len_uniq(ServiceType::Uuid, 16))
                    .col(integer(ServiceType::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_service_type_tenant")
                            .from(ServiceType::Table, ServiceType::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(ServiceType::Code, 50).not_null())
                    .col(string_len(ServiceType::Name, 255).not_null())
                    .col(string_len_null(ServiceType::Category, 255))
                    .col(boolean(ServiceType::Active).not_null())
                    .col(date_time(ServiceType::CreatedAt).null())
                    .col(string_len(ServiceType::CreatedBy, 50).null())
                    .col(date_time(ServiceType::UpdatedAt).null())
                    .col(string_len(ServiceType::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_service_type_tenant_code")
                    .table(ServiceType::Table)
                    .col(ServiceType::TenantId)
                    .col(ServiceType::Code)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PricedService::Table)
                    .if_not_exists()
                    .col(pk_auto(PricedService::Id).integer())
                    .col(binary_len_uniq(PricedService::Uuid, 16))
                    .col(integer(PricedService::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_priced_service_tenant")
                            .from(PricedService::Table, PricedService::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(PricedService::Name, 255).not_null())
                    .col(string_len_null(PricedService::Category, 255))
                    .col(big_integer_null(PricedService::DefaultValueCents))
                    .col(string_len_null(PricedService::Observation, 500))
                    .col(date_time(PricedService::CreatedAt).null())
                    .col(string_len(PricedService::CreatedBy, 50).null())
                    .col(date_time(PricedService::UpdatedAt).null())
                    .col(string_len(PricedService::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PricedService::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ServiceType::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ServiceType {
    Table,
    Id,
    Uuid,
    TenantId,
    Code,
    Name,
    Category,
    Active,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
pub enum PricedService {
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    Category,
    DefaultValueCents,
    Observation,
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

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-01-S01` (`HRMS-956`, `C-028`): a tenant's catalogue of garage
/// services (`servicosGaragemModelos`, `TRM-430`). `name_key` is the
/// normalised name (`TRM-433`), unique per tenant.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Model::Table)
                    .if_not_exists()
                    .col(pk_auto(Model::Id).integer())
                    .col(binary_len_uniq(Model::Uuid, 16))
                    .col(integer(Model::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_model_tenant")
                            .from(Model::Table, Model::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Model::Name, 120).not_null())
                    .col(string_len(Model::NameKey, 120).not_null())
                    .col(integer(Model::DisplayOrder).not_null())
                    .col(boolean(Model::Active).not_null())
                    .col(string_len(Model::ServiceGroup, 20).not_null())
                    .col(boolean(Model::RequiredForDeparture).not_null())
                    .col(date_time(Model::CreatedAt).null())
                    .col(string_len(Model::CreatedBy, 50).null())
                    .col(date_time(Model::UpdatedAt).null())
                    .col(string_len(Model::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_garage_service_model_tenant_name")
                    .table(Model::Table)
                    .col(Model::TenantId)
                    .col(Model::NameKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Model::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Model {
    #[sea_orm(iden = "garage_service_model")]
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    NameKey,
    DisplayOrder,
    Active,
    ServiceGroup,
    RequiredForDeparture,
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

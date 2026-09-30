use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S10` (`HRMS-715`, `C-027`): the technical-inspection template
/// (`modelosInspecao`, `TRM-323`/`TRM-333`) and its ordered items.
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
                            .name("fk_inspection_model_tenant")
                            .from(Model::Table, Model::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Model::Name, 120).not_null())
                    .col(boolean(Model::GeneratesWorkOrder).not_null())
                    .col(integer_null(Model::PeriodicityDays))
                    .col(string_len_null(Model::Observation, 500))
                    .col(boolean(Model::Active).not_null())
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
                    .name("uq_inspection_model_tenant_name")
                    .table(Model::Table)
                    .col(Model::TenantId)
                    .col(Model::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Item::Table)
                    .if_not_exists()
                    .col(pk_auto(Item::Id).integer())
                    .col(binary_len_uniq(Item::Uuid, 16))
                    .col(integer(Item::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inspection_model_item_tenant")
                            .from(Item::Table, Item::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Item::InspectionModelId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inspection_model_item_model")
                            .from(Item::Table, Item::InspectionModelId)
                            .to(Model::Table, Model::Id),
                    )
                    .col(string_len(Item::Description, 200).not_null())
                    .col(date_time(Item::CreatedAt).null())
                    .col(string_len(Item::CreatedBy, 50).null())
                    .col(date_time(Item::UpdatedAt).null())
                    .col(string_len(Item::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Item::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Model::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Model {
    #[sea_orm(iden = "inspection_model")]
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    GeneratesWorkOrder,
    PeriodicityDays,
    Observation,
    Active,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Item {
    #[sea_orm(iden = "inspection_model_item")]
    Table,
    Id,
    Uuid,
    TenantId,
    InspectionModelId,
    Description,
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

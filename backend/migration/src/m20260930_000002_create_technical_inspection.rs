use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S09` (`HRMS-714`, `C-027`): a technical inspection and its item
/// answers (`TRM-333`). A non-conforming item keeps the work-order item it
/// matched or opened, so the inspection alone names every order it touched.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Inspection::Table)
                    .if_not_exists()
                    .col(pk_auto(Inspection::Id).integer())
                    .col(binary_len_uniq(Inspection::Uuid, 16))
                    .col(integer(Inspection::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_technical_inspection_tenant")
                            .from(Inspection::Table, Inspection::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Inspection::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_technical_inspection_vehicle")
                            .from(Inspection::Table, Inspection::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len(Inspection::ModelName, 120).not_null())
                    .col(date(Inspection::InspectedAt).not_null())
                    .col(double(Inspection::OdometerKm).not_null())
                    .col(string_len_null(Inspection::Observation, 500))
                    .col(date_time(Inspection::CreatedAt).null())
                    .col(string_len(Inspection::CreatedBy, 50).null())
                    .col(date_time(Inspection::UpdatedAt).null())
                    .col(string_len(Inspection::UpdatedBy, 50).null())
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
                            .name("fk_technical_inspection_item_tenant")
                            .from(Item::Table, Item::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Item::TechnicalInspectionId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_technical_inspection_item_inspection")
                            .from(Item::Table, Item::TechnicalInspectionId)
                            .to(Inspection::Table, Inspection::Id),
                    )
                    .col(string_len(Item::Description, 200).not_null())
                    .col(boolean(Item::Conforming).not_null())
                    .col(string_len_null(Item::Observation, 500))
                    .col(integer_null(Item::WorkOrderItemId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_technical_inspection_item_work_order_item")
                            .from(Item::Table, Item::WorkOrderItemId)
                            .to(WorkOrderItem::Table, WorkOrderItem::Id),
                    )
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
        manager.drop_table(Table::drop().table(Inspection::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Inspection {
    #[sea_orm(iden = "technical_inspection")]
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    #[sea_orm(iden = "inspection_model")]
    ModelName,
    InspectedAt,
    OdometerKm,
    Observation,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Item {
    #[sea_orm(iden = "technical_inspection_item")]
    Table,
    Id,
    Uuid,
    TenantId,
    TechnicalInspectionId,
    Description,
    Conforming,
    Observation,
    WorkOrderItemId,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WorkOrderItem {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

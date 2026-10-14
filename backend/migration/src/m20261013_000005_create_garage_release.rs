use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Release::Table)
                    .if_not_exists()
                    .col(pk_auto(Release::Id).integer())
                    .col(binary_len_uniq(Release::Uuid, 16))
                    .col(integer(Release::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_tenant")
                            .from(Release::Table, Release::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Release::AttendanceId).not_null().unique_key())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_attendance")
                            .from(Release::Table, Release::AttendanceId)
                            .to(Attendance::Table, Attendance::Id),
                    )
                    .col(integer(Release::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_vehicle")
                            .from(Release::Table, Release::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(integer(Release::WorkOrderId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_work_order")
                            .from(Release::Table, Release::WorkOrderId)
                            .to(WorkOrder::Table, WorkOrder::Id),
                    )
                    .col(text(Release::Justification).not_null())
                    .col(integer(Release::ResponsibleUserId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_responsible_user")
                            .from(Release::Table, Release::ResponsibleUserId)
                            .to(User::Table, User::Id),
                    )
                    .col(date_time(Release::ReleasedAt).not_null())
                    .col(date_time(Release::CreatedAt).null())
                    .col(string_len(Release::CreatedBy, 50).null())
                    .col(date_time(Release::UpdatedAt).null())
                    .col(string_len(Release::UpdatedBy, 50).null())
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
                            .name("fk_garage_release_item_tenant")
                            .from(Item::Table, Item::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Item::GarageReleaseId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_item_release")
                            .from(Item::Table, Item::GarageReleaseId)
                            .to(Release::Table, Release::Id),
                    )
                    .col(integer(Item::GarageServiceId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_item_service")
                            .from(Item::Table, Item::GarageServiceId)
                            .to(GarageService::Table, GarageService::Id),
                    )
                    .col(integer(Item::WorkOrderItemId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_release_item_work_order_item")
                            .from(Item::Table, Item::WorkOrderItemId)
                            .to(WorkOrderItem::Table, WorkOrderItem::Id),
                    )
                    .col(string_len(Item::ServiceName, 120).not_null())
                    .col(string_len(Item::GarageState, 30).not_null())
                    .col(string_len(Item::WorkOrderStatus, 30).not_null())
                    .col(date_time(Item::CreatedAt).null())
                    .col(string_len(Item::CreatedBy, 50).null())
                    .col(date_time(Item::UpdatedAt).null())
                    .col(string_len(Item::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_garage_release_item_service")
                    .table(Item::Table)
                    .col(Item::GarageReleaseId)
                    .col(Item::GarageServiceId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Item::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Release::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Release {
    #[sea_orm(iden = "garage_release")]
    Table,
    Id,
    Uuid,
    TenantId,
    AttendanceId,
    VehicleId,
    WorkOrderId,
    Justification,
    ResponsibleUserId,
    ReleasedAt,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Item {
    #[sea_orm(iden = "garage_release_item")]
    Table,
    Id,
    Uuid,
    TenantId,
    GarageReleaseId,
    GarageServiceId,
    WorkOrderItemId,
    ServiceName,
    GarageState,
    WorkOrderStatus,
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
enum Attendance {
    #[sea_orm(iden = "garage_attendance")]
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Vehicle {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum WorkOrder {
    #[sea_orm(iden = "work_order")]
    Table,
    Id,
}
#[derive(DeriveIden)]
enum WorkOrderItem {
    #[sea_orm(iden = "work_order_item")]
    Table,
    Id,
}
#[derive(DeriveIden)]
enum GarageService {
    #[sea_orm(iden = "garage_service")]
    Table,
    Id,
}
#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-03-S01` (`HRMS-703`, `C-026`): a planned maintenance window,
/// plus the `work_order.maintenance_plan_id` FK that links a work order
/// into one -- a single normalized relation, not legacy's `os_id`/`os_ids[]`
/// pair.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MaintenancePlan::Table)
                    .if_not_exists()
                    .col(pk_auto(MaintenancePlan::Id).integer())
                    .col(binary_len_uniq(MaintenancePlan::Uuid, 16))
                    .col(integer(MaintenancePlan::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_plan_tenant")
                            .from(MaintenancePlan::Table, MaintenancePlan::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(MaintenancePlan::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_plan_vehicle")
                            .from(MaintenancePlan::Table, MaintenancePlan::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date(MaintenancePlan::Date).not_null())
                    .col(date_time_null(MaintenancePlan::PlannedStart))
                    .col(date_time_null(MaintenancePlan::PlannedEnd))
                    .col(string_len(MaintenancePlan::Status, 50).not_null())
                    .col(boolean(MaintenancePlan::AffectsSchedule).not_null())
                    .col(string_len(MaintenancePlan::Origin, 50).not_null())
                    .col(date_time(MaintenancePlan::CreatedAt).null())
                    .col(string_len(MaintenancePlan::CreatedBy, 50).null())
                    .col(date_time(MaintenancePlan::UpdatedAt).null())
                    .col(string_len(MaintenancePlan::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_maintenance_plan_vehicle_date")
                    .table(MaintenancePlan::Table)
                    .col(MaintenancePlan::VehicleId)
                    .col(MaintenancePlan::Date)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrder::Table)
                    .add_column(integer_null(WorkOrder::MaintenancePlanId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_work_order_maintenance_plan")
                    .from(WorkOrder::Table, WorkOrder::MaintenancePlanId)
                    .to(MaintenancePlan::Table, MaintenancePlan::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrder::Table)
                    .drop_foreign_key(Alias::new("fk_work_order_maintenance_plan"))
                    .drop_column(WorkOrder::MaintenancePlanId)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(MaintenancePlan::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MaintenancePlan {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    Date,
    PlannedStart,
    PlannedEnd,
    Status,
    AffectsSchedule,
    Origin,
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
enum Vehicle {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    MaintenancePlanId,
}

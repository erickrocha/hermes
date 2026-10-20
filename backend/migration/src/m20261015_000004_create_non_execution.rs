use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-04` (`C-026`, `TRM-260...273`): one history record per work order
/// and the occurrences it accumulates. `(tenant_id, business_key)` is unique, so
/// an occurrence can never be recorded twice however often the evaluation runs.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NonExecutionRecord::Table)
                    .if_not_exists()
                    .col(pk_auto(NonExecutionRecord::Id).integer())
                    .col(binary_len_uniq(NonExecutionRecord::Uuid, 16))
                    .col(integer(NonExecutionRecord::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_non_execution_record_tenant")
                            .from(NonExecutionRecord::Table, NonExecutionRecord::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer_uniq(NonExecutionRecord::WorkOrderId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_non_execution_record_work_order")
                            .from(NonExecutionRecord::Table, NonExecutionRecord::WorkOrderId)
                            .to(WorkOrder::Table, WorkOrder::Id),
                    )
                    .col(integer(NonExecutionRecord::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_non_execution_record_vehicle")
                            .from(NonExecutionRecord::Table, NonExecutionRecord::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date_time(NonExecutionRecord::CreatedAt).null())
                    .col(string_len(NonExecutionRecord::CreatedBy, 50).null())
                    .col(date_time(NonExecutionRecord::UpdatedAt).null())
                    .col(string_len(NonExecutionRecord::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(NonExecutionOccurrence::Table)
                    .if_not_exists()
                    .col(pk_auto(NonExecutionOccurrence::Id).integer())
                    .col(binary_len_uniq(NonExecutionOccurrence::Uuid, 16))
                    .col(integer(NonExecutionOccurrence::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_non_execution_occurrence_tenant")
                            .from(NonExecutionOccurrence::Table, NonExecutionOccurrence::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(NonExecutionOccurrence::RecordId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_non_execution_occurrence_record")
                            .from(NonExecutionOccurrence::Table, NonExecutionOccurrence::RecordId)
                            .to(NonExecutionRecord::Table, NonExecutionRecord::Id),
                    )
                    .col(integer(NonExecutionOccurrence::MaintenancePlanId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_non_execution_occurrence_plan")
                            .from(
                                NonExecutionOccurrence::Table,
                                NonExecutionOccurrence::MaintenancePlanId,
                            )
                            .to(MaintenancePlan::Table, MaintenancePlan::Id),
                    )
                    .col(string_len(NonExecutionOccurrence::Kind, 30).not_null())
                    .col(string_len(NonExecutionOccurrence::BusinessKey, 200).not_null())
                    .col(date_time(NonExecutionOccurrence::WindowStart).not_null())
                    .col(date_time(NonExecutionOccurrence::WindowEnd).not_null())
                    .col(date_time(NonExecutionOccurrence::ArrivedAt).null())
                    .col(date_time(NonExecutionOccurrence::DepartedAt).null())
                    .col(integer(NonExecutionOccurrence::DwellMinutes).null())
                    .col(integer(NonExecutionOccurrence::MinutesInWindow).null())
                    .col(string_len(NonExecutionOccurrence::Justification, 1000).null())
                    .col(string_len(NonExecutionOccurrence::JustifiedBy, 50).null())
                    .col(date_time(NonExecutionOccurrence::JustifiedAt).null())
                    .col(date_time(NonExecutionOccurrence::CreatedAt).null())
                    .col(string_len(NonExecutionOccurrence::CreatedBy, 50).null())
                    .col(date_time(NonExecutionOccurrence::UpdatedAt).null())
                    .col(string_len(NonExecutionOccurrence::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_non_execution_occurrence_tenant_key")
                    .table(NonExecutionOccurrence::Table)
                    .col(NonExecutionOccurrence::TenantId)
                    .col(NonExecutionOccurrence::BusinessKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NonExecutionOccurrence::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NonExecutionRecord::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum NonExecutionRecord {
    Table,
    Id,
    Uuid,
    TenantId,
    WorkOrderId,
    VehicleId,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum NonExecutionOccurrence {
    Table,
    Id,
    Uuid,
    TenantId,
    RecordId,
    MaintenancePlanId,
    Kind,
    BusinessKey,
    WindowStart,
    WindowEnd,
    ArrivedAt,
    DepartedAt,
    DwellMinutes,
    MinutesInWindow,
    Justification,
    JustifiedBy,
    JustifiedAt,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum MaintenancePlan {
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
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

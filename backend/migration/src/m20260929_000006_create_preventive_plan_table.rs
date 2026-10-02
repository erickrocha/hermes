use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S01` (`HRMS-706`, `C-027`): preventive-maintenance plans.
/// `uq_preventive_plan_vehicle_name` is `TRM-300`'s "at most one plan per
/// vehicle and plan type."
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PreventivePlan::Table)
                    .if_not_exists()
                    .col(pk_auto(PreventivePlan::Id).integer())
                    .col(binary_len_uniq(PreventivePlan::Uuid, 16))
                    .col(integer(PreventivePlan::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_tenant")
                            .from(PreventivePlan::Table, PreventivePlan::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(PreventivePlan::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_vehicle")
                            .from(PreventivePlan::Table, PreventivePlan::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len(PreventivePlan::PlanName, 255).not_null())
                    .col(string_len(PreventivePlan::ControlType, 20).not_null())
                    .col(double_null(PreventivePlan::IntervalKm))
                    .col(integer_null(PreventivePlan::IntervalDays))
                    .col(double_null(PreventivePlan::LastServiceKm))
                    .col(date_null(PreventivePlan::LastServiceDate))
                    .col(date_time(PreventivePlan::CreatedAt).null())
                    .col(string_len(PreventivePlan::CreatedBy, 50).null())
                    .col(date_time(PreventivePlan::UpdatedAt).null())
                    .col(string_len(PreventivePlan::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_preventive_plan_vehicle_name")
                    .table(PreventivePlan::Table)
                    .col(PreventivePlan::VehicleId)
                    .col(PreventivePlan::PlanName)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PreventivePlan::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum PreventivePlan {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    PlanName,
    ControlType,
    IntervalKm,
    IntervalDays,
    LastServiceKm,
    LastServiceDate,
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
enum Tenant {
    Table,
    Id,
}

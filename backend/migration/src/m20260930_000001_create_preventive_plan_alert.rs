use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S08` (`HRMS-713`, `C-027`): an intermediate inspection alert
/// declared on a preventive plan (`TRM-323…325`). `discharged_cycle` holds the
/// plan's cycle key (`last_service_km|last_service_date`) at discharge, so a
/// real service -- which changes the key -- reopens the alert (`TRM-325`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alert::Table)
                    .if_not_exists()
                    .col(pk_auto(Alert::Id).integer())
                    .col(binary_len_uniq(Alert::Uuid, 16))
                    .col(integer(Alert::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_alert_tenant")
                            .from(Alert::Table, Alert::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Alert::PreventivePlanId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_alert_plan")
                            .from(Alert::Table, Alert::PreventivePlanId)
                            .to(PreventivePlan::Table, PreventivePlan::Id),
                    )
                    .col(double(Alert::AtKm).not_null())
                    .col(string_len(Alert::Title, 120).not_null())
                    .col(string_len(Alert::InspectionModel, 120).not_null())
                    .col(string_len_null(Alert::DischargedCycle, 80))
                    .col(date_time(Alert::CreatedAt).null())
                    .col(string_len(Alert::CreatedBy, 50).null())
                    .col(date_time(Alert::UpdatedAt).null())
                    .col(string_len(Alert::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_preventive_plan_alert_plan_km")
                    .table(Alert::Table)
                    .col(Alert::PreventivePlanId)
                    .col(Alert::AtKm)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Alert::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Alert {
    #[sea_orm(iden = "preventive_plan_alert")]
    Table,
    Id,
    Uuid,
    TenantId,
    PreventivePlanId,
    AtKm,
    Title,
    InspectionModel,
    DischargedCycle,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum PreventivePlan {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

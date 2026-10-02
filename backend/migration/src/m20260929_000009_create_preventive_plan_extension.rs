use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-07-S05` (`HRMS-710`, `C-027`): a preventive-plan extension after a
/// technical inspection (`TRM-312…320`). `preventive_plan_extension` is the
/// auditable entry (`TRM-317`) and is never deleted; `preventive_plan.
/// extension_limit_km` is the active limit derived from the latest entry.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PreventivePlan::Table)
                    .add_column(double_null(PreventivePlan::ExtensionLimitKm))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Extension::Table)
                    .if_not_exists()
                    .col(pk_auto(Extension::Id).integer())
                    .col(binary_len_uniq(Extension::Uuid, 16))
                    .col(integer(Extension::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_extension_tenant")
                            .from(Extension::Table, Extension::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Extension::PreventivePlanId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_extension_plan")
                            .from(Extension::Table, Extension::PreventivePlanId)
                            .to(PreventivePlan::Table, PreventivePlan::Id),
                    )
                    .col(integer(Extension::WorkOrderItemId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preventive_plan_extension_item")
                            .from(Extension::Table, Extension::WorkOrderItemId)
                            .to(WorkOrderItem::Table, WorkOrderItem::Id),
                    )
                    .col(double(Extension::InspectionKm).not_null())
                    .col(double(Extension::GrantedKm).not_null())
                    .col(double(Extension::ResultingLimitKm).not_null())
                    .col(string_len(Extension::Description, 500).not_null())
                    .col(double_null(Extension::PreviousLastServiceKm))
                    .col(date_null(Extension::PreviousLastServiceDate))
                    .col(date_time(Extension::CreatedAt).null())
                    .col(string_len(Extension::CreatedBy, 50).null())
                    .col(date_time(Extension::UpdatedAt).null())
                    .col(string_len(Extension::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Extension::Table).to_owned()).await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PreventivePlan::Table)
                    .drop_column(PreventivePlan::ExtensionLimitKm)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Extension {
    #[sea_orm(iden = "preventive_plan_extension")]
    Table,
    Id,
    Uuid,
    TenantId,
    PreventivePlanId,
    WorkOrderItemId,
    InspectionKm,
    GrantedKm,
    ResultingLimitKm,
    Description,
    PreviousLastServiceKm,
    PreviousLastServiceDate,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum PreventivePlan {
    Table,
    Id,
    ExtensionLimitKm,
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

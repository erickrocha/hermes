use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `HRMS-954` (owner ruling 2026-09-30): the legacy `[TC]` values of the fuel
/// average, the receipt and reconciliation windows and the preventive margins,
/// as one tenant's own settings -- at most one row per tenant. No row means the
/// legacy values, which are only defaults: on hermes Transmega is one tenant.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Setting::Table)
                    .if_not_exists()
                    .col(pk_auto(Setting::Id).integer())
                    .col(binary_len_uniq(Setting::Uuid, 16))
                    .col(integer(Setting::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_rule_setting_tenant")
                            .from(Setting::Table, Setting::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(double(Setting::AvgMaxSegmentKm).not_null())
                    .col(double(Setting::AvgMinKmPerLiter).not_null())
                    .col(double(Setting::AvgMaxKmPerLiter).not_null())
                    .col(integer(Setting::AvgRecentWindowDays).not_null())
                    .col(integer(Setting::AvgFallbackSegments).not_null())
                    .col(integer(Setting::AvgMedianMinSegments).not_null())
                    .col(double(Setting::AvgMedianTolerance).not_null())
                    .col(integer(Setting::AvgMedianMinSurvivors).not_null())
                    .col(integer(Setting::AvgMinSegmentsForOwnAverage).not_null())
                    .col(double(Setting::ReceiptDuplicateVolumeToleranceLiters).not_null())
                    .col(integer(Setting::ReceiptDuplicateWindowMinutes).not_null())
                    .col(integer(Setting::ReconciliationWindowHours).not_null())
                    .col(double(Setting::ReconciliationVolumeToleranceLiters).not_null())
                    .col(double(Setting::ReconciliationVolumeToleranceRatio).not_null())
                    .col(double(Setting::PreventiveAttentionKmMargin).not_null())
                    .col(integer(Setting::PreventiveAttentionDaysMargin).not_null())
                    .col(double(Setting::PreventiveMaxExtensionKm).not_null())
                    .col(date_time(Setting::CreatedAt).null())
                    .col(string_len(Setting::CreatedBy, 50).null())
                    .col(date_time(Setting::UpdatedAt).null())
                    .col(string_len(Setting::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_tenant_rule_setting_tenant")
                    .table(Setting::Table)
                    .col(Setting::TenantId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Setting::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    Id,
    Uuid,
    TenantId,
    AvgMaxSegmentKm,
    AvgMinKmPerLiter,
    AvgMaxKmPerLiter,
    AvgRecentWindowDays,
    AvgFallbackSegments,
    AvgMedianMinSegments,
    AvgMedianTolerance,
    AvgMedianMinSurvivors,
    AvgMinSegmentsForOwnAverage,
    ReceiptDuplicateVolumeToleranceLiters,
    ReceiptDuplicateWindowMinutes,
    ReconciliationWindowHours,
    ReconciliationVolumeToleranceLiters,
    ReconciliationVolumeToleranceRatio,
    PreventiveAttentionKmMargin,
    PreventiveAttentionDaysMargin,
    PreventiveMaxExtensionKm,
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

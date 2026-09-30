use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-06-S03` (`HRMS-953`, `PD-016`'s `[TC]` values): a tenant's own
/// tank-gauge thresholds, at most one row per tenant. No row means the
/// platform defaults, which are the legacy values -- Transmega is one tenant.
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
                            .name("fk_fuel_gauge_setting_tenant")
                            .from(Setting::Table, Setting::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(double(Setting::SuspectMarginRatio).not_null())
                    .col(double(Setting::SetAsideMinExpectedLiters).not_null())
                    .col(double(Setting::SetAsideRatio).not_null())
                    .col(double(Setting::LargeFuellingRatio).not_null())
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
                    .name("uq_fuel_gauge_setting_tenant")
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
    #[sea_orm(iden = "fuel_gauge_setting")]
    Table,
    Id,
    Uuid,
    TenantId,
    SuspectMarginRatio,
    SetAsideMinExpectedLiters,
    SetAsideRatio,
    LargeFuellingRatio,
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

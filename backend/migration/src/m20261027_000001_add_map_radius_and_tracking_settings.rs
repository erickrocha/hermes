use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `HRMS-900`..`905`: the GPS provider account moves from the deployment
/// environment to the tenant, and the TV monitor's map gets its radius.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RuleSetting::Table)
                    .add_column(double(RuleSetting::GarageMapRadiusKm).not_null().default(100.0))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(TrackingSetting::Table)
                    .if_not_exists()
                    .col(pk_auto(TrackingSetting::Id).integer())
                    .col(binary_len_uniq(TrackingSetting::Uuid, 16))
                    .col(integer_uniq(TrackingSetting::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_tracking_setting_tenant")
                            .from(TrackingSetting::Table, TrackingSetting::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(TrackingSetting::BaseUrl, 255).not_null())
                    .col(string_len(TrackingSetting::Email, 255).not_null())
                    .col(string_len(TrackingSetting::Password, 255).not_null())
                    .col(date_time(TrackingSetting::CreatedAt).null())
                    .col(string_len(TrackingSetting::CreatedBy, 50).null())
                    .col(date_time(TrackingSetting::UpdatedAt).null())
                    .col(string_len(TrackingSetting::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(TrackingSetting::Table).to_owned()).await?;
        manager
            .alter_table(
                Table::alter().table(RuleSetting::Table).drop_column(RuleSetting::GarageMapRadiusKm).to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RuleSetting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageMapRadiusKm,
}

#[derive(DeriveIden)]
enum TrackingSetting {
    #[sea_orm(iden = "tenant_tracking_setting")]
    Table,
    Id,
    Uuid,
    TenantId,
    BaseUrl,
    Email,
    Password,
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

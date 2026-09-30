use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-05` (`HRMS-962`/`963`, `C-028`): the manager's manual call to base
/// (`TRM-483`) and the tenant's tank-alert cuts (`TRM-474`, `[TC]` 40 % / 95 %).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(double(Setting::GarageAlertLowPercent).not_null().default(40.0))
                    .add_column(double(Setting::GarageAlertTripPercent).not_null().default(95.0))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Call::Table)
                    .if_not_exists()
                    .col(pk_auto(Call::Id).integer())
                    .col(binary_len_uniq(Call::Uuid, 16))
                    .col(integer(Call::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_call_tenant")
                            .from(Call::Table, Call::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Call::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_call_vehicle")
                            .from(Call::Table, Call::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date_time(Call::CalledAt).not_null())
                    .col(integer_null(Call::CalledByUserId))
                    .col(date_time_null(Call::CancelledAt))
                    .col(date_time(Call::CreatedAt).null())
                    .col(string_len(Call::CreatedBy, 50).null())
                    .col(date_time(Call::UpdatedAt).null())
                    .col(string_len(Call::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Call::Table).to_owned()).await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::GarageAlertLowPercent)
                    .drop_column(Setting::GarageAlertTripPercent)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Call {
    #[sea_orm(iden = "garage_call")]
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    CalledAt,
    CalledByUserId,
    CancelledAt,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageAlertLowPercent,
    GarageAlertTripPercent,
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

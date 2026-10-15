use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-01-S02` (`HRMS-601`, `C-024`): a customer's day off. References
/// `customer` by internal id (not uuid) -- same shape
/// `vehicle_assignment`'s `vehicle_id`/`driver_id` use.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CustomerDayOff::Table)
                    .if_not_exists()
                    .col(pk_auto(CustomerDayOff::Id).integer())
                    .col(binary_len_uniq(CustomerDayOff::Uuid, 16))
                    .col(integer(CustomerDayOff::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customer_day_off_tenant")
                            .from(CustomerDayOff::Table, CustomerDayOff::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(CustomerDayOff::CustomerId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customer_day_off_customer")
                            .from(CustomerDayOff::Table, CustomerDayOff::CustomerId)
                            .to(Customer::Table, Customer::Id),
                    )
                    .col(date(CustomerDayOff::Date).not_null())
                    .col(string_len_null(CustomerDayOff::Reason, 500))
                    .col(date_time(CustomerDayOff::CreatedAt).null())
                    .col(string_len(CustomerDayOff::CreatedBy, 50).null())
                    .col(date_time(CustomerDayOff::UpdatedAt).null())
                    .col(string_len(CustomerDayOff::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // Guards against re-recording the same customer's same day off twice,
        // the same low-value-duplicate protection `uq_vehicle_tenant_plate`
        // gives the vehicle register.
        manager
            .create_index(
                Index::create()
                    .name("uq_customer_day_off_customer_date")
                    .table(CustomerDayOff::Table)
                    .col(CustomerDayOff::CustomerId)
                    .col(CustomerDayOff::Date)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CustomerDayOff::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum CustomerDayOff {
    Table,
    Id,
    Uuid,
    TenantId,
    CustomerId,
    Date,
    Reason,
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
enum Customer {
    Table,
    Id,
}

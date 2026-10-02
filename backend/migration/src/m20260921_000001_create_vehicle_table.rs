use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Vehicle::Table)
                    .if_not_exists()
                    .col(pk_auto(Vehicle::Id).integer())
                    .col(binary_len_uniq(Vehicle::Uuid, 16))
                    // EPIC-FO-01-S02 (HRMS-921, D-09): a vehicle belongs to
                    // exactly one tenant, so the column is NOT NULL at the
                    // schema level rather than left nullable and enforced only
                    // by the audit macro.
                    .col(integer(Vehicle::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_tenant")
                            .from(Vehicle::Table, Vehicle::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(Vehicle::Plate, 32).not_null())
                    .col(string_len(Vehicle::Model, 200).not_null())
                    // EPIC-FO-01-S03 (HRMS-922, D-23(b)): the vocabulary is a
                    // Rust enum persisted as its own name. No database enum
                    // type, so adding a status later is a code change and not
                    // a table rewrite.
                    .col(string_len(Vehicle::Status, 50).not_null())
                    .col(date_time(Vehicle::CreatedAt).null())
                    .col(string_len(Vehicle::CreatedBy, 50).null())
                    .col(date_time(Vehicle::UpdatedAt).null())
                    .col(string_len(Vehicle::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // EPIC-FO-01-S06 (HRMS-925, D-23(c)): unique *within a tenant*, the
        // same shape as `uq_tenant_country_tax_id` (HRMS-210). Platform-wide
        // uniqueness was rejected deliberately: a duplicate-plate rejection
        // would tell one customer that another customer already registered
        // that vehicle, which is the cross-tenant disclosure PD-034 exists to
        // prevent.
        manager
            .create_index(
                Index::create()
                    .name("uq_vehicle_tenant_plate")
                    .table(Vehicle::Table)
                    .col(Vehicle::TenantId)
                    .col(Vehicle::Plate)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Dropping the table drops its indexes with it. Dropping
        // `uq_vehicle_tenant_plate` first fails on MariaDB (error 1553): the
        // index leads with `tenant_id` and backs the tenant foreign key.
        manager
            .drop_table(Table::drop().table(Vehicle::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Vehicle {
    Table,
    Id,
    Uuid,
    TenantId,
    Plate,
    Model,
    Status,
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

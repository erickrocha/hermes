use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    // INT, não BIGINT: MariaDB exige tipos idênticos nos dois
                    // lados de uma FK, e todas as chaves primárias deste schema
                    // são INT (`pk_auto(..).integer()`). Com BIGINT aqui a
                    // constraint falhava com errno 150 e a cadeia inteira de
                    // migrações não subia em banco novo — nenhum ambiente podia
                    // ser provisionado do zero.
                    .add_column(ColumnDef::new(Tenant::BusinessPlanId).integer().null())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_tenant_business_plan")
                            .from_tbl(Tenant::Table)
                            .from_col(Tenant::BusinessPlanId)
                            .to_tbl(BusinessPlan::Table)
                            .to_col(BusinessPlan::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // `tenant_plan` only exists on a database that already ran migration
        // `000005` (dev instances created before this change). A fresh database
        // never creates it, so both the data carry-over and the drop are
        // conditional on the table actually being there.
        if manager.has_table("tenant_plan").await? {
            manager
                .get_connection()
                .execute_unprepared(
                    "UPDATE tenant t \
                     JOIN ( \
                         SELECT tenant_id, business_plan_id, \
                                ROW_NUMBER() OVER (PARTITION BY tenant_id ORDER BY id DESC) AS rn \
                         FROM tenant_plan \
                         WHERE active = TRUE \
                     ) current_plan ON current_plan.tenant_id = t.id AND current_plan.rn = 1 \
                     SET t.business_plan_id = current_plan.business_plan_id",
                )
                .await?;

            manager
                .drop_table(Table::drop().table(TenantPlan::Table).to_owned())
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TenantPlan::Table)
                    .if_not_exists()
                    .col(pk_auto(TenantPlan::Id).integer())
                    .col(binary_len_uniq(TenantPlan::Uuid, 16))
                    .col(integer(TenantPlan::TenantId).not_null())
                    .col(integer(TenantPlan::BusinessPlanId).not_null())
                    .col(date(TenantPlan::PaymentDate).not_null())
                    .col(boolean(TenantPlan::Active).not_null().default(true))
                    .col(date_time(TenantPlan::CreatedAt).null())
                    .col(string_len(TenantPlan::CreatedBy, 200).null())
                    .col(date_time(TenantPlan::UpdatedAt).null())
                    .col(string_len(TenantPlan::UpdatedBy, 200).null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_plan_tenant")
                            .from(TenantPlan::Table, TenantPlan::TenantId)
                            .to(Tenant::Table, Tenant::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_plan_business_plan")
                            .from(TenantPlan::Table, TenantPlan::BusinessPlanId)
                            .to(BusinessPlan::Table, BusinessPlan::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .engine("InnoDB")
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tenant::Table)
                    .drop_foreign_key(Alias::new("fk_tenant_business_plan"))
                    .drop_column(Tenant::BusinessPlanId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
    BusinessPlanId,
}

#[derive(DeriveIden)]
enum BusinessPlan {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TenantPlan {
    Table,
    Id,
    Uuid,
    TenantId,
    BusinessPlanId,
    PaymentDate,
    Active,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

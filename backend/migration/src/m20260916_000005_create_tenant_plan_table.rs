use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TenantPlan::Table).to_owned())
            .await
    }
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

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id
}

#[derive(DeriveIden)]
enum BusinessPlan {
    Table,
    Id,
}

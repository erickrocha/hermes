use sea_orm_migration::sea_query::Check;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BusinessPlan::Table)
                    .col(pk_auto(BusinessPlan::Id).integer())
                    .col(binary_len_uniq(BusinessPlan::Uuid, 16))
                    .col(string_len(BusinessPlan::Name, 200))
                    .col(big_integer(BusinessPlan::PriceInCents))
                    .col(integer(BusinessPlan::AvailableUsers))
                    .col(integer(BusinessPlan::PeriodDays))
                    .col(date(BusinessPlan::PaymentDate))
                    .col(date_time(BusinessPlan::CreatedAt))
                    .col(string_len(BusinessPlan::CreatedBy, 200).null())
                    .col(date_time(BusinessPlan::UpdatedAt))
                    .col(string_len(BusinessPlan::UpdatedBy, 200).null())
                    .check(Check::named(
                        Alias::new("ck_tenant_plan_available_users").into_iden(),
                        Expr::col(BusinessPlan::AvailableUsers).gt(0),
                    ))
                    .check(Check::named(
                        Alias::new("ck_tenant_plan_period_days").into_iden(),
                        Expr::col(BusinessPlan::PeriodDays).gt(0),
                    ))
                    .check(Check::named(
                        Alias::new("ck_tenant_plan_price").into_iden(),
                        Expr::col(BusinessPlan::PriceInCents).gte(0),
                    ))
                    .engine("InnoDB")
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BusinessPlan::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BusinessPlan {
    Table,
    Id,
    Uuid,
    Name,
    PriceInCents,
    AvailableUsers,
    PeriodDays,
    PaymentDate,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

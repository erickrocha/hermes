use sea_orm_migration::sea_query::Check;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `D-21`/`PD-042`: tenant billing configuration and a recoverable charge ledger.
/// The current plan remains on `tenant` (PD-021); this table stores only billing state.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TenantBilling::Table)
                    .col(pk_auto(TenantBilling::Id).integer())
                    .col(binary_len_uniq(TenantBilling::Uuid, 16))
                    .col(integer(TenantBilling::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_billing_tenant")
                            .from(TenantBilling::Table, TenantBilling::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer_null(TenantBilling::GraceDays))
                    .col(date_time_null(TenantBilling::CycleStartedAt))
                    .col(integer_null(TenantBilling::PendingBusinessPlanId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_billing_pending_plan")
                            .from(TenantBilling::Table, TenantBilling::PendingBusinessPlanId)
                            .to(BusinessPlan::Table, BusinessPlan::Id),
                    )
                    .col(date_time_null(TenantBilling::SuspendedAt))
                    .col(string_len_null(TenantBilling::SuspendedBy, 200))
                    .col(date_time(TenantBilling::CreatedAt))
                    .col(string_len_null(TenantBilling::CreatedBy, 200))
                    .col(date_time(TenantBilling::UpdatedAt))
                    .col(string_len_null(TenantBilling::UpdatedBy, 200))
                    .check(Check::named(
                        Alias::new("ck_tenant_billing_grace_days").into_iden(),
                        Expr::col(TenantBilling::GraceDays).gte(0),
                    ))
                    .engine("InnoDB")
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_tenant_billing_tenant")
                    .table(TenantBilling::Table)
                    .col(TenantBilling::TenantId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(BillingCharge::Table)
                    .if_not_exists()
                    .col(pk_auto(BillingCharge::Id).integer())
                    .col(binary_len_uniq(BillingCharge::Uuid, 16))
                    .col(integer(BillingCharge::TenantId).not_null())
                    .col(binary_len(BillingCharge::TenantUuid, 16).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_billing_charge_tenant")
                            .from(BillingCharge::Table, BillingCharge::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(date_time(BillingCharge::PeriodStart).not_null())
                    .col(date_time(BillingCharge::PeriodEnd).not_null())
                    .col(string_len(BillingCharge::TenantName, 200).not_null())
                    .col(string_len(BillingCharge::PlanName, 200).not_null())
                    .col(string_len(BillingCharge::Currency, 3).not_null().default("BRL"))
                    .col(big_integer(BillingCharge::PriceInCents).not_null())
                    .col(big_integer(BillingCharge::PricePerExtraUserInCents).not_null())
                    .col(integer(BillingCharge::IncludedUsers).not_null())
                    .col(integer(BillingCharge::ActiveUsers).not_null())
                    .col(integer(BillingCharge::ExtraUsers).not_null())
                    .col(integer(BillingCharge::GraceDays).not_null())
                    .col(big_integer(BillingCharge::TotalInCents).not_null())
                    .col(string_len(BillingCharge::Status, 20).not_null().default("Pending"))
                    .col(date_time_null(BillingCharge::PaidAt))
                    .col(string_len_null(BillingCharge::PaidBy, 200))
                    .col(date_time(BillingCharge::CreatedAt))
                    .col(string_len_null(BillingCharge::CreatedBy, 200))
                    .col(date_time(BillingCharge::UpdatedAt))
                    .col(string_len_null(BillingCharge::UpdatedBy, 200))
                    .check(Check::named(
                        Alias::new("ck_billing_charge_amounts").into_iden(),
                        Expr::col(BillingCharge::PriceInCents)
                            .gte(0)
                            .and(Expr::col(BillingCharge::PricePerExtraUserInCents).gte(0))
                            .and(Expr::col(BillingCharge::TotalInCents).gte(0)),
                    ))
                    .check(Check::named(
                        Alias::new("ck_billing_charge_user_counts").into_iden(),
                        Expr::col(BillingCharge::IncludedUsers)
                            .gte(0)
                            .and(Expr::col(BillingCharge::ActiveUsers).gte(0))
                            .and(Expr::col(BillingCharge::ExtraUsers).gte(0))
                            .and(Expr::col(BillingCharge::GraceDays).gte(0)),
                    ))
                    .check(Check::named(
                        Alias::new("ck_billing_charge_period").into_iden(),
                        Expr::col(BillingCharge::PeriodEnd).gt(Expr::col(BillingCharge::PeriodStart)),
                    ))
                    .engine("InnoDB")
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_billing_charge_tenant_period")
                    .table(BillingCharge::Table)
                    .col(BillingCharge::TenantId)
                    .col(BillingCharge::PeriodStart)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BillingCharge::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TenantBilling::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TenantBilling {
    Table,
    Id,
    Uuid,
    TenantId,
    GraceDays,
    CycleStartedAt,
    PendingBusinessPlanId,
    SuspendedAt,
    SuspendedBy,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum BillingCharge {
    Table,
    Id,
    Uuid,
    TenantId,
    TenantUuid,
    PeriodStart,
    PeriodEnd,
    TenantName,
    PlanName,
    Currency,
    PriceInCents,
    PricePerExtraUserInCents,
    IncludedUsers,
    ActiveUsers,
    ExtraUsers,
    GraceDays,
    TotalInCents,
    Status,
    PaidAt,
    PaidBy,
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
enum BusinessPlan {
    Table,
    Id,
}
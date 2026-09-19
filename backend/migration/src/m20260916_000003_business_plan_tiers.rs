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
                    .table(BusinessPlanTier::Table)
                    .if_not_exists()
                    .col(pk_auto(BusinessPlanTier::Id).integer())
                    .col(binary_len_uniq(BusinessPlanTier::Uuid, 16))
                    .col(integer(BusinessPlanTier::BusinessPlanId).not_null())
                    .col(integer(BusinessPlanTier::UpToUsers).not_null().default(0))
                    .col(big_integer(BusinessPlanTier::PricePerUserInCents).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_plan_tier_plan")
                            .from(BusinessPlanTier::Table, BusinessPlanTier::BusinessPlanId)
                            .to(BusinessPlan::Table, BusinessPlan::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("uq_tenant_plan_tier_plan_limit")
                            .col(BusinessPlanTier::BusinessPlanId)
                            .col(BusinessPlanTier::UpToUsers)
                            .unique(),
                    )
                    .index(
                        Index::create()
                            .name("ix_tenant_plan_tier_plan")
                            .col(BusinessPlanTier::BusinessPlanId),
                    )
                    .check(Check::named(
                        Alias::new("ck_tenant_plan_tier_price").into_iden(),
                        Expr::col(BusinessPlanTier::PricePerUserInCents).gte(0),
                    ))
                    .engine("InnoDB")
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BusinessPlanTier::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BusinessPlanTier {
    Table,
    Id,
    Uuid,
    BusinessPlanId,
    UpToUsers,
    PricePerUserInCents,
}

#[derive(DeriveIden)]
enum BusinessPlan {
    Table,
    Id,
}
